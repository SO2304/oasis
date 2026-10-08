//! OASIS — orders requiring k signatures out of n (`POSITIONING_GAPS.md` **C9**).
//!
//! The two-person rule, for the class of order where one compromised operator account must
//! not be enough: opening a critical valve, arming a fleet. The quorum machinery already
//! existed for revocation lists — [`oasis_operator_key::OperatorAuthority`] enforces **k
//! distinct** signatures from an authorised set — so this module is the order format and
//! the rule, not new cryptography.
//!
//! # Format `OAQ1` (little-endian)
//!
//! ```text
//! "OAQ1" | actuator_id u16 | cmd_seq u32 | boot_id u64 | deadline_ms u64
//!        | force f32 | torque f32 | velocity f32 | pos [f32;3]
//!        | reserved u8 = 0 | n_sigs u8 | n_sigs × (pubkey[32] ‖ signature[64])
//! ```
//!
//! The carried public keys are **selectors, not authority**: `verify_authorization` checks
//! each one is a member of the authorised set, so carrying them grants nothing. That is
//! what lets a node verify a quorum without holding the whole operator key set — the
//! alternative, a one-byte signer index, needs that set provisioned and is deferred.
//!
//! # It does not fit in one radio frame, and that is the finding
//!
//! | k | payload | on the wire (v0B = 99 B) |
//! |---|---:|---:|
//! | 1 | 148 | **247** — fits, barely |
//! | 2 | 244 | 343 |
//! | 3 | 340 | 439 |
//!
//! With one-byte signer indices instead of inline keys, and an order body stripped of its
//! setpoints, `k = 2` reaches **257 bytes** — it misses the 255-byte PHY limit by **two
//! bytes**. So the two-person rule **costs two frames at any spreading factor**, and no
//! contortion of the format changes that: two Ed25519 signatures are 128 bytes and the v0B
//! envelope is 99.
//!
//! Fragmentation exists and is silicon-proven (`OFR1`, phase 1.1). At SF12 two frames are
//! ≈ 10 s of the 36 s hourly duty-cycle budget (`docs/AUTHORITY_HARDENING_SPEC.md` §J.2),
//! so **about 3 two-signature orders per hour per band**. For "open a critical valve", rare
//! by nature, that is the right trade — but it has to be said rather than discovered.
//!
//! **Nothing here is a certified safety function.** See `docs/compliance/IEC_TS_63074.md`.

use crate::actuation::{actuation_decision_ctx, ActCommand, Decision, GateContext, GateInput, Reason};

pub const OAQ1_MAGIC: [u8; 4] = *b"OAQ1";
/// Everything before `n_sigs`: the order, plus the reserved byte.
pub const OAQ1_BODY_LEN: usize = 51;
pub const QUORUM_SIG_LEN: usize = 96;
/// Signatures a node will parse. Four is a practical ceiling: `k = 3` already needs
/// 439 bytes on the wire, so more is a fragmentation question, not a protocol one.
pub const MAX_QUORUM_SIGS: usize = 4;
/// Domain separation, so a quorum signature can never be replayed as any other signature
/// in the system.
pub const QUORUM_DOMAIN: &[u8] = b"OASIS-QUORUM-v1";
/// `domain (15) | network_id (8) | body (51)`.
pub const QUORUM_MSG_LEN: usize = 15 + 8 + OAQ1_BODY_LEN;

pub fn oaq1_len(n_sigs: usize) -> usize {
    OAQ1_BODY_LEN + 1 + n_sigs * QUORUM_SIG_LEN
}

/// A critical order and the signatures offered for it. No allocation: a fixed array, like
/// the pre-filter's key cache.
#[derive(Clone, Copy)]
pub struct QuorumOrder {
    pub cmd: ActCommand,
    pub n_sigs: u8,
    /// `(pubkey, signature)` pairs. Only the first `n_sigs` are meaningful.
    pub sigs: [([u8; 32], [u8; 64]); MAX_QUORUM_SIGS],
}

impl QuorumOrder {
    pub fn new(cmd: ActCommand) -> Self {
        QuorumOrder { cmd, n_sigs: 0, sigs: [([0u8; 32], [0u8; 64]); MAX_QUORUM_SIGS] }
    }

    pub fn push(&mut self, pk: [u8; 32], sig: [u8; 64]) -> bool {
        let i = self.n_sigs as usize;
        if i >= MAX_QUORUM_SIGS {
            return false;
        }
        self.sigs[i] = (pk, sig);
        self.n_sigs += 1;
        true
    }

    pub fn offered(&self) -> &[([u8; 32], [u8; 64])] {
        &self.sigs[..(self.n_sigs as usize).min(MAX_QUORUM_SIGS)]
    }
}

