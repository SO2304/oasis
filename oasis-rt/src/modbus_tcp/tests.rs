use super::*;
use crate::mesh::{inner_slice, mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter};
use crate::modbus_gateway::{encode_omb1, parse_omb1, OrderContext};
use rmodbus::server::context::ModbusContext;
use rmodbus::server::storage::ModbusStorageSmall;
use rmodbus::server::ModbusFrame;
use rmodbus::{ModbusFrameBuf, ModbusProto};

const NET: [u8; 8] = *b"OASISnet";
const UNIT: u8 = 0x11;
const MAP: [RegRule; 3] = [RegRule { addr: 0x0010, min: 50, max: 300 }, RegRule { addr: 0x0011, min: 0, max: 1 }, RegRule { addr: 0x0012, min: 0, max: 100 }];

fn order(seq: u32, start: u16, vals: &[u16]) -> MbOrder {
    let mut values = [0u16; MAX_REGS];
    values[..vals.len()].copy_from_slice(vals);
    MbOrder {
        gateway_id: 1,
        cmd_seq: seq,
        boot_id: 7,
        deadline_ms: 3_000,
        unit: UNIT,
        fc: if vals.len() == 1 { FC_WRITE_SINGLE } else { FC_WRITE_MULTIPLE },
        start,
        count: vals.len() as u8,
        values,
    }
}

fn ctx() -> OrderContext {
    OrderContext { v0b_ok: true, authorized: true, revoked: false, actuator_boot_id: 7, now_ms: 1_000, r14_safe: true }
}

/// The PLC: an independent Modbus TCP server (rmodbus). Returns its response.
fn plc(store: &mut ModbusStorageSmall, frame: &[u8]) -> Vec<u8> {
    let mut buf: ModbusFrameBuf = [0; 256];
    buf[..frame.len()].copy_from_slice(frame);
    let mut resp = Vec::new();
    let mut f = ModbusFrame::new(UNIT, &buf, ModbusProto::TcpUdp, &mut resp);
    f.parse().expect("rmodbus parses the TCP frame");
    if f.processing_required {
        let r = if f.readonly { f.process_read(store) } else { f.process_write(store) };
        r.expect("rmodbus processes the frame");
    }
    if f.response_required {
        f.finalize_response().unwrap();
    }
    resp
}

/// What an HMI sends: MBAP + FC06 or FC16.
fn hmi_write(tid: u16, start: u16, vals: &[u16]) -> Vec<u8> {
    let mut pdu = Vec::new();
    if vals.len() == 1 {
        pdu.push(FC_WRITE_SINGLE);
        pdu.extend_from_slice(&start.to_be_bytes());
        pdu.extend_from_slice(&vals[0].to_be_bytes());
    } else {
        pdu.push(FC_WRITE_MULTIPLE);
        pdu.extend_from_slice(&start.to_be_bytes());
        pdu.extend_from_slice(&(vals.len() as u16).to_be_bytes());
        pdu.push(2 * vals.len() as u8);
        for v in vals {
            pdu.extend_from_slice(&v.to_be_bytes());
        }
    }
    let mut b = tid.to_be_bytes().to_vec();
    b.extend_from_slice(&[0, 0]);
    b.extend_from_slice(&((1 + pdu.len()) as u16).to_be_bytes());
    b.push(UNIT);
    b.extend_from_slice(&pdu);
    b
}

#[test]
fn mbtcp_frames_applied_by_rmodbus() {
    let mut store = ModbusStorageSmall::default();
    let (d, _, f) = gateway_decision_tcp(&ctx(), &order(1, 0x0010, &[215]), UNIT, &MAP, None, 0x1234);
    assert_eq!(d, Decision::Act);
    let f = f.unwrap();
    assert_eq!(f.as_slice(), &hmi_write(0x1234, 0x0010, &[215])[..], "same bytes an HMI would send");
    let resp = plc(&mut store, f.as_slice());
    assert_eq!(check_tcp_response(&f, &resp), Response::Ack);
    assert_eq!(store.get_holding(0x0010).unwrap(), 215);

    let (_, _, f) = gateway_decision_tcp(&ctx(), &order(2, 0x0010, &[100, 1, 40]), UNIT, &MAP, Some(1), 7);
    let f = f.unwrap();
    let resp = plc(&mut store, f.as_slice());
    assert_eq!(check_tcp_response(&f, &resp), Response::Ack);
    assert_eq!([store.get_holding(0x0010).unwrap(), store.get_holding(0x0011).unwrap(), store.get_holding(0x0012).unwrap()], [100, 1, 40]);
}

