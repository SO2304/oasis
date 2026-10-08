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

// ---------------------------------------------------------------------------
// Part G — stop asymmetry (docs/AUTHORITY_HARDENING_SPEC.md)
// ---------------------------------------------------------------------------

fn stop_ok() -> StopInput {
    StopInput { v0b_ok: true, stop_authorized: true, revoked: false }
}

/// The central property, as a test as well as a proof: none of the conditions the stop
/// rule drops can refuse a stop. Each sub-case uses an input that WOULD refuse an `Act`.
#[test]
fn stop_is_not_blocked_by_anything_that_blocks_an_act() {
    // Stale sequence.
    let mut a = Actuator::new();
    assert_eq!(a.decide(GateInput { cmd_seq: 5, ..ok() }), Decision::Act);
    assert_eq!(a.decide(GateInput { cmd_seq: 5, ..ok() }), Decision::Reject(Reason::StaleOrReplayed), "an Act with a replayed sequence is refused");
    assert_eq!(a.decide_stop(&stop_ok()), StopDecision::Stop, "the same staleness must not block a stop");

    // Boot id from an earlier boot.
    let mut a = Actuator::new();
    assert_eq!(a.decide(GateInput { cmd_boot_id: 6, ..ok() }), Decision::Reject(Reason::Expired), "an Act stamped for a previous boot is refused");
    assert_eq!(a.decide_stop(&stop_ok()), StopDecision::Stop);

    // Past the deadline.
    let mut a = Actuator::new();
    assert_eq!(a.decide(GateInput { now_ms: 4_000, ..ok() }), Decision::Reject(Reason::Expired));
    assert_eq!(a.decide_stop(&stop_ok()), StopDecision::Stop);

    // Sensor-state lock: the sense is inverted, a lost sensor is a reason to stop.
    let mut a = Actuator::new();
    assert_eq!(a.decide(GateInput { r14_safe: false, ..ok() }), Decision::Reject(Reason::R14Unsafe));
    assert_eq!(a.decide_stop(&stop_ok()), StopDecision::Stop);

    // Out of physical limits.
    let mut a = Actuator::new();
    assert_eq!(a.decide(GateInput { within_limits: false, ..ok() }), Decision::Reject(Reason::OutOfLimits));
    assert_eq!(a.decide_stop(&stop_ok()), StopDecision::Stop);
}

#[test]
fn stop_keeps_its_three_conditions() {
    let mut a = Actuator::new();
    assert_eq!(a.decide_stop(&StopInput { v0b_ok: false, ..stop_ok() }), StopDecision::Reject(Reason::NotVerified), "an unauthenticated stop would be a free denial of service");
    assert_eq!(a.decide_stop(&StopInput { stop_authorized: false, ..stop_ok() }), StopDecision::Reject(Reason::NotAuthorized), "STOP is a permission of its own, distinct from ACTUATE");
    assert_eq!(a.decide_stop(&StopInput { revoked: true, ..stop_ok() }), StopDecision::Reject(Reason::Revoked), "the one documented case where a security condition refuses a stop");
    assert!(!a.stopped, "no refused stop may latch");
    assert_eq!(a.stops, 0);
}

#[test]
fn stop_latches_and_only_a_local_action_clears_it() {
    let mut a = Actuator::new();
    assert_eq!(a.decide_stop(&stop_ok()), StopDecision::Stop);
    assert!(a.stopped);
    // ISO 13850:2015 4.1.1.2: "it shall not be possible for any start command to be
    // effective". No network order clears the latch, however valid.
    assert_eq!(a.decide(ok()), Decision::Reject(Reason::Stopped));
    assert_eq!(a.decide(GateInput { cmd_seq: 99, ..ok() }), Decision::Reject(Reason::Stopped));
    assert_eq!(a.executed, 0);
    a.clear_stop();
    assert_eq!(a.decide(ok()), Decision::Act, "after an intentional local reset, acting resumes");
    assert_eq!(a.executed, 1);
}

#[test]
fn stop_is_idempotent() {
    let mut a = Actuator::new();
    for _ in 0..3 {
        assert_eq!(a.decide_stop(&stop_ok()), StopDecision::Stop, "replaying a stop is harmless");
    }
    assert_eq!(a.stops, 3);
    assert!(a.stopped);
}

/// An order that is not authentic, not authorized or revoked must learn nothing about the
/// actuator's internal state: those three reasons come before `Stopped`.
#[test]
fn stopped_is_not_leaked_to_an_untrusted_sender() {
    let mut a = Actuator::new();
    a.decide_stop(&stop_ok());
    assert_eq!(a.decide(GateInput { v0b_ok: false, ..ok() }), Decision::Reject(Reason::NotVerified));
    assert_eq!(a.decide(GateInput { authorized: false, ..ok() }), Decision::Reject(Reason::NotAuthorized));
    assert_eq!(a.decide(GateInput { revoked: true, ..ok() }), Decision::Reject(Reason::Revoked));
}

