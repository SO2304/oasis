use super::*;
use crate::authority::{kind, parse_oau1, verify_authority, AuthPolicy, AuthReject, SUITE_ED25519, SUITE_HYBRID};
use crate::test_support::{o1, o2, o3, TestOwner, NET};

fn owner_keys(o: &TestOwner) -> OwnerKeys {
    OwnerKeys::new(o.ed_pub, &o.ml_pub).unwrap()
}

fn initial() -> OwnerState {
    OwnerState { seq: 0, current: owner_keys(&o1()), previous: None }
}

/// What a node does with a kind-4 message: pick the verification keys from the
/// sub-type, verify, then decide. Returns the decision and the new state/pending.
fn node_handle(state: &OwnerState, pending: &Option<Pending>, msg: &[u8], rev_signed_by_current: bool) -> (OwnDecision, Option<OwnerState>, Option<Pending>) {
    let pol = AuthPolicy::default();
    let unverified = match parse_oau1(msg) {
        Ok(p) => p,
        Err(_) => return (OwnDecision::Reject(OwnReject::Malformed), None, None),
    };
    match content_sub(unverified.content) {
        Some(SUB_OFFER) => {
            let p = match verify_authority(&pol, &NET, &state.current.keys(), msg) {
                Ok(p) => p,
                Err(_) => return (OwnDecision::Reject(OwnReject::BadSignature), None, None),
            };
            let offer = match parse_offer(p.content) {
                Some(o) => o,
                None => return (OwnDecision::Reject(OwnReject::Malformed), None, None),
            };
            let digest = offer_digest(msg);
            let d = offer_decision(state.seq, pending.as_ref().map(|p| &p.digest), offer.seq, &digest, rev_signed_by_current);
            let np = (d == OwnDecision::OfferPending).then(|| Pending { seq: offer.seq, new_owner: offer.new_owner, digest });
            (d, None, np)
        }
        Some(SUB_ACCEPT) => {
            let verified = pending.as_ref().map_or(false, |p| verify_authority(&pol, &NET, &p.new_owner.keys(), msg).is_ok());
            let accept = match parse_accept(unverified.content) {
                Some(a) => a,
                None => return (OwnDecision::Reject(OwnReject::Malformed), None, None),
            };
            let d = accept_decision(state.seq, pending.as_ref().map(|p| (p.seq, &p.digest)), &accept, verified);
            let ns = (d == OwnDecision::Transferred).then(|| apply_transfer(state, pending.as_ref().unwrap()));
            (d, ns, None)
        }
        _ => (OwnDecision::Reject(OwnReject::Malformed), None, None),
    }
}

fn offer_msg(from: &TestOwner, to: &TestOwner, seq: u32) -> Vec<u8> {
    from.sign(kind::OWNERSHIP_TRANSFER, SUITE_HYBRID, &NET, &encode_offer(seq, &owner_keys(to)))
}

fn accept_msg(by: &TestOwner, seq: u32, offer: &[u8]) -> Vec<u8> {
    by.sign(kind::OWNERSHIP_TRANSFER, SUITE_HYBRID, &NET, &encode_accept(seq, &offer_digest(offer)))
}

#[test]
fn own_codec_roundtrip_and_sizes() {
    let k = owner_keys(&o2());
    let c = encode_offer(7, &k);
    assert_eq!(c.len(), OFFER_LEN);
    assert_eq!(parse_offer(&c), Some(Offer { seq: 7, new_owner: k.clone() }));
    assert_eq!(parse_offer(&c[..OFFER_LEN - 1]), None);
    let a = encode_accept(7, &[3; 32]);
    assert_eq!(parse_accept(&a), Some(Accept { seq: 7, offer_digest: [3; 32] }));
    assert_eq!(parse_accept(&c), None, "an offer is not an acceptance");
    assert_eq!(parse_offer(&a), None);
    // Both signed messages fit the 4 KiB reassembly cap (spec §4).
    let m = offer_msg(&o1(), &o2(), 1);
    assert_eq!(m.len(), 16 + 1349 + 64 + 2420);
    assert!(m.len() <= crate::fragment::MAX_ASSEMBLED);
    assert_eq!(crate::fragment::fragment(&m, 201).unwrap().len(), 21);
}

