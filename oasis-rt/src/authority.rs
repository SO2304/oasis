//! OASIS — authority container `OAU1`: crypto agility and hybrid Ed25519 + ML-DSA-44.
//!
//! Spec: `docs/specs/PQ_AUTHORITY_SPEC.md`. Authority messages (revocation,
//! enrollment, firmware manifest, ownership transfer, policy) are rare and valid for a
//! long time, so they can be signed **hybrid**: Ed25519 (RFC 8032) AND ML-DSA-44
//! (FIPS 204). Both must verify. Orders and per-hop v0B traffic stay Ed25519 only.
//!
//! `"OAU1" | kind u8 | suite u8 | network_id[8] | content_len u16 | content | sigblock`
//!
//! Signed data (identical for both algorithms; `suite` is signed, so a hybrid
//! message cannot be relabelled as Ed25519-only):
//! `"OASIS-AUTH-v1" | kind | suite | network_id | content_len | content`.
//! ML-DSA additionally uses `"OASIS-AUTH-v1"` as its FIPS 204 context string.
//!
//! A per-kind minimum suite ([`AuthPolicy`]), persisted and never lowered, refuses
//! downgrades before any expensive verification. Public keys never travel: they are
//! provisioned on the device.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use ml_dsa::{EncodedVerifyingKey, MlDsa44, Signature as MlDsaSignature, VerifyingKey as MlDsaVerifyingKey};
use sha2::{Digest, Sha256};

pub const OAU1_MAGIC: [u8; 4] = *b"OAU1";
pub const AUTH_DOMAIN: &[u8] = b"OASIS-AUTH-v1";

/// Ed25519 only (migration starting point).
pub const SUITE_ED25519: u8 = 0x01;
/// ML-DSA-44 alone: reserved, refused in v1.
pub const SUITE_MLDSA44: u8 = 0x02;
/// Ed25519 AND ML-DSA-44.
pub const SUITE_HYBRID: u8 = 0x03;

pub const ED25519_SIG_LEN: usize = 64;
pub const MLDSA44_SIG_LEN: usize = 2420;
pub const MLDSA44_PK_LEN: usize = 1312;
/// Upper bound on `content` (keeps a whole message under the 4 KiB reassembly cap).
pub const MAX_AUTH_CONTENT: usize = 1024;
const HEADER_LEN: usize = 4 + 1 + 1 + 8 + 2;

/// Message kinds.
pub mod kind {
    pub const REVOCATION: u8 = 1;
    pub const ENROLLMENT: u8 = 2;
    pub const FIRMWARE_MANIFEST: u8 = 3;
    pub const OWNERSHIP_TRANSFER: u8 = 4;
    /// Raises the minimum suite of a kind. Always requires the hybrid suite.
    pub const POLICY: u8 = 5;
}
pub const KIND_COUNT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthReject {
    Malformed,
    UnknownKind,
    UnsupportedSuite,
    WrongNetwork,
    /// Suite weaker than the policy minimum for this kind.
    Downgrade,
    BadSignature,
}

/// A parsed `OAU1` message (signatures not yet verified).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedAuthority<'a> {
    pub kind: u8,
    pub suite: u8,
    pub network_id: [u8; 8],
    pub content: &'a [u8],
    pub ed25519_sig: &'a [u8],
    /// Empty unless `suite == SUITE_HYBRID`.
    pub mldsa44_sig: &'a [u8],
}

fn sigblock_len(suite: u8) -> Option<usize> {
    match suite {
        SUITE_ED25519 => Some(ED25519_SIG_LEN),
        SUITE_HYBRID => Some(ED25519_SIG_LEN + MLDSA44_SIG_LEN),
        _ => None, // SUITE_MLDSA44 is reserved; everything else unknown
    }
}

/// Strength order used by the policy. Unknown suites have no rank.
pub fn suite_rank(suite: u8) -> u8 {
    match suite {
        SUITE_ED25519 => 1,
        SUITE_HYBRID => 3,
        _ => 0,
    }
}

