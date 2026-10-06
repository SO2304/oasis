//! OASIS — Multi-hop Mesh Routing (SPORE\x08 envelope)
//!
//! Problem: two drones out of direct radio range cannot communicate. Fix:
//! flood-with-dedup + TTL + echo suppression. Intermediate nodes re-broadcast
//! an incoming mesh envelope exactly once (first sighting) until TTL reaches 0.
//!
//! Why flooding and not AODV/DSR:
//!   - Digest traffic is tiny (≤1 KB). Bandwidth isn't the bottleneck.
//!   - Topology changes rapidly (drones move). Route tables stale instantly.
//!   - Simplicity = fewer bugs in safety-critical code.
//!
//! Wire format SPORE\x08:
//! ```text
//!   [0..6]    "SPORE\x08"    magic
//!   [6..14]   msg_id         u64 LE — unique per message, dedup key
//!   [14..22]  origin_fp      8 bytes — original sender fingerprint (echo guard)
//!   [22]      ttl            u8     — decremented on forward; 0 = terminal
//!   [23..25]  hops_so_far    u16 LE — 0 at origin, +1 each forward
//!   [25..]    inner_payload         — the wrapped lower-layer envelope
//! ```
//!
//! Overhead: 25 bytes per hop. Inner can be any SPORE\x03..\x07 or plaintext.
//!
//! Invariants (tested):
//!   1. TERMINATION: TTL strictly decreases each forward; capped at MAX_TTL.
//!   2. DEDUP: msg_id seen before ⇒ dropped silently.
//!   3. ECHO GUARD: origin_fp == my_fp ⇒ dropped (never re-broadcast own msg).
//!   4. PATH METRIC: hops_so_far monotonic increasing along the forwarding chain.
//!   5. INNER INTEGRITY: inner payload is byte-identical through all hops.

#[cfg(not(feature = "std"))]
use crate::fmath::F64Ext;
#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    collections::{BTreeSet as HashSet, VecDeque},
    string::String,
    vec,
    vec::Vec,
};
use sha2::{Digest, Sha256};
#[cfg(feature = "std")]
use std::collections::{HashSet, VecDeque};

pub const SPORE_V8_MAGIC: &[u8] = b"SPORE\x08";
pub const MESH_HEADER_LEN: usize = 25;
pub const DEFAULT_TTL: u8 = 8;
pub const DEFAULT_DEDUP_CAP: usize = 4096;

/// Fingerprint length — matches `spore_crypto::SENDER_FP_LEN`.
pub const FP_LEN: usize = 8;

// ─── SPORE\x09 signed mesh envelope ───────────────────────────────────────
// v8 is unauthenticated: any attacker can spoof `origin_fp` and pollute
// dedup caches across the mesh (dedup-DoS). v9 adds an 8-byte HMAC-SHA256
// tag over (magic || msg_id || origin_fp). TTL and hops are intentionally
// NOT covered — they change on every forward and must stay mutable.
//
// 64-bit tag = 2^32 birthday bound against random forgery. For a drone
// swarm at 100 pkt/s, that's ~1.3 y of continuous attacker effort. Keys
// should be rotated before that.
//
// Opt-in: `MeshRouter::new(fp)` still produces v8 for backward compat.
// `MeshRouter::new_signed(fp, key)` emits and enforces v9.

pub const SPORE_V9_MAGIC: &[u8] = b"SPORE\x09";
pub const MESH_TAG_LEN: usize = 8;
pub const MESH_V9_HEADER_LEN: usize = MESH_HEADER_LEN + MESH_TAG_LEN;

/// 32-byte pre-shared MAC key. Caller derives this from their swarm PSK
/// or via HKDF from the v5+ session key material. OASIS does not persist
/// it — responsibility of the binary that wires the router.
#[derive(Clone)]
pub struct MeshMacKey(pub [u8; 32]);

/// HMAC-SHA256 truncated to the first 8 bytes.
/// RFC 2104 construction: H(K XOR opad || H(K XOR ipad || msg)).
pub fn hmac_sha256_8(key: &[u8; 32], msg: &[u8]) -> [u8; MESH_TAG_LEN] {
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..32 {
        ipad[i] ^= key[i];
        opad[i] ^= key[i];
    }
    let mut h1 = Sha256::new();
    h1.update(ipad);
    h1.update(msg);
    let inner = h1.finalize();
    let mut h2 = Sha256::new();
    h2.update(opad);
    h2.update(inner);
    let out = h2.finalize();
    let mut r = [0u8; MESH_TAG_LEN];
    r.copy_from_slice(&out[..MESH_TAG_LEN]);
    r
}

/// Compute the v9 MAC tag for the immutable header fields.
pub fn mesh_v9_tag(key: &MeshMacKey, msg_id: u64, origin_fp: [u8; FP_LEN]) -> [u8; MESH_TAG_LEN] {
    let mut buf = [0u8; 6 + 8 + FP_LEN];
    buf[..6].copy_from_slice(SPORE_V9_MAGIC);
    buf[6..14].copy_from_slice(&msg_id.to_le_bytes());
    buf[14..14 + FP_LEN].copy_from_slice(&origin_fp);
    hmac_sha256_8(&key.0, &buf)
}

/// Constant-time tag verification.
pub fn mesh_v9_verify(key: &MeshMacKey, msg_id: u64, origin_fp: [u8; FP_LEN], got: &[u8; MESH_TAG_LEN]) -> bool {
    let expected = mesh_v9_tag(key, msg_id, origin_fp);
    let mut diff: u8 = 0;
    for i in 0..MESH_TAG_LEN {
        diff |= expected[i] ^ got[i];
    }
    diff == 0
}

// ─── SPORE\x0A per-node Ed25519-signed mesh envelope ──────────────────────
// v9 (HMAC-SHA256-8) closes EXTERNAL attackers via a shared MAC key, but
// every swarm node holds the same key — a compromised insider can forge
// any sender's envelopes. v0A adds per-node Ed25519 signatures, binding
// `origin_fp` to a specific private key that only the legitimate sender
// holds.
//
// Wire format (v0A):
//   [0..6]    "SPORE\x0A"
//   [6..14]   msg_id u64 LE
//   [14..22]  origin_fp 8 bytes
//   [22]      ttl
//   [23..25]  hops_so_far u16 LE — mutable, NOT covered by signature
//   [25..89]  Ed25519 signature(64) over (magic||msg_id||origin_fp)
//   [89..]    inner payload
//
// Signature scope identical to v9 MAC: covers immutable identity bytes
// only. TTL/hops mutate per hop and are excluded.
//
// Cost (predicted, will be measured):
//   v9  HMAC-SHA256-8 sign:   ~300 ns / verify ~300 ns
//   v0A Ed25519 sign:         ~30-100 µs / verify ~80-200 µs
// → v0A is ~100-1000× more expensive. Use for low-frequency authority
// broadcasts (revocation, swarm-wide commands), not 1 kHz control loops.

pub const SPORE_V10_MAGIC: &[u8] = b"SPORE\x0A";
pub const MESH_ED_SIG_LEN: usize = 64;
pub const MESH_V10_HEADER_LEN: usize = MESH_HEADER_LEN + MESH_ED_SIG_LEN;

// ─── v0B (SPORE\x0B): payload-bound, fresh, domain-separated, revocation-aware ───
// See docs/MESH_V0B_SPEC.md. v0B closes the four v0A defects: the signature now
// binds the payload (via SHA-256), the counter (verifiable freshness over a
// persisted sliding window), and the network_id (domain separation); relays
// enforce revocation. v0B is ADDITIVE — v8/v9/v0A behaviour is unchanged.
#[cfg(feature = "mesh_v10")]
pub const SPORE_V0B_MAGIC: &[u8] = b"SPORE\x0B";
/// Network identifier length (domain separation).
#[cfg(feature = "mesh_v10")]
pub const MESH_V0B_NETWORK_LEN: usize = 8;
/// v0B header: 6 magic + 8 network_id + 8 origin_fp + 8 counter + 1 ttl
/// + 2 hops + 2 payload_len + 64 signature = 99 bytes, then payload.
#[cfg(feature = "mesh_v10")]
pub const MESH_V0B_HEADER_LEN: usize = 99;
/// Max payload: bounded by the u16 `payload_len` field.
#[cfg(feature = "mesh_v10")]
pub const MESH_V0B_MAX_PAYLOAD: usize = u16::MAX as usize;
/// Hop cap enforced on receipt (unsigned ttl/hops are bounded here).
#[cfg(feature = "mesh_v10")]
pub const MESH_V0B_MAX_TTL: u8 = DEFAULT_TTL;
/// 14-byte ASCII domain-separation tag prefixed into the signed data.
#[cfg(feature = "mesh_v10")]
pub const MESH_V0B_DOMAIN: &[u8] = b"OASIS-MESH-v0B";
/// Length of the Ed25519-signed preimage:
/// 14 domain + 8 network_id + 8 origin_fp + 8 counter + 2 payload_len + 32 digest.
#[cfg(feature = "mesh_v10")]
pub const MESH_V0B_PREIMAGE_LEN: usize = 14 + 8 + 8 + 8 + 2 + 32;
pub const ED25519_PUB_LEN: usize = 32;
pub const ED25519_SEED_LEN: usize = 32;