#[test]
fn mbtcp_each_refusal_builds_no_frame() {
    let o = order(5, 0x0010, &[200]);
    let cases: [(OrderContext, MbOrder, Option<u32>); 8] = [
        (OrderContext { v0b_ok: false, ..ctx() }, o, None),
        (OrderContext { authorized: false, ..ctx() }, o, None),
        (OrderContext { revoked: true, ..ctx() }, o, None),
        (OrderContext { now_ms: 4_000, ..ctx() }, o, None),
        (OrderContext { r14_safe: false, ..ctx() }, o, None),
        (ctx(), order(5, 0x0013, &[1]), None),
        (ctx(), order(5, 0x0010, &[301]), None),
        (ctx(), o, Some(5)),
    ];
    for (c, o, last) in cases {
        let (d, _, f) = gateway_decision_tcp(&c, &o, UNIT, &MAP, last, 1);
        assert!(matches!(d, Decision::Reject(_)));
        assert!(f.is_none());
    }
}

#[test]
fn mbtcp_parse_hmi_writes() {
    let w = parse_tcp_write(&hmi_write(9, 0x0010, &[215])).unwrap();
    assert_eq!((w.tid, w.unit, w.fc, w.start, w.count, w.values[0]), (9, UNIT, FC_WRITE_SINGLE, 0x0010, 1, 215));
    let w = parse_tcp_write(&hmi_write(10, 0x0010, &[1, 2, 3, 4, 5, 6, 7, 8])).unwrap();
    assert_eq!((w.fc, w.count, w.values), (FC_WRITE_MULTIPLE, 8, [1, 2, 3, 4, 5, 6, 7, 8]));
    // the order the agent signs round-trips through OMB1
    let o = w.to_order(1, 42, 7, 3_000);
    let (b, n) = encode_omb1(&o).unwrap();
    assert_eq!(parse_omb1(&b[..n]), Some(o));
}

#[test]
fn mbtcp_parse_rejects_malformed() {
    let good = hmi_write(1, 0x0010, &[215]);
    assert!(parse_tcp_write(&good).is_some());
    let mut bad_proto = good.clone();
    bad_proto[3] = 1;
    let mut bad_len = good.clone();
    bad_len[5] += 1;
    let mut fc05 = good.clone();
    fc05[7] = 0x05;
    let read = [0, 1, 0, 0, 0, 6, UNIT, 0x03, 0x00, 0x10, 0x00, 0x01];
    let mut bc = hmi_write(1, 0x0010, &[1, 2]);
    bc[12] = 3;
    let nine = hmi_write(1, 0x0010, &[0; 9]);
    let zero = [0, 1, 0, 0, 0, 7, UNIT, 0x10, 0x00, 0x10, 0x00, 0x00, 0x00];
    for b in [&bad_proto[..], &bad_len, &fc05, &read, &bc, &nine, &zero, &good[..good.len() - 1], &[]] {
        assert_eq!(parse_tcp_write(b), None, "{b:02x?}");
    }
}

#[test]
fn mbtcp_response_checks() {
    let (_, _, f) = gateway_decision_tcp(&ctx(), &order(1, 0x0010, &[215]), UNIT, &MAP, None, 0x0102);
    let f = f.unwrap();
    let ack = f.as_slice().to_vec();
    assert_eq!(check_tcp_response(&f, &ack), Response::Ack);
    let mut other_tid = ack.clone();
    other_tid[1] ^= 1;
    assert_eq!(check_tcp_response(&f, &other_tid), Response::Mismatch);
    let mut other_value = ack.clone();
    other_value[11] ^= 1;
    assert_eq!(check_tcp_response(&f, &other_value), Response::Mismatch);
    let exc = [0x01, 0x02, 0, 0, 0, 3, UNIT, 0x86, 0x02];
    assert_eq!(check_tcp_response(&f, &exc), Response::Exception(0x02));
    let mut bad_len = exc;
    bad_len[5] = 4;
    assert_eq!(check_tcp_response(&f, &bad_len), Response::Mismatch);
    assert_eq!(check_tcp_response(&f, &ack[..8]), Response::Short);
}

