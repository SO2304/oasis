//! Tests for the read path. The question each one answers is "what reaches the PLC", not
//! "does the parser agree with itself".

use super::*;
use crate::modbus_gateway::RegRule;

const GW: u16 = 1;
const UNIT: u8 = 0x11;
/// Deliberately non-contiguous: 10, 11, 12 and 20. A span that crosses the hole must be
/// refused, which a contiguous map could never show.
const MAP: [RegRule; 4] = [RegRule { addr: 10, min: 0, max: 1000 }, RegRule { addr: 11, min: 0, max: 1 }, RegRule { addr: 12, min: 0, max: 100 }, RegRule { addr: 20, min: 0, max: 5 }];

fn q(start: u16, count: u8) -> MbQuery {
    MbQuery { gateway_id: GW, unit: UNIT, fc: FC_READ_HOLDING, start, count }
}

#[test]
fn rd_a_listed_span_is_allowed_and_frames_exactly_it() {
    for (start, count) in [(10u16, 1u8), (10, 3), (11, 2), (20, 1)] {
        let query = q(start, count);
        let (check, frame) = decide_read(&query, GW, UNIT, &MAP, 0x1234);
        assert_eq!(check, ReadCheck::Ok, "{start}+{count} is inside the map");
        let f = frame.expect("an allowed query is framed");
        let b = f.as_slice();
        assert_eq!(&b[0..2], &0x1234u16.to_be_bytes(), "the transaction id is carried");
        assert_eq!(u16::from_be_bytes([b[2], b[3]]), 0, "protocol 0");
        assert_eq!(u16::from_be_bytes([b[4], b[5]]) as usize, b.len() - 6, "MBAP length");
        assert_eq!(b[6], UNIT);
        assert_eq!(b[7], FC_READ_HOLDING);
        assert_eq!(u16::from_be_bytes([b[8], b[9]]), start, "and exactly the registers asked for");
        assert_eq!(u16::from_be_bytes([b[10], b[11]]), count as u16);
    }
}

/// The property that stops FC03 being a scanner: one register outside the map refuses the
/// whole span, and **no frame is built**.
#[test]
fn rd_an_unlisted_register_refuses_the_whole_span_and_frames_nothing() {
    for (start, count, culprit) in [(9u16, 1u8, 9u16), (10, 4, 13), (12, 2, 13), (19, 3, 19), (0, 1, 0)] {
        let (check, frame) = decide_read(&q(start, count), GW, UNIT, &MAP, 1);
        assert_eq!(check, ReadCheck::RegisterNotAllowed(culprit), "{start}+{count} crosses the map");
        assert!(frame.is_none(), "a refused query must not be framed");
        assert_eq!(check.exception_code(), 0x02, "illegal data address");
    }
}

#[test]
fn rd_wrong_gateway_unit_fc_and_count_are_each_refused_by_name() {
    let cases: [(MbQuery, ReadCheck); 6] = [
        (MbQuery { gateway_id: 2, ..q(10, 1) }, ReadCheck::WrongGateway(2)),
        (MbQuery { unit: 0x12, ..q(10, 1) }, ReadCheck::WrongUnit(0x12)),
        (MbQuery { fc: FC_READ_INPUT, ..q(10, 1) }, ReadCheck::UnsupportedFc(FC_READ_INPUT)),
        (MbQuery { fc: 0x06, ..q(10, 1) }, ReadCheck::UnsupportedFc(0x06)),
        (q(10, 0), ReadCheck::CountOutOfRange(0)),
        (q(10, MAX_READ_REGS as u8 + 1), ReadCheck::CountOutOfRange(MAX_READ_REGS as u8 + 1)),
    ];
    for (query, want) in cases {
        let (check, frame) = decide_read(&query, GW, UNIT, &MAP, 1);
        assert_eq!(check, want);
        assert!(frame.is_none(), "{want:?} must not be framed");
    }
    // FC04 is refused as a function, not as an address: the HMI should be told which.
    assert_eq!(ReadCheck::UnsupportedFc(FC_READ_INPUT).exception_code(), 0x01);
}

/// A span that would wrap past 0xFFFF is not a span.
#[test]
fn rd_a_span_wrapping_the_address_space_is_refused() {
    let map = [RegRule { addr: 0xFFFF, min: 0, max: 1 }, RegRule { addr: 0, min: 0, max: 1 }];
    let (check, frame) = decide_read(&q(0xFFFF, 2), GW, UNIT, &map, 1);
    assert!(matches!(check, ReadCheck::RegisterNotAllowed(_)), "got {check:?}");
    assert!(frame.is_none(), "and nothing is framed");
}

#[test]
fn rd_omq1_roundtrips_and_the_parser_is_total() {
    let query = q(0xBEEF, 5);
    assert_eq!(parse_omq1(&encode_omq1(&query)), Some(query));
    // Wrong magic, wrong length, empty, and a truncated copy: all None, none panicking.
    assert_eq!(parse_omq1(b"OMB1\x01\x00\x11\x03\x0a\x00\x01"), None);
    assert_eq!(parse_omq1(&[]), None);
    let full = encode_omq1(&query);
    for n in 0..OMQ1_LEN {
        assert_eq!(parse_omq1(&full[..n]), None, "a {n}-byte prefix is not a query");
    }
}

