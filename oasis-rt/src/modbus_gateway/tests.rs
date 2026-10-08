use super::*;
use crate::actuation::Reason;
use crate::mesh::{inner_slice, mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter};
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

/// The device: an independent Modbus RTU server (rmodbus). Returns its response.
fn device(store: &mut ModbusStorageSmall, frame: &[u8]) -> Vec<u8> {
    let mut buf: ModbusFrameBuf = [0; 256];
    buf[..frame.len()].copy_from_slice(frame);
    let mut resp = Vec::new();
    let mut f = ModbusFrame::new(UNIT, &buf, ModbusProto::Rtu, &mut resp);
    f.parse().expect("rmodbus parses the frame (CRC included)");
    if f.processing_required {
        let r = if f.readonly { f.process_read(store) } else { f.process_write(store) };
        r.expect("rmodbus processes the frame");
    }
    if f.response_required {
        f.finalize_response().unwrap();
    }
    resp
}

#[test]
fn mb_omb1_roundtrip() {
    for vals in [&[215u16][..], &[215, 1, 40][..], &[1, 2, 3, 4, 5, 6, 7, 8][..]] {
        let o = order(9, 0x0010, vals);
        let (b, n) = encode_omb1(&o).unwrap();
        assert_eq!(n, OMB1_HEADER_LEN + 2 * vals.len());
        assert_eq!(parse_omb1(&b[..n]), Some(o));
    }
}

#[test]
fn mb_omb1_rejects_malformed() {
    let o = order(1, 0x0010, &[215]);
    let (b, n) = encode_omb1(&o).unwrap();
    assert_eq!(parse_omb1(&b[..n - 1]), None, "short");
    let mut long = b[..n].to_vec();
    long.push(0);
    assert_eq!(parse_omb1(&long), None, "long");
    let mut bad = b[..n].to_vec();
    bad[0] = b'X';
    assert_eq!(parse_omb1(&bad), None, "magic");
    for fc in [0x03u8, 0x05, 0x08, 0x0F, 0x16, 0x17] {
        let mut x = b[..n].to_vec();
        x[27] = fc;
        assert_eq!(parse_omb1(&x), None, "fc {fc:#x}");
    }
    let mut x = b[..n].to_vec();
    x[30] = 2; // FC06 with count 2 (length still matches count 1)
    assert_eq!(parse_omb1(&x), None);
    assert_eq!(encode_omb1(&MbOrder { count: 0, ..o }), None);
    assert_eq!(encode_omb1(&MbOrder { fc: FC_WRITE_MULTIPLE, count: 9, ..o }), None);
}

#[test]
fn mb_crc_catalogue_check_value() {
    // CRC-16/MODBUS catalogue check value (reveng): "123456789" -> 0x4B37.
    assert_eq!(crc16(b"123456789"), 0x4B37);
}

#[test]
fn mb_frames_applied_by_rmodbus() {
    let mut store = ModbusStorageSmall::default();
    let mut gw = Gateway::new();
    let (d, rules, f) = gw.decide(&ctx(), &order(1, 0x0010, &[215]), UNIT, &MAP);
    assert_eq!((d, rules), (Decision::Act, RuleCheck::Ok));
    let f = f.unwrap();
    assert_eq!(f.as_slice()[..6], [UNIT, 0x06, 0x00, 0x10, 0x00, 0xD7]);
    let resp = device(&mut store, f.as_slice());
    assert_eq!(store.get_holding(0x0010).unwrap(), 215);
    assert_eq!(check_response(&f, &resp), Response::Ack);

    let (d, _, f) = gw.decide(&ctx(), &order(2, 0x0010, &[180, 1, 40]), UNIT, &MAP);
    assert_eq!(d, Decision::Act);
    let f = f.unwrap();
    let resp = device(&mut store, f.as_slice());
    assert_eq!([0x10, 0x11, 0x12].map(|r| store.get_holding(r).unwrap()), [180, 1, 40], "rmodbus applied exactly the order");
    assert_eq!(store.get_holding(0x0013).unwrap(), 0, "nothing else written");
    assert_eq!(check_response(&f, &resp), Response::Ack);
}

