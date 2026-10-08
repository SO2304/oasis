//! OASIS — `OSE1`, end-to-end confidentiality for a mesh payload (`POSITIONING_GAPS.md`
//! **C13**).
//!
//! v0B and v0C authenticate; they do not encrypt. An order's payload travels in clear,
//! so any relay — and anyone listening — reads the register, the setpoint and the
//! actuator id. This module seals the payload **between the origin and the addressed
//! actuator**, and nothing else changes:
//!
//! ```text
//! OAC1 order ──seal──▶ OSE1 blob ──v0B envelope (unchanged)──▶ relay ──▶ actuator ──open──▶ OAC1 ──▶ Part F gate
//! ```
//!
//! - **v0B is untouched.** The sealed blob is just a payload: `origin_wrap_v0b` signs
//!   `SHA-256(payload)` as before, so the signature, the counter window, the dedup and
//!   the revocation check all behave identically. No new wire version, no new magic on
//!   the envelope.
//! - **The gate is untouched.** `open` yields the original `OAC1` bytes and the existing
//!   `actuation_decision` runs on them. A confidentiality layer must not become a second
//!   authorisation path.
//! - **Relays cannot read it.** The key is origin↔destination, derived from the two
//!   **identity** keys, so a relay in the middle holds no key for traffic it only
//!   forwards. That is the difference from `mesh::prefilter`'s per-link key, which every
//!   neighbour must hold by design.
//!
//! Key: `K = HKDF-SHA256(ikm = X25519(origin_sk, dest_pk), salt = DOMAIN‖fp_lo‖fp_hi)`,
//! the same construction as [`crate::mesh::prefilter::link_key`] with its own domain
//! string, so the two keys are different even for the same pair. Nothing is distributed:
//! each side derives it from material it already has.
//!
//! AAD: `DOMAIN ‖ network_id ‖ origin_fp ‖ dest_fp ‖ counter`. Binding the counter and
//! both fingerprints means a ciphertext lifted out of one envelope cannot be replayed
//! inside another — the AEAD refuses before the gate is ever reached.
//!
//! Format: `"OSE1" | dest_fp[8] | AEAD envelope` — the envelope being the audited
//! ChaCha20-Poly1305 one from `spore_crypto`, which carries its own magic, nonce and
//! length before the ciphertext and tag. Overhead is **50 bytes**: 12 here, 22 there,
//! 16 of tag.
//!
//! ⚠️ **What this does not hide.** The envelope header stays in clear by construction:
//! origin fingerprint, counter, payload length, network id. An observer still sees *who*
//! talks to *whom*, *how often*, and *how long the message is* — only the content is
//! hidden. Traffic analysis is listed separately in the threat model and this does not
//! close it.
//!
//! ⚠️ **Nothing on silicon.** Host-side only; the firmware does not emit or accept `OSE1`
//! yet.

use crate::mesh::{MeshEdPub, MeshEdSeed, FP_LEN, MESH_V0B_NETWORK_LEN};
use crate::spore_crypto::{decrypt_envelope, encrypt_envelope_with_nonce, HEADER_LEN as AEAD_HEADER_LEN, KEY_LEN, NONCE_LEN, TAG_LEN};

use ed25519_compact::{x25519, KeyPair, PublicKey, Seed};
use hkdf::Hkdf;
use sha2::Sha256;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

pub const OSE1_MAGIC: [u8; 4] = *b"OSE1";
/// `magic 4 | dest_fp 8`. The nonce is **not** repeated here: the ChaCha20-Poly1305
/// envelope this wraps already carries its own magic, nonce and length. My first version
/// stored it twice and the round-trip test caught it at 116 bytes instead of 94.
pub const OSE1_HEADER_LEN: usize = 4 + FP_LEN;
/// Poly1305 tag, from the AEAD layer.
pub const OSE1_TAG_LEN: usize = TAG_LEN;
/// Sealing overhead: this header, the AEAD envelope's header, and the tag.
pub const OSE1_OVERHEAD: usize = OSE1_HEADER_LEN + AEAD_HEADER_LEN + OSE1_TAG_LEN;
/// Its own domain, so the sealing key differs from the per-link MAC key of the same pair.
pub const SEAL_DOMAIN: &[u8] = b"OASIS-SEAL-v1";

/// Sealed length for a plaintext of `n` bytes.
pub const fn ose1_len(n: usize) -> usize {
    OSE1_OVERHEAD + n
}

