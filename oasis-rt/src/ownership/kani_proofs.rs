use super::*;

// Unwind >= 33 for the [u8; 32] digest comparisons (memcmp loop of 32 iterations).

/// PROVE: ownership changes (`Transferred`) only if an offer is pending (it was
/// verified with the CURRENT owner's keys to become pending), the acceptance was
/// verified with the OFFERED keys, it names the same sequence and the exact offer,
/// and the sequence rises.
#[kani::proof]
#[kani::unwind(34)]
fn proof_own_transfer_requires_both_signatures() {
    let state_seq: u32 = kani::any();
    let has_pending: bool = kani::any();
    let p_seq: u32 = kani::any();
    let p_digest: [u8; 32] = kani::any();
    let accept = Accept { seq: kani::any(), offer_digest: kani::any() };
    let verified: bool = kani::any();
    let pending = if has_pending { Some((p_seq, &p_digest)) } else { None };
    if accept_decision(state_seq, pending, &accept, verified) == OwnDecision::Transferred {
        assert!(has_pending && verified);
        assert!(accept.seq == p_seq && accept.offer_digest == p_digest);
        assert!(p_seq > state_seq);
    }
}

/// PROVE: an offer becomes pending only with a sequence above the current one and
/// a revocation list signed by the current owner; the same offer twice is never
/// pending twice.
#[kani::proof]
#[kani::unwind(34)]
fn proof_own_offer_rules() {
    let state_seq: u32 = kani::any();
    let offer_seq: u32 = kani::any();
    let digest: [u8; 32] = kani::any();
    let has_pending: bool = kani::any();
    let pd: [u8; 32] = kani::any();
    let resigned: bool = kani::any();
    let d = offer_decision(state_seq, if has_pending { Some(&pd) } else { None }, offer_seq, &digest, resigned);
    if d == OwnDecision::OfferPending {
        assert!(offer_seq > state_seq && resigned);
        assert!(!(has_pending && pd == digest));
    }
}

/// PROVE: the parsers never panic on short inputs and accept only their sub-type.
#[kani::proof]
#[kani::unwind(34)]
fn proof_own_accept_parse_total() {
    let buf: [u8; ACCEPT_LEN + 1] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= ACCEPT_LEN + 1);
    if let Some(a) = parse_accept(&buf[..len]) {
        assert!(len == ACCEPT_LEN && buf[0] == SUB_ACCEPT);
        assert!(a.seq == u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]));
    }
    assert!(parse_offer(&buf[..len]).is_none(), "far shorter than an offer");
}
