//! OASIS — network ownership and its two-message transfer (`OAU1` kind 4, Phase 1.2).
//!
//! Spec: `docs/specs/ENROLLMENT_OWNERSHIP_SPEC.md` §4. The owner is the authority of
//! a network (hybrid Ed25519 + ML-DSA-44 keys). A transfer needs BOTH owners:
//!
//! 1. offer, signed by the CURRENT owner:
//!    `sub u8 (=1) | transfer_seq u32 | new_ed_pk[32] | new_mldsa_pk[1312]`;
//! 2. acceptance, signed by the NEW owner (keys taken from the pending offer):
//!    `sub u8 (=2) | transfer_seq u32 | SHA-256(complete offer message)`.
//!
//! The decisions are pure functions over sequence numbers and digests (proved by
//! Kani); signature checks are done by the caller with `authority::verify_authority`.

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, vec::Vec};
use sha2::{Digest, Sha256};

use crate::authority::{AuthorityKeys, MLDSA44_PK_LEN};

pub const SUB_OFFER: u8 = 1;
pub const SUB_ACCEPT: u8 = 2;
pub const OFFER_LEN: usize = 1 + 4 + 32 + MLDSA44_PK_LEN;
pub const ACCEPT_LEN: usize = 1 + 4 + 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerKeys {
    pub ed25519: [u8; 32],
    pub mldsa44: Box<[u8; MLDSA44_PK_LEN]>,
}

impl OwnerKeys {
    pub fn new(ed25519: [u8; 32], mldsa44: &[u8]) -> Option<Self> {
        if mldsa44.len() != MLDSA44_PK_LEN {
            return None;
        }
        let mut ml = Box::new([0u8; MLDSA44_PK_LEN]);
        ml.copy_from_slice(mldsa44);
        Some(OwnerKeys { ed25519, mldsa44: ml })
    }