#[test]
fn stop_order_class_is_on_the_wire_and_signed_bytes_differ() {
    let c = cmd(1);
    let act = encode_oac1(&c);
    let stop = encode_oac1_with_class(&c, OrderClass::Stop);
    assert_eq!(act[OAC1_CLASS_OFF], 0);
    assert_eq!(stop[OAC1_CLASS_OFF], 1);
    assert_ne!(act, stop, "the class is part of the signed payload");
    assert_eq!(parse_oac1_any(&act), Some((c, OrderClass::Act)));
    assert_eq!(parse_oac1_any(&stop), Some((c, OrderClass::Stop)));
}

/// Backward compatibility: bytes produced before part G decode as `Act`, and the old
/// entry point still refuses anything else, so a stop can never be mistaken for an act by
/// a caller that does not know about classes.
#[test]
fn parse_oac1_accepts_act_only() {
    let c = cmd(1);
    assert_eq!(parse_oac1(&encode_oac1(&c)), Some(c));
    assert_eq!(parse_oac1(&encode_oac1_with_class(&c, OrderClass::Stop)), None);
    let mut bad = encode_oac1(&c);
    bad[OAC1_CLASS_OFF] = 2;
    assert_eq!(parse_oac1_any(&bad), None, "an unknown class is refused, not defaulted");
    for off in OAC1_CLASS_OFF + 1..OAC1_LEN {
        let mut b = encode_oac1(&c);
        b[off] = 1;
        assert_eq!(parse_oac1_any(&b), None, "reserved byte {off} must still be zero");
    }
}

// ---------------------------------------------------------------------------
// Part H — supervision liveness
// ---------------------------------------------------------------------------

fn beacon(seq: u32, validity_ms: u64) -> SupervisionBeacon {
    SupervisionBeacon { supervisor_fp: [0x51; 8], actuator_boot_id: 7, beacon_seq: seq, validity_ms }
}

#[test]
fn supervision_is_an_explicit_choice_not_a_silent_default() {
    // A fixed actuator (the phase 1.4 Modbus case) has no supervisor and must keep working.
    let mut fixed = Actuator::new();
    assert_eq!(fixed.decide(ok()), Decision::Act);
    // Mobile machinery under Annex III part 3 may not operate without one.
    let mut mobile = Actuator::new_supervised();
    assert_eq!(mobile.decide(ok()), Decision::Reject(Reason::SupervisionLost));
}

#[test]
fn act_needs_a_live_beacon_and_stop_does_not() {
    let mut a = Actuator::new_supervised();
    assert!(a.apply_beacon(&beacon(1, 2_000), 1_000));
    assert_eq!(a.decide(ok()), Decision::Act, "live until 3_000");
    assert_eq!(a.decide(GateInput { cmd_seq: 2, now_ms: 3_001, deadline_ms: 6_000, ..ok() }), Decision::Reject(Reason::SupervisionLost), "one millisecond past the deadline");
    // The whole point of part G: a dead supervision link never blocks a stop.
    assert_eq!(a.decide_stop(&stop_ok()), StopDecision::Stop);
}

#[test]
fn beacon_replay_and_rollback_are_refused() {
    let mut a = Actuator::new_supervised();
    assert!(a.apply_beacon(&beacon(5, 2_000), 1_000));
    assert!(!a.apply_beacon(&beacon(5, 60_000), 1_000), "same sequence");
    assert!(!a.apply_beacon(&beacon(4, 60_000), 1_000), "older sequence");
    assert_eq!(a.supervision_until_ms, Some(3_000), "a refused beacon changes nothing");
    assert!(a.apply_beacon(&beacon(6, 1_000), 2_000));
    assert_eq!(a.supervision_until_ms, Some(3_000));
}

#[test]
fn beacon_validity_is_bounded_and_cannot_wrap() {
    let mut a = Actuator::new_supervised();
    assert!(a.apply_beacon(&beacon(1, u64::MAX), 1_000));
    assert_eq!(a.supervision_until_ms, Some(1_000 + MAX_SUPERVISION_MS), "clamped, not wrapped");
    let mut b = Actuator::new_supervised();
    assert!(b.apply_beacon(&beacon(1, u64::MAX), u64::MAX));
    assert_eq!(b.supervision_until_ms, Some(u64::MAX), "saturating, never in the past");
}