#[test]
fn mb_device_exception_reported() {
    // A map that (wrongly) allows a register the device does not have: the device
    // answers IllegalDataAddress (0x02) and the gateway reports it.
    let map = [RegRule { addr: 2000, min: 0, max: 10 }];
    let mut store = ModbusStorageSmall::default();
    let (d, _, f) = gateway_decision(&ctx(), &order(1, 2000, &[5]), UNIT, &map, None);
    assert_eq!(d, Decision::Act);
    let f = f.unwrap();
    let mut buf: ModbusFrameBuf = [0; 256];
    buf[..f.len].copy_from_slice(f.as_slice());
    let mut resp = Vec::new();
    let mut fr = ModbusFrame::new(UNIT, &buf, ModbusProto::Rtu, &mut resp);
    fr.parse().unwrap();
    if let Err(e) = fr.process_write(&mut store) {
        fr.set_modbus_error_if_unset(&e).unwrap();
    }
    fr.finalize_response().unwrap();
    assert_eq!(check_response(&f, &resp), Response::Exception(0x02));
}

#[test]
fn mb_response_checks() {
    let (_, _, f) = gateway_decision(&ctx(), &order(1, 0x0010, &[215]), UNIT, &MAP, None);
    let f = f.unwrap();
    let echo = f.as_slice().to_vec();
    assert_eq!(check_response(&f, &echo), Response::Ack);
    let mut bad = echo.clone();
    bad[7] ^= 1;
    assert_eq!(check_response(&f, &bad), Response::BadCrc);
    assert_eq!(check_response(&f, &echo[..4]), Response::Short);
    // a valid answer to another request (register 0x0011)
    let (_, _, g) = gateway_decision(&ctx(), &order(1, 0x0011, &[1]), UNIT, &MAP, None);
    assert_eq!(check_response(&f, g.unwrap().as_slice()), Response::Mismatch);
    // exception for another function code
    let mut exc = vec![UNIT, 0x90, 0x02];
    exc.extend_from_slice(&crc16(&exc).to_le_bytes());
    assert_eq!(check_response(&f, &exc), Response::Mismatch);
}

#[test]
fn mb_register_rules() {
    let c = |o: &MbOrder| check_rules(o, UNIT, &MAP);
    assert_eq!(c(&order(1, 0x0010, &[50])), RuleCheck::Ok);
    assert_eq!(c(&order(1, 0x0010, &[300])), RuleCheck::Ok);
    assert_eq!(c(&order(1, 0x0010, &[49])), RuleCheck::ValueOutOfRange(0x0010, 49));
    assert_eq!(c(&order(1, 0x0010, &[301])), RuleCheck::ValueOutOfRange(0x0010, 301));
    assert_eq!(c(&order(1, 0x0010, &[900])), RuleCheck::ValueOutOfRange(0x0010, 900));
    assert_eq!(c(&order(1, 0x0011, &[2])), RuleCheck::ValueOutOfRange(0x0011, 2));
    assert_eq!(c(&order(1, 0x0020, &[1])), RuleCheck::RegisterNotAllowed(0x0020));
    // starts inside the map, runs past it
    assert_eq!(c(&order(1, 0x0011, &[1, 50, 0])), RuleCheck::RegisterNotAllowed(0x0013));
    assert_eq!(c(&MbOrder { unit: 0x12, ..order(1, 0x0010, &[215]) }), RuleCheck::WrongUnit);
    // no wrap past 0xFFFF even if the map holds 0xFFFF and 0x0000
    let map = [RegRule { addr: 0xFFFF, min: 0, max: 9 }, RegRule { addr: 0, min: 0, max: 9 }];
    assert_eq!(check_rules(&order(1, 0xFFFF, &[1, 1]), UNIT, &map), RuleCheck::RegisterNotAllowed(0xFFFF));
}

