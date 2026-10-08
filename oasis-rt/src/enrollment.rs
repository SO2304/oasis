//! OASIS — enrollment attestations (`OAU1` kind 2) and the node registry (Phase 1.2).
//!
//! Spec: `docs/specs/ENROLLMENT_OWNERSHIP_SPEC.md` §3. The network owner signs (as an
//! `OAU1` authority message) an attestation binding a node's public key to a role
//! and permissions. A node's v0B registry is filled only from verified attestations,
//! so an unenrolled node is refused at the first hop ("unknown sender").
//!
//! Content: `version u8 (=1) | node_pk[32] | role u8 | permissions u32 LE | enroll_seq u32 LE`.
//! The fingerprint is derived (`SHA-256(pk)[0..8]`), never transmitted.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

pub type Fp = [u8; 8];

pub const ATT_VERSION: u8 = 1;
pub const ATT_LEN: usize = 1 + 32 + 1 + 4 + 4;
/// Registry capacity (one 4 KiB flash sector holds 83 entries of 49 B).
pub const MAX_ENTRIES: usize = 80;

/// Roles: logged only, no rule depends on them in this phase.
pub mod role {
    pub const SENSOR: u8 = 1;
    pub const RELAY: u8 = 2;
    pub const GATEWAY: u8 = 3;
    pub const ACTUATOR: u8 = 4;
}

