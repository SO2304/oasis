//! Modbus TCP reads (pilot A.6): the four parsers an attacker can reach, plus the two
//! round-trip identities and the rule that no frame exists without `Ok`.
//!
//! Added because the read path introduced four new parsers and the repo's assurance claim
//! is about the ones that are fuzzed. A parser that only Kani has seen is covered for the
//! properties the harness states; fuzzing is what finds the ones nobody thought to state.
#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "common.rs"]
mod common;
use oasis_rt::modbus_gateway::RegRule;
use oasis_rt::modbus_read::{
    check_read_response, decide_read, encode_omq1, encode_omv1, hmi_read_response, parse_omq1, parse_omv1, parse_tcp_read, MbQuery, ReadCheck,
    FC_READ_HOLDING, MAX_READ_REGS,
};

fuzz_target!(|data: &[u8]| {
    common::track("modbus_read", data);

    // 1. OMQ1 — what a gateway parses off the network. An accepted query must re-encode to
    // exactly the bytes that produced it, or two different byte strings mean one query.
    if let Some(q) = parse_omq1(data) {
        assert_eq!(&encode_omq1(&q)[..], data, "an accepted query must re-encode identically");
    }

    // 2. OMV1 — what an agent parses off the gateway. The declared count must always match
    // the bytes carried; a reply that lies is refused, never zero-padded.
    if let Some((values, count)) = parse_omv1(data) {
        assert!(count >= 1 && count as usize <= MAX_READ_REGS);
        let (b, n) = encode_omv1(&values[..count as usize]).expect("an accepted reply is encodable");
        assert_eq!(&b[..n], data, "an accepted reply must re-encode identically");
    }

    // 3. The HMI's own frame. It must never report a write as a read, which would route a
    // write around the actuation gate entirely.
    if let Some(q) = parse_tcp_read(data, 1) {
        assert!(q.fc == FC_READ_HOLDING || q.fc == oasis_rt::modbus_read::FC_READ_INPUT);
        assert!(q.count >= 1);
    }

    // 4. The device's answer, checked against a frame the rule actually decided. The
    // non-contiguous map is deliberate: a span crossing the hole must be refused.
    let map = [RegRule { addr: 10, min: 0, max: 1000 }, RegRule { addr: 11, min: 0, max: 1 }, RegRule { addr: 20, min: 0, max: 5 }];
    let q = MbQuery { gateway_id: 1, unit: 0x11, fc: FC_READ_HOLDING, start: 10, count: 2 };
    let (check, frame) = decide_read(&q, 1, 0x11, &map, 0x1234);
    assert_eq!(check, ReadCheck::Ok);
    let f = frame.expect("10..12 is inside the map");
    let _ = check_read_response(&f, data, q.count);
    // And with a count the answer was not built for: still total, never a panic.
    let _ = check_read_response(&f, data, 1);
    let _ = check_read_response(&f, data, MAX_READ_REGS as u8);

    // 5. The rule over attacker-chosen spans: a frame exists only when the rule allowed
    // it, and then it addresses exactly the query.
    if data.len() >= 4 {
        let start = u16::from_le_bytes([data[0], data[1]]);
        let count = data[2];
        let fc = data[3];
        let probe = MbQuery { gateway_id: 1, unit: 0x11, fc, start, count };
        let (check, frame) = decide_read(&probe, 1, 0x11, &map, 7);
        assert_eq!(check.is_ok(), frame.is_some(), "a frame exists exactly when the rule allowed it");
        if let Some(f) = frame {
            let b = f.as_slice();
            assert_eq!(b[7], FC_READ_HOLDING);
            assert_eq!(u16::from_be_bytes([b[8], b[9]]), start);
            assert_eq!(u16::from_be_bytes([b[10], b[11]]), count as u16);
            // Every register of the span is in the map: the property that stops FC03
            // being a scanner, over the whole span and not just its first address.
            for i in 0..count {
                let addr = start + i as u16;
                assert!(map.iter().any(|r| r.addr == addr), "register {addr} framed but not in the map");
            }
            // And the answer the HMI would get is well formed for that count.
            let vals = vec![0u16; count as usize];
            let (_, n) = hmi_read_response(&probe, 0xBEEF, &vals).expect("a decided read has a response");
            assert_eq!(n, 7 + 2 + 2 * count as usize);
        }
    }
});