/// Parse an `OAU1` message. Never panics.
pub fn parse_oau1(b: &[u8]) -> Result<ParsedAuthority<'_>, AuthReject> {
    if b.len() < HEADER_LEN || b[0..4] != OAU1_MAGIC {
        return Err(AuthReject::Malformed);
    }
    let kind = b[4];
    if kind == 0 || kind as usize > KIND_COUNT {
        return Err(AuthReject::UnknownKind);
    }
    let suite = b[5];
    let sig_len = sigblock_len(suite).ok_or(AuthReject::UnsupportedSuite)?;
    let mut network_id = [0u8; 8];
    network_id.copy_from_slice(&b[6..14]);
    let content_len = u16::from_le_bytes([b[14], b[15]]) as usize;
    if content_len > MAX_AUTH_CONTENT || b.len() != HEADER_LEN + content_len + sig_len {
        return Err(AuthReject::Malformed);
    }
    let content = &b[HEADER_LEN..HEADER_LEN + content_len];
    let sigs = &b[HEADER_LEN + content_len..];
    let (ed25519_sig, mldsa44_sig) = sigs.split_at(ED25519_SIG_LEN);
    Ok(ParsedAuthority { kind, suite, network_id, content, ed25519_sig, mldsa44_sig })
}

/// The bytes both algorithms sign.
pub fn signed_message(kind: u8, suite: u8, network_id: &[u8; 8], content: &[u8]) -> Vec<u8> {
    let mut m = Vec::with_capacity(AUTH_DOMAIN.len() + 1 + 1 + 8 + 2 + content.len());
    m.extend_from_slice(AUTH_DOMAIN);
    m.push(kind);
    m.push(suite);
    m.extend_from_slice(network_id);
    m.extend_from_slice(&(content.len() as u16).to_le_bytes());
    m.extend_from_slice(content);
    m
}

/// Encode an `OAU1` message. `mldsa44_sig` must be empty for `SUITE_ED25519`.
pub fn encode_oau1(kind: u8, suite: u8, network_id: &[u8; 8], content: &[u8], ed25519_sig: &[u8; ED25519_SIG_LEN], mldsa44_sig: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(HEADER_LEN + content.len() + ED25519_SIG_LEN + mldsa44_sig.len());
    b.extend_from_slice(&OAU1_MAGIC);
    b.push(kind);
    b.push(suite);
    b.extend_from_slice(network_id);
    b.extend_from_slice(&(content.len() as u16).to_le_bytes());
    b.extend_from_slice(content);
    b.extend_from_slice(ed25519_sig);
    b.extend_from_slice(mldsa44_sig);
    b
}

/// Per-kind minimum suite. Persisted on the device; [`AuthPolicy::raise`] never lowers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthPolicy {
    min_suite: [u8; KIND_COUNT],
}

impl Default for AuthPolicy {
    /// Migration start: Ed25519 accepted for kinds 1-4; POLICY always hybrid.
    fn default() -> Self {
        let mut min_suite = [SUITE_ED25519; KIND_COUNT];
        min_suite[(kind::POLICY - 1) as usize] = SUITE_HYBRID;
        AuthPolicy { min_suite }
    }
}

const POLICY_MAGIC: [u8; 4] = *b"APL1";
pub const POLICY_RECORD_LEN: usize = 4 + KIND_COUNT + 4;

impl AuthPolicy {
    pub fn min_suite(&self, kind: u8) -> u8 {
        if kind == 0 || kind as usize > KIND_COUNT {
            return u8::MAX; // unknown kind: nothing satisfies it
        }
        self.min_suite[(kind - 1) as usize]
    }

    /// Raise the minimum suite of `kind`. Never lowers it and never accepts an
    /// unknown suite. Returns `true` if the policy changed.
    pub fn raise(&mut self, kind: u8, suite: u8) -> bool {
        if kind == 0 || kind as usize > KIND_COUNT || suite_rank(suite) == 0 {
            return false;
        }
        let i = (kind - 1) as usize;
        if suite_rank(suite) > suite_rank(self.min_suite[i]) {
            self.min_suite[i] = suite;
            true
        } else {
            false
        }
    }

    /// May a legacy `ORV1` list (Ed25519 only) still be accepted?
    pub fn legacy_orv1_allowed(&self) -> bool {
        suite_rank(self.min_suite(kind::REVOCATION)) <= suite_rank(SUITE_ED25519)
    }

    fn check(body: &[u8]) -> [u8; 4] {
        let d = Sha256::new().chain_update(POLICY_MAGIC).chain_update(body).finalize();
        [d[0], d[1], d[2], d[3]]
    }

    /// `APL1 | min_suite[KIND_COUNT] | check4`.
    pub fn to_bytes(&self) -> [u8; POLICY_RECORD_LEN] {
        let mut r = [0u8; POLICY_RECORD_LEN];
        r[0..4].copy_from_slice(&POLICY_MAGIC);
        r[4..4 + KIND_COUNT].copy_from_slice(&self.min_suite);
        let c = Self::check(&self.min_suite);
        r[4 + KIND_COUNT..].copy_from_slice(&c);
        r
    }

