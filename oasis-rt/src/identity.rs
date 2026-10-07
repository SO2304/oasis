//! OASIS — node identity generated on the device (Phase 1.2).
//!
//! Spec: `docs/specs/ENROLLMENT_OWNERSHIP_SPEC.md` §2. The node derives its Ed25519
//! seed from raw entropy at first boot and never outputs it. This module holds the
//! pure parts, tested on the PC: SP 800-90B health tests on the raw bits, SHA-256
//! conditioning, the one-time mix-in of a tool nonce before enrollment, the proof of
//! possession, the persisted identity record and entropy statistics.
//!
//! Not an SP 800-90B *validated* entropy source: the RP2040 ring oscillator is
//! documented by its datasheet (§2.17.5) as not meeting the requirements of
//! randomness for security systems. The health tests only catch a stuck or grossly
//! biased source.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

pub const KEYGEN_DOMAIN: &[u8] = b"OASIS-KEYGEN-v1";
pub const REKEY_DOMAIN: &[u8] = b"OASIS-REKEY-v1";
pub const POP_DOMAIN: &[u8] = b"OASIS-POP-v1";
/// Raw samples (bits) per seed: 8x the 512 needed for 256 bits at H = 0.5.
pub const RAW_SAMPLES: usize = 4096;
/// SP 800-90B §4.4.1 repetition count cutoff, H = 0.5 bit/sample, alpha = 2^-20.
pub const RCT_CUTOFF: usize = 41;
/// SP 800-90B §4.4.2 adaptive proportion test, binary, window 1024, H = 0.5.
pub const APT_WINDOW: usize = 1024;
pub const APT_CUTOFF: usize = 793;

pub type Fp = [u8; 8];

#[inline]
fn bit(bits: &[u8], i: usize) -> bool {
    (bits[i / 8] >> (i % 8)) & 1 == 1
}

/// Repetition count test: no run of identical samples reaches `RCT_CUTOFF`.
pub fn rct_ok(bits: &[u8], nbits: usize) -> bool {
    if nbits == 0 || nbits > bits.len() * 8 {
        return false;
    }
    let mut run = 1usize;
    for i in 1..nbits {
        if bit(bits, i) == bit(bits, i - 1) {
            run += 1;
            if run >= RCT_CUTOFF {
                return false;
            }
        } else {
            run = 1;
        }
    }
    true
}

/// Adaptive proportion test: in each full window, the first sample's value occurs
/// fewer than `APT_CUTOFF` times.
pub fn apt_ok(bits: &[u8], nbits: usize) -> bool {
    if nbits < APT_WINDOW || nbits > bits.len() * 8 {
        return false;
    }
    let mut start = 0;
    while start + APT_WINDOW <= nbits {
        let a = bit(bits, start);
        let count = (start..start + APT_WINDOW).filter(|&i| bit(bits, i) == a).count();
        if count >= APT_CUTOFF {
            return false;
        }
        start += APT_WINDOW;
    }
    true
}

pub fn health_ok(bits: &[u8], nbits: usize) -> bool {
    rct_ok(bits, nbits) && apt_ok(bits, nbits)
}

/// Seed from raw samples (SHA-256 conditioning, SP 800-90B §3.1.5.1.1). The timer
/// and the chip id add no secret entropy; they only make two boards with a stuck
/// source diverge (the health tests refuse such a source anyway).
pub fn condition_seed(raw: &[u8], timer: u64, chip_id: &[u8; 8]) -> [u8; 32] {
    Sha256::new()
        .chain_update(KEYGEN_DOMAIN)
        .chain_update(raw)
        .chain_update(timer.to_le_bytes())
        .chain_update(chip_id)
        .finalize()
        .into()
}

/// One-time mix-in of a tool nonce before enrollment: the tool learns nothing of
/// the result if the old seed had entropy.
pub fn rekey(seed: &[u8; 32], tool_nonce: &[u8; 32], fresh_raw: &[u8]) -> [u8; 32] {
    Sha256::new()
        .chain_update(REKEY_DOMAIN)
        .chain_update(seed)
        .chain_update(tool_nonce)
        .chain_update(fresh_raw)
        .finalize()
        .into()
}

fn keypair(seed: &[u8; 32]) -> ed25519_compact::KeyPair {
    ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(*seed))
}

pub fn public_key(seed: &[u8; 32]) -> [u8; 32] {
    let mut p = [0u8; 32];
    p.copy_from_slice(keypair(seed).pk.as_ref());
    p
}

