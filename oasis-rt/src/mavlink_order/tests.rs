use super::*;
use crate::actuation::{encode_oac1, encode_oac1_with_class, parse_oac1_any, Reason, OAC1_CLASS_OFF, OAC1_LEN};
use crate::mavlink_min::parse_frame;
use crate::mesh::{inner_slice, mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter, FP_LEN, MESH_V0B_HEADER_LEN, MESH_V0B_NETWORK_LEN};

const NET: [u8; MESH_V0B_NETWORK_LEN] = *b"OASISnet";
const VEHICLE: u16 = 1;

fn fp(i: u8) -> [u8; FP_LEN] {
    [i, 0, 0, 0, 0, 0, 0, 0]
}

fn ed_seed(byte: u8) -> MeshEdSeed {
    let mut s = [0u8; 32];
    for (i, b) in s.iter_mut().enumerate() {
        *b = byte.wrapping_add(i as u8);
    }
    MeshEdSeed(s)
}

fn registry(entries: &[([u8; FP_LEN], &MeshEdSeed)]) -> MeshPubRegistry {
    let mut r = MeshPubRegistry::new();
    for (f, seed) in entries {
        r.insert(*f, mesh_v10_pubkey_from_seed(seed).unwrap());
    }
    r
}

/// The commander (fp 1) and the vehicle's router, which trusts fp 1 only.
fn commander_and_vehicle_router() -> (MeshRouter, MeshRouter) {
    (MeshRouter::new_v0b(fp(1), NET, ed_seed(1), registry(&[])), MeshRouter::new_v0b(fp(2), NET, ed_seed(2), registry(&[(fp(1), &ed_seed(1))])))
}

fn rules() -> ArmRules {
    ArmRules { vehicle_id: VEHICLE }
}

fn arm_order(seq: u32) -> ActCommand {
    ActCommand {
        actuator_id: VEHICLE,
        cmd_seq: seq,
        boot_id: 7,
        // Within MAX_VALIDITY_MS of now_ms: the gate refuses a deadline more than 10 s
        // ahead, so a pre-signed arming order cannot be kept for later.
        deadline_ms: 4_000,
        force: 1.0,
        torque: 0.0,
        velocity: 0.0,
        pos: [0.0, 0.0, 0.0],
    }
}

fn ok_ctx() -> ArmContext {
    ArmContext { v0b_ok: true, authorized: true, revoked: false, actuator_boot_id: 7, now_ms: 1_000, r14_safe: true }
}

/// The ARM command as PX4 would read it back off the wire.
fn parse_arm(frame: &[u8]) -> (u16, f32) {
    let (h, _) = parse_frame(frame).expect("a COMMAND_LONG frame");
    assert_eq!(h.msgid, 76, "COMMAND_LONG");
    let p = &frame[10..10 + h.len as usize];
    (u16::from_le_bytes([p[28], p[29]]), f32::from_le_bytes([p[0], p[1], p[2], p[3]]))
}