    /// Restore a persisted policy; `None` if absent, torn or corrupted. A restored
    /// policy is never weaker than the default.
    pub fn from_bytes(r: &[u8]) -> Option<Self> {
        if r.len() != POLICY_RECORD_LEN || r[0..4] != POLICY_MAGIC {
            return None;
        }
        let body = &r[4..4 + KIND_COUNT];
        if r[4 + KIND_COUNT..] != Self::check(body) {
            return None;
        }
        let mut p = AuthPolicy::default();
        for (k, &s) in body.iter().enumerate() {
            p.raise((k + 1) as u8, s);
        }
        Some(p)
    }
}

/// Cheap checks, before any signature verification: network, then downgrade.
pub fn precheck(policy: &AuthPolicy, my_network: &[u8; 8], p: &ParsedAuthority) -> Result<(), AuthReject> {
    if p.network_id != *my_network {
        return Err(AuthReject::WrongNetwork);
    }
    if suite_rank(p.suite) < suite_rank(policy.min_suite(p.kind)) {
        return Err(AuthReject::Downgrade);
    }
    Ok(())
}

/// Pure rule: do the signature results satisfy the suite? Hybrid requires BOTH.
pub fn signatures_satisfy(suite: u8, ed25519_ok: bool, mldsa44_ok: bool) -> bool {
    match suite {
        SUITE_ED25519 => ed25519_ok,
        SUITE_HYBRID => ed25519_ok && mldsa44_ok,
        _ => false,
    }
}

/// Ed25519 (RFC 8032) verification.
pub fn ed25519_verify(public_key: &[u8; 32], msg: &[u8], sig: &[u8]) -> bool {
    let pk = match ed25519_compact::PublicKey::from_slice(public_key) {
        Ok(p) => p,
        Err(_) => return false,
    };
    match ed25519_compact::Signature::from_slice(sig) {
        Ok(s) => pk.verify(msg, &s).is_ok(),
        Err(_) => false,
    }
}

/// ML-DSA-44 (FIPS 204, external/pure interface) verification with a context string.
pub fn mldsa44_verify(public_key: &[u8], msg: &[u8], ctx: &[u8], sig: &[u8]) -> bool {
    if public_key.len() != MLDSA44_PK_LEN || sig.len() != MLDSA44_SIG_LEN || ctx.len() > 255 {
        return false;
    }
    let enc = match EncodedVerifyingKey::<MlDsa44>::try_from(public_key) {
        Ok(e) => e,
        Err(_) => return false,
    };
    let vk = MlDsaVerifyingKey::<MlDsa44>::decode(&enc);
    match MlDsaSignature::<MlDsa44>::try_from(sig) {
        Ok(s) => vk.verify_with_context(msg, ctx, &s),
        Err(_) => false,
    }
}

/// Authority public keys provisioned on the device.
pub struct AuthorityKeys<'k> {
    pub ed25519: [u8; 32],
    pub mldsa44: &'k [u8; MLDSA44_PK_LEN],
}

/// Full check of an `OAU1` message: parse, cheap prechecks, then signatures. For
/// the hybrid suite ML-DSA-44 is verified first (cheaper than Ed25519 on a
/// Cortex-M0+), and both must pass.
pub fn verify_authority<'a>(policy: &AuthPolicy, my_network: &[u8; 8], keys: &AuthorityKeys, b: &'a [u8]) -> Result<ParsedAuthority<'a>, AuthReject> {
    let p = parse_oau1(b)?;
    precheck(policy, my_network, &p)?;
    let msg = signed_message(p.kind, p.suite, &p.network_id, p.content);
    let mldsa44_ok = p.suite == SUITE_HYBRID && mldsa44_verify(keys.mldsa44, &msg, AUTH_DOMAIN, p.mldsa44_sig);
    if p.suite == SUITE_HYBRID && !mldsa44_ok {
        return Err(AuthReject::BadSignature);
    }
    let ed25519_ok = ed25519_verify(&keys.ed25519, &msg, p.ed25519_sig);
    if signatures_satisfy(p.suite, ed25519_ok, mldsa44_ok) {
        Ok(p)
    } else {
        Err(AuthReject::BadSignature)
    }
}

/// Content of a `kind::POLICY` message: `target_kind u8 | min_suite u8`.
pub fn parse_policy_update(content: &[u8]) -> Option<(u8, u8)> {
    if content.len() != 2 {
        return None;
    }
    Some((content[0], content[1]))
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