#[test]
fn mbtcp_hmi_responses() {
    let w = parse_tcp_write(&hmi_write(77, 0x0010, &[215])).unwrap();
    let (b, n) = hmi_response(&w, Outcome::Done);
    assert_eq!(&b[..n], &hmi_write(77, 0x0010, &[215])[..], "FC06 echo");
    let w16 = parse_tcp_write(&hmi_write(78, 0x0010, &[1, 0])).unwrap();
    let (b, n) = hmi_response(&w16, Outcome::Done);
    assert_eq!(&b[..n], &[0, 78, 0, 0, 0, 6, UNIT, 0x10, 0x00, 0x10, 0x00, 0x02]);
    let exc = |o| {
        let (b, n) = hmi_response(&w, o);
        assert_eq!((n, &b[..7], b[7]), (EXC_LEN, &[0, 77, 0, 0, 0, 3, UNIT][..], 0x86));
        b[8]
    };
    assert_eq!(exc(Outcome::Refused(Reason::OutOfLimits, RuleCheck::RegisterNotAllowed(0x13))), EXC_ILLEGAL_ADDRESS);
    assert_eq!(exc(Outcome::Refused(Reason::OutOfLimits, RuleCheck::ValueOutOfRange(0x10, 301))), EXC_ILLEGAL_VALUE);
    assert_eq!(exc(Outcome::Refused(Reason::NotAuthorized, RuleCheck::Ok)), EXC_GATEWAY_PATH_UNAVAILABLE);
    assert_eq!(exc(Outcome::PlcException(0x04)), 0x04);
    assert_eq!(exc(Outcome::NoAnswer), EXC_GATEWAY_TARGET_FAILED);
}

fn node(id: u8, seed: u8, peers: &[(u8, u8)]) -> MeshRouter {
    let mut r = MeshPubRegistry::new();
    for &(p, s) in peers {
        r.insert([p; 8], mesh_v10_pubkey_from_seed(&MeshEdSeed([s; 32])).unwrap());
    }
    MeshRouter::new_v0b([id; 8], NET, MeshEdSeed([seed; 32]), r)
}

