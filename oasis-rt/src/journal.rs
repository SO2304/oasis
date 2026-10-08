//! OASIS — tamper-evident decision journal (part I of `docs/AUTHORITY_HARDENING_SPEC.md`).
//!
//! One entry per gate decision, **accepted or refused**, linked into a SHA-256 hash chain.
//! Pure and `no_std`: this module computes and verifies, it never touches flash. The
//! caller persists [`JournalHead`] and the entry bytes.
//!
//! # Why the refusals matter
//!
//! Règlement (UE) 2023/1230, annexe III, 1.1.9, dernier alinéa, verbatim: « La machine ou
//! le produit connexe **recueillent la preuve d'une intervention légitime ou illégitime**
//! dans les logiciels ou d'une modification des logiciels installés sur ceux-ci ou de sa
//! configuration. » It is the word *illégitime* that forces refused orders to be recorded,
//! not only executed ones. The same obligation appears in the CRA (annexe I, partie I,
//! point 2 l)) and in IEC 62443-4-2 (CR 2.8 and its cluster). See
//! `docs/compliance/MACHINERY_REGULATION_2023_1230.md`.
//!
//! # What may and may not be claimed
//!
//! A hash chain proves nothing if the attacker can rewrite the head. On an RP2040,
//! physical access (BOOTSEL, SWD) rewrites everything, flash included.
//!
//! - **Permitted claim**: tamper-**evident** against a *remote* attacker, and detects
//!   accidental corruption. A modified, deleted or inserted entry is detected.
//! - **Forbidden claim**: "tamper-proof", or any resistance to physical access.
//!
//! The external anchor that would survive physical access — the node signing `(seq, head)`
//! periodically so the operator holds an earlier anchor — is specified in the spec and
//! **not implemented here**.
//!
//! # Format (little-endian)
//!
//! Entry, 32 bytes: `seq u32 | boot_id u64 | origin_fp [u8;8] | cmd_seq u32 | class u8
//! | decision u8 | flags u8 | reserved [u8;5] = 0`.
//!
//! **All five reserved bytes must be zero**, so an entry has exactly one encoding — the
//! lesson of the phase 2.3 fuzzing finding on `parse_oac1`. The first version of this
//! module checked only byte 27 and left bytes 28..32 unconstrained: `proof_journal_parse_total`
//! refuted the round-trip property immediately, which is the same defect class caught a
//! second time, this time by a proof rather than by a fuzzer.

use crate::actuation::{Decision, OrderClass, Reason, StopDecision, REASON_COUNT};
use sha2::{Digest, Sha256};

pub const ENTRY_LEN: usize = 32;
/// First reserved byte of an entry. Bytes `RESERVED_OFF..ENTRY_LEN` must all be zero.
pub const RESERVED_OFF: usize = 27;
pub const DOMAIN: &[u8] = b"OASIS-JOURNAL-v1";

/// Flag bits recorded alongside the decision, so a reader can tell *why* the gate saw
/// what it saw without replaying the whole state.
pub const FLAG_R14_SAFE: u8 = 1 << 0;
pub const FLAG_WITHIN_LIMITS: u8 = 1 << 1;
pub const FLAG_SUPERVISION_LIVE: u8 = 1 << 2;
pub const FLAG_STOPPED: u8 = 1 << 3;

/// Wire encoding of a decision. `Act` and `Stop` are the two acceptances; a refusal
/// carries its reason, offset so the two spaces never collide.
pub const DEC_ACT: u8 = 0;
pub const DEC_STOP: u8 = 1;
pub const DEC_REJECT_BASE: u8 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoggedDecision {
    Act,
    Stop,
    Reject(Reason),
}

impl LoggedDecision {
    pub fn to_byte(self) -> u8 {
        match self {
            LoggedDecision::Act => DEC_ACT,
            LoggedDecision::Stop => DEC_STOP,
            LoggedDecision::Reject(r) => DEC_REJECT_BASE + r as u8,
        }
    }

    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            DEC_ACT => Some(LoggedDecision::Act),
            DEC_STOP => Some(LoggedDecision::Stop),
            _ => {
                let i = b.checked_sub(DEC_REJECT_BASE)?;
                if (i as usize) < REASON_COUNT {
                    Some(LoggedDecision::Reject(reason_from_index(i)?))
                } else {
                    None
                }
            }
        }
    }
}