/// `SHA-256(pk)[0..8]`: the existing spore sender fingerprint, derived, never claimed.
pub fn fingerprint(pk: &[u8; 32]) -> Fp {
    crate::spore_crypto::sender_fingerprint(pk)
}

/// `"OASIS-POP-v1" | network_id | challenge`.
pub fn pop_message(network_id: &[u8; 8], challenge: &[u8; 32]) -> Vec<u8> {
    let mut m = Vec::with_capacity(POP_DOMAIN.len() + 8 + 32);
    m.extend_from_slice(POP_DOMAIN);
    m.extend_from_slice(network_id);
    m.extend_from_slice(challenge);
    m
}

/// Proof of possession: the node signs the tool's challenge with its identity key.
pub fn pop_sign(seed: &[u8; 32], network_id: &[u8; 8], challenge: &[u8; 32]) -> [u8; 64] {
    let mut s = [0u8; 64];
    s.copy_from_slice(keypair(seed).sk.sign(pop_message(network_id, challenge), None).as_ref());
    s
}

pub fn pop_verify(pk: &[u8; 32], network_id: &[u8; 8], challenge: &[u8; 32], sig: &[u8; 64]) -> bool {
    crate::authority::ed25519_verify(pk, &pop_message(network_id, challenge), sig)
}

// ── persisted identity record ────────────────────────────────────────────────
const ID_MAGIC: [u8; 4] = *b"OID1";
/// `OID1 | seed[32] | mixed u8 | check4`.
pub const ID_RECORD_LEN: usize = 4 + 32 + 1 + 4;

fn id_check(body: &[u8]) -> [u8; 4] {
    let d = Sha256::new().chain_update(ID_MAGIC).chain_update(body).finalize();
    [d[0], d[1], d[2], d[3]]
}

pub fn encode_identity(seed: &[u8; 32], mixed: bool) -> [u8; ID_RECORD_LEN] {
    let mut r = [0u8; ID_RECORD_LEN];
    r[0..4].copy_from_slice(&ID_MAGIC);
    r[4..36].copy_from_slice(seed);
    r[36] = mixed as u8;
    let c = id_check(&r[4..37]);
    r[37..41].copy_from_slice(&c);
    r
}

/// `ed25519-compact` panics on an all-zero seed: such a seed is never used.
pub fn seed_usable(seed: &[u8; 32]) -> bool {
    seed.iter().any(|&b| b != 0)
}

/// `(seed, mixed)`, or `None` if absent, torn, corrupted or unusable (an all-zero
/// seed with a valid checksum would otherwise panic at every boot).
pub fn decode_identity(r: &[u8]) -> Option<([u8; 32], bool)> {
    if r.len() < ID_RECORD_LEN || r[0..4] != ID_MAGIC || r[36] > 1 || r[37..41] != id_check(&r[4..37]) {
        return None;
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&r[4..36]);
    if !seed_usable(&seed) {
        return None;
    }
    Some((seed, r[36] == 1))
}

// ── entropy statistics (published by the node; never the seed) ──────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitStats {
    pub n: u32,
    pub ones: u32,
    pub longest_run: u32,
}

pub fn bit_stats(bits: &[u8], nbits: usize) -> BitStats {
    let n = nbits.min(bits.len() * 8);
    let (mut ones, mut run, mut longest) = (0u32, 0u32, 0u32);
    for i in 0..n {
        ones += bit(bits, i) as u32;
        run = if i > 0 && bit(bits, i) == bit(bits, i - 1) { run + 1 } else { 1 };
        longest = longest.max(run);
    }
    BitStats { n: n as u32, ones, longest_run: longest }
}

/// SP 800-90B §6.3.1 most-common-value estimate, in milli-bits per sample:
/// `p_u = min(1, p + 2.576 sqrt(p(1-p)/(n-1)))`, `H = -log2(p_u)`.
pub fn mcv_min_entropy_milli(s: &BitStats) -> u32 {
    if s.n < 2 {
        return 0;
    }
    let n = s.n as f64;
    let p = (s.ones.max(s.n - s.ones)) as f64 / n;
    let pu = (p + 2.576 * libm::sqrt(p * (1.0 - p) / (n - 1.0))).min(1.0);
    (-libm::log2(pu) * 1000.0) as u32
}

#[cfg(test)]
mod tests;
