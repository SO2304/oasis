//! Modbus gateway: OMB1 orders (an accepted order re-encodes to the same bytes) and
//! device responses checked against a decided request.
#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "common.rs"]
mod common;
use oasis_rt::modbus_gateway::{
    check_response, encode_omb1, gateway_decision, parse_omb1, MbOrder, OrderContext, RegRule, MAX_REGS,
};

fuzz_target!(|data: &[u8]| {
    common::track("modbus", data);
    if let Some(o) = parse_omb1(data) {
        let (b, n) = encode_omb1(&o).expect("an accepted order is encodable");
        assert_eq!(&b[..n], data);
    }
    let ctx = OrderContext { v0b_ok: true, authorized: true, revoked: false, actuator_boot_id: 7, now_ms: 0, r14_safe: true };
    let mut values = [0u16; MAX_REGS];
    values[0] = 215;
    let o = MbOrder { gateway_id: 1, cmd_seq: 1, boot_id: 7, deadline_ms: 1_000, unit: 0x11, fc: 6, start: 0x10, count: 1, values };
    let map = [RegRule { addr: 0x10, min: 50, max: 300 }];
    let (_, _, f) = gateway_decision(&ctx, &o, 0x11, &map, None);
    let _ = check_response(&f.expect("valid order"), data);
});