// All v0A code below gated on `mesh_v10` feature. The `ed25519-compact/ed25519`
// subfeature (activated via our `mesh_v10` feature) pulls in the `signature`
// crate which isn't cleanly no_std at v2.2.x. MCU builds that don't need
// v0A signing opt out via `--no-default-features` and keep v8/v9 only.

/// 32-byte Ed25519 seed used to derive the signing keypair. Caller stores it.
#[cfg(feature = "mesh_v10")]
#[derive(Clone)]
pub struct MeshEdSeed(pub [u8; ED25519_SEED_LEN]);

/// 32-byte Ed25519 public key. One per swarm node, indexed by `origin_fp`.
#[cfg(feature = "mesh_v10")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MeshEdPub(pub [u8; ED25519_PUB_LEN]);

/// Caller-managed registry mapping `origin_fp` → Ed25519 public key.
/// At verify time we look up the claimed origin's pubkey here. An
/// `origin_fp` not in the registry is rejected ("unknown sender").
#[cfg(all(feature = "std", feature = "mesh_v10"))]
pub type MeshPubRegistry = std::collections::BTreeMap<[u8; FP_LEN], MeshEdPub>;
#[cfg(all(not(feature = "std"), feature = "mesh_v10"))]
pub type MeshPubRegistry = alloc::collections::BTreeMap<[u8; FP_LEN], MeshEdPub>;

/// Compute the Ed25519 signature for v0A's immutable header fields.
/// Signs `magic || msg_id || origin_fp` (22 bytes).
#[cfg(feature = "mesh_v10")]
pub fn mesh_v10_sign(seed: &MeshEdSeed, msg_id: u64, origin_fp: [u8; FP_LEN]) -> Result<[u8; MESH_ED_SIG_LEN], &'static str> {
    let s = ed25519_compact::Seed::from_slice(&seed.0).map_err(|_| "bad ed25519 seed")?;
    let kp = ed25519_compact::KeyPair::from_seed(s);
    Ok(mesh_v10_sign_with_kp(&kp, msg_id, origin_fp))
}

/// Hot-path signing: use a pre-derived `KeyPair` to skip the
/// seed→pubkey derivation.
#[cfg(feature = "mesh_v10")]
#[inline]
pub fn mesh_v10_sign_with_kp(kp: &ed25519_compact::KeyPair, msg_id: u64, origin_fp: [u8; FP_LEN]) -> [u8; MESH_ED_SIG_LEN] {
    let mut buf = [0u8; 6 + 8 + FP_LEN];
    buf[..6].copy_from_slice(SPORE_V10_MAGIC);
    buf[6..14].copy_from_slice(&msg_id.to_le_bytes());
    buf[14..14 + FP_LEN].copy_from_slice(&origin_fp);
    let sig = kp.sk.sign(&buf, None);
    let mut out = [0u8; MESH_ED_SIG_LEN];
    out.copy_from_slice(sig.as_ref());
    out
}

/// Verify the Ed25519 signature of a v0A envelope.
#[cfg(feature = "mesh_v10")]
pub fn mesh_v10_verify(pubkey: &MeshEdPub, msg_id: u64, origin_fp: [u8; FP_LEN], got: &[u8; MESH_ED_SIG_LEN]) -> bool {
    let mut buf = [0u8; 6 + 8 + FP_LEN];
    buf[..6].copy_from_slice(SPORE_V10_MAGIC);
    buf[6..14].copy_from_slice(&msg_id.to_le_bytes());
    buf[14..14 + FP_LEN].copy_from_slice(&origin_fp);
    let pk = match ed25519_compact::PublicKey::from_slice(&pubkey.0) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let sig = match ed25519_compact::Signature::from_slice(got) {
        Ok(s) => s,
        Err(_) => return false,
    };
    pk.verify(&buf, &sig).is_ok()
}

/// Derive the Ed25519 public key from the seed.
#[cfg(feature = "mesh_v10")]
pub fn mesh_v10_pubkey_from_seed(seed: &MeshEdSeed) -> Result<MeshEdPub, &'static str> {
    let s = ed25519_compact::Seed::from_slice(&seed.0).map_err(|_| "bad ed25519 seed")?;
    let kp = ed25519_compact::KeyPair::from_seed(s);
    let mut out = [0u8; ED25519_PUB_LEN];
    out.copy_from_slice(kp.pk.as_ref());
    Ok(MeshEdPub(out))
}

// ─── v0B crypto: payload-bound, domain-separated Ed25519 over a SHA-256 digest ───

/// SHA-256 digest of the payload (bound into the v0B signed data).
#[cfg(feature = "mesh_v10")]
pub fn mesh_v0b_payload_digest(payload: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(payload);
    let out = h.finalize();
    let mut d = [0u8; 32];
    d.copy_from_slice(&out);
    d
}

/// Build the v0B Ed25519 signed preimage (pure — Kani-friendly):
/// `"OASIS-MESH-v0B" || network_id || origin_fp || counter || payload_len || SHA-256(payload)`.
/// Content, freshness (counter) and network are all bound here.
#[cfg(feature = "mesh_v10")]
pub fn mesh_v0b_signed_preimage(network_id: [u8; MESH_V0B_NETWORK_LEN], origin_fp: [u8; FP_LEN], counter: u64, payload_len: u16, payload_digest: &[u8; 32]) -> [u8; MESH_V0B_PREIMAGE_LEN] {
    let mut buf = [0u8; MESH_V0B_PREIMAGE_LEN];
    buf[0..14].copy_from_slice(MESH_V0B_DOMAIN);
    buf[14..22].copy_from_slice(&network_id);
    buf[22..30].copy_from_slice(&origin_fp);
    buf[30..38].copy_from_slice(&counter.to_le_bytes());
    buf[38..40].copy_from_slice(&payload_len.to_le_bytes());
    buf[40..72].copy_from_slice(payload_digest);
    buf
}

/// Sign a v0B envelope with a pre-derived keypair (hot path).
#[cfg(feature = "mesh_v10")]
pub fn mesh_v0b_sign_with_kp(kp: &ed25519_compact::KeyPair, network_id: [u8; MESH_V0B_NETWORK_LEN], origin_fp: [u8; FP_LEN], counter: u64, payload: &[u8]) -> [u8; MESH_ED_SIG_LEN] {
    let digest = mesh_v0b_payload_digest(payload);
    let pre = mesh_v0b_signed_preimage(network_id, origin_fp, counter, payload.len() as u16, &digest);
    let sig = kp.sk.sign(&pre, None);
    let mut out = [0u8; MESH_ED_SIG_LEN];
    out.copy_from_slice(sig.as_ref());
    out
}

/// Verify a v0B signature. Recomputes the payload digest, so any change to the
/// payload, network_id, origin_fp, counter or payload_len fails verification.
#[cfg(feature = "mesh_v10")]
pub fn mesh_v0b_verify(pubkey: &MeshEdPub, network_id: [u8; MESH_V0B_NETWORK_LEN], origin_fp: [u8; FP_LEN], counter: u64, payload: &[u8], got: &[u8; MESH_ED_SIG_LEN]) -> bool {
    let digest = mesh_v0b_payload_digest(payload);
    let pre = mesh_v0b_signed_preimage(network_id, origin_fp, counter, payload.len() as u16, &digest);
    let pk = match ed25519_compact::PublicKey::from_slice(&pubkey.0) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let sig = match ed25519_compact::Signature::from_slice(got) {
        Ok(s) => s,
        Err(_) => return false,
    };
    pk.verify(&pre, &sig).is_ok()
}

/// Parsed v0B header fields.
#[cfg(feature = "mesh_v10")]
pub struct V0bHeader {
    pub network_id: [u8; MESH_V0B_NETWORK_LEN],
    pub origin_fp: [u8; FP_LEN],
    pub counter: u64,
    pub ttl: u8,
    pub hops: u16,
    pub payload_len: u16,
    pub sig: [u8; MESH_ED_SIG_LEN],
}

/// Pure, panic-free v0B header parser (Kani-proven total). Returns `None` for a
/// buffer shorter than the fixed header or with the wrong magic; otherwise the
/// parsed fields. All slice bounds are inside `[0, MESH_V0B_HEADER_LEN)` after
/// the length check, so no indexing can panic for any input.
#[cfg(feature = "mesh_v10")]
pub fn v0b_try_parse_header(envelope: &[u8]) -> Option<V0bHeader> {
    if envelope.len() < MESH_V0B_HEADER_LEN {
        return None;
    }
    if &envelope[0..6] != SPORE_V0B_MAGIC {
        return None;
    }
    Some(V0bHeader {
        network_id: envelope[6..14].try_into().ok()?,
        origin_fp: envelope[14..22].try_into().ok()?,
        counter: u64::from_le_bytes(envelope[22..30].try_into().ok()?),
        ttl: envelope[30],
        hops: u16::from_le_bytes(envelope[31..33].try_into().ok()?),
        payload_len: u16::from_le_bytes(envelope[33..35].try_into().ok()?),
        sig: envelope[35..MESH_V0B_HEADER_LEN].try_into().ok()?,
    })
}