#[test]
fn mb_each_gate_condition_blocks_the_frame() {
    let o = order(1, 0x0010, &[215]);
    let cases: [(OrderContext, MbOrder, Reason); 8] = [
        (OrderContext { v0b_ok: false, ..ctx() }, o, Reason::NotVerified),
        (OrderContext { authorized: false, ..ctx() }, o, Reason::NotAuthorized),
        (OrderContext { revoked: true, ..ctx() }, o, Reason::Revoked),
        (OrderContext { actuator_boot_id: 8, ..ctx() }, o, Reason::Expired),
        (OrderContext { now_ms: 3_001, ..ctx() }, o, Reason::Expired),
        (ctx(), MbOrder { deadline_ms: 1_000 + 10_001, ..o }, Reason::Expired),
        (OrderContext { r14_safe: false, ..ctx() }, o, Reason::R14Unsafe),
        (ctx(), order(1, 0x0010, &[900]), Reason::OutOfLimits),
    ];
    for (c, o, r) in cases {
        let (d, _, f) = gateway_decision(&c, &o, UNIT, &MAP, None);
        assert_eq!(d, Decision::Reject(r));
        assert!(f.is_none(), "no frame on {r:?}");
    }
}

#[test]
fn mb_gateway_replay_and_counters() {
    let mut gw = Gateway::new();
    assert_eq!(gw.decide(&ctx(), &order(5, 0x0010, &[215]), UNIT, &MAP).0, Decision::Act);
    let (d, _, f) = gw.decide(&ctx(), &order(5, 0x0010, &[215]), UNIT, &MAP);
    assert_eq!(d, Decision::Reject(Reason::StaleOrReplayed));
    assert!(f.is_none());
    assert_eq!(gw.decide(&ctx(), &order(4, 0x0010, &[216]), UNIT, &MAP).0, Decision::Reject(Reason::StaleOrReplayed));
    assert_eq!(gw.decide(&ctx(), &order(6, 0x0020, &[1]), UNIT, &MAP).0, Decision::Reject(Reason::OutOfLimits));
    // an out-of-limits order does not consume its sequence number
    assert_eq!(gw.decide(&ctx(), &order(6, 0x0010, &[216]), UNIT, &MAP).0, Decision::Act);
    assert_eq!(gw.act.executed, 2);
    assert_eq!(gw.act.rejects[Reason::StaleOrReplayed as usize], 2);
    assert_eq!(gw.act.rejects[Reason::OutOfLimits as usize], 1);
}

fn node(id: u8, seed: u8, peers: &[(u8, u8)]) -> MeshRouter {
    let mut r = MeshPubRegistry::new();
    for &(p, s) in peers {
        r.insert([p; 8], mesh_v10_pubkey_from_seed(&MeshEdSeed([s; 32])).unwrap());
    }
    MeshRouter::new_v0b([id; 8], NET, MeshEdSeed([seed; 32]), r)
}