/// `K = HKDF-SHA256(X25519(our_sk, peer_pk), SEAL_DOMAIN‖fp_lo‖fp_hi)`.
///
/// Fingerprints are sorted, so origin and destination derive the same key from opposite
/// sides. `None` on any key that is not a valid Ed25519 point.
pub fn seal_key(our_seed: &MeshEdSeed, our_fp: [u8; FP_LEN], peer_pub: &MeshEdPub, peer_fp: [u8; FP_LEN]) -> Option<[u8; KEY_LEN]> {
    let kp = KeyPair::from_seed(Seed::from_slice(&our_seed.0).ok()?);
    let x_sk = x25519::SecretKey::from_ed25519(&kp.sk).ok()?;
    let x_pk = x25519::PublicKey::from_ed25519(&PublicKey::from_slice(&peer_pub.0).ok()?).ok()?;
    let shared = x_pk.dh(&x_sk).ok()?;

    let (lo, hi) = if our_fp <= peer_fp { (our_fp, peer_fp) } else { (peer_fp, our_fp) };
    let mut salt = [0u8; 13 + 2 * FP_LEN];
    salt[..13].copy_from_slice(SEAL_DOMAIN);
    salt[13..13 + FP_LEN].copy_from_slice(&lo);
    salt[13 + FP_LEN..].copy_from_slice(&hi);

    let hk = Hkdf::<Sha256>::new(Some(&salt), &shared[..]);
    let mut key = [0u8; KEY_LEN];
    hk.expand(&[], &mut key).ok()?;
    Some(key)
}

/// `DOMAIN ‖ network_id ‖ origin_fp ‖ dest_fp ‖ counter`.
///
/// The counter is the one the origin will put in the v0B header, so a ciphertext is bound
/// to the exact envelope that carries it.
pub fn seal_aad(network_id: &[u8; MESH_V0B_NETWORK_LEN], origin_fp: [u8; FP_LEN], dest_fp: [u8; FP_LEN], counter: u64) -> Vec<u8> {
    let mut aad = Vec::with_capacity(SEAL_DOMAIN.len() + MESH_V0B_NETWORK_LEN + 2 * FP_LEN + 8);
    aad.extend_from_slice(SEAL_DOMAIN);
    aad.extend_from_slice(network_id);
    aad.extend_from_slice(&origin_fp);
    aad.extend_from_slice(&dest_fp);
    aad.extend_from_slice(&counter.to_le_bytes());
    aad
}

/// Seal `inner` for `dest_fp` with an explicit nonce.
///
/// The nonce is a parameter rather than drawn inside, because the MCU build has no OS
/// RNG — the same reason `spore_crypto` exposes `*_with_nonce`. A caller that reuses a
/// nonce for the same key breaks ChaCha20-Poly1305; derive it from the counter, which is
/// already unique per origin and persisted, rather than from a clock.
pub fn seal_with_nonce(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], dest_fp: [u8; FP_LEN], aad: &[u8], inner: &[u8]) -> Option<Vec<u8>> {
    let sealed = encrypt_envelope_with_nonce(key, nonce, inner, aad).ok()?;
    let mut out = Vec::with_capacity(OSE1_HEADER_LEN + sealed.len());
    out.extend_from_slice(&OSE1_MAGIC);
    out.extend_from_slice(&dest_fp);
    out.extend_from_slice(&sealed);
    Some(out)
}

/// A nonce from the v0B counter: `counter_le(8) ‖ 0000`. Unique per origin because the
/// counter is strictly increasing and persisted across reboots (`tx_lease`), which is
/// exactly the property a nonce needs and a clock does not have.
pub fn nonce_from_counter(counter: u64) -> [u8; NONCE_LEN] {
    let mut n = [0u8; NONCE_LEN];
    n[..8].copy_from_slice(&counter.to_le_bytes());
    n
}

/// The destination fingerprint a sealed blob names, without opening it. `None` if this is
/// not an `OSE1` blob. A relay may read this — it is how a node knows a payload is not
/// addressed to it — and it is also why `OSE1` hides content, not addressing.
pub fn sealed_dest(blob: &[u8]) -> Option<[u8; FP_LEN]> {
    if blob.len() < ose1_len(0) || blob[0..4] != OSE1_MAGIC {
        return None;
    }
    let mut fp = [0u8; FP_LEN];
    fp.copy_from_slice(&blob[4..4 + FP_LEN]);
    Some(fp)
}

/// Open a sealed blob addressed to `our_fp`.
///
/// `None` — never a partial result — when the blob is malformed, is addressed to someone
/// else, or fails authentication. A wrong AAD (another network, another counter, another
/// pair) lands here too: the ciphertext is refused before anything reaches the gate.
pub fn open(key: &[u8; KEY_LEN], our_fp: [u8; FP_LEN], aad: &[u8], blob: &[u8]) -> Option<Vec<u8>> {
    if sealed_dest(blob)? != our_fp {
        return None;
    }
    decrypt_envelope(key, &blob[OSE1_HEADER_LEN..], aad).ok()
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