#[test]
fn osb1_roundtrip_and_parser_is_strict() {
    let b = beacon(3, 10_000);
    assert_eq!(parse_osb1(&encode_osb1(&b)), Some(b));
    assert_eq!(parse_osb1(&[]), None);
    assert_eq!(parse_osb1(&encode_osb1(&b)[..OSB1_LEN - 1]), None);
    let mut bad = encode_osb1(&b);
    bad[0] = b'X';
    assert_eq!(parse_osb1(&bad), None);
}

// ---------------------------------------------------------------------------
// Part J — compact stop order OAS1
// ---------------------------------------------------------------------------

#[test]
fn oas1_roundtrip_and_parser_is_strict() {
    let o = StopOrder { actuator_id: 1, cmd_seq: 77 };
    let b = encode_oas1(&o);
    assert_eq!(b.len(), OAS1_LEN);
    assert_eq!(b.len(), 11, "the whole point of part J is the size");
    assert_eq!(parse_oas1(&b), Some(o));
    assert_eq!(parse_oas1(&[]), None);
    assert_eq!(parse_oas1(&b[..OAS1_LEN - 1]), None);
    let mut bad = b;
    bad[0] = b'X';
    assert_eq!(parse_oas1(&bad), None, "wrong magic");
    let mut res = b;
    res[10] = 1;
    assert_eq!(parse_oas1(&res), None, "the reserved byte must be zero: one encoding per stop");
}

/// On the wire a stop costs 110 bytes instead of 153 — the number section J.4 justifies the
/// whole addition with. Asserted so it cannot drift silently.
#[test]
fn oas1_is_43_bytes_shorter_than_oac1() {
    assert_eq!(OAC1_LEN - OAS1_LEN, 43);
    let v0b_header = crate::mesh::MESH_V0B_HEADER_LEN;
    assert_eq!(v0b_header + OAC1_LEN, 153);
    assert_eq!(v0b_header + OAS1_LEN, 110);
}

#[test]
fn oas1_actuator_zero_addresses_every_actuator() {
    let all = StopOrder { actuator_id: OAS1_ALL_ACTUATORS, cmd_seq: 1 };
    assert!(all.addresses(1), "0 means all, not actuator number zero");
    assert!(all.addresses(2));
    assert!(all.addresses(u16::MAX));
    let one = StopOrder { actuator_id: 2, cmd_seq: 1 };
    assert!(one.addresses(2));
    assert!(!one.addresses(1));
    assert!(!one.addresses(OAS1_ALL_ACTUATORS));
}

/// The equivalence that justifies dropping 43 bytes: a compact stop and an `OAC1` class
/// `Stop` carrying the same id and sequence are decided identically, whatever the dropped
/// fields held. Sampled here over values that would each refuse an `Act`; the general
/// statement is `proof_oas1_equivalent_to_oac1_stop`.
#[test]
fn oas1_decides_like_an_oac1_stop() {
    let o = StopOrder { actuator_id: 1, cmd_seq: 5 };
    let long = encode_oac1_with_class(&o.as_act_command(), OrderClass::Stop);
    let (parsed_long, class) = parse_oac1_any(&long).unwrap();
    assert_eq!(class, OrderClass::Stop);
    let parsed_short = parse_oas1(&encode_oas1(&o)).unwrap();
    assert_eq!(parsed_long.actuator_id, parsed_short.actuator_id);
    assert_eq!(parsed_long.cmd_seq, parsed_short.cmd_seq);

    // Both route to the same rule, and the rule reads neither of them.
    for i in [
        StopInput { v0b_ok: true, stop_authorized: true, revoked: false },
        StopInput { v0b_ok: false, stop_authorized: true, revoked: false },
        StopInput { v0b_ok: true, stop_authorized: false, revoked: false },
        StopInput { v0b_ok: true, stop_authorized: true, revoked: true },
    ] {
        let mut a = Actuator::new();
        let mut b = Actuator::new();
        assert_eq!(a.decide_stop(&i), b.decide_stop(&i));
        assert_eq!(a.stopped, b.stopped);
    }

    // And the dropped fields really are arbitrary: an OAC1 stop whose setpoints would be
    // refused as an act is still a stop.
    let mut wild = o.as_act_command();
    wild.force = f32::NAN;
    wild.boot_id = u64::MAX;
    wild.deadline_ms = 0;
    let (p, c) = parse_oac1_any(&encode_oac1_with_class(&wild, OrderClass::Stop)).unwrap();
    assert_eq!(c, OrderClass::Stop);
    assert_eq!(p.cmd_seq, o.cmd_seq);
}
