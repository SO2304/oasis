//! The link framing, and the configuration parser (pilot A.8).
//!
//! Two parsers that nothing fuzzed until 2026-10-09:
//!
//! **The length-prefixed link framing.** `read_frame` is what the gateway runs on the
//! first four bytes an agent — or anyone who reached the port — sends. Its whole job is to
//! refuse a declared length **before** allocating, so a hostile peer cannot pick this
//! side's memory. `modbus_read_request` does the same for an MBAP header. Both took a
//! concrete `TcpStream`, which is why they had no target; they are generic over `Read`
//! now, so a byte slice can drive them. `ORV1` reaches the gateway through this framing,
//! which is the "ORV1 over TCP" half that was missing — `parse_orv1` itself has been
//! fuzzed since 2026-10-07 by the `revocation` target, and claiming it as new here would
//! be dishonest.
//!
//! **The configuration.** Reached by whoever can edit `/etc/oasis`, not by the network,
//! so this is not an attack surface — but an integrator editing a file by hand is the
//! normal case, and a malformed line must give an error with a line number, never a
//! panic. `Config::from_text` exists so this can run without touching a file.
#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "common.rs"]
mod common;
use oasis_rt::mbtcp_conf::Config;
use oasis_rt::mbtcp_net::{modbus_read_request, read_frame, MAX_LINK_FRAME};

fuzz_target!(|data: &[u8]| {
    common::track("link_frames", data);

    // 1. The link frame. Whatever comes back must be within the cap and must be exactly
    // what the prefix declared — a reader that returns more or less than it promised is
    // how a stream desynchronises.
    let mut cur = data;
    if let Ok(body) = read_frame(&mut cur) {
        assert!(!body.is_empty(), "a zero-length frame was accepted");
        assert!(body.len() <= MAX_LINK_FRAME, "frame of {} over the {MAX_LINK_FRAME} cap", body.len());
        let declared = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        assert_eq!(body.len(), declared, "returned {} bytes for a declared {declared}", body.len());
        assert_eq!(&body[..], &data[4..4 + declared], "the body is not the bytes that followed the prefix");
    }

    // 2. The Modbus TCP header. Same discipline: the frame carries its own declared
    // length, and the cap is the one in the function.
    let mut cur = data;
    if let Ok(f) = modbus_read_request(&mut cur) {
        assert!(f.len() >= 6, "a frame shorter than its own header was accepted");
        let rest = u16::from_be_bytes([f[4], f[5]]) as usize;
        assert_eq!(f.len(), 6 + rest, "length field {rest} does not describe {} bytes", f.len());
        assert!(rest >= 1 && rest <= 260, "declared length {rest} outside 1..=260");
        assert_eq!(&f[..6], &data[..6], "the header was altered on the way out");
    }

    // 3. The configuration. An error is fine, a panic is not. Text that is not UTF-8
    // cannot reach `from_text` through `load` either, since `read_to_string` refuses it
    // first, so only the valid-UTF-8 case is a real input.
    if let Ok(text) = core::str::from_utf8(data) {
        match Config::from_text(text, "fuzz.conf") {
            Ok(c) => {
                // A loaded config must be self-consistent in the ways the gate relies on.
                assert!(c.timeout_ms > 0, "a zero timeout would make every read fail at once");
                for r in &c.map {
                    assert!(r.min <= r.max, "register {} has min {} over max {}", r.addr, r.min, r.max);
                }
                for e in &c.origin_map {
                    assert!(e.rule.min <= e.rule.max, "origin rule for register {} has min over max", e.rule.addr);
                }
            }
            Err(e) => assert!(!e.is_empty(), "a refusal with no reason is not a refusal"),
        }
    }
});