/// Permissions: `ACTUATE` is enforced by the actuation gate; other bits are reserved.
pub mod perm {
    pub const ACTUATE: u32 = 1 << 0;
    /// Part G: may send a **stop** order. Deliberately distinct from [`ACTUATE`] — a
    /// supervisor may be allowed to stop a machine without being allowed to move it, and
    /// conversely a node that can act must not get the stop (and therefore the latch,
    /// and therefore a denial of service) for free.
    pub const STOP: u32 = 1 << 1;
    /// Part H: may issue a supervision beacon.
    pub const SUPERVISE: u32 = 1 << 2;
    pub const KNOWN: u32 = ACTUATE | STOP | SUPERVISE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attestation {
    pub node_pk: [u8; 32],
    pub role: u8,
    pub permissions: u32,
    pub enroll_seq: u32,
}

impl Attestation {
    pub fn fp(&self) -> Fp {
        crate::identity::fingerprint(&self.node_pk)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollReject {
    Malformed,
    UnknownRole,
    ReservedPermission,
    /// The fingerprint is revoked: revocation is permanent.
    Revoked,
    /// `enroll_seq` not strictly greater than the one held for this fingerprint.
    StaleSeq,
    Full,
    /// Set by the caller when the new registry could not be made durable.
    PersistFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollDecision {
    Enrolled,
    /// Same node, higher `enroll_seq`: role or permissions changed.
    Updated,
    Reject(EnrollReject),
}

/// Parse an attestation content. Never panics.
pub fn parse_attestation(c: &[u8]) -> Result<Attestation, EnrollReject> {
    if c.len() != ATT_LEN || c[0] != ATT_VERSION {
        return Err(EnrollReject::Malformed);
    }
    let mut node_pk = [0u8; 32];
    node_pk.copy_from_slice(&c[1..33]);
    let role = c[33];
    if !(role::SENSOR..=role::ACTUATOR).contains(&role) {
        return Err(EnrollReject::UnknownRole);
    }
    let permissions = u32::from_le_bytes([c[34], c[35], c[36], c[37]]);
    if permissions & !perm::KNOWN != 0 {
        return Err(EnrollReject::ReservedPermission);
    }
    let enroll_seq = u32::from_le_bytes([c[38], c[39], c[40], c[41]]);
    Ok(Attestation { node_pk, role, permissions, enroll_seq })
}

pub fn encode_attestation(a: &Attestation) -> [u8; ATT_LEN] {
    let mut c = [0u8; ATT_LEN];
    c[0] = ATT_VERSION;
    c[1..33].copy_from_slice(&a.node_pk);
    c[33] = a.role;
    c[34..38].copy_from_slice(&a.permissions.to_le_bytes());
    c[38..42].copy_from_slice(&a.enroll_seq.to_le_bytes());
    c
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub fp: Fp,
    pub pk: [u8; 32],
    pub role: u8,
    pub permissions: u32,
    pub seq: u32,
}

/// Enrolled nodes. `gen` increases on every change (picks the newest flash slot).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registry {
    pub gen: u32,
    pub entries: Vec<Entry>,
}

/// Pure rule. `fp` must be `att.fp()` (computed by the caller, so that this rule
/// stays free of hashing for Kani). A revoked fingerprint is never enrolled; a
/// node's `enroll_seq` only ever increases.
pub fn enrollment_transition(reg: &Registry, fp: &Fp, att: &Attestation, revoked: bool) -> (EnrollDecision, Option<Registry>) {
    if revoked {
        return (EnrollDecision::Reject(EnrollReject::Revoked), None);
    }
    let entry = Entry { fp: *fp, pk: att.node_pk, role: att.role, permissions: att.permissions, seq: att.enroll_seq };
    let mut new = reg.clone();
    new.gen = reg.gen.wrapping_add(1);
    match reg.entries.iter().position(|e| e.fp == *fp) {
        Some(i) => {
            if att.enroll_seq <= reg.entries[i].seq {
                return (EnrollDecision::Reject(EnrollReject::StaleSeq), None);
            }
            new.entries[i] = entry;
            (EnrollDecision::Updated, Some(new))
        }
        None => {
            if reg.entries.len() >= MAX_ENTRIES {
                return (EnrollDecision::Reject(EnrollReject::Full), None);
            }
            new.entries.push(entry);
            (EnrollDecision::Enrolled, Some(new))
        }
    }
}

const REG_MAGIC: [u8; 4] = *b"ORG1";
const ENTRY_LEN: usize = 8 + 32 + 1 + 4 + 4;

fn reg_check(body: &[u8]) -> [u8; 4] {
    let d = Sha256::new().chain_update(REG_MAGIC).chain_update(body).finalize();
    [d[0], d[1], d[2], d[3]]
}

impl Registry {
    pub fn get(&self, fp: &Fp) -> Option<&Entry> {
        self.entries.iter().find(|e| e.fp == *fp)
    }

    /// Enrolled AND granted `p`.
    pub fn allows(&self, fp: &Fp, p: u32) -> bool {
        self.get(fp).is_some_and(|e| e.permissions & p == p)
    }

    /// `ORG1 | gen u32 | n u16 | n × (fp, pk, role, perms, seq) | check4`.
    pub fn to_record(&self) -> Vec<u8> {
        let mut r = Vec::with_capacity(10 + self.entries.len() * ENTRY_LEN + 4);
        r.extend_from_slice(&REG_MAGIC);
        r.extend_from_slice(&self.gen.to_le_bytes());
        r.extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        for e in &self.entries {
            r.extend_from_slice(&e.fp);
            r.extend_from_slice(&e.pk);
            r.push(e.role);
            r.extend_from_slice(&e.permissions.to_le_bytes());
            r.extend_from_slice(&e.seq.to_le_bytes());
        }
        let c = reg_check(&r[4..]);
        r.extend_from_slice(&c);
        r
    }

    /// Integrity-checked (not signature-verified) restore; `None` if absent, torn,
    /// corrupted, or inconsistent (fingerprint not derived from the key, unknown
    /// role or permission bit). Trailing bytes (erased flash) are ignored.
    pub fn from_record(r: &[u8]) -> Option<Self> {
        if r.len() < 14 || r[0..4] != REG_MAGIC {
            return None;
        }
        let gen = u32::from_le_bytes([r[4], r[5], r[6], r[7]]);
        let n = u16::from_le_bytes([r[8], r[9]]) as usize;
        let end = 10 + n * ENTRY_LEN;
        if n > MAX_ENTRIES || r.len() < end + 4 || r[end..end + 4] != reg_check(&r[4..end]) {
            return None;
        }
        let mut entries = Vec::with_capacity(n);
        for k in 0..n {
            let e = &r[10 + k * ENTRY_LEN..10 + (k + 1) * ENTRY_LEN];
            let mut fp = [0u8; 8];
            fp.copy_from_slice(&e[0..8]);
            let mut pk = [0u8; 32];
            pk.copy_from_slice(&e[8..40]);
            let role = e[40];
            let permissions = u32::from_le_bytes([e[41], e[42], e[43], e[44]]);
            let seq = u32::from_le_bytes([e[45], e[46], e[47], e[48]]);
            if fp != crate::identity::fingerprint(&pk) || !(role::SENSOR..=role::ACTUATOR).contains(&role) || permissions & !perm::KNOWN != 0 || entries.iter().any(|x: &Entry| x.fp == fp) {
                return None;
            }
            entries.push(Entry { fp, pk, role, permissions, seq });
        }
        Some(Registry { gen, entries })
    }

    /// Which slot to overwrite: never the one holding the newest valid record, so a
    /// torn write leaves the previous registry loadable.
    pub fn slot_to_overwrite(a: &[u8], b: &[u8]) -> usize {
        match (Registry::from_record(a), Registry::from_record(b)) {
            (Some(x), Some(y)) => {
                if x.gen >= y.gen {
                    1
                } else {
                    0
                }
            }
            (Some(_), None) => 1,
            _ => 0,
        }
    }

    /// The valid slot with the highest `gen`, or an empty registry.
    pub fn from_slots(a: &[u8], b: &[u8]) -> Self {
        match (Registry::from_record(a), Registry::from_record(b)) {
            (Some(x), Some(y)) => {
                if x.gen >= y.gen {
                    x
                } else {
                    y
                }
            }
            (Some(x), None) | (None, Some(x)) => x,
            (None, None) => Registry::default(),
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
