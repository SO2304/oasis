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
    link_key_with_ed_sk(&ed_kp.sk, our_fp, peer_pub, peer_fp)
}

/// As [`link_key`], but from an already-built Ed25519 secret key (the router caches a
/// `KeyPair`, so it avoids rebuilding one from the seed on every hop).
pub(crate) fn link_key_with_ed_sk(ed_sk: &ed25519_compact::SecretKey, our_fp: [u8; FP_LEN], peer_pub: &MeshEdPub, peer_fp: [u8; FP_LEN]) -> Option<[u8; 32]> {
    let x_sk = x25519::SecretKey::from_ed25519(ed_sk).ok()?;
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

// ── v0C envelope (SPORE\x0C): v0B body + forwarder_fp + link tag ──────────────
use super::{MeshDecision, MeshRouter, MESH_ED_SIG_LEN, MESH_V0B_HEADER_LEN, MESH_V0B_MAX_TTL};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// v0C magic. Distinct from v0B so a v0C-only router drops v0B at the magic check,
/// before any Ed25519 work (the anti-downgrade property).
pub const SPORE_V0C_MAGIC: &[u8] = b"SPORE\x0C";
/// v0C header = v0B header (99) + forwarder_fp (8) + tag (16) = 123 bytes, then payload.
pub const MESH_V0C_HEADER_LEN: usize = MESH_V0B_HEADER_LEN + FP_LEN + LINK_TAG_LEN;
const FWD_OFF: usize = MESH_V0B_HEADER_LEN; // 99
const TAG_OFF: usize = MESH_V0B_HEADER_LEN + FP_LEN; // 107

/// Test-only counter of actual Ed25519 verifications reached, so a test can prove that
/// a bad link tag or an exhausted budget stops an envelope BEFORE the expensive check.
#[cfg(test)]
pub static ED_VERIFY_CALLS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

impl MeshRouter {
    fn v0c_ed_sk(&self) -> Option<&ed25519_compact::SecretKey> {
        self.ed_keypair.as_ref().map(|kp| &kp.sk)
    }

    /// Originate a v0C envelope carrying `inner`, tagged for the single downstream
    /// neighbour `next_hop_fp` (point-to-point link). The Ed25519 signature is the v0B
    /// one (payload-bound); the link tag is added on top. `None` if not v0B-capable,
    /// the payload is too large, or `next_hop_fp` is not a known peer.
    pub fn origin_wrap_v0c(&mut self, inner: &[u8], next_hop_fp: [u8; FP_LEN]) -> Option<Vec<u8>> {
        if inner.len() > super::MESH_V0B_MAX_PAYLOAD {
            return None;
        }
        let ed_sk = self.v0c_ed_sk()?;
        let next_pub = *self.ed_registry.get(&next_hop_fp)?;
        let key_down = link_key_with_ed_sk(ed_sk, self.my_fp, &next_pub, next_hop_fp)?;
        self.tx_counter = self.tx_counter.wrapping_add(1);
        let counter = self.tx_counter;
        let (network_id, my_fp, ttl) = (self.network_id, self.my_fp, self.default_ttl);
        let kp = self.ed_keypair.as_ref().unwrap();
        let sig = super::mesh_v0b_sign_with_kp(kp, network_id, my_fp, counter, inner);
        let tag = link_tag(&key_down, my_fp, counter, inner.len() as u16, inner);
        Some(assemble_v0c(network_id, my_fp, counter, ttl, 0, inner, &sig, my_fp, &tag))
    }

    /// Re-seal a received v0C envelope for the next hop: `forwarder_fp = me`, `ttl-1`,
    /// `hops+1`, and the tag recomputed for the link `me → next_hop_fp`. The origin
    /// signature and payload are untouched. Call before forwarding.
    pub fn reseal_v0c(&self, envelope: &[u8], next_hop_fp: [u8; FP_LEN]) -> Option<Vec<u8>> {
        if envelope.len() < MESH_V0C_HEADER_LEN {
            return None;
        }
        let ed_sk = self.v0c_ed_sk()?;
        let next_pub = *self.ed_registry.get(&next_hop_fp)?;
        let key_down = link_key_with_ed_sk(ed_sk, self.my_fp, &next_pub, next_hop_fp)?;
        let mut out = envelope.to_vec();
        let ttl = out[30];
        out[30] = ttl.saturating_sub(1);
        let hops = u16::from_le_bytes([out[31], out[32]]).saturating_add(1);
        out[31..33].copy_from_slice(&hops.to_le_bytes());
        out[FWD_OFF..FWD_OFF + FP_LEN].copy_from_slice(&self.my_fp);
        let origin_fp: [u8; FP_LEN] = out[14..22].try_into().ok()?;
        let counter = u64::from_le_bytes(out[22..30].try_into().ok()?);
        let payload_len = u16::from_le_bytes([out[33], out[34]]);
        let tag = link_tag(&key_down, origin_fp, counter, payload_len, &out[MESH_V0C_HEADER_LEN..]);
        out[TAG_OFF..TAG_OFF + LINK_TAG_LEN].copy_from_slice(&tag);
        Some(out)
    }

    /// Process an incoming v0C envelope. Cheap keyed checks (network, known forwarder,
    /// **link tag**, then the per-link **budget**) run BEFORE the ~185 ms Ed25519
    /// verification, so an outsider without the link key, or a flood beyond the budget,
    /// never reaches it. `budget` is the token bucket for THIS ingress link; `now_ms`
    /// the actuator clock. Anything that is not a well-formed v0C envelope is dropped at
    /// the magic check (anti-downgrade): route all mesh bytes here on a v0C node.
    pub fn process_v0c(&mut self, envelope: &[u8], budget: &mut LinkBudget, now_ms: u64) -> MeshDecision {
        if envelope.len() < MESH_V0C_HEADER_LEN || &envelope[..6] != SPORE_V0C_MAGIC {
            return MeshDecision::Drop("not a v0C envelope (downgrade refused)");
        }
        if self.counter_tracker.is_none() {
            return MeshDecision::Drop("v0C received but router not in v0B/v0C mode");
        }
        let network_id: [u8; super::MESH_V0B_NETWORK_LEN] = match envelope[6..14].try_into() {
            Ok(n) => n,
            Err(_) => return MeshDecision::Drop("short"),
        };
        if network_id != self.network_id {
            return MeshDecision::Drop("foreign network");
        }
        let origin_fp: [u8; FP_LEN] = envelope[14..22].try_into().unwrap();
        if origin_fp == self.my_fp {
            return MeshDecision::Drop("own echo");
        }
        let counter = u64::from_le_bytes(envelope[22..30].try_into().unwrap());
        let ttl = envelope[30];
        let hops = u16::from_le_bytes([envelope[31], envelope[32]]);
        let payload_len = u16::from_le_bytes([envelope[33], envelope[34]]);
        let sig: [u8; MESH_ED_SIG_LEN] = match envelope[35..MESH_V0B_HEADER_LEN].try_into() {
            Ok(s) => s,
            Err(_) => return MeshDecision::Drop("short"),
        };
        let forwarder_fp: [u8; FP_LEN] = envelope[FWD_OFF..FWD_OFF + FP_LEN].try_into().unwrap();
        let tag: [u8; LINK_TAG_LEN] = envelope[TAG_OFF..TAG_OFF + LINK_TAG_LEN].try_into().unwrap();
        let payload = &envelope[MESH_V0C_HEADER_LEN..];

        // ── cheap keyed pre-filter, BEFORE Ed25519 ──
        // 1. forwarder must be a known peer (so we can derive the link key).
        let fwd_pub = match self.ed_registry.get(&forwarder_fp) {
            Some(p) => *p,
            None => return MeshDecision::Drop("unknown forwarder"),
        };
        let ed_sk = match self.v0c_ed_sk() {
            Some(k) => k,
            None => return MeshDecision::Drop("router has no key"),
        };
        let link = match link_key_with_ed_sk(ed_sk, self.my_fp, &fwd_pub, forwarder_fp) {
            Some(k) => k,
            None => return MeshDecision::Drop("link key derivation failed"),
        };
        // 2. link tag (one HMAC-SHA256) — an outsider without the link key stops here.
        if !link_tag_verify(&link, origin_fp, counter, payload_len, payload, &tag) {
            return MeshDecision::Drop("bad link tag");
        }
        // 3. per-link budget — an insider with the key is capped here, before Ed25519.
        if !budget.try_take(now_ms) {
            return MeshDecision::Drop("link budget exceeded");
        }

        // ── the v0B checks (unchanged semantics) ──
        if self.is_revoked(&origin_fp) {
            return MeshDecision::Drop("origin revoked");
        }
        let origin_pub = match self.ed_registry.get(&origin_fp) {
            Some(p) => *p,
            None => return MeshDecision::Drop("unknown sender"),
        };
        if ttl > MESH_V0B_MAX_TTL || hops > MESH_V0B_MAX_TTL as u16 || (ttl as u16).saturating_add(hops) > MESH_V0B_MAX_TTL as u16 {
            return MeshDecision::Drop("ttl/hops out of range");
        }
        if payload.len() != payload_len as usize {
            return MeshDecision::Drop("length mismatch");
        }
        if self.counter_tracker.as_ref().unwrap().is_stale(&origin_fp, counter) {
            return MeshDecision::Drop("stale counter");
        }
        #[cfg(test)]
        ED_VERIFY_CALLS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        if !super::mesh_v0b_verify(&origin_pub, network_id, origin_fp, counter, payload, &sig) {
            return MeshDecision::Drop("bad mesh signature");
        }
        match self.counter_tracker.as_mut().unwrap().check_and_update(origin_fp, counter) {
            Ok(()) => {}
            Err("replay detected (bit set in sliding window)") => return MeshDecision::Drop("replay detected"),
            Err(_) => return MeshDecision::Drop("stale counter"),
        }
        let msg_id = super::origin_msg_id(origin_fp, counter);
        MeshDecision::Arrived { envelope: envelope.to_vec(), msg_id, hops_seen: hops, forward: ttl > 0 }
    }
}

/// Lay out a v0C envelope. `forwarder_fp`/`tag` are the link fields; everything else is
/// the v0B body (byte-identical, so the Ed25519 preimage is unchanged).
#[allow(clippy::too_many_arguments)]
fn assemble_v0c(
    network_id: [u8; super::MESH_V0B_NETWORK_LEN],
    origin_fp: [u8; FP_LEN],
    counter: u64,
    ttl: u8,
    hops: u16,
    payload: &[u8],
    sig: &[u8; MESH_ED_SIG_LEN],
    forwarder_fp: [u8; FP_LEN],
    tag: &[u8; LINK_TAG_LEN],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(MESH_V0C_HEADER_LEN + payload.len());
    out.extend_from_slice(SPORE_V0C_MAGIC);
    out.extend_from_slice(&network_id);
    out.extend_from_slice(&origin_fp);
    out.extend_from_slice(&counter.to_le_bytes());
    out.push(ttl);
    out.extend_from_slice(&hops.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    out.extend_from_slice(sig);
    out.extend_from_slice(&forwarder_fp);
    out.extend_from_slice(tag);
    out.extend_from_slice(payload);
    debug_assert_eq!(out.len(), MESH_V0C_HEADER_LEN + payload.len());
    out
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
