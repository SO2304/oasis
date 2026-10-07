use super::*;
use crate::authority::{kind, verify_authority, AuthPolicy, AuthReject, SUITE_ED25519, SUITE_HYBRID};
use crate::identity::{fingerprint, public_key};
use crate::test_support::{o1, o3, NET};

fn att(seed: u8, perms: u32, seq: u32) -> Attestation {
    Attestation { node_pk: public_key(&[seed; 32]), role: role::RELAY, permissions: perms, enroll_seq: seq }
}

fn apply(reg: &Registry, a: &Attestation, revoked: bool) -> (EnrollDecision, Option<Registry>) {
    enrollment_transition(reg, &a.fp(), a, revoked)
}

#[test]
fn enr_parse_roundtrip_and_rejects() {
    let a = att(1, perm::ACTUATE, 7);
    let c = encode_attestation(&a);
    assert_eq!(parse_attestation(&c), Ok(a));
    assert_eq!(parse_attestation(&c[..ATT_LEN - 1]), Err(EnrollReject::Malformed));
    assert_eq!(parse_attestation(&[&c[..], &[0][..]].concat()), Err(EnrollReject::Malformed));
    let mut v = c;
    v[0] = 2;
    assert_eq!(parse_attestation(&v), Err(EnrollReject::Malformed), "unknown version");
    for r in [0u8, 5, 0xFF] {
        let mut x = c;
        x[33] = r;
        assert_eq!(parse_attestation(&x), Err(EnrollReject::UnknownRole), "role {}", r);
    }
    for b in 1..32 {
        let mut x = c;
        x[34..38].copy_from_slice(&(perm::ACTUATE | (1u32 << b)).to_le_bytes());
        assert_eq!(parse_attestation(&x), Err(EnrollReject::ReservedPermission), "bit {}", b);
    }
}

#[test]
fn enr_transition_rules() {
    let r0 = Registry::default();
    let a = att(1, 0, 1);
    let (d, r1) = apply(&r0, &a, false);
    assert_eq!(d, EnrollDecision::Enrolled);
    let r1 = r1.unwrap();
    assert_eq!((r1.gen, r1.entries.len()), (1, 1));
    assert!(!r1.allows(&a.fp(), perm::ACTUATE));
    // Same or lower sequence: refused, nothing changes (replay of an old attestation).
    assert_eq!(apply(&r1, &a, false), (EnrollDecision::Reject(EnrollReject::StaleSeq), None));
    assert_eq!(apply(&r1, &att(1, perm::ACTUATE, 0), false).0, EnrollDecision::Reject(EnrollReject::StaleSeq));
    // Higher sequence: permissions updated.
    let (d, r2) = apply(&r1, &att(1, perm::ACTUATE, 2), false);
    assert_eq!(d, EnrollDecision::Updated);
    let r2 = r2.unwrap();
    assert!(r2.allows(&a.fp(), perm::ACTUATE) && r2.entries.len() == 1 && r2.gen == 2);
    // ... and an old, more generous attestation cannot be replayed after a downgrade.
    let (_, r3) = apply(&r2, &att(1, 0, 3), false);
    let r3 = r3.unwrap();
    assert_eq!(apply(&r3, &att(1, perm::ACTUATE, 2), false).0, EnrollDecision::Reject(EnrollReject::StaleSeq));
    assert!(!r3.allows(&a.fp(), perm::ACTUATE));
    // Revoked: never enrolled, whatever the sequence.
    assert_eq!(apply(&r3, &att(9, 0, 100), true), (EnrollDecision::Reject(EnrollReject::Revoked), None));
    // Unknown node: not allowed anything.
    assert!(!r3.allows(&fingerprint(&public_key(&[8; 32])), 0));
}

#[test]
fn enr_registry_full() {
    let mut r = Registry::default();
    for i in 1..=MAX_ENTRIES {
        let a = att(i as u8, 0, 1); // seed 0 is unusable (ed25519-compact panics on it)
        r = apply(&r, &a, false).1.unwrap();
    }
    assert_eq!(apply(&r, &att(200, 0, 1), false).0, EnrollDecision::Reject(EnrollReject::Full));
    // An existing node can still be updated when full.
    assert_eq!(apply(&r, &att(1, perm::ACTUATE, 2), false).0, EnrollDecision::Updated);
}