    pub fn keys(&self) -> AuthorityKeys<'_> {
        AuthorityKeys { ed25519: self.ed25519, mldsa44: &self.mldsa44 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub seq: u32,
    pub new_owner: OwnerKeys,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accept {
    pub seq: u32,
    pub offer_digest: [u8; 32],
}

/// The sub-type, read BEFORE verification only to choose which keys verify the
/// message (current owner for an offer, pending new owner for an acceptance). A
/// forged sub-type makes verification fail.
pub fn content_sub(content: &[u8]) -> Option<u8> {
    content.first().copied()
}

pub fn parse_offer(c: &[u8]) -> Option<Offer> {
    if c.len() != OFFER_LEN || c[0] != SUB_OFFER {
        return None;
    }
    let seq = u32::from_le_bytes([c[1], c[2], c[3], c[4]]);
    let mut ed = [0u8; 32];
    ed.copy_from_slice(&c[5..37]);
    Some(Offer { seq, new_owner: OwnerKeys::new(ed, &c[37..])? })
}

pub fn parse_accept(c: &[u8]) -> Option<Accept> {
    if c.len() != ACCEPT_LEN || c[0] != SUB_ACCEPT {
        return None;
    }
    let seq = u32::from_le_bytes([c[1], c[2], c[3], c[4]]);
    let mut offer_digest = [0u8; 32];
    offer_digest.copy_from_slice(&c[5..37]);
    Some(Accept { seq, offer_digest })
}

pub fn encode_offer(seq: u32, new_owner: &OwnerKeys) -> Vec<u8> {
    let mut c = Vec::with_capacity(OFFER_LEN);
    c.push(SUB_OFFER);
    c.extend_from_slice(&seq.to_le_bytes());
    c.extend_from_slice(&new_owner.ed25519);
    c.extend_from_slice(&new_owner.mldsa44[..]);
    c
}

pub fn encode_accept(seq: u32, offer_digest: &[u8; 32]) -> [u8; ACCEPT_LEN] {
    let mut c = [0u8; ACCEPT_LEN];
    c[0] = SUB_ACCEPT;
    c[1..5].copy_from_slice(&seq.to_le_bytes());
    c[5..37].copy_from_slice(offer_digest);
    c
}

/// SHA-256 of the complete offer message (header, content and the old owner's
/// signatures): the acceptance is bound to this exact offer, network included.
pub fn offer_digest(offer_msg: &[u8]) -> [u8; 32] {
    Sha256::digest(offer_msg).into()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerState {
    pub seq: u32,
    pub current: OwnerKeys,
    /// The owner before the last transfer: only used to re-verify, at boot, a
    /// revocation list it signed (one level, see `NeedsResign`).
    pub previous: Option<OwnerKeys>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub seq: u32,
    pub new_owner: OwnerKeys,
    pub digest: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnReject {
    Malformed,
    /// `transfer_seq` not above the current one (replayed or old offer).
    StaleSeq,
    /// The persisted revocation list is not signed by the current owner: a second
    /// transfer could make it unverifiable at boot. The owner re-signs it first.
    NeedsResign,
    NoPendingOffer,
    /// Not signed by the keys of the pending offer.
    BadSignature,
    SeqMismatch,
    DigestMismatch,
    PersistFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnDecision {
    OfferPending,
    /// Same offer already pending: no change, not forwarded.
    DuplicateOffer,
    Transferred,
    Reject(OwnReject),
}

/// Decision for an offer that the caller verified with the CURRENT owner's keys.
pub fn offer_decision(state_seq: u32, pending_digest: Option<&[u8; 32]>, offer_seq: u32, digest: &[u8; 32], rev_signed_by_current: bool) -> OwnDecision {
    if offer_seq <= state_seq {
        return OwnDecision::Reject(OwnReject::StaleSeq);
    }
    if !rev_signed_by_current {
        return OwnDecision::Reject(OwnReject::NeedsResign);
    }
    if pending_digest == Some(digest) {
        return OwnDecision::DuplicateOffer;
    }
    OwnDecision::OfferPending
}

/// Decision for an acceptance. `verified_by_pending` = the caller's
/// `verify_authority` with the PENDING offer's keys succeeded. `Transferred` only if
/// an offer is pending, the acceptance is signed by the offered keys, and it names
/// the same sequence and the exact offer, and the sequence is still above the
/// current one.
pub fn accept_decision(state_seq: u32, pending: Option<(u32, &[u8; 32])>, accept: &Accept, verified_by_pending: bool) -> OwnDecision {
    let (p_seq, p_digest) = match pending {
        Some(p) => p,
        None => return OwnDecision::Reject(OwnReject::NoPendingOffer),
    };
    if !verified_by_pending {
        return OwnDecision::Reject(OwnReject::BadSignature);
    }
    if accept.seq != p_seq {
        return OwnDecision::Reject(OwnReject::SeqMismatch);
    }
    if accept.offer_digest != *p_digest {
        return OwnDecision::Reject(OwnReject::DigestMismatch);
    }
    if p_seq <= state_seq {
        return OwnDecision::Reject(OwnReject::StaleSeq);
    }
    OwnDecision::Transferred
}

/// The state after `Transferred`.
pub fn apply_transfer(state: &OwnerState, pending: &Pending) -> OwnerState {
    OwnerState { seq: pending.seq, current: pending.new_owner.clone(), previous: Some(state.current.clone()) }
}

// ── persisted owner record: two slots, the highest valid `seq` wins ──────────
const OWN_MAGIC: [u8; 4] = *b"OWN1";
const KEYS_LEN: usize = 32 + MLDSA44_PK_LEN;
/// `OWN1 | seq u32 | current (ed, mldsa) | has_prev u8 | previous (ed, mldsa) | check4`.
pub const OWNER_RECORD_LEN: usize = 4 + 4 + KEYS_LEN + 1 + KEYS_LEN + 4;

fn own_check(body: &[u8]) -> [u8; 4] {
    let d = Sha256::new().chain_update(OWN_MAGIC).chain_update(body).finalize();
    [d[0], d[1], d[2], d[3]]
}

fn put_keys(r: &mut Vec<u8>, k: Option<&OwnerKeys>) {
    match k {
        Some(k) => {
            r.extend_from_slice(&k.ed25519);
            r.extend_from_slice(&k.mldsa44[..]);
        }
        None => r.resize(r.len() + KEYS_LEN, 0),
    }
}

impl OwnerState {
    pub fn to_record(&self) -> Vec<u8> {
        let mut r = Vec::with_capacity(OWNER_RECORD_LEN);
        r.extend_from_slice(&OWN_MAGIC);
        r.extend_from_slice(&self.seq.to_le_bytes());
        put_keys(&mut r, Some(&self.current));
        r.push(self.previous.is_some() as u8);
        put_keys(&mut r, self.previous.as_ref());
        let c = own_check(&r[4..]);
        r.extend_from_slice(&c);
        r
    }

    /// Integrity-checked restore (spec §3: on an RP2040, flash is trusted like the
    /// firmware). `None` if absent, torn or corrupted. Trailing bytes are ignored.
    pub fn from_record(r: &[u8]) -> Option<Self> {
        if r.len() < OWNER_RECORD_LEN || r[0..4] != OWN_MAGIC {
            return None;
        }
        let r = &r[..OWNER_RECORD_LEN];
        if r[OWNER_RECORD_LEN - 4..] != own_check(&r[4..OWNER_RECORD_LEN - 4]) || r[8 + KEYS_LEN] > 1 {
            return None;
        }
        let seq = u32::from_le_bytes([r[4], r[5], r[6], r[7]]);
        let keys_at = |o: usize| {
            let mut ed = [0u8; 32];
            ed.copy_from_slice(&r[o..o + 32]);
            OwnerKeys::new(ed, &r[o + 32..o + KEYS_LEN])
        };
        let current = keys_at(8)?;
        let previous = if r[8 + KEYS_LEN] == 1 { Some(keys_at(9 + KEYS_LEN)?) } else { None };
        Some(OwnerState { seq, current, previous })
    }

    /// The valid slot with the highest `seq`, else the compiled initial owner.
    pub fn from_slots(a: &[u8], b: &[u8], initial: OwnerKeys) -> Self {
        match (OwnerState::from_record(a), OwnerState::from_record(b)) {
            (Some(x), Some(y)) => {
                if x.seq >= y.seq {
                    x
                } else {
                    y
                }
            }
            (Some(x), None) | (None, Some(x)) => x,
            (None, None) => OwnerState { seq: 0, current: initial, previous: None },
        }
    }

    /// Never overwrite the slot holding the newest valid record.
    pub fn slot_to_overwrite(a: &[u8], b: &[u8]) -> usize {
        match (OwnerState::from_record(a), OwnerState::from_record(b)) {
            (Some(x), Some(y)) => {
                if x.seq >= y.seq {
                    1
                } else {
                    0
                }
            }
            (Some(_), None) => 1,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