#[test]
fn own_transfer_needs_both_owners() {
    let s0 = initial();
    let offer = offer_msg(&o1(), &o2(), 1);
    // 1. Offer from the current owner: pending, owner unchanged.
    let (d, ns, pending) = node_handle(&s0, &None, &offer, true);
    assert_eq!((d, ns.is_none()), (OwnDecision::OfferPending, true));
    let pending = pending;
    // The same offer again: duplicate, not forwarded.
    assert_eq!(node_handle(&s0, &pending, &offer, true).0, OwnDecision::DuplicateOffer);
    // 2a. Acceptance signed by a third party for that offer: refused.
    assert_eq!(node_handle(&s0, &pending, &accept_msg(&o3(), 1, &offer), true).0, OwnDecision::Reject(OwnReject::BadSignature));
    // 2b. Acceptance by the old owner (not the offered keys): refused.
    assert_eq!(node_handle(&s0, &pending, &accept_msg(&o1(), 1, &offer), true).0, OwnDecision::Reject(OwnReject::BadSignature));
    // 2c. Right signer, wrong sequence / other offer: refused.
    assert_eq!(node_handle(&s0, &pending, &accept_msg(&o2(), 2, &offer), true).0, OwnDecision::Reject(OwnReject::SeqMismatch));
    let other = offer_msg(&o1(), &o2(), 5);
    assert_eq!(node_handle(&s0, &pending, &accept_msg(&o2(), 1, &other), true).0, OwnDecision::Reject(OwnReject::DigestMismatch));
    // 2d. Valid acceptance: transferred.
    let (d, ns, _) = node_handle(&s0, &pending, &accept_msg(&o2(), 1, &offer), true);
    assert_eq!(d, OwnDecision::Transferred);
    let s1 = ns.unwrap();
    assert_eq!((s1.seq, s1.current.clone(), s1.previous.clone()), (1, owner_keys(&o2()), Some(owner_keys(&o1()))));
    // After: o1 can no longer sign authority messages, o2 can.
    let pol = AuthPolicy::default();
    let rev_by_o1 = o1().sign(kind::REVOCATION, SUITE_HYBRID, &NET, b"x");
    assert_eq!(verify_authority(&pol, &NET, &s1.current.keys(), &rev_by_o1), Err(AuthReject::BadSignature));
    let rev_by_o2 = o2().sign(kind::REVOCATION, SUITE_HYBRID, &NET, b"x");
    assert!(verify_authority(&pol, &NET, &s1.current.keys(), &rev_by_o2).is_ok());
    // Replaying the old offer (seq 1) after the transfer: stale (and signed by o1 anyway).
    assert_ne!(node_handle(&s1, &None, &offer, true).0, OwnDecision::OfferPending);
    let replay_signed_by_o2 = offer_msg(&o2(), &o3(), 1);
    assert_eq!(node_handle(&s1, &None, &replay_signed_by_o2, true).0, OwnDecision::Reject(OwnReject::StaleSeq));
}

#[test]
fn own_acceptance_without_offer_and_forged_offers() {
    let s0 = initial();
    let offer = offer_msg(&o1(), &o2(), 1);
    assert_eq!(node_handle(&s0, &None, &accept_msg(&o2(), 1, &offer), true).0, OwnDecision::Reject(OwnReject::NoPendingOffer));
    // An offer signed by the would-be new owner (or anyone but the current owner).
    assert_eq!(node_handle(&s0, &None, &offer_msg(&o2(), &o2(), 1), true).0, OwnDecision::Reject(OwnReject::BadSignature));
    assert_eq!(node_handle(&s0, &None, &offer_msg(&o3(), &o3(), 1), true).0, OwnDecision::Reject(OwnReject::BadSignature));
    // An Ed25519-only offer is a downgrade (kind 4 is hybrid by default).
    let ed_only = o1().sign(kind::OWNERSHIP_TRANSFER, SUITE_ED25519, &NET, &encode_offer(1, &owner_keys(&o2())));
    assert_eq!(verify_authority(&AuthPolicy::default(), &NET, &s0.current.keys(), &ed_only), Err(AuthReject::Downgrade));
    // Relabelling an acceptance as an offer only changes which keys are tried.
    let mut relabelled = accept_msg(&o2(), 1, &offer);
    relabelled[16] = SUB_OFFER;
    assert_eq!(node_handle(&s0, &None, &relabelled, true).0, OwnDecision::Reject(OwnReject::BadSignature));
}

#[test]
fn own_needs_resign_before_a_second_transfer() {
    let s0 = initial();
    assert_eq!(node_handle(&s0, &None, &offer_msg(&o1(), &o2(), 1), false).0, OwnDecision::Reject(OwnReject::NeedsResign));
    assert_eq!(offer_decision(3, None, 4, &[0; 32], false), OwnDecision::Reject(OwnReject::NeedsResign));
    assert_eq!(offer_decision(3, None, 4, &[0; 32], true), OwnDecision::OfferPending);
}

#[test]
fn own_record_roundtrip_corruption_and_slots() {
    let s0 = initial();
    let s1 = OwnerState { seq: 1, current: owner_keys(&o2()), previous: Some(owner_keys(&o1())) };
    for s in [&s0, &s1] {
        let r = s.to_record();
        assert_eq!(r.len(), OWNER_RECORD_LEN);
        assert_eq!(OwnerState::from_record(&r).as_ref(), Some(s));
    }
    let r1 = s1.to_record();
    for i in (0..r1.len()).step_by(7) {
        let mut c = r1.clone();
        c[i] ^= 0x01;
        assert_eq!(OwnerState::from_record(&c), None, "corrupted byte {}", i);
    }
    let init = owner_keys(&o1());
    assert_eq!(OwnerState::from_slots(&[0xFF; 4096], &[0xFF; 4096], init.clone()), s0, "nothing stored: initial owner");
    let r0 = s0.to_record();
    assert_eq!(OwnerState::from_slots(&r0, &r1, init.clone()), s1);
    assert_eq!(OwnerState::from_slots(&r1, &r0, init.clone()), s1);
    assert_eq!(OwnerState::from_slots(&r1[..100], &r0, init), s0, "torn newer slot ignored");
    assert_eq!(OwnerState::slot_to_overwrite(&r1, &r0), 1);
    assert_eq!(OwnerState::slot_to_overwrite(&r0, &r1), 0);
}