#[test]
fn enr_record_roundtrip_corruption_and_slots() {
    let mut r = Registry::default();
    for (i, p) in [(1u8, 0u32), (2, perm::ACTUATE), (3, 0)] {
        r = apply(&r, &att(i, p, 1), false).1.unwrap();
    }
    let rec = r.to_record();
    assert_eq!(rec.len(), 10 + 3 * 49 + 4);
    assert_eq!(Registry::from_record(&rec), Some(r.clone()));
    // Erased flash after the record is ignored.
    let mut padded = rec.clone();
    padded.extend_from_slice(&[0xFF; 100]);
    assert_eq!(Registry::from_record(&padded), Some(r.clone()));
    for i in 0..rec.len() {
        let mut c = rec.clone();
        c[i] ^= 0x01;
        assert_eq!(Registry::from_record(&c), None, "corrupted byte {}", i);
    }
    assert_eq!(Registry::from_record(&[0xFF; 4096]), None);
    // A record whose fingerprint does not derive from its key is refused even with a
    // valid checksum (a record is never trusted to name a fingerprint).
    let mut bad = r.clone();
    bad.entries[0].fp = [0xAA; 8];
    assert_eq!(Registry::from_record(&bad.to_record()), None);
    // Two slots: the highest valid generation wins; a torn newer slot is ignored.
    let newer = apply(&r, &att(4, 0, 1), false).1.unwrap();
    let (old_rec, new_rec) = (r.to_record(), newer.to_record());
    assert_eq!(Registry::from_slots(&old_rec, &new_rec), newer);
    assert_eq!(Registry::from_slots(&new_rec, &old_rec), newer);
    let torn = &new_rec[..new_rec.len() / 2];
    assert_eq!(Registry::from_slots(torn, &old_rec), r);
    assert_eq!(Registry::from_slots(&[0xFF; 64], &[]), Registry::default());
    // Writes never target the newest valid slot.
    assert_eq!(Registry::slot_to_overwrite(&new_rec, &old_rec), 1);
    assert_eq!(Registry::slot_to_overwrite(&old_rec, &new_rec), 0);
    assert_eq!(Registry::slot_to_overwrite(&new_rec, torn), 1);
    assert_eq!(Registry::slot_to_overwrite(torn, &new_rec), 0);
}

#[test]
fn enr_signed_attestation_end_to_end() {
    // Owner-signed (hybrid) attestation, verified like any authority message.
    let pol = AuthPolicy::default();
    let a = att(5, perm::ACTUATE, 1);
    let m = o1().sign(kind::ENROLLMENT, SUITE_HYBRID, &NET, &encode_attestation(&a));
    let p = verify_authority(&pol, &NET, &o1().keys(), &m).unwrap();
    assert_eq!(p.kind, kind::ENROLLMENT);
    let parsed = parse_attestation(p.content).unwrap();
    let (d, reg) = enrollment_transition(&Registry::default(), &parsed.fp(), &parsed, false);
    assert_eq!(d, EnrollDecision::Enrolled);
    assert!(reg.unwrap().allows(&fingerprint(&a.node_pk), perm::ACTUATE));
    // Signed by someone else: refused before any registry change.
    let forged = o3().sign(kind::ENROLLMENT, SUITE_HYBRID, &NET, &encode_attestation(&a));
    assert_eq!(verify_authority(&pol, &NET, &o1().keys(), &forged), Err(AuthReject::BadSignature));
    // Another network: refused.
    let other = o1().sign(kind::ENROLLMENT, SUITE_ED25519, b"OTHERnet", &encode_attestation(&a));
    assert_eq!(verify_authority(&pol, &NET, &o1().keys(), &other), Err(AuthReject::WrongNetwork));
}

#[test]
fn enr_parse_never_panics() {
    let c = encode_attestation(&att(1, 0, 1));
    for n in 0..=c.len() + 2 {
        let mut v = c.to_vec();
        v.resize(n, 0xA5);
        let _ = parse_attestation(&v);
        let _ = Registry::from_record(&v);
    }
}