fn reason_from_index(i: u8) -> Option<Reason> {
    Some(match i {
        0 => Reason::NotVerified,
        1 => Reason::NotAuthorized,
        2 => Reason::Revoked,
        3 => Reason::Expired,
        4 => Reason::R14Unsafe,
        5 => Reason::OutOfLimits,
        6 => Reason::StaleOrReplayed,
        7 => Reason::Stopped,
        8 => Reason::SupervisionLost,
        9 => Reason::QuorumMissing,
        _ => return None,
    })
}

impl From<Decision> for LoggedDecision {
    fn from(d: Decision) -> Self {
        match d {
            Decision::Act => LoggedDecision::Act,
            Decision::Reject(r) => LoggedDecision::Reject(r),
        }
    }
}

impl From<StopDecision> for LoggedDecision {
    fn from(d: StopDecision) -> Self {
        match d {
            StopDecision::Stop => LoggedDecision::Stop,
            StopDecision::Reject(r) => LoggedDecision::Reject(r),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub seq: u32,
    pub boot_id: u64,
    pub origin_fp: [u8; 8],
    pub cmd_seq: u32,
    pub class: OrderClass,
    pub decision: LoggedDecision,
    pub flags: u8,
}

pub fn encode_entry(e: &Entry) -> [u8; ENTRY_LEN] {
    let mut b = [0u8; ENTRY_LEN];
    b[0..4].copy_from_slice(&e.seq.to_le_bytes());
    b[4..12].copy_from_slice(&e.boot_id.to_le_bytes());
    b[12..20].copy_from_slice(&e.origin_fp);
    b[20..24].copy_from_slice(&e.cmd_seq.to_le_bytes());
    b[24] = e.class as u8;
    b[25] = e.decision.to_byte();
    b[26] = e.flags;
    b
}

/// Total on every input: any 32 bytes that are not exactly one well-formed entry yield
/// `None`. Proved by `proof_journal_parse_total`.
pub fn parse_entry(b: &[u8]) -> Option<Entry> {
    if b.len() != ENTRY_LEN || b[RESERVED_OFF..ENTRY_LEN] != [0u8; ENTRY_LEN - RESERVED_OFF] {
        return None;
    }
    let mut boot = [0u8; 8];
    boot.copy_from_slice(&b[4..12]);
    let mut fp = [0u8; 8];
    fp.copy_from_slice(&b[12..20]);
    Some(Entry {
        seq: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        boot_id: u64::from_le_bytes(boot),
        origin_fp: fp,
        cmd_seq: u32::from_le_bytes([b[20], b[21], b[22], b[23]]),
        class: OrderClass::from_byte(b[24])?,
        decision: LoggedDecision::from_byte(b[25])?,
        flags: b[26],
    })
}

/// `h_0 = SHA-256(DOMAIN || boot_id)`.
pub fn genesis(boot_id: u64) -> [u8; 32] {
    let d = Sha256::new().chain_update(DOMAIN).chain_update(boot_id.to_le_bytes()).finalize();
    let mut h = [0u8; 32];
    h.copy_from_slice(&d);
    h
}

/// `h_n = SHA-256(DOMAIN || h_(n-1) || entry_n)`. A single bit changed anywhere in the
/// entry, or a different predecessor, gives a different `h_n`.
pub fn chain(prev: &[u8; 32], entry: &[u8; ENTRY_LEN]) -> [u8; 32] {
    let d = Sha256::new().chain_update(DOMAIN).chain_update(prev).chain_update(entry).finalize();
    let mut h = [0u8; 32];
    h.copy_from_slice(&d);
    h
}

/// What the caller persists. Two alternating flash slots, as `tx_lease::DualSlotStore`
/// already does — the mechanism proved across a real power cut in phase 1.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalHead {
    pub boot_id: u64,
    /// Sequence of the last entry folded into `hash`. `None` = nothing logged yet.
    pub seq: Option<u32>,
    pub hash: [u8; 32],
    /// Entries overwritten by the ring, for IEC 62443-4-2 CR 2.9 (storage capacity).
    pub overwritten: u32,
}

impl JournalHead {
    pub fn new(boot_id: u64) -> Self {
        JournalHead { boot_id, seq: None, hash: genesis(boot_id), overwritten: 0 }
    }
}

/// Appends decisions to the chain. Holds no buffer: [`Journal::append`] returns the bytes
/// and the caller writes them (entry first, then the head — a power cut in between leaves
/// one unconfirmed entry, which [`verify`] reports rather than hides).
#[derive(Debug, Clone, Copy)]
pub struct Journal {
    pub head: JournalHead,
}

impl Journal {
    pub fn new(boot_id: u64) -> Self {
        Journal { head: JournalHead::new(boot_id) }
    }

