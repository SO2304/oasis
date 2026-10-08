use super::*;
use crate::modbus_gateway::OrderContext;

fn any_order() -> MbOrder {
    let fc: u8 = kani::any();
    let count: u8 = kani::any();
    kani::assume((fc == FC_WRITE_SINGLE && count == 1) || (fc == FC_WRITE_MULTIPLE && count >= 1 && count as usize <= MAX_REGS));
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

/// A TCP frame exists iff the decision is `Act`, and it carries exactly the order:
/// MBAP (tid, protocol 0, consistent length, unit) + the order's function, start and values.
#[kani::proof]
#[kani::unwind(10)]
fn proof_mbtcp_frame_iff_act_and_matches_order() {
    let o = any_order();
    let map = [RegRule { addr: kani::any(), min: kani::any(), max: kani::any() }];
    let tid: u16 = kani::any();
    let (d, _, f) = gateway_decision_tcp(&any_ctx(), &o, kani::any(), &map, kani::any(), tid);
    assert_eq!(d == Decision::Act, f.is_some());
    if let Some(f) = f {
        let b = f.as_slice();
        assert!(b.len() >= ACK_LEN && b.len() <= MAX_TCP_FRAME);
        assert_eq!(u16_be(b, 0), tid);
        assert_eq!(u16_be(b, 2), 0);
        assert_eq!(u16_be(b, 4) as usize, b.len() - 6);
        assert_eq!((b[6], b[7], u16_be(b, 8)), (o.unit, o.fc, o.start));
        if o.fc == FC_WRITE_SINGLE {
            assert_eq!(u16_be(b, 10), o.values[0]);
        } else {
            let i: usize = kani::any();
            kani::assume(i < o.count as usize);
            assert_eq!(u16_be(b, 13 + 2 * i), o.values[i]);
        }
    }
}

/// The HMI-side parser is total and only returns bounded writes. Inputs go 8 bytes past
/// the largest valid frame, so over-long FC16 frames (9..=12 registers) are explored.
#[kani::proof]
#[kani::unwind(40)]
fn proof_mbtcp_parse_total() {
    let n: usize = kani::any();
    kani::assume(n <= MAX_TCP_FRAME + 8);
    let buf: [u8; MAX_TCP_FRAME + 8] = kani::any();
    if let Some(w) = parse_tcp_write(&buf[..n]) {
        assert!((w.fc == FC_WRITE_SINGLE && w.count == 1) || (w.fc == FC_WRITE_MULTIPLE && w.count >= 1 && w.count as usize <= MAX_REGS));
    }
}
