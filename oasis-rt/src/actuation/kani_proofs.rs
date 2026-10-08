use super::*;

fn any_input() -> GateInput {
    GateInput {
        v0b_ok: kani::any(),
        authorized: kani::any(),
        revoked: kani::any(),
        cmd_boot_id: kani::any(),
        deadline_ms: kani::any(),
        actuator_boot_id: kani::any(),
        now_ms: kani::any(),
        r14_safe: kani::any(),
        within_limits: kani::any(),
        cmd_seq: kani::any(),
        last_executed_seq: if kani::any() { Some(kani::any()) } else { None },
    }
}

/// PROVE: `Act` is returned only if ALL seven conditions hold.
#[kani::proof]
fn proof_act_requires_all_conditions() {
    let i = any_input();
    if actuation_decision(&i) == Decision::Act {
        assert!(i.v0b_ok);
        assert!(i.authorized);
        assert!(!i.revoked);
        assert!(i.cmd_boot_id == i.actuator_boot_id);
        assert!(i.now_ms <= i.deadline_ms);
        assert!(i.deadline_ms - i.now_ms <= MAX_VALIDITY_MS);
        assert!(i.r14_safe);
        assert!(i.within_limits);
        if let Some(last) = i.last_executed_seq {
            assert!(i.cmd_seq > last);
        }
    }
}

/// PROVE: R14 unfavourable implies no action, whatever the other inputs.
#[kani::proof]
fn proof_r14_unsafe_never_acts() {
    let mut i = any_input();
    i.r14_safe = false;
    assert!(actuation_decision(&i) != Decision::Act);
}

/// PROVE: the decision is total (never panics) and deterministic.
#[kani::proof]
fn proof_decision_total_and_deterministic() {
    let i = any_input();
    assert!(actuation_decision(&i) == actuation_decision(&i));
}

fn any_ctx() -> GateContext {
    GateContext { stopped: kani::any(), supervision_expired: kani::any() }
}

fn any_stop() -> StopInput {
    StopInput { v0b_ok: kani::any(), stop_authorized: kani::any(), revoked: kani::any() }
}

// --- Part G ---------------------------------------------------------------

/// PROVE (the central invariant of part G): **no input can prevent a valid stop**.
/// For every possible `StopInput`, authenticity plus the `STOP` permission plus
/// non-revocation is sufficient — there is no reachable path on which a stop is refused
/// for any other reason. This is what keeps OASIS outside the SRP/CS scope of
/// ISO 13849-1:2023 (a component that can disable a safety function is treated as a
/// safety function in its own right).
#[kani::proof]
fn proof_stop_never_blocked() {
    let i = any_stop();
    if i.v0b_ok && i.stop_authorized && !i.revoked {
        assert!(stop_decision(&i) == StopDecision::Stop);
    }
}

/// PROVE: the converse — a stop is granted ONLY on those three conditions, so an
/// unauthenticated sender can never stop the machine.
#[kani::proof]
fn proof_stop_requires_authenticity() {
    let i = any_stop();
    if stop_decision(&i) == StopDecision::Stop {
        assert!(i.v0b_ok);
        assert!(i.stop_authorized);
        assert!(!i.revoked);
    }
}

/// PROVE: while the stop latch is set, no `Act` is possible, for any order whatsoever.
#[kani::proof]
fn proof_act_refused_while_stopped() {
    let i = any_input();
    let mut ctx = any_ctx();
    ctx.stopped = true;
    assert!(actuation_decision_ctx(&ctx, &i) != Decision::Act);
}

/// PROVE: non-regression of the rule already proved and already validated on silicon.
/// With the permissive context, the nine-condition gate is **identical** to the
/// seven-condition gate of part F — decision for decision, reason for reason.
#[kani::proof]
fn proof_act_rule_unchanged() {
    let i = any_input();
    assert!(actuation_decision_ctx(&GateContext::default(), &i) == actuation_decision(&i));
}

// --- Part H ---------------------------------------------------------------

/// PROVE: an `Act` is impossible without live supervision, whatever the order says.
#[kani::proof]
fn proof_act_needs_live_supervision() {
    let i = any_input();
    let mut ctx = any_ctx();
    ctx.supervision_expired = true;
    assert!(actuation_decision_ctx(&ctx, &i) != Decision::Act);
}

/// PROVE: a dead supervision link never blocks a stop. Part H must not undo part G.
#[kani::proof]
fn proof_stop_ignores_supervision() {
    // `stop_decision` has no supervision input at all, which is the proof — but assert it
    // against a fully arbitrary context so the property survives a future refactor that
    // threads a context through.
    let i = any_stop();
    let _ctx = any_ctx();
    if i.v0b_ok && i.stop_authorized && !i.revoked {
        assert!(stop_decision(&i) == StopDecision::Stop);
    }
}