#[test]
fn rd_omv1_roundtrips_and_refuses_a_count_that_lies() {
    for n in 1..=MAX_READ_REGS {
        let vals: Vec<u16> = (0..n).map(|i| 1000 + i as u16).collect();
        let (b, len) = encode_omv1(&vals).unwrap();
        let (got, count) = parse_omv1(&b[..len]).unwrap();
        assert_eq!(count as usize, n);
        assert_eq!(&got[..n], &vals[..]);
    }
    assert!(encode_omv1(&[]).is_none(), "an empty reply is not a reply");
    assert!(encode_omv1(&vec![0u16; MAX_READ_REGS + 1]).is_none());

    // A reply claiming 4 registers while carrying 2 is refused, not zero-padded: a zero
    // the operator believes is a reading is worse than no reading.
    let (b, len) = encode_omv1(&[7, 8]).unwrap();
    let mut lying = b[..len].to_vec();
    lying[4] = 4;
    assert_eq!(parse_omv1(&lying), None);
    for n in 0..len {
        assert_eq!(parse_omv1(&b[..n]), None, "a {n}-byte prefix is not a reply");
    }
}

#[test]
fn rd_parse_tcp_read_accepts_fc03_and_fc04_and_nothing_else() {
    let mut req = [0u8, 7, 0, 0, 0, 6, UNIT, FC_READ_HOLDING, 0, 10, 0, 3];
    let got = parse_tcp_read(&req, GW).expect("a well-formed FC03 read");
    assert_eq!(got, MbQuery { gateway_id: GW, unit: UNIT, fc: FC_READ_HOLDING, start: 10, count: 3 });

    req[7] = FC_READ_INPUT;
    assert!(parse_tcp_read(&req, GW).is_some(), "FC04 parses so the rule can name it");

    // A write is not a read, a zero quantity is not a quantity, and a wrong length is not
    // a frame.
    req[7] = 0x06;
    assert_eq!(parse_tcp_read(&req, GW), None);
    req[7] = FC_READ_HOLDING;
    req[10] = 0;
    req[11] = 0;
    assert_eq!(parse_tcp_read(&req, GW), None);
    for n in 0..req.len() {
        assert_eq!(parse_tcp_read(&req[..n], GW), None);
    }
}

/// The device's answer has to match the request, and an exception must stay an exception.
#[test]
fn rd_check_read_response_rejects_a_mismatched_or_short_answer() {
    let (_, frame) = decide_read(&q(10, 2), GW, UNIT, &MAP, 0x2211);
    let f = frame.unwrap();
    let good = [0x22, 0x11, 0, 0, 0, 7, UNIT, FC_READ_HOLDING, 4, 0x01, 0x02, 0x03, 0x04];
    let (vals, n) = check_read_response(&f, &good, 2).expect("a matching answer");
    assert_eq!(n, 2);
    assert_eq!(&vals[..2], &[0x0102, 0x0304]);

    // An exception from the device.
    let exc = [0x22, 0x11, 0, 0, 0, 3, UNIT, FC_READ_HOLDING | 0x80, 0x02];
    assert_eq!(check_read_response(&f, &exc, 2), Err(Some(0x02)));

    // A different transaction, a short body, a byte count that lies, a truncated frame.
    let mut other = good;
    other[1] = 0x12;
    assert_eq!(check_read_response(&f, &other, 2), Err(None));
    let mut lying = good;
    lying[8] = 2;
    assert_eq!(check_read_response(&f, &lying, 2), Err(None));
    for k in 0..good.len() {
        assert!(check_read_response(&f, &good[..k], 2).is_err(), "a {k}-byte answer is not an answer");
    }
    // And the same bytes read as one register rather than two.
    assert_eq!(check_read_response(&f, &good, 1), Err(None));
}

#[test]
fn rd_hmi_read_response_echoes_the_transaction_and_the_values() {
    let query = q(10, 3);
    let (b, n) = hmi_read_response(&query, 0xABCD, &[1, 2, 3]).unwrap();
    assert_eq!(n, MBAP_LEN + 2 + 6);
    assert_eq!(&b[0..2], &[0xAB, 0xCD], "the HMI matches the reply by this field");
    assert_eq!(u16::from_be_bytes([b[4], b[5]]) as usize, n - 6);
    assert_eq!(b[6], UNIT);
    assert_eq!(b[7], FC_READ_HOLDING);
    assert_eq!(b[8], 6, "byte count is 2 per register");
    assert_eq!(&b[9..15], &[0, 1, 0, 2, 0, 3], "values big-endian, in order");
    assert!(hmi_read_response(&query, 1, &[]).is_none());
    assert!(hmi_read_response(&query, 1, &vec![0u16; MAX_READ_REGS + 1]).is_none());
}
