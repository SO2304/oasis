use super::*;
use crate::hal::{clamp_command, PhysicalConstraints};
use crate::hyper_state::{agent_new, inject_sensory, is_action_safe};
use crate::mesh::{inner_slice, mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter};

const NET: [u8; 8] = *b"OASISnet";
const R14_THRESHOLD: f64 = 0.6;

fn ok() -> GateInput {
    GateInput {
        v0b_ok: true,
        authorized: true,
        revoked: false,
        cmd_boot_id: 7,
        deadline_ms: 3_000,
        actuator_boot_id: 7,
        now_ms: 1_000,
        r14_safe: true,
        within_limits: true,
        cmd_seq: 1,
        last_executed_seq: None,
    }
}

fn cmd(seq: u32) -> ActCommand {
    ActCommand {
        actuator_id: 1,
        cmd_seq: seq,
        boot_id: 7,
        deadline_ms: 3_000,
        force: 1.0,
        torque: 0.5,
        velocity: 0.2,
        pos: [0.0, 0.0, 1.0],
    }
}

#[test]
fn act_valid_executes_exactly_once() {
    let mut a = Actuator::new();
    assert_eq!(a.decide(ok()), Decision::Act);
    assert_eq!(a.decide(ok()), Decision::Reject(Reason::StaleOrReplayed), "the same command must not run twice");
    assert_eq!(a.executed, 1);
}

#[test]
fn act_not_verified() {
    assert_eq!(actuation_decision(&GateInput { v0b_ok: false, ..ok() }), Decision::Reject(Reason::NotVerified));
}

#[test]
fn act_unauthorized_origin() {
    assert_eq!(actuation_decision(&GateInput { authorized: false, ..ok() }), Decision::Reject(Reason::NotAuthorized));
}

#[test]
fn act_revoked_origin() {
    assert_eq!(actuation_decision(&GateInput { revoked: true, ..ok() }), Decision::Reject(Reason::Revoked));
}

#[test]
fn act_expired() {
    assert_eq!(actuation_decision(&GateInput { now_ms: 3_001, ..ok() }), Decision::Reject(Reason::Expired));
    assert_eq!(actuation_decision(&GateInput { now_ms: 3_000, ..ok() }), Decision::Act, "deadline itself is still valid");
}

#[test]
fn act_wrong_boot_id() {
    // A command stamped against the actuator's previous boot: expired by construction.
    assert_eq!(actuation_decision(&GateInput { cmd_boot_id: 6, ..ok() }), Decision::Reject(Reason::Expired));
}

#[test]
fn act_far_future_deadline() {
    assert_eq!(actuation_decision(&GateInput { deadline_ms: 1_000 + MAX_VALIDITY_MS + 1, ..ok() }), Decision::Reject(Reason::Expired));
}

#[test]
fn act_r14_unsafe_sensor_loss() {
    // Real R14 path: calm agent is safe, a shock (stand-in for a lost sensor's
    // entropy spike, R15) is not. Precondition asserted so the test can't be vacuous.
    let mut calm = agent_new(3);
    inject_sensory(&mut calm, 0.0);
    let mut shocked = agent_new(3);
    inject_sensory(&mut shocked, 1.0);
    assert!(is_action_safe(&calm, R14_THRESHOLD), "calm entropy {}", calm.entropy);
    assert!(!is_action_safe(&shocked, R14_THRESHOLD), "shock entropy {}", shocked.entropy);
    let i = GateInput { r14_safe: is_action_safe(&shocked, R14_THRESHOLD), ..ok() };
    assert_eq!(actuation_decision(&i), Decision::Reject(Reason::R14Unsafe));
}

#[test]
fn act_replayed() {
    let mut a = Actuator::new();
    assert_eq!(a.decide(GateInput { cmd_seq: 4, ..ok() }), Decision::Act);
    assert_eq!(a.decide(GateInput { cmd_seq: 4, ..ok() }), Decision::Reject(Reason::StaleOrReplayed));
}

#[test]
fn act_reordered_older() {
    // v0B accepts reordering within its window; the gate must not run an older setpoint.
    let mut a = Actuator::new();
    assert_eq!(a.decide(GateInput { cmd_seq: 5, ..ok() }), Decision::Act);
    assert_eq!(a.decide(GateInput { cmd_seq: 3, ..ok() }), Decision::Reject(Reason::StaleOrReplayed));
    assert_eq!(a.decide(GateInput { cmd_seq: 6, ..ok() }), Decision::Act);
}

#[test]
fn act_out_of_limits() {
    let k = PhysicalConstraints::default_robot();
    assert!(command_within_limits(&cmd(1), &k));
    assert!(!command_within_limits(&ActCommand { force: 1_000.0, ..cmd(1) }, &k), "force over the limit");
    assert!(!command_within_limits(&ActCommand { pos: [99.0, 0.0, 0.0], ..cmd(1) }, &k), "outside the geofence");
    let i = GateInput { within_limits: command_within_limits(&ActCommand { velocity: 50.0, ..cmd(1) }, &k), ..ok() };
    assert_eq!(actuation_decision(&i), Decision::Reject(Reason::OutOfLimits));
}