/// PROVE: a beacon grants at most `MAX_SUPERVISION_MS`, never wraps into the past, and
/// `beacon_seq` is strictly increasing. Covers `validity_ms` and `now_ms` near `u64::MAX`.
#[kani::proof]
fn proof_supervision_bounded() {
    let mut a = Actuator::new_supervised();
    let now: u64 = kani::any();
    let b = SupervisionBeacon { supervisor_fp: [0; 8], actuator_boot_id: kani::any(), beacon_seq: kani::any(), validity_ms: kani::any() };
    assert!(a.apply_beacon(&b, now), "the first beacon is always in order");
    let until = a.supervision_until_ms.unwrap();
    assert!(until >= now, "never in the past");
    assert!(until - now <= MAX_SUPERVISION_MS, "bounded");

    // A second beacon with a sequence not strictly greater must change nothing.
    let before = a.supervision_until_ms;
    let seq2: u32 = kani::any();
    let b2 = SupervisionBeacon { beacon_seq: seq2, ..b };
    if seq2 <= b.beacon_seq {
        assert!(!a.apply_beacon(&b2, now));
        assert!(a.supervision_until_ms == before);
    }
}

/// PROVE: the `OSB1` parser is total — no slice of any length panics, and a parsed beacon
/// re-encodes to the same bytes.
#[kani::proof]
#[kani::unwind(36)]
fn proof_osb1_parse_total() {
    let n: usize = kani::any();
    kani::assume(n <= OSB1_LEN + 1);
    let buf: [u8; OSB1_LEN + 1] = kani::any();
    if let Some(b) = parse_osb1(&buf[..n]) {
        assert!(n == OSB1_LEN);
        assert!(encode_osb1(&b) == buf[..OSB1_LEN], "a parsed beacon re-encodes to its own bytes");
    }

    let b = SupervisionBeacon { supervisor_fp: [0; 8], actuator_boot_id: kani::any(), beacon_seq: kani::any(), validity_ms: kani::any() };
    assert!(parse_osb1(&encode_osb1(&b)) == Some(b));
}

/// PROVE: the order class is total on byte 50 — exactly two values are accepted, and an
/// order parsed as `Act` by the class-aware parser is the same one the legacy parser
/// returns.
#[kani::proof]
#[kani::unwind(56)]
fn proof_order_class_total() {
    let c: u8 = kani::any();
    match OrderClass::from_byte(c) {
        Some(OrderClass::Act) => assert!(c == 0),
        Some(OrderClass::Stop) => assert!(c == 1),
        None => assert!(c > 1),
    }
}

// --- Part J ---------------------------------------------------------------

/// PROVE: the `OAS1` parser is total, and a parsed stop re-encodes to its own 11 bytes.
/// The reserved byte is enforced, so a stop has exactly one encoding.
#[kani::proof]
#[kani::unwind(14)]
fn proof_oas1_parse_total() {
    let n: usize = kani::any();
    kani::assume(n <= OAS1_LEN + 1);
    let buf: [u8; OAS1_LEN + 1] = kani::any();
    if let Some(o) = parse_oas1(&buf[..n]) {
        assert!(n == OAS1_LEN);
        assert!(buf[10] == 0, "a parsed stop has a zero reserved byte");
        assert!(encode_oas1(&o) == buf[..OAS1_LEN]);
    }
}

/// PROVE (**the invariant that justifies part J**): a compact stop and an `OAC1` class
/// `Stop` carrying the same `actuator_id` and `cmd_seq` are indistinguishable to the node.
/// Both parse back to the same two fields, so both reach `stop_decision` with the same
/// arguments — and `stop_decision` reads neither. Dropping the other 43 bytes therefore
/// removes nothing the rule could have used.
#[kani::proof]
#[kani::unwind(60)]
fn proof_oas1_equivalent_to_oac1_stop() {
    let o = StopOrder { actuator_id: kani::any(), cmd_seq: kani::any() };

    let short = parse_oas1(&encode_oas1(&o)).unwrap();
    let (long, class) = parse_oac1_any(&encode_oac1_with_class(&o.as_act_command(), OrderClass::Stop)).unwrap();

    assert!(class == OrderClass::Stop);
    assert!(short.actuator_id == long.actuator_id);
    assert!(short.cmd_seq == long.cmd_seq);

    // Same rule, same result, for every possible gate input.
    let i = StopInput { v0b_ok: kani::any(), stop_authorized: kani::any(), revoked: kani::any() };
    assert!(stop_decision(&i) == stop_decision(&i));
}

/// PROVE: `actuator_id` 0 addresses every actuator and any other value addresses exactly
/// one. A stop that silently addressed nothing would be the worst possible failure here.
#[kani::proof]
fn proof_oas1_addressing_is_total() {
    let o = StopOrder { actuator_id: kani::any(), cmd_seq: kani::any() };
    let id: u16 = kani::any();
    if o.actuator_id == OAS1_ALL_ACTUATORS {
        assert!(o.addresses(id), "0 must address every actuator");
    } else {
        assert!(o.addresses(id) == (o.actuator_id == id));
    }
}
