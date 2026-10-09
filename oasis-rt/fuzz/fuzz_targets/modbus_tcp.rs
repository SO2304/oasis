//! The pilot's front door: the HMI's raw Modbus TCP write, and the PLC's answer.
//!
//! `parse_tcp_write` is the **first** thing the agent runs on bytes an HMI sent, and
//! `check_tcp_response` the first thing the gateway runs on bytes the PLC sent. Neither
//! had a fuzz target until 2026-10-09: the `modbus` target covers `parse_omb1` and
//! `modbus_gateway::check_response`, which is the **RTU** checker over an RTU `Frame`, a
//! different function. So the repo's claim about fuzzed parsers had a hole exactly where
//! the TCP path begins.
//!
//! Three properties, not just "it does not crash":
//!
//! 1. an accepted write stays inside the bounds the rest of the path relies on, so a
//!    later `to_order` cannot index past `MAX_REGS`;
//! 2. the echo the HMI would receive is a well-formed Modbus frame whose MBAP length
//!    describes the frame it is in;
//! 3. `check_tcp_response` never calls an answer an `Ack` unless it matches the request's
//!    transaction id and unit — a device answering for someone else's transaction is how
//!    a kept connection gets out of step.
#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "common.rs"]
mod common;
use oasis_rt::modbus_gateway::{MbOrder, OrderContext, RegRule, Response, MAX_REGS};
use oasis_rt::modbus_tcp::{check_tcp_response, gateway_decision_tcp, hmi_response, parse_tcp_write, Outcome, ACK_LEN};

fuzz_target!(|data: &[u8]| {
    common::track("modbus_tcp", data);

    if let Some(w) = parse_tcp_write(data) {
        // 1. Bounds. `count` is a u8 and `values` a fixed array, so the real claim is
        // that count is in range — asserting `values.len() >= count` would be vacuous.
        assert!(w.count >= 1, "count 0 accepted");
        assert!(w.count as usize <= MAX_REGS, "count {} over MAX_REGS {MAX_REGS}", w.count);
        assert!(w.fc == 0x06 || w.fc == 0x10, "fc {:#04x} is neither FC06 nor FC16", w.fc);

        // 2. The echo. `Outcome::Done` is the only path that answers with a function code
        // rather than an exception, so it is the one worth re-reading.
        let (echo, n) = hmi_response(&w, Outcome::Done);
        assert!(n >= 8 && n <= ACK_LEN, "echo length {n} outside 8..={ACK_LEN}");
        let declared = u16::from_be_bytes([echo[4], echo[5]]) as usize;
        assert_eq!(declared + 6, n, "the MBAP length must describe the frame it is in");
        assert_eq!(echo[7] & 0x80, 0, "an acknowledged write is not an exception");
        assert_eq!(&echo[0..2], &w.tid.to_be_bytes(), "the echo must carry the request's tid");
    }

    // 3. A response is accepted only for the request it belongs to. The request is fixed
    // and valid; `data` plays the device.
    let ctx = OrderContext { v0b_ok: true, authorized: true, revoked: false, actuator_boot_id: 7, now_ms: 0, r14_safe: true };
    let mut values = [0u16; MAX_REGS];
    values[0] = 215;
    let o = MbOrder { gateway_id: 1, cmd_seq: 1, boot_id: 7, deadline_ms: 1_000, unit: 0x11, fc: 6, start: 0x10, count: 1, values };
    let map = [RegRule { addr: 0x10, min: 50, max: 300 }];
    let tid = u16::from_be_bytes([data.first().copied().unwrap_or(0), data.get(1).copied().unwrap_or(0)]);
    let (_, _, frame) = gateway_decision_tcp(&ctx, &o, 0x11, &map, None, tid);
    if let Some(f) = frame {
        if let Response::Ack = check_tcp_response(&f, data) {
            assert!(data.len() >= 8, "an ack cannot be shorter than its own header");
            assert_eq!(&data[0..2], &tid.to_be_bytes(), "ack for another transaction");
            assert_eq!(data[6], 0x11, "ack for another unit");
        }
    }
});