#[test]
fn mbtcp_end_to_end_hmi_agent_gateway_plc() {
    // HMI -> agent (0xAA, signs) -> gateway (0xCC, verifies + decides) -> PLC (rmodbus TCP).
    // The PLC sees exactly the writes of `Act` decisions; the HMI always gets an answer.
    let mut agent = node(0xAA, 0xAA, &[]);
    let mut forger = node(0xAA, 0xEE, &[]); // claims the agent's fingerprint, other key
    let mut gwr = node(0xCC, 0xCC, &[(0xAA, 0xAA)]);
    let mut gw = Gateway::new();
    let mut store = ModbusStorageSmall::default();
    let mut plc_writes = 0;
    let mut seq = 0u32;
    let mut via_gateway = |env: &[u8], w: &TcpWrite, gwr: &mut MeshRouter, gw: &mut Gateway, store: &mut ModbusStorageSmall| -> Vec<u8> {
        let (v0b_ok, inner) = match gwr.process(env) {
            MeshDecision::Arrived { envelope, .. } => (true, inner_slice(&envelope).to_vec()),
            MeshDecision::Drop(_) => (false, Vec::new()),
        };
        let outcome = match parse_omb1(&inner) {
            None => Outcome::NoAnswer, // nothing verifiable reached the gate
            Some(o) => match decide_tcp(gw, &OrderContext { v0b_ok, ..ctx() }, &o, UNIT, &MAP, 0x5000 + o.cmd_seq as u16) {
                (Decision::Act, _, Some(f)) => {
                    plc_writes += 1;
                    match check_tcp_response(&f, &plc(store, f.as_slice())) {
                        Response::Ack => Outcome::Done,
                        Response::Exception(c) => Outcome::PlcException(c),
                        _ => Outcome::NoAnswer,
                    }
                }
                (Decision::Reject(r), rules, _) => Outcome::Refused(r, rules),
                (Decision::Act, _, None) => unreachable!("Act always carries a frame"),
            },
        };
        let (b, n) = hmi_response(w, outcome);
        b[..n].to_vec()
    };
    let mut sign = |agent: &mut MeshRouter, w: &TcpWrite| {
        seq += 1;
        let (p, n) = encode_omb1(&w.to_order(1, seq, 7, 3_000)).unwrap();
        (agent.origin_wrap_v0b(&p[..n]).unwrap().to_vec(), p, n)
    };

    // 1. a valid HMI write reaches the PLC and the HMI gets the echo
    let w1 = parse_tcp_write(&hmi_write(1, 0x0010, &[215])).unwrap();
    let (env1, _, _) = sign(&mut agent, &w1);
    assert_eq!(via_gateway(&env1, &w1, &mut gwr, &mut gw, &mut store), hmi_write(1, 0x0010, &[215]));
    assert_eq!(store.get_holding(0x0010).unwrap(), 215);
    // 2. the same envelope replayed: dropped by v0B, the HMI gets "target failed"
    assert_eq!(via_gateway(&env1, &w1, &mut gwr, &mut gw, &mut store)[8], EXC_GATEWAY_TARGET_FAILED);
    // 3. out of range: refused by the rules, HMI gets "illegal value"
    let w3 = parse_tcp_write(&hmi_write(3, 0x0010, &[301])).unwrap();
    let (env3, _, _) = sign(&mut agent, &w3);
    assert_eq!(via_gateway(&env3, &w3, &mut gwr, &mut gw, &mut store)[8], EXC_ILLEGAL_VALUE);
    // 4. unlisted register: "illegal address"
    let w4 = parse_tcp_write(&hmi_write(4, 0x0020, &[1])).unwrap();
    let (env4, _, _) = sign(&mut agent, &w4);
    assert_eq!(via_gateway(&env4, &w4, &mut gwr, &mut gw, &mut store)[8], EXC_ILLEGAL_ADDRESS);
    // 5. forged origin: dropped before the gate
    let w5 = parse_tcp_write(&hmi_write(5, 0x0010, &[300])).unwrap();
    let (env5, _, _) = sign(&mut forger, &w5);
    assert_eq!(via_gateway(&env5, &w5, &mut gwr, &mut gw, &mut store)[8], EXC_GATEWAY_TARGET_FAILED);
    // 6. modified value inside a genuine envelope (300 -> 301 in the payload)
    let (mut env6, p6, n6) = sign(&mut agent, &w5);
    let at = env6.windows(n6).position(|x| x == &p6[..n6]).unwrap() + n6 - 2;
    env6[at] ^= 0x01;
    assert_eq!(via_gateway(&env6, &w5, &mut gwr, &mut gw, &mut store)[8], EXC_GATEWAY_TARGET_FAILED);
    // 7. a raw Modbus TCP write wrapped as a valid envelope: not an order
    let raw = agent.origin_wrap_v0b(&hmi_write(7, 0x0010, &[50])).unwrap().to_vec();
    assert_eq!(via_gateway(&raw, &w5, &mut gwr, &mut gw, &mut store)[8], EXC_GATEWAY_TARGET_FAILED);
    assert_eq!(store.get_holding(0x0010).unwrap(), 215, "PLC untouched by every refusal");
    // 8. a later valid FC16 still goes through
    let w8 = parse_tcp_write(&hmi_write(8, 0x0010, &[120, 1])).unwrap();
    let (env8, _, _) = sign(&mut agent, &w8);
    assert_eq!(via_gateway(&env8, &w8, &mut gwr, &mut gw, &mut store), vec![0, 8, 0, 0, 0, 6, UNIT, 0x10, 0x00, 0x10, 0x00, 0x02]);
    assert_eq!([store.get_holding(0x0010).unwrap(), store.get_holding(0x0011).unwrap()], [120, 1]);
    assert_eq!(plc_writes, 2);
    assert_eq!(gw.act.executed, 2);
}