// ─── Bloom-filter second-level dedup ──────────────────────────────────────
// The exact `HashSet<u64>` is the primary dedup cache but evicts oldest
// entries at `dedup_cap`. Past that point, an attacker (or simply normal
// reorder / replay) can get an evicted msg_id re-accepted as fresh — a
// replay-amplification window. The Bloom filter below covers the long
// memory: it never evicts.
//
// False-positive rate, verified against `FPR(n) = (1 - exp(-k·n/m))^k`
// with m = BLOOM_BITS and k = BLOOM_HASHES = 5. Current sizing:
// m = 524 288 bits (8 192 × u64, 64 KiB per router):
//
//   n     FPR
//   20 k  0.03 %
//   40 k  0.44 %
//   52 k  1.0 %
//   80 k  4.3 %
//  100 k  10.0 %
//  160 k  50.0 %
//
// An earlier version used m = 131 072 and an audit comment claimed
// "≈ 0.3 % at 20 k inserts". That was wrong by ~14×. The bench
// `bench_mesh_signed` measured the true curve (~61 % after 20 k + 100 k
// probes), which matched theory for the smaller filter. This commit
// quadruples the Bloom (16 KiB → 64 KiB per router) so the 1 % FPR
// threshold moves from ~13 k to ~52 k inserts.
//
// Operator guidance (updated): call `bloom_reset()` every ~40 k inserts
// to stay under 1 % FPR. At 100 pkt/s that's ~400 s — much more
// reasonable. The RAM cost is unavoidable without a wire-format change
// (timestamp dedup); feature-flag to opt out on MCU targets is still
// pending.

/// Number of u64 words backing the Bloom filter. Feature-gated:
///   default         → 8 192 words = 64 KiB per router (1 % FPR at ~52 k).
///   mesh_bloom_mcu  →   256 words =  2 KiB per router (1 % FPR at ~1.6 k).
///
/// NOTE: the `mesh_bloom_mcu` feature shrinks RAM on HOSTS that compile.
/// As of 2026-04-22, oasis-rt does NOT cross-compile to
/// `thumbv7em-none-eabi` because `serde` and `getrandom` pull `std`.
/// See webots/OASIS_strict_audit_mcu_reality_check.md. The feature is
/// useful for minimizing sim RAM, not for actual MCU deployment — yet.
#[cfg(not(feature = "mesh_bloom_mcu"))]
pub const BLOOM_WORDS: usize = 8192;
#[cfg(feature = "mesh_bloom_mcu")]
pub const BLOOM_WORDS: usize = 256;
/// Total bit capacity of the default Bloom filter.
pub const BLOOM_BITS: usize = BLOOM_WORDS * 64;
/// Number of independent hash positions per insert / contains.
pub const BLOOM_HASHES: usize = 5;

/// Conservative estimate of the Bloom 1%-FPR insert capacity for the
/// current feature configuration. Computed from the standard formula:
///   capacity ≈ -m × ln(1 - p^(1/k)) / k
/// where m = BLOOM_BITS, p = 0.01, k = BLOOM_HASHES.
///
/// Returned as a const for callers' planning (no floating-point at
/// runtime). For default 64 KiB Bloom (524 288 bits, k=5) ≈ 52 000.
/// For mesh_bloom_mcu 2 KiB Bloom (16 384 bits, k=5) ≈ 1 600.
pub const fn bloom_capacity_estimate_1pct_fpr() -> u32 {
    // Pre-computed at config time; the formula is well-known and the
    // constants are stable. See SHADOW_AUDIT_AB for the derivation.
    #[cfg(not(feature = "mesh_bloom_mcu"))]
    {
        52_000
    }
    #[cfg(feature = "mesh_bloom_mcu")]
    {
        1_600
    }
}

/// Health snapshot of the mesh router's Bloom dedup layer (AE round,
/// 2026-05-12). Exposes the 4 telemetry fields needed by operator
/// dashboards to detect FPR drift BEFORE it impacts throughput.
///
/// Honest naming: this is a SNAPSHOT, not a stream. Operators poll
/// at their preferred cadence (the AC 7-day + AD 30-day soaks show
/// per-hour polling is sufficient — drift develops over many hours,
/// never within a single tick). For higher-frequency monitoring,
/// poll faster; the call is O(1) and lock-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BloomHealthSnapshot {
    /// Total inserts since router construction.
    pub bloom_inserts_total: u64,
    /// Inserts in the current cycle (since last reset).
    pub bloom_inserts_since_reset: u64,
    /// Number of resets (manual + auto) fired so far.
    pub bloom_reset_count: u64,
    /// Currently configured auto-reset threshold; `None` = legacy.
    pub auto_reset_threshold: Option<u64>,
    /// Pre-computed capacity at 1% FPR for this build's Bloom size.
    /// Constant for the binary lifetime; included in the snapshot so
    /// downstream serializers don't need to know the build config.
    pub bloom_capacity_estimate_1pct_fpr: u32,
}

impl BloomHealthSnapshot {
    /// Fraction of the 1%-FPR capacity consumed in the current cycle.
    /// Returns `bloom_inserts_since_reset / bloom_capacity_estimate_1pct_fpr`
    /// scaled to milli-units (0-1000) for fixed-point arithmetic.
    /// Values > 1000 mean we have CROSSED the documented 1% FPR line
    /// and false-positive drops are imminent. Operators should configure
    /// auto-reset thresholds so this stays < 800 (i.e., 80% of capacity).
    pub fn capacity_consumed_milli(&self) -> u32 {
        if self.bloom_capacity_estimate_1pct_fpr == 0 {
            return 0;
        }
        let consumed = self.bloom_inserts_since_reset.saturating_mul(1000);
        let cap = self.bloom_capacity_estimate_1pct_fpr as u64;
        let result = consumed / cap;
        if result > u32::MAX as u64 {
            u32::MAX
        } else {
            result as u32
        }
    }

    /// Returns true if the current cycle has crossed 80% of 1%-FPR
    /// capacity. A common dashboard alerting threshold — gives the
    /// operator enough lead time to issue a manual `bloom_reset()`
    /// before throughput is impacted.
    pub fn capacity_alert(&self) -> bool {
        self.capacity_consumed_milli() >= 800
    }

    /// Serialize snapshot to a fixed 36-byte binary record (AF round,
    /// 2026-05-12). Wire format v1 (little-endian):
    ///
    /// ```text
    ///   offset  size  field
    ///   ------  ----  ----------------------------------------
    ///        0   1   magic byte (b'B' = 0x42) — identifies BloomHealth-v1
    ///        1   1   version (1)
    ///        2   2   reserved (zero, future-proofs alignment)
    ///        4   8   bloom_inserts_total (u64 le)
    ///       12   8   bloom_inserts_since_reset (u64 le)
    ///       20   8   bloom_reset_count (u64 le)
    ///       28   8   auto_reset_threshold (0 = None, else value as u64 le)
    /// ```
    ///
    /// `bloom_capacity_estimate_1pct_fpr` is NOT serialized — it's a
    /// build-time constant; receivers compute it from their own
    /// build config or from a separate "build hash" topic. Including
    /// it here would risk cross-build mismatches going unnoticed.
    ///
    /// The 36-byte size is deliberate: small enough to fit in a v8
    /// mesh envelope's inner payload (max ~480 bytes after header)
    /// with room for envelope overhead, and a power-of-2-friendly
    /// alignment for binary parsers.
    pub fn serialize_topic_v1(&self) -> [u8; 36] {
        let mut out = [0u8; 36];
        out[0] = b'B';
        out[1] = 1;
        // bytes 2-3 reserved (0)
        out[4..12].copy_from_slice(&self.bloom_inserts_total.to_le_bytes());
        out[12..20].copy_from_slice(&self.bloom_inserts_since_reset.to_le_bytes());
        out[20..28].copy_from_slice(&self.bloom_reset_count.to_le_bytes());
        let threshold_wire = self.auto_reset_threshold.unwrap_or(0);
        out[28..36].copy_from_slice(&threshold_wire.to_le_bytes());
        out
    }

    /// Parse a 36-byte BloomHealth-v1 record. Returns `None` on bad
    /// magic / version / reserved bytes. The receiver must compute
    /// `bloom_capacity_estimate_1pct_fpr` from its own build config;
    /// for dashboards that's typically static per fleet generation.
    pub fn deserialize_topic_v1(bytes: &[u8; 36], local_capacity_estimate: u32) -> Option<Self> {
        if bytes[0] != b'B' {
            return None;
        }
        if bytes[1] != 1 {
            return None;
        }
        if bytes[2] != 0 || bytes[3] != 0 {
            return None;
        }
        let bloom_inserts_total = u64::from_le_bytes(bytes[4..12].try_into().ok()?);
        let bloom_inserts_since_reset = u64::from_le_bytes(bytes[12..20].try_into().ok()?);
        let bloom_reset_count = u64::from_le_bytes(bytes[20..28].try_into().ok()?);
        let threshold_wire = u64::from_le_bytes(bytes[28..36].try_into().ok()?);
        let auto_reset_threshold = if threshold_wire == 0 { None } else { Some(threshold_wire) };
        Some(Self {
            bloom_inserts_total,
            bloom_inserts_since_reset,
            bloom_reset_count,
            auto_reset_threshold,
            bloom_capacity_estimate_1pct_fpr: local_capacity_estimate,
        })
    }
}

