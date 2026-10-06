//! ORV1 mesh revocation verified through the operator authority (single or k-of-n).
//! Complements oasis-rt/src/mesh_revocation/tests.rs, which cannot depend on this
//! crate (this crate depends on oasis-rt).

use oasis_operator_key::{sign_with_seed, AuthorityError, OperatorAuthority, Pub};
use oasis_rt::mesh_revocation::{
    encode_orv1, parse_orv1, revocation_transition, signed_message, signed_message_parts, Fp, RevDecision, RevReject,
    RevState,
};

const NET: [u8; 8] = *b"OASISnet";
const SEEDS: [[u8; 32]; 3] = [[0x51; 32], [0x52; 32], [0x53; 32]];

fn pub_of(seed: &[u8; 32]) -> Pub {
    let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::from_slice(seed).unwrap());
    let mut p = [0u8; 32];
    p.copy_from_slice(kp.pk.as_ref());
    p
}

fn list_signed_by(signers: &[&[u8; 32]], epoch: u64, fps: &[Fp]) -> Vec<u8> {
    let msg = signed_message_parts(&NET, epoch, 1_700_000_000, fps);
    let sigs: Vec<(Pub, [u8; 64])> = signers.iter().map(|s| (pub_of(s), sign_with_seed(s, &msg))).collect();
    encode_orv1(&NET, epoch, 1_700_000_000, fps, &sigs)
}

/// Relay path exactly as the firmware runs it.
fn ingest(auth: &OperatorAuthority, st: &mut RevState, blob: &[u8]) -> RevDecision {
    let p = match parse_orv1(blob) {
        Ok(p) => p,
        Err(e) => return RevDecision::Reject(e),
    };
    let sig_ok = auth.verify_authorization(&signed_message(&p), &p.sigs).is_ok();
    let (d, new) = revocation_transition(st, &NET, &p, sig_ok);
    if let Some(n) = new {
        *st = n;
    }
    d
}

#[test]
fn rev_k_of_n_quorum() {
    let auth = OperatorAuthority::multisig_from_seeds(&SEEDS, 2).unwrap();
    let fps = [[0xCC; 8]];
    let mut st = RevState::default();
    // k-1 = 1 signature: rejected, state unchanged.
    assert_eq!(ingest(&auth, &mut st, &list_signed_by(&[&SEEDS[0]], 1, &fps)), RevDecision::Reject(RevReject::BadOperatorSig));
    assert_eq!(st, RevState::default());
    // k = 2 distinct members: applied.
    assert_eq!(ingest(&auth, &mut st, &list_signed_by(&[&SEEDS[0], &SEEDS[2]], 1, &fps)), RevDecision::Applied);
    assert_eq!(st.epoch, 1);
}

#[test]
fn rev_quorum_rejects_duplicate_and_outsider_signers() {
    let auth = OperatorAuthority::multisig_from_seeds(&SEEDS, 2).unwrap();
    let fps = [[0xCC; 8]];
    let mut st = RevState::default();
    // The same member signing twice does not make a quorum.
    let dup = list_signed_by(&[&SEEDS[1], &SEEDS[1]], 1, &fps);
    let p = parse_orv1(&dup).unwrap();
    assert!(matches!(auth.verify_authorization(&signed_message(&p), &p.sigs), Err(AuthorityError::DuplicateSigner)));
    assert_eq!(ingest(&auth, &mut st, &dup), RevDecision::Reject(RevReject::BadOperatorSig));
    // A captured node's key is not a quorum member.
    let outsider = list_signed_by(&[&SEEDS[0], &[0x77; 32]], 1, &fps);
    assert_eq!(ingest(&auth, &mut st, &outsider), RevDecision::Reject(RevReject::BadOperatorSig));
    assert_eq!(st, RevState::default());
}

#[test]
fn rev_single_operator_authority() {
    let auth = OperatorAuthority::single_from_seed(&SEEDS[0]).unwrap();
    let mut st = RevState::default();
    assert_eq!(ingest(&auth, &mut st, &list_signed_by(&[&SEEDS[0]], 3, &[[0xAA; 8]])), RevDecision::Applied);
    // Another key, even a former quorum member, is not this operator.
    assert_eq!(
        ingest(&auth, &mut st, &list_signed_by(&[&SEEDS[1]], 4, &[[0xAA; 8], [0xBB; 8]])),
        RevDecision::Reject(RevReject::BadOperatorSig)
    );
    assert_eq!(st.epoch, 3);
}