#[test]
fn act_nan_setpoint_rejected() {
    let k = PhysicalConstraints::default_robot();
    // clamp_command itself now fails closed on NaN (it used to pass it through).
    let raw = clamp_command(f64::NAN, 0.0, 0.0, &[f64::NAN, 0.0, 0.0], &k);
    assert!(!limits_ok(&raw), "clamp_command must flag NaN input");
    // The gate's own finiteness check stays as a second layer.
    for bad in [ActCommand { force: f32::NAN, ..cmd(1) }, ActCommand { velocity: f32::INFINITY, ..cmd(1) }, ActCommand { pos: [0.0, f32::NAN, 0.0], ..cmd(1) }] {
        assert!(!command_within_limits(&bad, &k));
    }
}

#[test]
fn act_rejection_counters() {
    let mut a = Actuator::new();
    a.decide(GateInput { authorized: false, ..ok() });
    a.decide(GateInput { r14_safe: false, ..ok() });
    a.decide(GateInput { r14_safe: false, ..ok() });
    assert_eq!(a.rejects[Reason::NotAuthorized as usize], 1);
    assert_eq!(a.rejects[Reason::R14Unsafe as usize], 2);
    assert_eq!(a.executed, 0, "no partial action on any rejection");
}

#[test]
fn act_oac1_reserved_bytes_must_be_zero() {
    // OAC1_LEN is 54 but the fields end at byte 50: bytes 50..54 are reserved. Found by
    // cargo-fuzz (round-trip assertion, 2026-10-07): the parser accepted any value there,
    // so one command had 2^32 encodings.
    let b = encode_oac1(&cmd(1));
    assert_eq!(parse_oac1(&b), Some(cmd(1)));
    for i in 50..OAC1_LEN {
        let mut x = b;
        x[i] = 1;
        assert_eq!(parse_oac1(&x), None, "reserved byte {i}");
    }
}

#[test]
fn act_formats_roundtrip() {
    let c = cmd(42);
    assert_eq!(parse_oac1(&encode_oac1(&c)), Some(c));
    assert_eq!(parse_oac1(&encode_oac1(&c)[..OAC1_LEN - 1]), None);
    assert_eq!(parse_otm1(&encode_otm1(9, 12_345)), Some((9, 12_345)));
    let mut bad = encode_otm1(9, 1);
    bad[0] = b'X';
    assert_eq!(parse_otm1(&bad), None);
}

fn seed_of(id: u8) -> [u8; 32] {
    [id; 32]
}
fn node(id: u8, peers: &[u8]) -> MeshRouter {
    let mut r = MeshPubRegistry::new();
    for &p in peers {
        r.insert([p; 8], mesh_v10_pubkey_from_seed(&MeshEdSeed(seed_of(p))).unwrap());
    }
    MeshRouter::new_v0b([id; 8], NET, MeshEdSeed(seed_of(id)), r)
}

#[test]
fn act_end_to_end_over_v0b() {
    // C is the actuator; A is its only command authority; B is a registered but
    // unauthorized node. Both commands are perfectly valid v0B envelopes.
    let mut c = node(0xCC, &[0xAA, 0xBB]);
    let mut a = node(0xAA, &[]);
    let mut b = node(0xBB, &[]);
    let authorities = [[0xAA; 8]];
    let k = PhysicalConstraints::default_robot();
    let mut act = Actuator::new();
    let mut run = |env: &[u8], origin: [u8; 8], c: &mut MeshRouter, act: &mut Actuator| -> Decision {
        let (v0b_ok, inner) = match c.process(env) {
            MeshDecision::Arrived { envelope, .. } => (true, inner_slice(&envelope).to_vec()),
            MeshDecision::Drop(_) => (false, Vec::new()),
        };
        let cm = parse_oac1(&inner).unwrap_or(cmd(0));
        act.decide(GateInput {
            v0b_ok,
            authorized: authorities.contains(&origin),
            revoked: c.is_revoked(&origin),
            cmd_boot_id: cm.boot_id,
            deadline_ms: cm.deadline_ms,
            actuator_boot_id: 7,
            now_ms: 1_000,
            r14_safe: true,
            within_limits: command_within_limits(&cm, &k),
            cmd_seq: cm.cmd_seq,
            last_executed_seq: None,
        })
    };
    let ok_cmd = a.origin_wrap_v0b(&encode_oac1(&cmd(1))).unwrap();
    assert_eq!(run(&ok_cmd, [0xAA; 8], &mut c, &mut act), Decision::Act);
    let rogue = b.origin_wrap_v0b(&encode_oac1(&cmd(2))).unwrap();
    assert_eq!(run(&rogue, [0xBB; 8], &mut c, &mut act), Decision::Reject(Reason::NotAuthorized));
    assert_eq!(run(&ok_cmd, [0xAA; 8], &mut c, &mut act), Decision::Reject(Reason::NotVerified), "v0B replay");
    assert_eq!(act.executed, 1);
}