/// SplitMix64-derived bit position for a given (msg_id, hash index) pair,
/// modulo `total_bits`. Pure — Kani-verifiable on small slices.
///
/// Fast path: if `total_bits` is a power of two, uses mask (bitwise AND).
/// This is the expected production case (`BLOOM_BITS = 131_072 = 2^17`).
/// The masked path is also what makes this function SAT-tractable for Kani.
#[inline]
pub fn bloom_bit_index(x: u64, k: u64, total_bits: u64) -> u64 {
    let mut z = x.wrapping_add(k.wrapping_mul(0x9E3779B97F4A7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^= z >> 31;
    if total_bits == 0 {
        0
    } else if total_bits.is_power_of_two() {
        z & (total_bits - 1)
    } else {
        z % total_bits
    }
}

/// Bloom `contains`: all `num_hashes` positions derived from `x` must be set.
/// Returns `false` for the trivial empty-slice case.
#[inline]
pub fn bloom_contains(bits: &[u64], x: u64, num_hashes: usize) -> bool {
    let total = (bits.len() as u64) * 64;
    if total == 0 {
        return false;
    }
    for k in 0..num_hashes as u64 {
        let bit = bloom_bit_index(x, k, total) as usize;
        let w = bit >> 6;
        let m = 1u64 << (bit & 63);
        if (bits[w] & m) == 0 {
            return false;
        }
    }
    true
}

/// Bloom `insert`: set all `num_hashes` positions for `x`.
#[inline]
pub fn bloom_insert(bits: &mut [u64], x: u64, num_hashes: usize) {
    let total = (bits.len() as u64) * 64;
    if total == 0 {
        return;
    }
    for k in 0..num_hashes as u64 {
        let bit = bloom_bit_index(x, k, total) as usize;
        let w = bit >> 6;
        let m = 1u64 << (bit & 63);
        bits[w] |= m;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshDecision {
    /// Duplicate / malformed / own echo / unknown magic → drop silently.
    Drop(&'static str),
    /// The envelope arrived and is ready to be used. `forward = true` means
    /// the caller MUST re-broadcast `envelope` via their transport before
    /// (or while) processing the inner locally. `forward = false` means TTL
    /// reached zero — process locally, do NOT forward.
    ///
    /// The inner payload can be extracted via `mesh::inner_slice(&envelope)`
    /// — a zero-copy view. This replaces the old dual-Vec `ProcessAndForward`
    /// variant which allocated twice (once for inner copy, once for wrapped
    /// forward). Single owning Vec keeps memory footprint minimal.
    Arrived { envelope: Vec<u8>, msg_id: u64, hops_seen: u16, forward: bool },
}

/// Zero-copy inner-payload slice from an envelope buffer.
/// Handles v8 (25-byte header), v9 (33-byte header + MAC), and v0A
/// (89-byte header + Ed25519 sig) magics.
/// Returns `&[]` for unknown magic or truncated buffer.
#[inline]
pub fn inner_slice(envelope: &[u8]) -> &[u8] {
    if envelope.len() < 6 {
        return &[];
    }
    let m = &envelope[..6];
    #[cfg(feature = "mesh_v10")]
    let header_len = if m == SPORE_V0B_MAGIC {
        MESH_V0B_HEADER_LEN
    } else if m == SPORE_V10_MAGIC {
        MESH_V10_HEADER_LEN
    } else if m == SPORE_V9_MAGIC {
        MESH_V9_HEADER_LEN
    } else if m == SPORE_V8_MAGIC {
        MESH_HEADER_LEN
    } else {
        return &[];
    };
    #[cfg(not(feature = "mesh_v10"))]
    let header_len = if m == SPORE_V9_MAGIC {
        MESH_V9_HEADER_LEN
    } else if m == SPORE_V8_MAGIC {
        MESH_HEADER_LEN
    } else {
        return &[];
    };
    if envelope.len() <= header_len {
        &[]
    } else {
        &envelope[header_len..]
    }
}

pub struct MeshRouter {
    my_fp: [u8; FP_LEN],
    default_ttl: u8,
    seen: VecDeque<u64>,
    seen_set: HashSet<u64>,
    dedup_cap: usize,
    /// Second-level dedup, 64 KiB Bloom (524 288 bits × 5 hashes).
    /// Never evicts. FPR ≤ 1 % up to ~52 k inserts, 10 % at ~100 k.
    /// Operators should call `bloom_reset()` roughly every 40 k inserts
    /// to stay under 1 % legitimate-drop rate. (Prior sizing used 16 KiB
    /// / 131 072 bits and hit 1 % at just ~13 k inserts — not enough.)
    long_memory: Box<[u64]>,
    bloom_inserts: u64,
    /// Optional auto-reset threshold (AC round, 2026-05-11). When `Some(n)`
    /// and `bloom_inserts_since_reset >= n`, the next call to `remember`
    /// triggers `bloom_reset()` automatically. Lets long-uptime deployments
    /// (24h+) self-manage Bloom cleanup without operator intervention. The
    /// `bloom_reset_count` field tracks how many auto-resets have fired.
    bloom_auto_reset_threshold: Option<u64>,
    bloom_inserts_since_reset: u64,
    bloom_reset_count: u64,
    /// Monotonic counter mixed into msg_id generation at origin. Ensures that
    /// a node restarting with the same fingerprint won't collide with its own
    /// past msg_ids — at the price of requiring persistence across reboots
    /// for the strongest guarantee. Not persisted here; caller's responsibility.
    tx_counter: u64,
    /// Optional MAC key. When `Some`, this router:
    ///  (a) emits SPORE\x09 signed envelopes from `origin_wrap`;
    ///  (b) REJECTS incoming SPORE\x08 unsigned envelopes as untrusted;
    ///  (c) verifies the MAC on incoming SPORE\x09 envelopes before dedup.
    /// When `None` (default), behaves exactly as the pre-v9 router.
    signing_key: Option<MeshMacKey>,
    /// Cached Ed25519 `KeyPair` derived from the seed at router construction.
    #[cfg(feature = "mesh_v10")]
    ed_keypair: Option<ed25519_compact::KeyPair>,
    /// Pubkey registry for v0A verify. Maps origin_fp → Ed25519 pubkey.
    #[cfg(feature = "mesh_v10")]
    ed_registry: MeshPubRegistry,
    /// v0B network identifier (domain separation). Default `[0;8]`; set by
    /// `new_v0b`. Only consulted on the v0B path.
    #[cfg(feature = "mesh_v10")]
    network_id: [u8; MESH_V0B_NETWORK_LEN],
    /// v0B freshness: `Some` iff this router is in v0B mode. IPsec-style
    /// persisted sliding window (per origin). Authoritative anti-replay —
    /// independent of the RAM Bloom, so it survives reboot/Bloom-reset.
    #[cfg(feature = "mesh_v10")]
    counter_tracker: Option<crate::spore_crypto::CounterTracker>,
    /// v0B relay-enforced revocation set. A revoked origin is dropped at the
    /// first hop, before the signature check.
    #[cfg(feature = "mesh_v10")]
    revoked: HashSet<[u8; FP_LEN]>,
    /// v0B strict mode: when `false` (the default for a v0B router), every
    /// non-v0B envelope (v8/v9/v0A) is dropped. Accepting them would let an
    /// attacker downgrade to v0A, whose payload is unsigned and whose replay
    /// protection is RAM-only. Set to `true` only during a migration.
    #[cfg(feature = "mesh_v10")]
    allow_legacy: bool,
}

impl MeshRouter {
    pub fn new(my_fp: [u8; FP_LEN]) -> Self {
        // BTreeSet (no_std) lacks `with_capacity`; HashSet has it. Use cfg.
        #[cfg(feature = "std")]
        let seen_set = HashSet::with_capacity(DEFAULT_DEDUP_CAP);
        #[cfg(not(feature = "std"))]
        let seen_set = HashSet::new();
        Self {
            my_fp,
            default_ttl: DEFAULT_TTL,
            seen: VecDeque::with_capacity(DEFAULT_DEDUP_CAP),
            seen_set,
            dedup_cap: DEFAULT_DEDUP_CAP,
            long_memory: vec![0u64; BLOOM_WORDS].into_boxed_slice(),
            bloom_inserts: 0,
            bloom_auto_reset_threshold: None,
            bloom_inserts_since_reset: 0,
            bloom_reset_count: 0,
            tx_counter: 0,
            signing_key: None,
            #[cfg(feature = "mesh_v10")]
            ed_keypair: None,
            #[cfg(feature = "mesh_v10")]
            ed_registry: MeshPubRegistry::new(),
            #[cfg(feature = "mesh_v10")]
            network_id: [0u8; MESH_V0B_NETWORK_LEN],
            #[cfg(feature = "mesh_v10")]
            counter_tracker: None,
            #[cfg(feature = "mesh_v10")]
            revoked: HashSet::new(),
            #[cfg(feature = "mesh_v10")]
            allow_legacy: false,
        }
    }

    pub fn with_config(my_fp: [u8; FP_LEN], ttl: u8, dedup_cap: usize) -> Self {
        let cap = dedup_cap.max(16);
        #[cfg(feature = "std")]
        let seen_set = HashSet::with_capacity(cap);
        #[cfg(not(feature = "std"))]
        let seen_set = HashSet::new();
        Self {
            my_fp,
            default_ttl: ttl,
            seen: VecDeque::with_capacity(cap),
            seen_set,
            dedup_cap: cap,
            long_memory: vec![0u64; BLOOM_WORDS].into_boxed_slice(),
            bloom_inserts: 0,
            bloom_auto_reset_threshold: None,
            bloom_inserts_since_reset: 0,
            bloom_reset_count: 0,
            tx_counter: 0,
            signing_key: None,
            #[cfg(feature = "mesh_v10")]
            ed_keypair: None,
            #[cfg(feature = "mesh_v10")]
            ed_registry: MeshPubRegistry::new(),
            #[cfg(feature = "mesh_v10")]
            network_id: [0u8; MESH_V0B_NETWORK_LEN],
            #[cfg(feature = "mesh_v10")]
            counter_tracker: None,
            #[cfg(feature = "mesh_v10")]
            revoked: HashSet::new(),
            #[cfg(feature = "mesh_v10")]
            allow_legacy: false,
        }
    }

    /// Signed-mode constructor. Emits v9 envelopes, rejects v8 as untrusted,
    /// verifies incoming v9 MAC before dedup.
    pub fn new_signed(my_fp: [u8; FP_LEN], mac_key: MeshMacKey) -> Self {
        let mut r = Self::new(my_fp);
        r.signing_key = Some(mac_key);
        r
    }

    pub fn is_signed(&self) -> bool {
        self.signing_key.is_some()
    }

    /// v0A Ed25519-signed mode constructor. Emits SPORE\x0A envelopes,
    /// rejects v8/v9 as untrusted, verifies the per-envelope Ed25519
    /// signature against the caller-supplied pubkey registry before dedup.
    ///
    /// `seed` is this node's Ed25519 secret seed.
    /// `registry` maps every fp this router will accept envelopes from
    /// to that node's Ed25519 public key. An origin_fp not in the
    /// registry causes `Drop("unknown sender")`.
    ///
    /// Cost (Linux laptop, measured 2026-04-22):
    ///   sign:   ~30-100 µs per envelope (will be measured)
    ///   verify: ~80-200 µs per envelope
    /// Use only for low-frequency authority broadcasts. v9 HMAC stays
    /// the right choice for high-frequency intra-swarm traffic.
    #[cfg(feature = "mesh_v10")]
    pub fn new_ed25519_signed(my_fp: [u8; FP_LEN], seed: MeshEdSeed, registry: MeshPubRegistry) -> Self {
        let s = ed25519_compact::Seed::from_slice(&seed.0).expect("bad ed25519 seed in new_ed25519_signed — 32 bytes required");
        let kp = ed25519_compact::KeyPair::from_seed(s);
        let mut r = Self::new(my_fp);
        r.ed_keypair = Some(kp);
        r.ed_registry = registry;
        r
    }

    /// v0B constructor (SPORE\x0B). Like `new_ed25519_signed`, plus a
    /// `network_id` for domain separation and a persisted per-origin counter
    /// window for verifiable freshness. Strict by default: v8/v9/v0A envelopes
    /// are dropped (no downgrade). `set_allow_legacy(true)` re-enables them for
    /// a migration; v0B traffic always goes through the stricter `process_v0b`.
    #[cfg(feature = "mesh_v10")]
    pub fn new_v0b(my_fp: [u8; FP_LEN], network_id: [u8; MESH_V0B_NETWORK_LEN], seed: MeshEdSeed, registry: MeshPubRegistry) -> Self {
        let s = ed25519_compact::Seed::from_slice(&seed.0).expect("bad ed25519 seed in new_v0b — 32 bytes required");
        let kp = ed25519_compact::KeyPair::from_seed(s);
        let mut r = Self::new(my_fp);
        r.ed_keypair = Some(kp);
        r.ed_registry = registry;
        r.network_id = network_id;
        r.counter_tracker = Some(crate::spore_crypto::CounterTracker::new());
        r
    }

    /// Accept (or refuse, the default) legacy v8/v9/v0A envelopes on a v0B
    /// router. Opting in re-opens the v0A downgrade (unsigned payload,
    /// RAM-only replay protection); use it only while migrating a fleet.
    #[cfg(feature = "mesh_v10")]
    pub fn set_allow_legacy(&mut self, allow: bool) {
        self.allow_legacy = allow;
    }

    /// True if this router is in v0B mode.
    #[cfg(feature = "mesh_v10")]
    pub fn is_v0b(&self) -> bool {
        self.counter_tracker.is_some()
    }

    /// This router's v0B network id.
    #[cfg(feature = "mesh_v10")]
    pub fn v0b_network_id(&self) -> [u8; MESH_V0B_NETWORK_LEN] {
        self.network_id
    }

    /// Add an origin fingerprint to the relay revocation set. A revoked
    /// origin is dropped at the first hop, before signature verification.
    #[cfg(feature = "mesh_v10")]
    pub fn revoke(&mut self, fp: [u8; FP_LEN]) {
        self.revoked.insert(fp);
    }

    /// True if `fp` is revoked on this relay.
    #[cfg(feature = "mesh_v10")]
    pub fn is_revoked(&self, fp: &[u8; FP_LEN]) -> bool {
        self.revoked.contains(fp)
    }

    /// Highest v0B counter this relay has accepted from `origin_fp` (0 if
    /// none / not in v0B mode).
    #[cfg(feature = "mesh_v10")]
    pub fn v0b_last_seen(&self, origin_fp: &[u8; FP_LEN]) -> u64 {
        self.counter_tracker.as_ref().map(|t| t.last_seen(origin_fp)).unwrap_or(0)
    }

    /// Serialize the v0B freshness window for durable storage (CTR\x03). On
    /// boot, restore it with `restore_counter_tracker` BEFORE processing any
    /// v0B envelope — this is what defeats post-reboot replay. Returns `None`
    /// if not in v0B mode.
    #[cfg(feature = "mesh_v10")]
    pub fn counter_tracker_bytes(&self) -> Option<Vec<u8>> {
        self.counter_tracker.as_ref().map(|t| t.to_bytes())
    }

    /// Restore the v0B freshness window from `counter_tracker_bytes`. Also puts
    /// the router into v0B mode if it was not already.
    #[cfg(feature = "mesh_v10")]
    pub fn restore_counter_tracker(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        let t = crate::spore_crypto::CounterTracker::from_bytes(bytes)?;
        self.counter_tracker = Some(t);
        Ok(())
    }

    #[cfg(feature = "mesh_v10")]
    pub fn is_ed25519_signed(&self) -> bool {
        self.ed_keypair.is_some()
    }

    #[cfg(feature = "mesh_v10")]
    pub fn ed_registry_insert(&mut self, fp: [u8; FP_LEN], pubkey: MeshEdPub) {
        self.ed_registry.insert(fp, pubkey);
    }
    #[cfg(feature = "mesh_v10")]
    pub fn ed_registry_remove(&mut self, fp: &[u8; FP_LEN]) -> bool {
        self.ed_registry.remove(fp).is_some()
    }
    #[cfg(feature = "mesh_v10")]
    pub fn ed_registry_len(&self) -> usize {
        self.ed_registry.len()
    }

    /// Current tx counter value. Callers should snapshot this periodically
    /// (e.g., to disk) and restore it with `set_tx_counter()` on the next
    /// boot. Without persistence, a reboot resets the counter to 0 and
    /// risks msg_id collisions with in-flight messages from the prior run.
    pub fn tx_counter(&self) -> u64 {
        self.tx_counter
    }

    /// Restore the tx counter from persisted state. Use on boot, BEFORE
    /// the first `origin_wrap`. The next emitted envelope will use
    /// `value + 1` as its counter. Accepts `u64::MAX` gracefully
    /// (wrapping_add on next call — document your persistence strategy).
    pub fn set_tx_counter(&mut self, value: u64) {
        self.tx_counter = value;
    }

    /// Reset the Bloom layer. Use on reboot or when false-positive rate
    /// becomes unacceptable (empirically past ~50 k inserts for the default
    /// 64 KiB filter). Does NOT clear the exact `seen_set` cache, so
    /// short-term replay protection (last DEFAULT_DEDUP_CAP = 4096 msg_ids)
    /// is preserved across resets. Increments `bloom_reset_count`.
    ///
    /// AE-round refinement (2026-05-12): `bloom_inserts` is the LIFETIME
    /// total across router uptime and is NOT cleared by reset — making
    /// it a true monotonic counter (AB3 Kani proof now holds). Per-cycle
    /// state lives in `bloom_inserts_since_reset` which IS cleared here.
    pub fn bloom_reset(&mut self) {
        for w in self.long_memory.iter_mut() {
            *w = 0;
        }
        // bloom_inserts is now the lifetime total — monotonic, no reset.
        self.bloom_inserts_since_reset = 0;
        self.bloom_reset_count = self.bloom_reset_count.saturating_add(1);
    }

    /// Current number of Bloom inserts. Consumers can use this to decide
    /// when to call `bloom_reset()`.
    pub fn bloom_inserts(&self) -> u64 {
        self.bloom_inserts
    }

    /// Configure auto-reset of the Bloom long-memory layer (AC round,
    /// 2026-05-11). When `Some(n)`, the next `remember()` call after
    /// `bloom_inserts_since_reset >= n` triggers `bloom_reset()`
    /// automatically. When `None` (default), no auto-reset; operators
    /// must call `bloom_reset()` manually.
    ///
    /// Recommended threshold for default 64 KiB Bloom: 40 000 (keeps
    /// FPR ≤ 1 % at all times). For 2 KiB MCU Bloom: 1 200.
    ///
    /// SECURITY NOTE: auto-reset weakens long-memory replay protection
    /// past the seen_set window (last DEFAULT_DEDUP_CAP = 4096 msg_ids).
    /// For most deployments where msg_ids embed a monotonic counter
    /// (per `origin_msg_id`), this is acceptable: an attacker who
    /// replays a pre-reset envelope still loses to the seen_set cache
    /// for at least 4096 future msg_ids; only after both the seen_set
    /// AND the Bloom have evicted that msg_id can the replay succeed.
    /// At 1 envelope/sec/origin, that's 4096+threshold seconds of
    /// guaranteed replay protection.
    pub fn set_bloom_auto_reset_threshold(&mut self, threshold: Option<u64>) {
        self.bloom_auto_reset_threshold = threshold;
    }

    /// Current configured auto-reset threshold. `None` means no auto-reset.
    pub fn bloom_auto_reset_threshold(&self) -> Option<u64> {
        self.bloom_auto_reset_threshold
    }

    /// Number of times the Bloom has been reset (manual + auto). Telemetry
    /// for operators monitoring long-uptime nodes.
    pub fn bloom_reset_count(&self) -> u64 {
        self.bloom_reset_count
    }

    /// Bloom inserts since the last reset (manual or auto). Differs from
    /// `bloom_inserts()` only after at least one reset has fired. With
    /// auto-reset configured, this stays bounded by the threshold +1.
    pub fn bloom_inserts_since_reset(&self) -> u64 {
        self.bloom_inserts_since_reset
    }

    /// Single-call snapshot of all 4 Bloom-layer telemetry fields (AE
    /// round, 2026-05-12). Convenient for operator dashboards that
    /// serialize health state periodically — one method call instead
    /// of 4, atomic across the call (no torn reads under future
    /// thread-local routers).
    ///
    /// Combined with `bloom_capacity_estimate_1pct_fpr()`, operators
    /// can compute a "headroom" metric: how close the router is to
    /// the FPR-1% capacity in its current insertion cycle. The
    /// long-soak audits showed that exceeding capacity is the only
    /// failure mode for Bloom dedup; this struct exposes the inputs
    /// to predict it.
    pub fn bloom_health_snapshot(&self) -> BloomHealthSnapshot {
        BloomHealthSnapshot {
            bloom_inserts_total: self.bloom_inserts,
            bloom_inserts_since_reset: self.bloom_inserts_since_reset,
            bloom_reset_count: self.bloom_reset_count,
            auto_reset_threshold: self.bloom_auto_reset_threshold,
            bloom_capacity_estimate_1pct_fpr: bloom_capacity_estimate_1pct_fpr(),
        }
    }

    /// Pre-seed the dedup cache with our next-to-emit msg_id so we don't
    /// echo our own broadcast back through a neighbour.
    fn remember(&mut self, msg_id: u64) {
        if self.seen_set.contains(&msg_id) {
            return;
        }
        if self.seen.len() >= self.dedup_cap {
            if let Some(old) = self.seen.pop_front() {
                self.seen_set.remove(&old);
            }
        }
        self.seen.push_back(msg_id);
        self.seen_set.insert(msg_id);
        // Auto-reset check (AC round): if configured threshold reached,
        // wipe the Bloom BEFORE inserting the current msg_id. This
        // guarantees `bloom_inserts_since_reset` stays bounded by the
        // threshold +1 — the +1 being the current insert that fits into
        // the freshly cleared filter.
        if let Some(threshold) = self.bloom_auto_reset_threshold {
            if self.bloom_inserts_since_reset >= threshold {
                self.bloom_reset();
            }
        }
        // Persistent second-level memory. Never evicts; bounded FPR
        // unless auto-reset configured.
        bloom_insert(&mut self.long_memory, msg_id, BLOOM_HASHES);
        self.bloom_inserts = self.bloom_inserts.saturating_add(1);
        self.bloom_inserts_since_reset = self.bloom_inserts_since_reset.saturating_add(1);
    }

    fn has_seen(&self, msg_id: u64) -> bool {
        // Fast path: exact recent cache.
        if self.seen_set.contains(&msg_id) {
            return true;
        }
        // Long path: Bloom. Small FPR; caller treats positive as "probably seen".
        bloom_contains(&self.long_memory, msg_id, BLOOM_HASHES)
    }

    /// Wrap an `inner` payload as the ORIGIN node. Returns the mesh envelope
    /// ready to broadcast. TTL defaults to the router's configured value.
    pub fn origin_wrap(&mut self, inner: &[u8]) -> Vec<u8> {
        self.origin_wrap_with_ttl(inner, self.default_ttl)
    }

    pub fn origin_wrap_with_ttl(&mut self, inner: &[u8], ttl: u8) -> Vec<u8> {
        self.tx_counter = self.tx_counter.wrapping_add(1);
        let msg_id = origin_msg_id(self.my_fp, self.tx_counter);
        self.remember(msg_id); // don't re-process our own broadcast on echo
                               // Dispatch on configured signing mode. v0A takes priority over v9.
        #[cfg(feature = "mesh_v10")]
        if let Some(kp) = &self.ed_keypair {
            let sig = mesh_v10_sign_with_kp(kp, msg_id, self.my_fp);
            return build_v10_envelope(msg_id, self.my_fp, ttl, 0, inner, &sig);
        }
        match &self.signing_key {
            Some(key) => build_v9_envelope(msg_id, self.my_fp, ttl, 0, inner, key),
            None => build_envelope(msg_id, self.my_fp, ttl, 0, inner),
        }
    }

    /// Zero-intermediate-Vec mesh envelope builder. Caller supplies a closure
    /// that writes the inner payload directly into the same buffer that holds
    /// the mesh header. Saves one full payload memcpy + one allocation
    /// compared to `origin_wrap(&inner_vec)`.
    ///
    /// `inner_len` MUST equal the number of bytes the closure will write.
    /// (Used to pre-size the Vec.) Underspecifying triggers a reallocation;
    /// overspecifying wastes a few bytes of capacity. Both are correct, just
    /// not optimal.
    ///
    /// Only the unsigned (v8) path is supported here. v9 signing requires the
    /// inner bytes to compute the MAC, which forces a separate buffer; if you
    /// need v9 + payload-zero-copy you must restructure further (out of scope
    /// for this round). Returns `None` to indicate that — caller falls back to
    /// `origin_wrap(&inner)` for signed routers.
    ///
    /// Example:
    /// ```ignore
    /// let inner_len = topics::TOPIC_HEADER_LEN + payload.len();
    /// let env = router.origin_wrap_with(inner_len, |buf| {
    ///     topics::write_topic_envelope_into(buf, hash, &payload);
    /// }).expect("unsigned router");
    /// ```
    pub fn origin_wrap_with<F>(&mut self, inner_len: usize, write_inner: F) -> Option<Vec<u8>>
    where
        F: FnOnce(&mut Vec<u8>),
    {
        if self.signing_key.is_some() {
            return None; // signed path requires separate buffer for MAC input
        }
        self.tx_counter = self.tx_counter.wrapping_add(1);
        let msg_id = origin_msg_id(self.my_fp, self.tx_counter);
        self.remember(msg_id);
        let total = MESH_HEADER_LEN + inner_len;
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(SPORE_V8_MAGIC);
        out.extend_from_slice(&msg_id.to_le_bytes());
        out.extend_from_slice(&self.my_fp);
        out.push(self.default_ttl);
        out.extend_from_slice(&0u16.to_le_bytes()); // hops_so_far
        write_inner(&mut out);
        Some(out)
    }

    /// Originate a v0B envelope (SPORE\x0B) at the default TTL. Returns `None`
    /// if the router is not v0B-capable or the payload exceeds
    /// `MESH_V0B_MAX_PAYLOAD`.
    #[cfg(feature = "mesh_v10")]
    pub fn origin_wrap_v0b(&mut self, inner: &[u8]) -> Option<Vec<u8>> {
        self.origin_wrap_v0b_with_ttl(inner, self.default_ttl)
    }

    /// Originate a v0B envelope at an explicit TTL. The signature binds the
    /// payload (via SHA-256), the monotonic counter and the network_id.
    #[cfg(feature = "mesh_v10")]
    pub fn origin_wrap_v0b_with_ttl(&mut self, inner: &[u8], ttl: u8) -> Option<Vec<u8>> {
        if self.ed_keypair.is_none() {
            return None;
        }
        if inner.len() > MESH_V0B_MAX_PAYLOAD {
            return None;
        }
        self.tx_counter = self.tx_counter.wrapping_add(1);
        let counter = self.tx_counter;
        let network_id = self.network_id;
        let my_fp = self.my_fp;
        let sig = {
            let kp = self.ed_keypair.as_ref().unwrap();
            mesh_v0b_sign_with_kp(kp, network_id, my_fp, counter, inner)
        };
        let mut out = Vec::with_capacity(MESH_V0B_HEADER_LEN + inner.len());
        out.extend_from_slice(SPORE_V0B_MAGIC);
        out.extend_from_slice(&network_id);
        out.extend_from_slice(&my_fp);
        out.extend_from_slice(&counter.to_le_bytes());
        out.push(ttl.min(MESH_V0B_MAX_TTL));
        out.extend_from_slice(&0u16.to_le_bytes()); // hops
        out.extend_from_slice(&(inner.len() as u16).to_le_bytes());
        out.extend_from_slice(&sig);
        out.extend_from_slice(inner);
        Some(out)
    }

    /// Process an incoming mesh envelope. Caller supplies the full packet
    /// as received on UDP (or other transport). Returns what to do next.
    ///
    /// Fast path (optimized): for ProcessAndForward, we clone the envelope ONCE
    /// and mutate TTL + hops bytes in place. Previously this case allocated
    /// 2× (inner copy + freshly-built forward envelope).
    pub fn process(&mut self, envelope: &[u8]) -> MeshDecision {
        #[cfg(feature = "mesh_v10")]
        if self.counter_tracker.is_some() && envelope.len() >= 6 && &envelope[..6] == SPORE_V0B_MAGIC {
            return self.process_v0b(envelope);
        }
        #[cfg(feature = "mesh_v10")]
        if self.counter_tracker.is_some() && !self.allow_legacy {
            return MeshDecision::Drop("legacy envelope rejected by strict v0B router");
        }
        let parsed = match parse_and_verify(
            envelope,
            self.signing_key.as_ref(),
            #[cfg(feature = "mesh_v10")]
            if self.ed_keypair.is_some() { Some(&self.ed_registry) } else { None },
        ) {
            Ok(p) => p,
            Err(e) => return MeshDecision::Drop(e),
        };
        if parsed.origin_fp == self.my_fp {
            return MeshDecision::Drop("own echo");
        }
        if self.has_seen(parsed.msg_id) {
            return MeshDecision::Drop("duplicate");
        }
        self.remember(parsed.msg_id);

        if parsed.ttl == 0 {
            return MeshDecision::Arrived { envelope: envelope.to_vec(), msg_id: parsed.msg_id, hops_seen: parsed.hops_so_far, forward: false };
        }
        let mut out = envelope.to_vec();
        out[22] = parsed.ttl - 1;
        let new_hops = parsed.hops_so_far.saturating_add(1);
        out[23..25].copy_from_slice(&new_hops.to_le_bytes());
        // MAC covers only magic||msg_id||origin_fp — TTL/hops mutation preserves it.
        MeshDecision::Arrived { envelope: out, msg_id: parsed.msg_id, hops_seen: parsed.hops_so_far, forward: true }
    }

    /// Zero-copy variant: caller owns envelope bytes (e.g., the UDP recv buffer
    /// into a Vec). Avoids the one memcpy in `process()` by mutating in place.
    /// For ProcessLocalOnly case, the envelope is returned unchanged.
    pub fn process_owned(&mut self, mut envelope: Vec<u8>) -> MeshDecision {
        #[cfg(feature = "mesh_v10")]
        if self.counter_tracker.is_some() && envelope.len() >= 6 && &envelope[..6] == SPORE_V0B_MAGIC {
            return self.process_v0b(&envelope);
        }
        #[cfg(feature = "mesh_v10")]
        if self.counter_tracker.is_some() && !self.allow_legacy {
            return MeshDecision::Drop("legacy envelope rejected by strict v0B router");
        }
        let parsed = match parse_and_verify(
            &envelope,
            self.signing_key.as_ref(),
            #[cfg(feature = "mesh_v10")]
            if self.ed_keypair.is_some() { Some(&self.ed_registry) } else { None },
        ) {
            Ok(p) => p,
            Err(e) => return MeshDecision::Drop(e),
        };
        if parsed.origin_fp == self.my_fp {
            return MeshDecision::Drop("own echo");
        }
        if self.has_seen(parsed.msg_id) {
            return MeshDecision::Drop("duplicate");
        }
        self.remember(parsed.msg_id);

        if parsed.ttl == 0 {
            return MeshDecision::Arrived { envelope, msg_id: parsed.msg_id, hops_seen: parsed.hops_so_far, forward: false };
        }
        envelope[22] = parsed.ttl - 1;
        let new_hops = parsed.hops_so_far.saturating_add(1);
        envelope[23..25].copy_from_slice(&new_hops.to_le_bytes());
        MeshDecision::Arrived { envelope, msg_id: parsed.msg_id, hops_seen: parsed.hops_so_far, forward: true }
    }

    /// v0B receive pipeline. Verification order is cheapest → most expensive,
    /// and router state (the counter window) mutates ONLY after every check —
    /// including the Ed25519 signature — has passed (verify-before-remember).
    /// A rejected message never advances the freshness window.
    #[cfg(feature = "mesh_v10")]
    fn process_v0b(&mut self, envelope: &[u8]) -> MeshDecision {
        // 1. length + magic (pure, panic-free parse).
        let hdr = match v0b_try_parse_header(envelope) {
            Some(h) => h,
            None => {
                if envelope.len() < MESH_V0B_HEADER_LEN {
                    return MeshDecision::Drop("mesh envelope too short");
                }
                return MeshDecision::Drop("bad mesh magic");
            }
        };
        if self.counter_tracker.is_none() {
            return MeshDecision::Drop("v0B received but router not in v0B mode");
        }
        // 2. network (domain separation) — reject foreign networks cheaply.
        if hdr.network_id != self.network_id {
            return MeshDecision::Drop("foreign network");
        }
        // own re-broadcast bounced back
        if hdr.origin_fp == self.my_fp {
            return MeshDecision::Drop("own echo");
        }
        // 3. origin must be known AND not revoked — both before signature cost.
        if self.is_revoked(&hdr.origin_fp) {
            return MeshDecision::Drop("origin revoked");
        }
        let pubkey = match self.ed_registry.get(&hdr.origin_fp) {
            Some(p) => *p,
            None => return MeshDecision::Drop("unknown sender"),
        };
        // 4. ttl/hops/length sanity (unsigned fields bounded here).
        if hdr.ttl > MESH_V0B_MAX_TTL || hdr.hops > MESH_V0B_MAX_TTL as u16 || (hdr.ttl as u16).saturating_add(hdr.hops) > MESH_V0B_MAX_TTL as u16 {
            return MeshDecision::Drop("ttl/hops out of range");
        }
        let payload = &envelope[MESH_V0B_HEADER_LEN..];
        if payload.len() != hdr.payload_len as usize {
            return MeshDecision::Drop("length mismatch");
        }
        // 5. freshness pre-check (read-only; never mutates) — cheap replay reject.
        if self.counter_tracker.as_ref().unwrap().is_stale(&hdr.origin_fp, hdr.counter) {
            return MeshDecision::Drop("stale counter");
        }
        // 6. Ed25519 over domain||network||origin||counter||len||sha256(payload).
        if !mesh_v0b_verify(&pubkey, hdr.network_id, hdr.origin_fp, hdr.counter, payload, &hdr.sig) {
            return MeshDecision::Drop("bad mesh signature");
        }
        // 7. authoritative freshness update — the FIRST and ONLY state mutation.
        match self.counter_tracker.as_mut().unwrap().check_and_update(hdr.origin_fp, hdr.counter) {
            Ok(()) => {}
            Err("replay detected (bit set in sliding window)") => return MeshDecision::Drop("replay detected"),
            Err(_) => return MeshDecision::Drop("stale counter"),
        }

        let msg_id = origin_msg_id(hdr.origin_fp, hdr.counter);
        // 8. forward / terminal decision (ttl/hops are the only mutated bytes).
        if hdr.ttl == 0 {
            return MeshDecision::Arrived { envelope: envelope.to_vec(), msg_id, hops_seen: hdr.hops, forward: false };
        }
        let mut out = envelope.to_vec();
        out[30] = hdr.ttl - 1;
        let new_hops = hdr.hops.saturating_add(1);
        out[31..33].copy_from_slice(&new_hops.to_le_bytes());
        MeshDecision::Arrived { envelope: out, msg_id, hops_seen: hdr.hops, forward: true }
    }

    pub fn dedup_cache_size(&self) -> usize {
        self.seen.len()
    }
}

/// Compute a deterministic-but-unique msg_id from (fp, counter).
/// Uses SplitMix-style hashing to avoid low-entropy collisions for low counters.
/// Pure function — Kani-verifiable.
#[inline]
pub fn origin_msg_id(fp: [u8; FP_LEN], counter: u64) -> u64 {
    let fp_u64 = u64::from_le_bytes(fp);
    let mut z = fp_u64.wrapping_mul(0x9E3779B97F4A7C15) ^ counter;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Pure TTL-decrement predicate: after one forward, new_ttl = ttl - 1.
/// Proves bounded-hops invariant: after ≤ initial_ttl forwards, TTL reaches 0.
#[inline]
pub fn ttl_after_forward(ttl: u8) -> u8 {
    if ttl == 0 {
        0
    } else {
        ttl - 1
    }
}

/// Pure predicate: given ttl, should this hop forward the envelope?
#[inline]
pub fn should_forward(ttl: u8) -> bool {
    ttl > 0
}

fn build_envelope(msg_id: u64, origin_fp: [u8; FP_LEN], ttl: u8, hops_so_far: u16, inner: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(MESH_HEADER_LEN + inner.len());
    out.extend_from_slice(SPORE_V8_MAGIC);
    out.extend_from_slice(&msg_id.to_le_bytes());
    out.extend_from_slice(&origin_fp);
    out.push(ttl);
    out.extend_from_slice(&hops_so_far.to_le_bytes());
    out.extend_from_slice(inner);
    out
}

fn build_v9_envelope(msg_id: u64, origin_fp: [u8; FP_LEN], ttl: u8, hops_so_far: u16, inner: &[u8], key: &MeshMacKey) -> Vec<u8> {
    let tag = mesh_v9_tag(key, msg_id, origin_fp);
    let mut out = Vec::with_capacity(MESH_V9_HEADER_LEN + inner.len());
    out.extend_from_slice(SPORE_V9_MAGIC);
    out.extend_from_slice(&msg_id.to_le_bytes());
    out.extend_from_slice(&origin_fp);
    out.push(ttl);
    out.extend_from_slice(&hops_so_far.to_le_bytes());
    out.extend_from_slice(&tag);
    out.extend_from_slice(inner);
    out
}

#[cfg(feature = "mesh_v10")]
fn build_v10_envelope(msg_id: u64, origin_fp: [u8; FP_LEN], ttl: u8, hops_so_far: u16, inner: &[u8], sig: &[u8; MESH_ED_SIG_LEN]) -> Vec<u8> {
    let mut out = Vec::with_capacity(MESH_V10_HEADER_LEN + inner.len());
    out.extend_from_slice(SPORE_V10_MAGIC);
    out.extend_from_slice(&msg_id.to_le_bytes());
    out.extend_from_slice(&origin_fp);
    out.push(ttl);
    out.extend_from_slice(&hops_so_far.to_le_bytes());
    out.extend_from_slice(sig);
    out.extend_from_slice(inner);
    out
}

struct ParsedHeader {
    msg_id: u64,
    origin_fp: [u8; FP_LEN],
    ttl: u8,
    hops_so_far: u16,
}

/// Unified v8/v9/v0A parse + verify. Mode is determined by the router's
/// configured signing state (mac_key / ed_seed):
///   unsigned router:    accepts v8, rejects v9/v0A
///   v9 MAC router:      accepts v9, rejects v8/v0A
///   v0A Ed25519 router: accepts v0A (verified vs registry), rejects v8/v9
fn parse_and_verify(envelope: &[u8], mac_key: Option<&MeshMacKey>, #[cfg(feature = "mesh_v10")] ed_registry: Option<&MeshPubRegistry>) -> Result<ParsedHeader, &'static str> {
    if envelope.len() < 6 {
        return Err("mesh envelope too short");
    }
    let magic = &envelope[..6];
    let is_v9 = magic == SPORE_V9_MAGIC;
    let is_v8 = magic == SPORE_V8_MAGIC;
    #[cfg(feature = "mesh_v10")]
    let is_v10 = magic == SPORE_V10_MAGIC;
    #[cfg(not(feature = "mesh_v10"))]
    let is_v10 = false;
    if !is_v8 && !is_v9 && !is_v10 {
        return Err("bad mesh magic");
    }

    #[cfg(feature = "mesh_v10")]
    let header_len = if is_v10 {
        MESH_V10_HEADER_LEN
    } else if is_v9 {
        MESH_V9_HEADER_LEN
    } else {
        MESH_HEADER_LEN
    };
    #[cfg(not(feature = "mesh_v10"))]
    let header_len = if is_v9 { MESH_V9_HEADER_LEN } else { MESH_HEADER_LEN };
    if envelope.len() < header_len {
        return Err("mesh envelope too short");
    }

    // Policy enforcement (mode mismatch).
    #[cfg(feature = "mesh_v10")]
    let in_v10_mode = ed_registry.is_some();
    #[cfg(not(feature = "mesh_v10"))]
    let in_v10_mode = false;
    let in_v9_mode = mac_key.is_some();
    match (is_v8, is_v9, is_v10, in_v9_mode, in_v10_mode) {
        (true, _, _, true, false) => return Err("unsigned v8 rejected by signed router"),
        (true, _, _, false, true) => return Err("unsigned v8 rejected by signed router"),
        (_, true, _, false, true) => return Err("v9 rejected by v0A-only router"),
        (_, true, _, false, false) => return Err("v9 received but router has no key"),
        (_, _, true, true, false) => return Err("v0A rejected by v9-only router"),
        (_, _, true, false, false) => return Err("v0A received but router has no registry"),
        _ => {}
    }

    let msg_id = u64::from_le_bytes(envelope[6..14].try_into().unwrap());
    let origin_fp: [u8; FP_LEN] = envelope[14..22].try_into().unwrap();
    let ttl = envelope[22];
    let hops_so_far = u16::from_le_bytes(envelope[23..25].try_into().unwrap());

    if is_v9 {
        let key = mac_key.unwrap();
        let got: [u8; MESH_TAG_LEN] = envelope[25..25 + MESH_TAG_LEN].try_into().unwrap();
        if !mesh_v9_verify(key, msg_id, origin_fp, &got) {
            return Err("bad mesh mac");
        }
    }
    #[cfg(feature = "mesh_v10")]
    if is_v10 {
        let registry = ed_registry.unwrap();
        let pubkey = registry.get(&origin_fp).ok_or("unknown sender")?;
        let got: [u8; MESH_ED_SIG_LEN] = envelope[25..25 + MESH_ED_SIG_LEN].try_into().unwrap();
        if !mesh_v10_verify(pubkey, msg_id, origin_fp, &got) {
            return Err("bad mesh signature");
        }
    }
    Ok(ParsedHeader { msg_id, origin_fp, ttl, hops_so_far })
}

pub struct MeshHeader<'a> {
    pub msg_id: u64,
    pub origin_fp: [u8; FP_LEN],
    pub ttl: u8,
    pub hops_so_far: u16,
    pub inner: &'a [u8],
}

pub fn parse_envelope(envelope: &[u8]) -> Result<MeshHeader<'_>, &'static str> {
    if envelope.len() < 6 {
        return Err("mesh envelope too short");
    }
    let magic = &envelope[..6];
    #[cfg(feature = "mesh_v10")]
    let header_len = if magic == SPORE_V10_MAGIC {
        MESH_V10_HEADER_LEN
    } else if magic == SPORE_V9_MAGIC {
        MESH_V9_HEADER_LEN
    } else if magic == SPORE_V8_MAGIC {
        MESH_HEADER_LEN
    } else {
        return Err("bad mesh magic");
    };
    #[cfg(not(feature = "mesh_v10"))]
    let header_len = if magic == SPORE_V9_MAGIC {
        MESH_V9_HEADER_LEN
    } else if magic == SPORE_V8_MAGIC {
        MESH_HEADER_LEN
    } else {
        return Err("bad mesh magic");
    };
    if envelope.len() < header_len {
        return Err("mesh envelope too short");
    }
    let msg_id = u64::from_le_bytes(envelope[6..14].try_into().unwrap());
    let origin_fp: [u8; FP_LEN] = envelope[14..22].try_into().unwrap();
    let ttl = envelope[22];
    let hops = u16::from_le_bytes(envelope[23..25].try_into().unwrap());
    Ok(MeshHeader { msg_id, origin_fp, ttl, hops_so_far: hops, inner: &envelope[header_len..] })
}

// Tests and Kani proofs live in sibling files to keep this module focused on
// the mesh protocol core (HMAC/Ed25519 signing, Bloom dedup, routing).
#[cfg(kani)]
mod kani_proofs;

#[cfg(test)]
mod tests;
