//! OASIS — signed mesh revocation (`ORV1`), carried as the payload of v0B envelopes.
//!
//! Spec: `docs/REVOCATION_AND_ACTUATION_SPEC.md` Part E. Transport (origin auth,
//! network binding, accept-once, strict mode) is v0B's, unchanged. Authority is the
//! operator signature *inside* the content, so any registered node can carry or
//! re-serve a list (catch-up). Signatures are verified by the caller with
//! `oasis_operator_key::OperatorAuthority::verify_authorization(&signed_message(p), &p.sigs)`
//! (single key or k-of-n); this module stays pure and takes the result as `sig_ok`,
//! which keeps [`revocation_transition`] provable by Kani.
//!
//! Content (little-endian):
//! `"ORV1" | network_id[8] | epoch u64 | issued_at u64 | count u16 | fp[8]*count
//!  | nsig u8 | (pub[32] | sig[64]) * nsig`
//! Signed message: `"OASIS-REVOKE-v1" | network_id | epoch | issued_at | count | fps`.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

pub const ORV1_MAGIC: [u8; 4] = *b"ORV1";
pub const OEP1_MAGIC: [u8; 4] = *b"OEP1";
pub const REVOKE_DOMAIN: &[u8] = b"OASIS-REVOKE-v1";
/// Revoked fingerprints per list (MCU profile; see spec E.2 for the frame budget).
pub const MAX_REVOKED: usize = 16;
/// Signatures per list (k-of-n quorum members carried).
pub const MAX_SIGS: usize = 8;
pub const FP_LEN: usize = 8;
pub type Fp = [u8; FP_LEN];
pub type OpPub = [u8; 32];
pub type OpSig = [u8; 64];

const HEAD_LEN: usize = 4 + 8 + 8 + 8 + 2; // magic..count
const SIG_ENTRY_LEN: usize = 32 + 64;

/// A parsed, canonical `ORV1` list (signature NOT yet verified).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRevocation {
    pub network_id: [u8; 8],
    pub epoch: u64,
    pub issued_at: u64,
    /// Strictly ascending, no duplicates.
    pub fps: Vec<Fp>,
    pub sigs: Vec<(OpPub, OpSig)>,
}

/// Why a revocation list was not applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevReject {
    /// Bad magic, truncated, trailing bytes, too many entries, non-canonical order.
    Malformed,
    WrongNetwork,
    /// Epoch lower than the one already held.
    Rollback,
    BadOperatorSig,
    /// The new list drops a fingerprint already revoked (revocation is permanent).
    Shrink,
    /// The new state could not be made durable (set by the caller, not by the pure fn).
    PersistFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevDecision {
    /// Fresh epoch accepted: persist, apply, then forward once.
    Applied,
    /// Same epoch as held: ignore, do NOT forward.
    Duplicate,
    Reject(RevReject),
}

/// Relay revocation state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RevState {
    pub epoch: u64,
    /// Strictly ascending.
    pub revoked: Vec<Fp>,
}

impl RevState {
    pub fn is_revoked(&self, fp: &Fp) -> bool {
        self.revoked.binary_search(fp).is_ok()
    }
}

/// Canonical signed message for a list.
pub fn signed_message_parts(network_id: &[u8; 8], epoch: u64, issued_at: u64, fps: &[Fp]) -> Vec<u8> {
    let mut m = Vec::with_capacity(REVOKE_DOMAIN.len() + 8 + 8 + 8 + 2 + fps.len() * FP_LEN);
    m.extend_from_slice(REVOKE_DOMAIN);
    m.extend_from_slice(network_id);
    m.extend_from_slice(&epoch.to_le_bytes());
    m.extend_from_slice(&issued_at.to_le_bytes());
    m.extend_from_slice(&(fps.len() as u16).to_le_bytes());
    for fp in fps {
        m.extend_from_slice(fp);
    }
    m
}

/// Message the operator quorum signs for a parsed list.
pub fn signed_message(p: &ParsedRevocation) -> Vec<u8> {
    signed_message_parts(&p.network_id, p.epoch, p.issued_at, &p.fps)
}

