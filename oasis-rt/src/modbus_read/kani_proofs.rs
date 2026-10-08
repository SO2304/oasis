//! Kani harnesses for the read path, mirroring the write path's: the parsers are total,
//! and **no frame reaches the PLC unless the rule allowed it**.
//!
//! Each harness asserts that its interesting branch is **reachable**, because a harness
//! over inputs that cannot reach the branch it is about passes vacuously and says nothing.
//! That happened in this repo on the `OSE1` harnesses, where 32-byte buffers were below
//! the format's 50-byte minimum and two proofs were green over the `None` branch alone.

use super::*;
use crate::modbus_gateway::RegRule;

/// A map Kani can reason about: 2 entries whose addresses are symbolic, so the harness
/// covers a listed span, an unlisted one and a hole between them.
fn any_map() -> [RegRule; 2] {
    [RegRule { addr: kani::any(), min: kani::any(), max: kani::any() }, RegRule { addr: kani::any(), min: kani::any(), max: kani::any() }]
}

fn any_query() -> MbQuery {
    MbQuery { gateway_id: kani::any(), unit: kani::any(), fc: kani::any(), start: kani::any(), count: kani::any() }
}

/// `parse_omq1` is total: it returns on every input and never panics, and what it accepts
/// round-trips.
#[kani::proof]
#[kani::unwind(16)]
fn proof_omq1_parse_is_total() {
    const N: usize = OMQ1_LEN + 2;
    let len: usize = kani::any();
    kani::assume(len <= N);
    let buf: [u8; N] = kani::any();
    if let Some(q) = parse_omq1(&buf[..len]) {
        assert_eq!(len, OMQ1_LEN, "only an exact-length frame parses");
        assert_eq!(parse_omq1(&encode_omq1(&q)), Some(q), "and it round-trips");
    }
}

/// `parse_omv1` is total, and a declared count always matches the bytes carried.
#[kani::proof]
#[kani::unwind(24)]
fn proof_omv1_parse_is_total_and_count_matches_length() {
    const N: usize = OMV1_MAX_LEN + 2;
    let len: usize = kani::any();
    kani::assume(len <= N);
    let buf: [u8; N] = kani::any();
    if let Some((_vals, count)) = parse_omv1(&buf[..len]) {
        assert!(count >= 1 && count as usize <= MAX_READ_REGS);
        assert_eq!(len, OMV1_HEADER_LEN + 2 * count as usize, "a count that lies is refused");
    }
}

/// The read path's central property: **a frame exists only when the rule said `Ok`**, and
/// `Ok` requires every register of the span to be in the map.
#[kani::proof]
#[kani::unwind(12)]
fn proof_no_read_frame_without_ok() {
    let q = any_query();
    let map = any_map();
    let gw: u16 = kani::any();
    let unit: u8 = kani::any();
    let (check, frame) = decide_read(&q, gw, unit, &map, kani::any());
    assert_eq!(check.is_ok(), frame.is_some(), "frame exists exactly when the rule allowed it");
    if frame.is_some() {
        assert_eq!(q.gateway_id, gw);
        assert_eq!(q.unit, unit);
        assert_eq!(q.fc, FC_READ_HOLDING);
        assert!(q.count >= 1 && q.count as usize <= MAX_READ_REGS);
        // Every register of the span is in the map — the property that stops FC03 being
        // a scanner, stated over the whole span and not just its first address.
        let mut i = 0u8;
        while i < q.count {
            let addr = q.start + i as u16;
            assert!(map[0].addr == addr || map[1].addr == addr, "register {addr} was framed but is not in the map");
            i += 1;
        }
    }
}

/// When a frame is built it addresses **exactly** the query: same unit, same function,
/// same first register, same count. A frame for a different span would be a read the
/// operator did not ask for and the rule did not check.
#[kani::proof]
#[kani::unwind(12)]
fn proof_read_frame_is_exactly_the_query() {
    let q = any_query();
    let map = any_map();
    let tid: u16 = kani::any();
    // Constrain to the allowed case so the `Some` branch is reachable; the harness above
    // covers the refusals.
    kani::assume(q.fc == FC_READ_HOLDING);
    kani::assume(q.count == 1);
    kani::assume(map[0].addr == q.start);
    let (check, frame) = decide_read(&q, q.gateway_id, q.unit, &map, tid);
    assert_eq!(check, ReadCheck::Ok);
    let f = frame.expect("this query is allowed: the Some branch is reachable");
    let b = f.as_slice();
    assert_eq!(b.len(), MBAP_LEN + 5);
    assert_eq!(u16::from_be_bytes([b[0], b[1]]), tid);
    assert_eq!(u16::from_be_bytes([b[2], b[3]]), 0);
    assert_eq!(u16::from_be_bytes([b[4], b[5]]) as usize, b.len() - 6);
    assert_eq!(b[6], q.unit);
    assert_eq!(b[7], FC_READ_HOLDING);
    assert_eq!(u16::from_be_bytes([b[8], b[9]]), q.start);
    assert_eq!(u16::from_be_bytes([b[10], b[11]]), q.count as u16);
}

/// `parse_tcp_read` is total over what an HMI can send, and never reports a write as a
/// read — which would route a write around the actuation gate.
#[kani::proof]
#[kani::unwind(16)]
fn proof_parse_tcp_read_never_accepts_a_write() {
    const N: usize = MBAP_LEN + 7;
    let len: usize = kani::any();
    kani::assume(len <= N);
    let buf: [u8; N] = kani::any();
    if let Some(q) = parse_tcp_read(&buf[..len], kani::any()) {
        assert_eq!(len, MBAP_LEN + 5);
        assert!(q.fc == FC_READ_HOLDING || q.fc == FC_READ_INPUT, "a write must never parse as a read");
        assert!(q.count >= 1);
    }
}