#[test]
fn mb_end_to_end_over_v0b() {
    // B (0xBB) orders, C (0xCC) is the gateway, rmodbus is the device. The only
    // writes the device ever sees are the frames of `Act` decisions.
    let mut c = node(0xCC, 0xCC, &[(0xBB, 0xBB)]);
    let mut b = node(0xBB, 0xBB, &[]);
    let mut forger = node(0xBB, 0xEE, &[]); // claims B's fingerprint, other key
    let mut gw = Gateway::new();
    let mut store = ModbusStorageSmall::default();
    let mut frames_to_device = 0;
    let mut run = |env: &[u8], c: &mut MeshRouter, gw: &mut Gateway, store: &mut ModbusStorageSmall| -> Option<Decision> {
        let (v0b_ok, inner) = match c.process(env) {
            MeshDecision::Arrived { envelope, .. } => (true, inner_slice(&envelope).to_vec()),
            MeshDecision::Drop(_) => (false, Vec::new()),
        };
        let o = parse_omb1(&inner)?; // not an order: nothing to decide, nothing sent
        let (d, _, f) = gw.decide(&OrderContext { v0b_ok, ..ctx() }, &o, UNIT, &MAP);
        if let Some(f) = f {
            device(store, f.as_slice());
            frames_to_device += 1;
        }
        Some(d)
    };
    let (p1, n1) = encode_omb1(&order(1, 0x0010, &[215])).unwrap();
    let ok = b.origin_wrap_v0b(&p1[..n1]).unwrap();
    assert_eq!(run(&ok, &mut c, &mut gw, &mut store), Some(Decision::Act));
    assert_eq!(store.get_holding(0x0010).unwrap(), 215);

    // replayed byte for byte: dropped by the v0B window, nothing parsed
    assert_eq!(run(&ok, &mut c, &mut gw, &mut store), None);
    // same order in a fresh envelope: the gate refuses the sequence number
    let again = b.origin_wrap_v0b(&p1[..n1]).unwrap();
    assert_eq!(run(&again, &mut c, &mut gw, &mut store), Some(Decision::Reject(Reason::StaleOrReplayed)));
    // forged: B's fingerprint, another key
    let (p2, n2) = encode_omb1(&order(2, 0x0010, &[300])).unwrap();
    let forged = forger.origin_wrap_v0b(&p2[..n2]).unwrap();
    assert_eq!(run(&forged, &mut c, &mut gw, &mut store), None);
    // modified: one payload byte of a genuine envelope (the value: 300 -> 301)
    let mut modified = b.origin_wrap_v0b(&p2[..n2]).unwrap().to_vec();
    let at = modified.windows(n2).position(|w| w == &p2[..n2]).expect("payload inside the envelope") + n2 - 2;
    modified[at] ^= 0x01;
    assert_eq!(run(&modified, &mut c, &mut gw, &mut store), None);
    // a raw Modbus write as the payload of a valid envelope: not an order
    let mut raw = vec![UNIT, 0x06, 0x00, 0x10, 0x03, 0x84];
    raw.extend_from_slice(&crc16(&raw).to_le_bytes());
    let wrapped = b.origin_wrap_v0b(&raw).unwrap();
    assert_eq!(run(&wrapped, &mut c, &mut gw, &mut store), None);
    assert_eq!(store.get_holding(0x0010).unwrap(), 215, "device untouched by every refusal");
    // a later valid order still goes through
    let (p3, n3) = encode_omb1(&order(3, 0x0010, &[220])).unwrap();
    let next = b.origin_wrap_v0b(&p3[..n3]).unwrap();
    assert_eq!(run(&next, &mut c, &mut gw, &mut store), Some(Decision::Act));
    assert_eq!(store.get_holding(0x0010).unwrap(), 220);
    assert_eq!(frames_to_device, 2);
    assert_eq!(gw.act.executed, 2);
}

/// The silicon run of 2026-10-08 caught the gateway writing to the device while the stop
/// latch was set: `gateway_decision` read no actuator context, so setting the flag on
/// `Gateway::act` changed nothing. A latched stop must refuse the order and, above all,
/// produce **no frame** — the frame is the only thing that ever reaches the device.
#[test]
fn mb_latched_stop_refuses_and_emits_no_frame() {
    let mut gw = Gateway::new();
    let c = ctx();
    let (d, _, frame) = gw.decide(&c, &order(1, 0x0010, &[231]), UNIT, &MAP);
    assert_eq!(d, Decision::Act);
    assert!(frame.is_some(), "a legitimate order yields a frame");

    gw.act.stopped = true;
    let (d2, _, frame2) = gw.decide(&c, &order(2, 0x0010, &[232]), UNIT, &MAP);
    assert_eq!(d2, Decision::Reject(Reason::Stopped));
    assert!(frame2.is_none(), "no frame may be built while a stop is latched");

    gw.act.clear_stop();
    let (d3, _, frame3) = gw.decide(&c, &order(3, 0x0010, &[233]), UNIT, &MAP);
    assert_eq!(d3, Decision::Act);
    assert!(frame3.is_some(), "after a local clear the gateway works again");
}

/// Same for part H: a dead supervision link must stop the gateway too, not only the LED.
#[test]
fn mb_dead_supervision_refuses_and_emits_no_frame() {
    let mut gw = Gateway::new();
    gw.act.supervision_required = true;
    let c = ctx();
    let (d, _, frame) = gw.decide(&c, &order(1, 0x0010, &[231]), UNIT, &MAP);
    assert_eq!(d, Decision::Reject(Reason::SupervisionLost));
    assert!(frame.is_none());
}
