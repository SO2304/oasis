use super::*;
use crate::actuation::MAX_VALIDITY_MS;

fn any_order() -> MbOrder {
    let fc: u8 = if kani::any() { FC_WRITE_SINGLE } else { FC_WRITE_MULTIPLE };
    let count: u8 = kani::any();
    kani::assume(count_ok(fc, count));
    MbOrder {
        gateway_id: kani::any(),
        cmd_seq: kani::any(),
        boot_id: kani::any(),
        deadline_ms: kani::any(),
        unit: kani::any(),
        fc,
        start: kani::any(),
        count,
        values: kani::any(),
    }
}

fn any_ctx() -> OrderContext {
    OrderContext {
        v0b_ok: kani::any(),
        authorized: kani::any(),
        revoked: kani::any(),
        actuator_boot_id: kani::any(),
        now_ms: kani::any(),
        r14_safe: kani::any(),
    }
}

fn any_rule() -> RegRule {
    RegRule { addr: kani::any(), min: kani::any(), max: kani::any() }
}

/// PROVE: a frame exists only for an `Act` decision, i.e. only when every Part F
/// condition holds: verified, authorized, not revoked, unexpired in the gateway's
/// clock, R14-safe, within the register rules, newer than the last executed order.
#[kani::proof]
#[kani::unwind(26)]
fn proof_mb_no_frame_without_act() {
    let (ctx, o, unit) = (any_ctx(), any_order(), kani::any());
    let map = [any_rule(), any_rule(), any_rule()];
    let last: Option<u32> = if kani::any() { Some(kani::any()) } else { None };
    let (d, rules, f) = gateway_decision(&ctx, &o, unit, &map, last);
    if f.is_some() {
        assert!(d == Decision::Act);
        assert!(ctx.v0b_ok && ctx.authorized && !ctx.revoked && ctx.r14_safe);
        assert!(o.boot_id == ctx.actuator_boot_id);
        assert!(ctx.now_ms <= o.deadline_ms && o.deadline_ms - ctx.now_ms <= MAX_VALIDITY_MS);
        assert!(rules == RuleCheck::Ok);
        if let Some(l) = last {
            assert!(o.cmd_seq > l);
        }
    } else {
        assert!(d != Decision::Act);
    }
}

/// PROVE: a produced frame writes only registers of the map, each value inside its
/// range, to the configured unit, and encodes exactly the order (function, start,
/// quantity, values big-endian) with a valid CRC-16/MODBUS.
#[kani::proof]
#[kani::unwind(26)]
fn proof_mb_frame_matches_rules() {
    let (ctx, o, unit) = (any_ctx(), any_order(), kani::any());
    let map = [any_rule(), any_rule(), any_rule()];
    let last: Option<u32> = if kani::any() { Some(kani::any()) } else { None };
    let (_, _, f) = gateway_decision(&ctx, &o, unit, &map, last);
    if let Some(f) = f {
        assert!(o.unit == unit && f.bytes[0] == unit && f.bytes[1] == o.fc);
        assert!(u16::from_be_bytes([f.bytes[2], f.bytes[3]]) == o.start);
        let n = o.count as usize;
        let mut i = 0;
        while i < n {
            let addr = o.start as u32 + i as u32;
            assert!(addr <= 0xFFFF);
            let mut found = false;
            for r in map.iter() {
                if r.addr as u32 == addr {
                    found = true;
                    assert!(o.values[i] >= r.min && o.values[i] <= r.max);
                    break;
                }
            }
            assert!(found);
            i += 1;
        }
        if o.fc == FC_WRITE_SINGLE {
            assert!(f.len == 8 && u16::from_be_bytes([f.bytes[4], f.bytes[5]]) == o.values[0]);
        } else {
            assert!(f.len == 9 + 2 * n);
            assert!(u16::from_be_bytes([f.bytes[4], f.bytes[5]]) as usize == n && f.bytes[6] as usize == 2 * n);
            let mut j = 0;
            while j < n {
                assert!(u16::from_be_bytes([f.bytes[7 + 2 * j], f.bytes[8 + 2 * j]]) == o.values[j]);
                j += 1;
            }
        }
        assert!(crc_ok(f.as_slice()));
    }
}

/// PROVE: the order parser and the response check never panic; an accepted order
/// has exactly the length its count implies, a supported function code, and zero
/// values past its count.
#[kani::proof]
#[kani::unwind(50)]
fn proof_mb_parsers_total() {
    let buf: [u8; OMB1_MAX_LEN + 1] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= buf.len());
    if let Some(o) = parse_omb1(&buf[..len]) {
        assert!(len == OMB1_HEADER_LEN + 2 * o.count as usize);
        assert!(count_ok(o.fc, o.count));
        let mut i = o.count as usize;
        while i < MAX_REGS {
            assert!(o.values[i] == 0);
            i += 1;
        }
    }
    let req = Frame { bytes: kani::any(), len: kani::any() };
    let resp: [u8; 10] = kani::any();
    let rl: usize = kani::any();
    kani::assume(rl <= resp.len());
    let _ = check_response(&req, &resp[..rl]);
}

fn any_origin_rule(origin: [u8; 8]) -> OriginRule {
    OriginRule { origin, rule: any_rule() }
}

/// PROVE: **no order is within limits outside its own origin's map.**
///
/// This is the per-origin half of "within limits". With one shared map, any authorised key
/// could write every register in it; a pilot needs "this HMI may move axis 1, that one may
/// only reset the counter", which is an authorisation question and not a range check.
///
/// Both directions are stated, because the second is what makes the first safe to add:
/// an origin **with** a map is judged only by its own entries, and an origin **without**
/// one gets exactly `check_rules` on the shared map -- so configuring a rule for one
/// origin cannot widen what another may write.
#[kani::proof]
#[kani::unwind(26)]
fn proof_mb_no_frame_outside_the_origin_map() {
    let o = any_order();
    let unit: u8 = kani::any();
    let origin: [u8; 8] = kani::any();
    let other: [u8; 8] = kani::any();
    let shared = [any_rule(), any_rule()];
    // A table holding one rule for this origin and one for a symbolic other origin, so the
    // harness covers both "mine" and "someone else's" without an unwind on a nested map.
    let table = [any_origin_rule(origin), any_origin_rule(other)];

    let got = check_rules_for_origin(&o, unit, &origin, &table, &shared);

    if got == RuleCheck::Ok {
        // The order is within limits, so: the unit matched, and every register it writes
        // is in THIS origin's entries with its value inside that entry's range.
        assert!(o.unit == unit);
        let mut i = 0usize;
        while i < o.count as usize {
            let addr = o.start.checked_add(i as u16).expect("Ok implies no wrap");
            let mut mine = false;
            let mut j = 0usize;
            while j < table.len() {
                let e = table[j];
                if e.origin == origin && e.rule.addr == addr && o.values[i] >= e.rule.min && o.values[i] <= e.rule.max {
                    mine = true;
                }
                j += 1;
            }
            assert!(mine, "a register was allowed that is not in this origin's own map");
            i += 1;
        }
    }

    // The other direction: an origin named nowhere in the table falls back to the shared
    // map, and to `check_rules` itself rather than to a reimplementation of it.
    let stranger: [u8; 8] = kani::any();
    kani::assume(stranger != origin && stranger != other);
    assert!(!origin_has_own_map(&stranger, &table));
    assert!(check_rules_for_origin(&o, unit, &stranger, &table, &shared) == check_rules(&o, unit, &shared));
}