/// What became of one received MAVLink frame.
#[derive(Debug, PartialEq)]
enum Rx {
    /// Not a frame of ours, or its MAVLink CRC was wrong.
    NotOurs,
    /// The mesh layer refused it — the gate was never reached.
    Dropped(&'static str),
    /// v0B verified, so the gate decided.
    Decided(Decision),
}

/// The whole vehicle side: unwrap the MAVLink frame, verify v0B, run the gate. The ARM
/// frame comes back only if the gate said `Act`.
fn vehicle_rx(v: &mut Vehicle, router: &mut MeshRouter, frame: &[u8], revoked: bool, r14_safe: bool, now_ms: u64) -> (Rx, Option<Vec<u8>>) {
    let env = match extract_envelope(frame, OASIS_MESSAGE_TYPE) {
        Some(e) => e,
        None => return (Rx::NotOurs, None),
    };
    let arrived = match router.process(&env) {
        MeshDecision::Drop(why) => return (Rx::Dropped(why), None),
        MeshDecision::Arrived { envelope, .. } => envelope,
    };
    let (cmd, class) = match parse_oac1_any(inner_slice(&arrived)) {
        Some(x) => x,
        None => return (Rx::Dropped("not an OAC1 order"), None),
    };
    let ctx = ArmContext { revoked, r14_safe, now_ms, ..ok_ctx() };
    let (d, _, f) = v.decide(&ctx, &cmd, class);
    (Rx::Decided(d), f)
}

fn carried(commander: &mut MeshRouter, cmd: &ActCommand) -> Vec<u8> {
    let env = commander.origin_wrap_v0b(&encode_oac1(cmd)).unwrap();
    encode_v2_extension(0, 1, 191, 0, 1, 1, OASIS_MESSAGE_TYPE, &env).unwrap()
}

// ─── the carrier ───

#[test]
fn mo_v2_extension_roundtrip_and_sizes() {
    // The size claim that picks the carrier, asserted so it cannot drift.
    let envelope_len = MESH_V0B_HEADER_LEN + OAC1_LEN;
    assert_eq!(envelope_len, 153, "a v0B envelope carrying an OAC1 order");
    assert_eq!(ENVELOPE_CAP, 247);
    assert!(envelope_len <= ENVELOPE_CAP, "fits in V2_EXTENSION");
    assert!(envelope_len > 128, "does NOT fit in TUNNEL's 128-byte payload");

    let env: Vec<u8> = (0..envelope_len).map(|i| (i % 251) as u8 | 1).collect();
    let f = encode_v2_extension(0, 1, 191, 0, 1, 1, OASIS_MESSAGE_TYPE, &env).unwrap();
    assert_eq!(f.len(), 12 + V2EXT_MSG_LEN);
    assert_eq!(extract_envelope(&f, OASIS_MESSAGE_TYPE).as_deref(), Some(&env[..]));

    // A different message_type is not ours.
    assert_eq!(extract_envelope(&f, OASIS_MESSAGE_TYPE ^ 1), None);
    // Too big to carry.
    assert_eq!(encode_v2_extension(0, 1, 191, 0, 1, 1, OASIS_MESSAGE_TYPE, &vec![1u8; ENVELOPE_CAP + 1]), None);
    // No single-byte corruption yields the same envelope: the MAVLink CRC or the
    // content changes, and neither is ever a wrong accept.
    for i in 0..f.len() {
        let mut c = f.clone();
        c[i] ^= 0x01;
        assert_ne!(extract_envelope(&c, OASIS_MESSAGE_TYPE).as_deref(), Some(&env[..]), "byte {} accepted", i);
    }
}

/// MAVLink 2 senders trim trailing zeros. The length prefix is why that is recoverable.
#[test]
fn mo_survives_mavlink2_zero_truncation() {
    let env: Vec<u8> = (0..153).map(|i| (i % 251) as u8 | 1).collect();
    let f = encode_v2_extension(3, 1, 191, 0, 1, 1, OASIS_MESSAGE_TYPE, &env).unwrap();
    let t = truncate_trailing_zeros(&f).unwrap();
    assert!(t.len() < f.len(), "the padding was trimmed: {} -> {}", f.len(), t.len());
    assert_eq!(t.len(), 12 + V2EXT_PAYLOAD_OFF + 2 + env.len(), "trimmed to the content");
    assert_eq!(extract_envelope(&t, OASIS_MESSAGE_TYPE).as_deref(), Some(&env[..]));

    // An envelope that itself ends in zeros is still reassembled byte for byte.
    let mut z = env.clone();
    let n = z.len();
    z[n - 3..].copy_from_slice(&[0, 0, 0]);
    let f2 = encode_v2_extension(4, 1, 191, 0, 1, 1, OASIS_MESSAGE_TYPE, &z).unwrap();
    let t2 = truncate_trailing_zeros(&f2).unwrap();
    assert_eq!(extract_envelope(&t2, OASIS_MESSAGE_TYPE).as_deref(), Some(&z[..]));
}

// ─── the four B1 cases, over a real v0B envelope ───

#[test]
fn mo_b1_a_valid_order_arms() {
    let (mut cmdr, mut router) = commander_and_vehicle_router();
    let mut v = Vehicle::new(rules());
    let frame = carried(&mut cmdr, &arm_order(10));

    let (rx, f) = vehicle_rx(&mut v, &mut router, &frame, false, true, 1_000);
    assert_eq!(rx, Rx::Decided(Decision::Act));
    assert_eq!(parse_arm(&f.expect("an ARM frame")), (MAV_CMD_COMPONENT_ARM_DISARM, 1.0));
    assert_eq!(v.act.executed, 1);
}

/// Forged, replayed and revoked, each one its own barrier. Two are stopped by the mesh
/// layer before the gate is reached; the revoked one is stopped by the gate. None of
/// the three produces a single byte of ARM.
#[test]
fn mo_b1_forged_replayed_and_revoked_do_not_arm() {
    // Forged — signed by a key the vehicle's registry does not hold.
    {
        let mut attacker = MeshRouter::new_v0b(fp(3), NET, ed_seed(3), registry(&[]));
        let (_, mut router) = commander_and_vehicle_router();
        let mut v = Vehicle::new(rules());
        let frame = carried(&mut attacker, &arm_order(10));
        let (rx, f) = vehicle_rx(&mut v, &mut router, &frame, false, true, 1_000);
        assert_eq!(rx, Rx::Dropped("unknown sender"));
        assert!(f.is_none(), "no ARM frame for a forged order");
        assert_eq!(v.act.executed, 0);
    }

    // Forged — a genuine envelope whose order was swapped afterwards, which is the v0A
    // attack v0B exists to stop.
    {
        let (mut cmdr, mut router) = commander_and_vehicle_router();
        let mut v = Vehicle::new(rules());
        let good = carried(&mut cmdr, &arm_order(10));
        let mut env = extract_envelope(&good, OASIS_MESSAGE_TYPE).unwrap();
        let n = env.len();
        env[n - 1] ^= 0x01;
        let tampered = encode_v2_extension(0, 1, 191, 0, 1, 1, OASIS_MESSAGE_TYPE, &env).unwrap();
        let (rx, f) = vehicle_rx(&mut v, &mut router, &tampered, false, true, 1_000);
        assert_eq!(rx, Rx::Dropped("bad mesh signature"));
        assert!(f.is_none(), "no ARM frame for a swapped order");
    }

    // Replayed — the same MAVLink frame, byte for byte, twice.
    {
        let (mut cmdr, mut router) = commander_and_vehicle_router();
        let mut v = Vehicle::new(rules());
        let frame = carried(&mut cmdr, &arm_order(10));
        let (rx1, f1) = vehicle_rx(&mut v, &mut router, &frame, false, true, 1_000);
        assert_eq!(rx1, Rx::Decided(Decision::Act));
        assert!(f1.is_some());
        let (rx2, f2) = vehicle_rx(&mut v, &mut router, &frame, false, true, 1_100);
        // "stale counter", not "duplicate": the refusal comes from the persisted 128-bit
        // counter window, which survives a reboot, and not from the RAM Bloom filter,
        // which does not. That is the stronger of the two barriers.
        assert_eq!(rx2, Rx::Dropped("stale counter"), "a byte-exact replay must not arm");
        assert!(f2.is_none(), "no second ARM frame");
        assert_eq!(v.act.executed, 1, "exactly one arming for one order");
    }

    // Replayed differently — a fresh envelope carrying an already-executed cmd_seq.
    // The mesh layer cannot see this one; the gate does.
    {
        let (mut cmdr, mut router) = commander_and_vehicle_router();
        let mut v = Vehicle::new(rules());
        let first = carried(&mut cmdr, &arm_order(10));
        assert_eq!(vehicle_rx(&mut v, &mut router, &first, false, true, 1_000).0, Rx::Decided(Decision::Act));
        let again = carried(&mut cmdr, &arm_order(10)); // new counter, same cmd_seq
        let (rx, f) = vehicle_rx(&mut v, &mut router, &again, false, true, 1_100);
        assert_eq!(rx, Rx::Decided(Decision::Reject(Reason::StaleOrReplayed)));
        assert!(f.is_none());
        assert_eq!(v.act.executed, 1);
    }

    // Revoked — v0B verifies, and the gate refuses.
    {
        let (mut cmdr, mut router) = commander_and_vehicle_router();
        let mut v = Vehicle::new(rules());
        let frame = carried(&mut cmdr, &arm_order(10));
        let (rx, f) = vehicle_rx(&mut v, &mut router, &frame, true, true, 1_000);
        assert_eq!(rx, Rx::Decided(Decision::Reject(Reason::Revoked)));
        assert!(f.is_none(), "no ARM frame for a revoked origin");
        assert_eq!(v.act.executed, 0);
    }
}

/// The other gate conditions, and the class refusal. None of them emits a frame.
#[test]
fn mo_no_arm_frame_without_act() {
    let cases: Vec<(&str, ArmContext, ActCommand, OrderClass, Reason)> = vec![
        ("not verified", ArmContext { v0b_ok: false, ..ok_ctx() }, arm_order(1), OrderClass::Act, Reason::NotVerified),
        ("unauthorized", ArmContext { authorized: false, ..ok_ctx() }, arm_order(1), OrderClass::Act, Reason::NotAuthorized),
        ("previous boot", ok_ctx(), ActCommand { boot_id: 6, ..arm_order(1) }, OrderClass::Act, Reason::Expired),
        ("expired", ArmContext { now_ms: 999_999, ..ok_ctx() }, arm_order(1), OrderClass::Act, Reason::Expired),
        ("sensors locked", ArmContext { r14_safe: false, ..ok_ctx() }, arm_order(1), OrderClass::Act, Reason::R14Unsafe),
        ("another vehicle", ok_ctx(), ActCommand { actuator_id: VEHICLE + 1, ..arm_order(1) }, OrderClass::Act, Reason::OutOfLimits),
        ("force 0.5", ok_ctx(), ActCommand { force: 0.5, ..arm_order(1) }, OrderClass::Act, Reason::OutOfLimits),
        ("force NaN", ok_ctx(), ActCommand { force: f32::NAN, ..arm_order(1) }, OrderClass::Act, Reason::OutOfLimits),
        ("stop class", ok_ctx(), arm_order(1), OrderClass::Stop, Reason::OutOfLimits),
    ];
    for (name, ctx, cmd, class, expect) in cases {
        let mut v = Vehicle::new(rules());
        let (d, action, f) = v.decide(&ctx, &cmd, class);
        assert_eq!(d, Decision::Reject(expect), "{}", name);
        assert!(action.is_none() && f.is_none(), "{} produced a frame", name);
        assert_eq!(v.act.executed, 0, "{}", name);
    }
}

/// A disarm is the same path with param1 = 0, and it is an order like any other.
#[test]
fn mo_disarm_is_the_same_gate() {
    let mut v = Vehicle::new(rules());
    let (d, action, f) = v.decide(&ok_ctx(), &ActCommand { force: 0.0, ..arm_order(1) }, OrderClass::Act);
    assert_eq!(d, Decision::Act);
    assert_eq!(action, Some(ArmAction::Disarm));
    assert_eq!(parse_arm(&f.unwrap()), (MAV_CMD_COMPONENT_ARM_DISARM, 0.0));
}

/// The class byte is inside the signed region, so a `Stop` cannot be promoted to an
/// `Act` on the wire.
#[test]
fn mo_class_byte_is_signed() {
    let (mut cmdr, mut router) = commander_and_vehicle_router();
    let stop = encode_oac1_with_class(&arm_order(10), OrderClass::Stop);
    let env = cmdr.origin_wrap_v0b(&stop).unwrap();
    let mut flipped = env.clone();
    let n = flipped.len();
    flipped[n - OAC1_LEN + OAC1_CLASS_OFF] = OrderClass::Act as u8;
    assert!(matches!(router.process(&flipped), MeshDecision::Drop("bad mesh signature")));
    // And the untouched one is a Stop, refused by this carrier.
    match router.process(&env) {
        MeshDecision::Arrived { envelope, .. } => {
            let (_, class) = parse_oac1_any(inner_slice(&envelope)).unwrap();
            assert_eq!(class, OrderClass::Stop);
        }
        d => panic!("the genuine envelope must arrive: {:?}", d),
    }
}