    pub fn next_seq(&self) -> u32 {
        match self.head.seq {
            None => 0,
            Some(s) => s.wrapping_add(1),
        }
    }

    /// Record one decision. Returns the entry bytes to persist.
    ///
    /// There is no path through this function that returns without producing an entry:
    /// that is what `proof_every_decision_is_logged` checks.
    pub fn append(&mut self, origin_fp: [u8; 8], cmd_seq: u32, class: OrderClass, decision: LoggedDecision, flags: u8) -> [u8; ENTRY_LEN] {
        let e = Entry { seq: self.next_seq(), boot_id: self.head.boot_id, origin_fp, cmd_seq, class, decision, flags };
        let bytes = encode_entry(&e);
        self.head.hash = chain(&self.head.hash, &bytes);
        self.head.seq = Some(e.seq);
        bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyResult {
    /// The chain recomputes to the head over all entries.
    Intact { entries: u32 },
    /// The chain does not reach the head: an entry was modified, or its predecessor
    /// differs.
    ///
    /// ⚠️ **`at` is where verification ended, not where the tampering is.** A single head
    /// hash detects a modification but **cannot localise it**: every hash after the
    /// altered entry differs, so the mismatch is only observable at the end. Localising
    /// would need a per-entry anchor. A deletion *is* localised, by the `seq` hole —
    /// hence [`VerifyResult::SeqGap`] being a separate verdict.
    Broken { at: u32 },
    /// `seq` jumps: an entry was removed. Reported separately from `Broken` because the
    /// two have different causes and a reader must be able to tell them apart.
    SeqGap { expected: u32, found: u32 },
    /// The chain is intact up to the last entry, which is not folded into the head: a
    /// power cut between writing the entry and updating the head.
    Unconfirmed { entries: u32 },
}

/// Recompute the chain over `entries` (each `ENTRY_LEN` bytes, oldest first) and compare
/// with `head`.
pub fn verify(boot_id: u64, entries: &[[u8; ENTRY_LEN]], head: &JournalHead) -> VerifyResult {
    let mut h = genesis(boot_id);
    let mut expected_seq: u32 = 0;
    for (i, raw) in entries.iter().enumerate() {
        let e = match parse_entry(raw) {
            Some(e) => e,
            None => return VerifyResult::Broken { at: i as u32 },
        };
        if e.seq != expected_seq {
            return VerifyResult::SeqGap { expected: expected_seq, found: e.seq };
        }
        h = chain(&h, raw);
        expected_seq = expected_seq.wrapping_add(1);
        // The head matches here: everything after this point is unconfirmed.
        if h == head.hash && head.seq == Some(e.seq) {
            let remaining = entries.len() - i - 1;
            return if remaining == 0 {
                VerifyResult::Intact { entries: entries.len() as u32 }
            } else {
                VerifyResult::Unconfirmed { entries: (i + 1) as u32 }
            };
        }
    }
    if entries.is_empty() && head.seq.is_none() && h == head.hash {
        return VerifyResult::Intact { entries: 0 };
    }
    VerifyResult::Broken { at: entries.len().saturating_sub(1) as u32 }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