/// Encode an `ORV1` content block. `fps` must be strictly ascending.
pub fn encode_orv1(network_id: &[u8; 8], epoch: u64, issued_at: u64, fps: &[Fp], sigs: &[(OpPub, OpSig)]) -> Vec<u8> {
    let mut b = Vec::with_capacity(HEAD_LEN + fps.len() * FP_LEN + 1 + sigs.len() * SIG_ENTRY_LEN);
    b.extend_from_slice(&ORV1_MAGIC);
    b.extend_from_slice(network_id);
    b.extend_from_slice(&epoch.to_le_bytes());
    b.extend_from_slice(&issued_at.to_le_bytes());
    b.extend_from_slice(&(fps.len() as u16).to_le_bytes());
    for fp in fps {
        b.extend_from_slice(fp);
    }
    b.push(sigs.len() as u8);
    for (pk, sg) in sigs {
        b.extend_from_slice(pk);
        b.extend_from_slice(sg);
    }
    b
}

fn rd_u64(b: &[u8], at: usize) -> u64 {
    let mut x = [0u8; 8];
    x.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(x)
}

/// Parse and canonicality-check an `ORV1` block. Never panics.
pub fn parse_orv1(b: &[u8]) -> Result<ParsedRevocation, RevReject> {
    if b.len() < HEAD_LEN + 1 || b[0..4] != ORV1_MAGIC {
        return Err(RevReject::Malformed);
    }
    let mut network_id = [0u8; 8];
    network_id.copy_from_slice(&b[4..12]);
    let epoch = rd_u64(b, 12);
    let issued_at = rd_u64(b, 20);
    let count = u16::from_le_bytes([b[28], b[29]]) as usize;
    if count > MAX_REVOKED {
        return Err(RevReject::Malformed);
    }
    let fps_end = HEAD_LEN + count * FP_LEN;
    if b.len() < fps_end + 1 {
        return Err(RevReject::Malformed);
    }
    let mut fps: Vec<Fp> = Vec::with_capacity(count);
    for i in 0..count {
        let mut fp = [0u8; FP_LEN];
        fp.copy_from_slice(&b[HEAD_LEN + i * FP_LEN..HEAD_LEN + (i + 1) * FP_LEN]);
        if let Some(prev) = fps.last() {
            if fp <= *prev {
                return Err(RevReject::Malformed); // not strictly ascending
            }
        }
        fps.push(fp);
    }
    let nsig = b[fps_end] as usize;
    if nsig == 0 || nsig > MAX_SIGS || b.len() != fps_end + 1 + nsig * SIG_ENTRY_LEN {
        return Err(RevReject::Malformed);
    }
    let mut sigs = Vec::with_capacity(nsig);
    for i in 0..nsig {
        let at = fps_end + 1 + i * SIG_ENTRY_LEN;
        let mut pk = [0u8; 32];
        let mut sg = [0u8; 64];
        pk.copy_from_slice(&b[at..at + 32]);
        sg.copy_from_slice(&b[at + 32..at + SIG_ENTRY_LEN]);
        sigs.push((pk, sg));
    }
    Ok(ParsedRevocation { network_id, epoch, issued_at, fps, sigs })
}

/// `a ⊇ b` for strictly ascending slices.
pub fn is_superset(a: &[Fp], b: &[Fp]) -> bool {
    let mut i = 0;
    for x in b {
        while i < a.len() && a[i] < *x {
            i += 1;
        }
        if i == a.len() || a[i] != *x {
            return false;
        }
        i += 1;
    }
    true
}

/// Pure relay transition (spec E.3 steps 2-5). Returns the new state only on
/// `Applied`; on every other outcome the caller's state is untouched. The caller
/// then persists the new state (step 6) BEFORE applying it to the router (step 7)
/// and forwarding the carrying envelope once (step 8).
pub fn revocation_transition(
    state: &RevState,
    my_network: &[u8; 8],
    p: &ParsedRevocation,
    sig_ok: bool,
) -> (RevDecision, Option<RevState>) {
    if p.network_id != *my_network {
        return (RevDecision::Reject(RevReject::WrongNetwork), None);
    }
    if p.epoch == state.epoch {
        return (RevDecision::Duplicate, None);
    }
    if p.epoch < state.epoch {
        return (RevDecision::Reject(RevReject::Rollback), None);
    }
    if !sig_ok {
        return (RevDecision::Reject(RevReject::BadOperatorSig), None);
    }
    if !is_superset(&p.fps, &state.revoked) {
        return (RevDecision::Reject(RevReject::Shrink), None);
    }
    (RevDecision::Applied, Some(RevState { epoch: p.epoch, revoked: p.fps.clone() }))
}