/// The order body, which is what the operators sign. Deliberately **excludes** the
/// signatures: otherwise the second signer would have to sign over the first signature and
/// the order of signing would matter.
pub fn oaq1_body(c: &ActCommand) -> [u8; OAQ1_BODY_LEN] {
    let mut b = [0u8; OAQ1_BODY_LEN];
    b[0..4].copy_from_slice(&OAQ1_MAGIC);
    b[4..6].copy_from_slice(&c.actuator_id.to_le_bytes());
    b[6..10].copy_from_slice(&c.cmd_seq.to_le_bytes());
    b[10..18].copy_from_slice(&c.boot_id.to_le_bytes());
    b[18..26].copy_from_slice(&c.deadline_ms.to_le_bytes());
    b[26..30].copy_from_slice(&c.force.to_le_bytes());
    b[30..34].copy_from_slice(&c.torque.to_le_bytes());
    b[34..38].copy_from_slice(&c.velocity.to_le_bytes());
    for i in 0..3 {
        b[38 + 4 * i..42 + 4 * i].copy_from_slice(&c.pos[i].to_le_bytes());
    }
    // b[50] reserved, stays zero: one encoding per order.
    b
}

/// What each operator signs: domain, network and the order body. The network binding stops
/// a quorum authorised for one fleet from authorising another.
pub fn quorum_signed_message(network_id: &[u8; 8], c: &ActCommand) -> [u8; QUORUM_MSG_LEN] {
    let mut m = [0u8; QUORUM_MSG_LEN];
    m[0..15].copy_from_slice(QUORUM_DOMAIN);
    m[15..23].copy_from_slice(network_id);
    m[23..].copy_from_slice(&oaq1_body(c));
    m
}

/// `None` if `n_sigs` exceeds [`MAX_QUORUM_SIGS`] or the buffer is too small.
pub fn encode_oaq1(o: &QuorumOrder, out: &mut [u8]) -> Option<usize> {
    let n = o.n_sigs as usize;
    if n > MAX_QUORUM_SIGS {
        return None;
    }
    let total = oaq1_len(n);
    if out.len() < total {
        return None;
    }
    out[..OAQ1_BODY_LEN].copy_from_slice(&oaq1_body(&o.cmd));
    out[OAQ1_BODY_LEN] = o.n_sigs;
    for (i, (pk, sig)) in o.offered().iter().enumerate() {
        let at = OAQ1_BODY_LEN + 1 + i * QUORUM_SIG_LEN;
        out[at..at + 32].copy_from_slice(pk);
        out[at + 32..at + QUORUM_SIG_LEN].copy_from_slice(sig);
    }
    Some(total)
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
fn u64_at(b: &[u8], at: usize) -> u64 {
    let mut x = [0u8; 8];
    x.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(x)
}

/// Total on every input: the length must match `n_sigs` exactly, the reserved byte must be
/// zero, and `n_sigs` must be in `1..=MAX_QUORUM_SIGS`. A quorum order with no signature
/// at all is refused here rather than later, so it can never be mistaken for an ordinary
/// order that happens to lack a quorum.
pub fn parse_oaq1(b: &[u8]) -> Option<QuorumOrder> {
    if b.len() < OAQ1_BODY_LEN + 1 || b[0..4] != OAQ1_MAGIC || b[50] != 0 {
        return None;
    }
    let n = b[OAQ1_BODY_LEN] as usize;
    if n == 0 || n > MAX_QUORUM_SIGS || b.len() != oaq1_len(n) {
        return None;
    }
    let mut o = QuorumOrder::new(ActCommand {
        actuator_id: u16::from_le_bytes([b[4], b[5]]),
        cmd_seq: u32::from_le_bytes([b[6], b[7], b[8], b[9]]),
        boot_id: u64_at(b, 10),
        deadline_ms: u64_at(b, 18),
        force: f32_at(b, 26),
        torque: f32_at(b, 30),
        velocity: f32_at(b, 34),
        pos: [f32_at(b, 38), f32_at(b, 42), f32_at(b, 46)],
    });
    for i in 0..n {
        let at = OAQ1_BODY_LEN + 1 + i * QUORUM_SIG_LEN;
        let mut pk = [0u8; 32];
        pk.copy_from_slice(&b[at..at + 32]);
        let mut sig = [0u8; 64];
        sig.copy_from_slice(&b[at + 32..at + QUORUM_SIG_LEN]);
        o.sigs[i] = (pk, sig);
    }
    o.n_sigs = n as u8;
    Some(o)
}

/// The rule for a critical order: **ten** ordered conditions.
///
/// `quorum_ok` is a `bool` for the same reason `v0b_ok` is: the expensive cryptography is
/// the caller's job, and the rule stays pure and provable. The caller sets it from
/// `OperatorAuthority::verify_authorization(&quorum_signed_message(..), order.offered())`.
///
/// Authenticity and authorisation are checked **before** the quorum, so an unauthenticated
/// or unauthorised sender is told `NotVerified` or `NotAuthorized` and learns nothing about
/// the quorum policy.
pub fn quorum_decision(ctx: &GateContext, i: &GateInput, quorum_ok: bool) -> Decision {
    if !i.v0b_ok {
        return Decision::Reject(Reason::NotVerified);
    }
    if !i.authorized {
        return Decision::Reject(Reason::NotAuthorized);
    }
    if !quorum_ok {
        return Decision::Reject(Reason::QuorumMissing);
    }
    // Everything the ordinary rule requires still applies, in its own order.
    actuation_decision_ctx(ctx, i)
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
