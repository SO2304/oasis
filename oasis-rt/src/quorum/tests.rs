use super::*;
use crate::actuation::{Actuator, Decision, GateContext, GateInput, Reason};
use oasis_operator_key::{sign_with_seed, OperatorAuthority};

const NET: [u8; 8] = *b"OASISnet";
const S1: [u8; 32] = [0x11; 32];
const S2: [u8; 32] = [0x22; 32];
const S3: [u8; 32] = [0x33; 32];
const S4: [u8; 32] = [0x44; 32];

fn pk_of(seed: &[u8; 32]) -> [u8; 32] {
    let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::from_slice(seed).unwrap());
    let mut pk = [0u8; 32];
    pk.copy_from_slice(kp.pk.as_ref());
    pk
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

fn ok_input(seq: u32) -> GateInput {
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
        cmd_seq: seq,
        last_executed_seq: None,
    }
}

/// Sign the order with each listed seed.
fn signed(c: &ActCommand, seeds: &[[u8; 32]]) -> QuorumOrder {
    let msg = quorum_signed_message(&NET, c);
    let mut o = QuorumOrder::new(*c);
    for s in seeds {
        assert!(o.push(pk_of(s), sign_with_seed(s, &msg)));
    }
    o
}

fn two_of_three() -> OperatorAuthority {
    OperatorAuthority::multisig_from_seeds(&[S1, S2, S3], 2).unwrap()
}

fn verify(auth: &OperatorAuthority, o: &QuorumOrder) -> bool {
    auth.verify_authorization(&quorum_signed_message(&NET, &o.cmd), o.offered()).is_ok()
}

#[test]
fn q_two_of_three_needs_two_distinct_operators() {
    let auth = two_of_three();
    let c = cmd(1);
    assert!(!verify(&auth, &signed(&c, &[S1])), "one signature is not a quorum");
    assert!(verify(&auth, &signed(&c, &[S1, S2])));
    assert!(verify(&auth, &signed(&c, &[S2, S3])));
    assert!(verify(&auth, &signed(&c, &[S1, S2, S3])), "more than k is still fine");
    assert!(!verify(&auth, &signed(&c, &[S1, S1])), "the same operator twice is not two");
    assert!(!verify(&auth, &signed(&c, &[S4, S4])), "an outsider, twice, is nobody");
    assert!(!verify(&auth, &signed(&c, &[S1, S4])), "one insider plus one outsider is one");
}

/// A quorum authorises **this** order and nothing else: the signatures are over the body,
/// so changing any field of it invalidates them.
#[test]
fn q_signatures_are_bound_to_the_order() {
    let auth = two_of_three();
    let c = cmd(1);
    let good = signed(&c, &[S1, S2]);
    assert!(verify(&auth, &good));

    for mutate in [
        (|k: &mut ActCommand| k.cmd_seq += 1) as fn(&mut ActCommand),
        |k: &mut ActCommand| k.actuator_id += 1,
        |k: &mut ActCommand| k.boot_id += 1,
        |k: &mut ActCommand| k.deadline_ms += 1,
        |k: &mut ActCommand| k.force += 1.0,
        |k: &mut ActCommand| k.torque += 1.0,
        |k: &mut ActCommand| k.velocity += 1.0,
        |k: &mut ActCommand| k.pos[0] += 1.0,
        |k: &mut ActCommand| k.pos[1] += 1.0,
        |k: &mut ActCommand| k.pos[2] += 1.0,
    ] {
        let mut swapped = good;
        mutate(&mut swapped.cmd);
        assert!(!verify(&auth, &swapped), "a changed field must invalidate the quorum");
    }
}

/// And it authorises this order on **this** network only.
#[test]
fn q_quorum_is_bound_to_the_network() {
    let auth = two_of_three();
    let c = cmd(1);
    let o = signed(&c, &[S1, S2]);
    let other: [u8; 8] = *b"OTHERnet";
    assert!(auth.verify_authorization(&quorum_signed_message(&NET, &c), o.offered()).is_ok());
    assert!(auth.verify_authorization(&quorum_signed_message(&other, &c), o.offered()).is_err(), "a quorum for one fleet must not authorise another");
}