// ─── epoch beacon (catch-up, spec E.5) ───

/// `"OEP1" | epoch u64`.
pub fn encode_oep1(epoch: u64) -> [u8; 12] {
    let mut b = [0u8; 12];
    b[0..4].copy_from_slice(&OEP1_MAGIC);
    b[4..12].copy_from_slice(&epoch.to_le_bytes());
    b
}

pub fn parse_oep1(b: &[u8]) -> Option<u64> {
    if b.len() != 12 || b[0..4] != OEP1_MAGIC {
        return None;
    }
    Some(rd_u64(b, 4))
}

/// A node holding `my_epoch` should re-serve its stored list to a neighbour that
/// announced `neighbour_epoch`.
pub fn should_serve_catchup(my_epoch: u64, neighbour_epoch: u64) -> bool {
    my_epoch > neighbour_epoch
}

// ─── persistence: two alternating slots holding the last accepted ORV1 blob ───

/// Raw access to two slots of up to `cap()` bytes (e.g. two flash sectors).
pub trait BlobSlots {
    fn cap(&self) -> usize;
    /// Raw slot bytes (len == cap()).
    fn read(&self, slot: usize) -> Vec<u8>;
    /// Write a record into one slot; may be torn by a power loss.
    fn write(&mut self, slot: usize, rec: &[u8]) -> bool;
}

const RVB_MAGIC: [u8; 4] = *b"RVB1";

fn rec_check(blob: &[u8]) -> [u8; 4] {
    let mut h = Sha256::new();
    h.update(RVB_MAGIC);
    h.update(blob);
    let d = h.finalize();
    [d[0], d[1], d[2], d[3]]
}

/// `RVB1 | len u16 | blob | check4`.
pub fn encode_blob_record(blob: &[u8]) -> Vec<u8> {
    let mut r = Vec::with_capacity(4 + 2 + blob.len() + 4);
    r.extend_from_slice(&RVB_MAGIC);
    r.extend_from_slice(&(blob.len() as u16).to_le_bytes());
    r.extend_from_slice(blob);
    r.extend_from_slice(&rec_check(blob));
    r
}

pub fn decode_blob_record(r: &[u8]) -> Option<Vec<u8>> {
    if r.len() < 10 || r[0..4] != RVB_MAGIC {
        return None;
    }
    let len = u16::from_le_bytes([r[4], r[5]]) as usize;
    if r.len() < 6 + len + 4 {
        return None;
    }
    let blob = &r[6..6 + len];
    if r[6 + len..6 + len + 4] != rec_check(blob) {
        return None;
    }
    Some(blob.to_vec())
}

fn slot_epoch<S: BlobSlots>(s: &S, slot: usize) -> Option<(u64, Vec<u8>)> {
    let blob = decode_blob_record(&s.read(slot))?;
    let p = parse_orv1(&blob).ok()?;
    Some((p.epoch, blob))
}

/// The newest valid persisted blob (highest epoch), if any. The caller must
/// re-verify its operator signature before applying it.
pub fn load_latest_blob<S: BlobSlots>(s: &S) -> Option<Vec<u8>> {
    match (slot_epoch(s, 0), slot_epoch(s, 1)) {
        (Some(a), Some(b)) => Some(if a.0 >= b.0 { a.1 } else { b.1 }),
        (Some(a), None) => Some(a.1),
        (None, Some(b)) => Some(b.1),
        (None, None) => None,
    }
}

/// Persist `blob` into the slot NOT holding the newest epoch, then read back.
/// A torn write damages only that slot; the previous list stays loadable.
pub fn store_blob<S: BlobSlots>(s: &mut S, blob: &[u8]) -> bool {
    let rec = encode_blob_record(blob);
    if rec.len() > s.cap() {
        return false;
    }
    let target = match (slot_epoch(s, 0), slot_epoch(s, 1)) {
        (Some(a), Some(b)) => {
            if a.0 >= b.0 {
                1
            } else {
                0
            }
        }
        (Some(_), None) => 1,
        _ => 0,
    };
    if !s.write(target, &rec) {
        return false;
    }
    decode_blob_record(&s.read(target)).as_deref() == Some(blob)
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
