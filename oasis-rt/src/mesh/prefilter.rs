//! Relay pre-filter primitives (spec `docs/specs/RELAY_PREFILTER_SPEC.md`).
//!
//! Two cheap, keyed checks a relay runs BEFORE the ~185 ms Ed25519 verification, so an
//! outsider cannot force that cost by flooding (POSITIONING_GAPS C1):
//!   1. a **per-link** HMAC-SHA256 tag (`link_tag`), keyed by a secret derived from the
//!      two nodes' identities (`link_key`) — never distributed, so one stolen node
//!      exposes only its own links;
//!   2. a **token bucket** per ingress link (`LinkBudget`), the CPU backstop against an
//!      insider who holds a link key.
//!
//! All pure / self-contained: no router state, no I/O, no clock read (the caller passes
//! `now_ms`), so Kani can reach them.

use super::{MeshEdPub, MeshEdSeed, FP_LEN};
use ed25519_compact::{x25519, KeyPair, PublicKey, Seed};
use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Domain separation for everything in this layer (distinct from v0B's and v9's).
pub const LINK_DOMAIN: &[u8] = b"OASIS-LINK-v0C";
/// Truncated HMAC tag length carried in a v0C envelope.
pub const LINK_TAG_LEN: usize = 16;
/// Token-bucket defaults: 2 verifications/s sustained, burst of 24 (covers one
/// fragmented authority message — a revocation is ~14 fragments, an ownership offer
/// ~21, paced 100 ms apart — so a legitimate burst is absorbed, a flood is not).
pub const BUDGET_RATE_PER_S: u64 = 2;
pub const BUDGET_BURST: u64 = 24;
const MILLI: u64 = 1000; // fixed-point: 1000 milli-tokens = 1 token

/// Derive the symmetric link key `K(self ↔ peer)` from the two identities.
///
/// `K = HKDF-SHA256(ikm = X25519(our_x25519_sk, peer_x25519_pk),
///                  salt = LINK_DOMAIN ‖ fp_min ‖ fp_max)`.
/// The X25519 keys are the birational images of the Ed25519 identity keys, so no new
/// key material exists: ours comes from our seed, the peer's from its registry pubkey.
/// Fingerprints are sorted so both ends derive the same key. `None` on any bad key.
pub fn link_key(our_seed: &MeshEdSeed, our_fp: [u8; FP_LEN], peer_pub: &MeshEdPub, peer_fp: [u8; FP_LEN]) -> Option<[u8; 32]> {
    let ed_seed = Seed::from_slice(&our_seed.0).ok()?;
    let ed_kp = KeyPair::from_seed(ed_seed);
    let x_sk = x25519::SecretKey::from_ed25519(&ed_kp.sk).ok()?;
    let ed_pk = PublicKey::from_slice(&peer_pub.0).ok()?;
    let x_pk = x25519::PublicKey::from_ed25519(&ed_pk).ok()?;
    let shared = x_pk.dh(&x_sk).ok()?; // DHOutput derefs to [u8; 32]

    let (lo, hi) = if our_fp <= peer_fp { (our_fp, peer_fp) } else { (peer_fp, our_fp) };
    let mut salt = [0u8; 14 + 2 * FP_LEN];
    salt[..14].copy_from_slice(LINK_DOMAIN);
    salt[14..14 + FP_LEN].copy_from_slice(&lo);
    salt[14 + FP_LEN..].copy_from_slice(&hi);

    let hk = Hkdf::<Sha256>::new(Some(&salt), &shared[..]);
    let mut key = [0u8; 32];
    hk.expand(&[], &mut key).ok()?;
    Some(key)
}

/// HMAC preimage: `LINK_DOMAIN ‖ origin_fp ‖ counter ‖ payload_len ‖ payload`. This is
/// the origin-signed core only — it is INVARIANT across hops, so a relay recomputes the
/// tag for the downstream link with the same preimage and the downstream key, without
/// re-signing. `ttl`/`hops`/`forwarder_fp` are not covered (they mutate per hop).
fn mac(key: &[u8; 32], origin_fp: [u8; FP_LEN], counter: u64, payload_len: u16, payload: &[u8]) -> HmacSha256 {
    let mut m = <HmacSha256 as KeyInit>::new_from_slice(key).expect("HMAC takes any key length");
    m.update(LINK_DOMAIN);
    m.update(&origin_fp);
    m.update(&counter.to_le_bytes());
    m.update(&payload_len.to_le_bytes());
    m.update(payload);
    m
}

/// Compute the 16-byte link tag.
pub fn link_tag(key: &[u8; 32], origin_fp: [u8; FP_LEN], counter: u64, payload_len: u16, payload: &[u8]) -> [u8; LINK_TAG_LEN] {
    let out = mac(key, origin_fp, counter, payload_len, payload).finalize().into_bytes();
    let mut tag = [0u8; LINK_TAG_LEN];
    tag.copy_from_slice(&out[..LINK_TAG_LEN]);
    tag
}

/// Constant-time verification of a received 16-byte tag against the recomputed HMAC
/// (truncated-left compare, so no early-exit timing leak).
pub fn link_tag_verify(key: &[u8; 32], origin_fp: [u8; FP_LEN], counter: u64, payload_len: u16, payload: &[u8], got: &[u8; LINK_TAG_LEN]) -> bool {
    mac(key, origin_fp, counter, payload_len, payload).verify_truncated_left(got).is_ok()
}

/// A per-ingress-link token bucket. Integer "milli-token" fixed point (no float), so it
/// is deterministic and Kani-checkable. Refills `rate_per_s` tokens/s up to `burst`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkBudget {
    milli: u64,
    last_ms: u64,
    rate_per_s: u64,
    burst: u64,
}

impl LinkBudget {
    /// A full bucket (starts able to absorb one burst immediately).
    pub fn new(rate_per_s: u64, burst: u64) -> Self {
        LinkBudget { milli: burst.saturating_mul(MILLI), last_ms: 0, rate_per_s, burst }
    }
    /// The spec defaults (2/s, burst 24).
    pub fn default_budget() -> Self {
        Self::new(BUDGET_RATE_PER_S, BUDGET_BURST)
    }

    fn refill(&mut self, now_ms: u64) {
        // Clock went backwards (reboot, wrap): just re-anchor, add nothing.
        if now_ms <= self.last_ms {
            self.last_ms = now_ms;
            return;
        }
        let elapsed = now_ms - self.last_ms;
        // rate_per_s tokens/s = rate_per_s milli-tokens/ms.
        let added = elapsed.saturating_mul(self.rate_per_s);
        self.milli = (self.milli.saturating_add(added)).min(self.burst.saturating_mul(MILLI));
        self.last_ms = now_ms;
    }

    /// Try to spend one token (one Ed25519 verification). `true` = allowed.
    pub fn try_take(&mut self, now_ms: u64) -> bool {
        self.refill(now_ms);
        if self.milli >= MILLI {
            self.milli -= MILLI;
            true
        } else {
            false
        }
    }

    /// Whole tokens currently available (for logs/tests).
    pub fn tokens(&self) -> u64 {
        self.milli / MILLI
    }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