#[test]
fn q_roundtrip_and_parser_is_strict() {
    let o = signed(&cmd(5), &[S1, S2]);
    let mut buf = [0u8; 512];
    let n = encode_oaq1(&o, &mut buf).unwrap();
    assert_eq!(n, oaq1_len(2));
    assert_eq!(n, 244, "two inline signatures: 244 bytes of payload");
    let back = parse_oaq1(&buf[..n]).unwrap();
    assert_eq!(back.cmd, o.cmd);
    assert_eq!(back.n_sigs, 2);
    assert_eq!(back.offered(), o.offered());

    assert!(parse_oaq1(&[]).is_none());
    assert!(parse_oaq1(&buf[..n - 1]).is_none(), "length must match n_sigs exactly");
    assert!(parse_oaq1(&buf[..OAQ1_BODY_LEN + 1]).is_none(), "n_sigs=2 but no signatures");

    let mut bad = buf;
    bad[0] = b'X';
    assert!(parse_oaq1(&bad[..n]).is_none(), "wrong magic");

    let mut res = buf;
    res[50] = 1;
    assert!(parse_oaq1(&res[..n]).is_none(), "the reserved byte must be zero");

    // n_sigs = 0 is refused, so a quorum order can never look like an ordinary one.
    let mut zero = buf;
    zero[OAQ1_BODY_LEN] = 0;
    assert!(parse_oaq1(&zero[..oaq1_len(0)]).is_none());

    // n_sigs beyond the ceiling is refused rather than truncated.
    let mut over = buf;
    over[OAQ1_BODY_LEN] = (MAX_QUORUM_SIGS + 1) as u8;
    assert!(parse_oaq1(&over[..n]).is_none());
}

/// The sizes that say the two-person rule costs two frames. Asserted so the claim in the
/// module doc and in the spec cannot drift.
#[test]
fn q_sizes_do_not_fit_one_radio_frame() {
    let v0b = crate::mesh::MESH_V0B_HEADER_LEN;
    assert_eq!(oaq1_len(1), 148);
    assert_eq!(v0b + oaq1_len(1), 247, "k=1 fits in 255, barely");
    assert_eq!(oaq1_len(2), 244);
    assert_eq!(v0b + oaq1_len(2), 343, "k=2 does not fit, so it costs two frames");
    assert!(v0b + oaq1_len(2) > 255);
}

#[test]
fn q_gate_refuses_without_a_quorum_and_says_why() {
    let ctx = GateContext::default();
    assert_eq!(quorum_decision(&ctx, &ok_input(1), true), Decision::Act);
    assert_eq!(quorum_decision(&ctx, &ok_input(1), false), Decision::Reject(Reason::QuorumMissing));
}

/// Authenticity and authorisation come first, so an untrusted sender learns nothing about
/// the quorum policy.
#[test]
fn q_quorum_is_not_leaked_to_an_untrusted_sender() {
    let ctx = GateContext::default();
    let unverified = GateInput { v0b_ok: false, ..ok_input(1) };
    assert_eq!(quorum_decision(&ctx, &unverified, false), Decision::Reject(Reason::NotVerified));
    let unauthorized = GateInput { authorized: false, ..ok_input(1) };
    assert_eq!(quorum_decision(&ctx, &unauthorized, false), Decision::Reject(Reason::NotAuthorized));
}

/// A quorum does not excuse any of the other nine conditions.
#[test]
fn q_quorum_does_not_override_the_rest() {
    let ctx = GateContext::default();
    for (i, expected) in [
        (GateInput { revoked: true, ..ok_input(1) }, Reason::Revoked),
        (GateInput { cmd_boot_id: 6, ..ok_input(1) }, Reason::Expired),
        (GateInput { r14_safe: false, ..ok_input(1) }, Reason::R14Unsafe),
        (GateInput { within_limits: false, ..ok_input(1) }, Reason::OutOfLimits),
    ] {
        assert_eq!(quorum_decision(&ctx, &i, true), Decision::Reject(expected));
    }
    // And a latched stop still wins over a quorum.
    let stopped = GateContext { stopped: true, supervision_expired: false };
    assert_eq!(quorum_decision(&stopped, &ok_input(1), true), Decision::Reject(Reason::Stopped));
}

/// End to end: a real 2-of-3 quorum, through the real gate, with the counters.
#[test]
fn q_end_to_end_two_of_three() {
    let auth = two_of_three();
    let mut act = Actuator::new();
    let c = cmd(10);
    let one = signed(&c, &[S1]);
    let two = signed(&c, &[S1, S3]);

    let mut decide = |o: &QuorumOrder, seq: u32| {
        let ok = verify(&auth, o);
        let ctx = act.context(1_000);
        let d = quorum_decision(&ctx, &GateInput { cmd_seq: seq, last_executed_seq: act.last_executed_seq, ..ok_input(seq) }, ok);
        if d == Decision::Act {
            act.last_executed_seq = Some(seq);
            act.executed += 1;
        } else if let Decision::Reject(r) = d {
            act.rejects[r as usize] += 1;
        }
        d
    };

    assert_eq!(decide(&one, 10), Decision::Reject(Reason::QuorumMissing));
    assert_eq!(decide(&two, 10), Decision::Act);
    assert_eq!(decide(&two, 10), Decision::Reject(Reason::StaleOrReplayed), "a quorum is not a replay licence");
    assert_eq!(act.executed, 1);
    assert_eq!(act.rejects[Reason::QuorumMissing as usize], 1);
}
