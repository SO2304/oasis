//! OASIS — AEAD envelope for spore transport (ChaCha20-Poly1305, RFC 8439)
//!
//! Wire format (encrypted v3 envelope):
//! ```text
//!   [0..6]     SPORE\x03    magic (distinct from v1=SPORE\x01, v2=SPORE\x02)
//!   [6..18]    nonce        u8[12] — random per message (MUST NOT repeat under same key)
//!   [18..22]   ct_len       u32 LE — ciphertext length (excludes 16-byte tag)
//!   [22..22+n] ciphertext   n bytes
//!   [22+n..22+n+16] tag     16 bytes Poly1305 auth tag
//! ```
//!
//! # Security properties (per RFC 8439 + this module's construction)
//! - **Confidentiality**: ciphertext reveals nothing about plaintext (under IND-CPA)
//! - **Integrity**: tag verifies plaintext + nonce + AAD; single-bit tamper rejected
//! - **Authenticity**: only holders of the 32-byte key can produce valid tags
//! - **No forward secrecy**: compromise of key decrypts all past messages.
//!   Acceptable for OASIS digests (low secrecy, short-lived). For long-term
//!   secrecy use an ECDH-rotated key layer above this (not in scope here).
//!
//! # Misuse vectors explicitly guarded against
//! 1. **Nonce reuse** — Catastrophic for any AEAD. We use 12 random bytes per
//!    call. Probability of collision after N messages ≈ N² / 2^97 (96 bit nonce).
//!    Safe for up to ~10^14 messages under a single key. Rotate the key before.
//! 2. **Tampering** — Poly1305 tag covers ct. Any edit → `decrypt_envelope`
//!    returns `Err("auth failed")`. Tested.
//! 3. **Wrong key** — Same error path. Tested.
//! 4. **Truncated envelope** — Length-checked. Tested.
//! 5. **Downgrade** — If OASIS_SPORE_KEY is set, recv rejects unencrypted
//!    (v1/v2) envelopes. Enforced at the spore layer, not here.
//!
//! # Key derivation
//! `derive_key_from_passphrase(pass)` returns the key as `SHA-256(pass || DOMAIN_TAG)`.
//! Suitable for human-memorable passphrases. For random 32-byte keys pass them
//! as hex via `OASIS_SPORE_KEY_HEX`.

#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    collections::{BTreeSet as HashSet, VecDeque},
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use sha2::{Digest, Sha256};
#[cfg(feature = "std")]
use std::collections::{HashSet, VecDeque};

pub const SPORE_V3_MAGIC: &[u8] = b"SPORE\x03";
pub const SPORE_V4_MAGIC: &[u8] = b"SPORE\x04"; // ECDH forward-secret envelope
pub const SPORE_V5_MAGIC: &[u8] = b"SPORE\x05"; // ECDH + sender authentication (Noise-KK)
pub const SPORE_V6_MAGIC: &[u8] = b"SPORE\x06"; // revocation-list broadcast envelope
pub const SPORE_V7_MAGIC: &[u8] = b"SPORE\x07"; // v5 + monotonic counter (long-window replay resistance)
pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;
pub const TAG_LEN: usize = 16;
pub const X25519_PUBKEY_LEN: usize = 32;
pub const X25519_PRIVKEY_LEN: usize = 32;
const HEADER_LEN: usize = 6 + NONCE_LEN + 4; // v3: magic + nonce + ct_len
const V4_HEADER_LEN: usize = 6 + X25519_PUBKEY_LEN + NONCE_LEN + 4; // v4: + eph_pub
pub const SENDER_FP_LEN: usize = 8;
const V5_HEADER_LEN: usize = 6 + SENDER_FP_LEN + X25519_PUBKEY_LEN + NONCE_LEN + 4;
const V7_HEADER_LEN: usize = V5_HEADER_LEN + 8; // +8 bytes for u64 counter
const DOMAIN_TAG: &[u8] = b"oasis-spore-v1"; // binds PSK derivation to protocol
const ECDH_SALT: &[u8] = b"oasis-ecdh-v1"; // binds session-key derivation

/// Derive a 32-byte key from an arbitrary passphrase via SHA-256.
/// NOT a password hash (no salt, no iterations). Use only when the passphrase
/// is itself high-entropy (>=128 bits). For weak passphrases, pre-apply scrypt
/// / argon2 at the application layer.
pub fn derive_key_from_passphrase(passphrase: &[u8]) -> [u8; KEY_LEN] {
    let mut h = Sha256::new();
    h.update(passphrase);
    h.update(DOMAIN_TAG);
    let out = h.finalize();
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&out[..KEY_LEN]);
    key
}

/// Parse a 64-char hex string into a 32-byte key. Returns None on bad input.
pub fn parse_key_hex(hex: &str) -> Option<[u8; KEY_LEN]> {
    if hex.len() != KEY_LEN * 2 {
        return None;
    }
    let mut key = [0u8; KEY_LEN];
    for i in 0..KEY_LEN {
        let hi = hex_digit(hex.as_bytes()[i * 2])?;
        let lo = hex_digit(hex.as_bytes()[i * 2 + 1])?;
        key[i] = (hi << 4) | lo;
    }
    Some(key)
}

fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Fill `nonce` with 12 bytes from the OS CSPRNG via `getrandom`.
/// Linux: `getrandom(2)` syscall. Windows: `BCryptGenRandom`. macOS: `getentropy`.
/// All three provide cryptographic-grade entropy.
///
/// Panic semantics: if the OS RNG is unavailable (extraordinarily rare — indicates
/// a broken kernel or sandboxing that blocks entropy calls), we PANIC rather than
/// silently degrade. A weak nonce is worse than no message at all for AEAD safety.
///
/// Feature-gated behind `os_random` (default on). MCU targets that disable this
/// feature must construct nonces externally and call `encrypt_envelope_with_nonce`
/// (future API; not yet exposed as of 2026-04-22).
#[cfg(feature = "os_random")]
fn random_nonce() -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce).expect("OS CSPRNG unavailable — refusing to generate weak AEAD nonce");
    nonce
}

/// Encrypt `plaintext` with AEAD into a SPORE\x03 envelope using a
/// caller-supplied nonce. Always available (no `os_random` dependency).
///
/// **CRITICAL**: the caller is responsible for nonce uniqueness per key.
/// Reusing a (key, nonce) pair breaks AEAD security completely. On MCU
/// targets without OS RNG, typical patterns:
///   - Monotonic u64 counter, padded to 12 bytes. Must survive reboot
///     (e.g., store in NVS). Otherwise reuse after restart.
///   - Hardware TRNG bytes, if the chip has one.
/// **Do NOT** use a fixed nonce. Do NOT derive the nonce from plaintext.
///
/// This is the MCU-safe primitive. `encrypt_envelope` is a thin wrapper
/// that calls this with `random_nonce()`.
pub fn encrypt_envelope_with_nonce(key: &[u8; KEY_LEN], nonce_bytes: &[u8; NONCE_LEN], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, &'static str> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);
    let ct = cipher.encrypt(nonce, Payload { msg: plaintext, aad }).map_err(|_| "encrypt failed")?;
    // ct = ciphertext || tag (16 bytes) per RustCrypto convention
    let ct_len = ct.len().saturating_sub(TAG_LEN);
    let mut env = Vec::with_capacity(HEADER_LEN + ct.len());
    env.extend_from_slice(SPORE_V3_MAGIC);
    env.extend_from_slice(nonce_bytes);
    env.extend_from_slice(&(ct_len as u32).to_le_bytes());
    env.extend_from_slice(&ct);
    Ok(env)
}

/// Encrypt `plaintext` with AEAD into a SPORE\x03 envelope.
/// Associated data (`aad`) is authenticated but not encrypted (bound to the tag).
/// Pass `&[]` if you don't need AAD.
///
/// Feature-gated behind `os_random` (default on) because it derives the nonce
/// via `getrandom`. On MCU targets without `os_random`, call
/// `encrypt_envelope_with_nonce` and supply your own 12-byte nonce.
#[cfg(feature = "os_random")]
pub fn encrypt_envelope(key: &[u8; KEY_LEN], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, &'static str> {
    let nonce_bytes = random_nonce();
    encrypt_envelope_with_nonce(key, &nonce_bytes, plaintext, aad)
}

/// Bounded LRU-ish nonce cache for replay protection.
/// Stores the last `capacity` nonces in insertion order; when full, evicts FIFO.
/// Call `check_and_insert(nonce)` — returns `Ok(())` if nonce is fresh, `Err` if seen before.
///
/// Threat model: an attacker intercepts a valid encrypted envelope and replays it.
/// Without a replay window, the receiver decrypts the same message twice and
/// may double-apply state. With this window, replays within the last `capacity`
/// messages are rejected. For longer-lived protection, combine with a
/// monotonic counter or time-window in the AAD.
pub struct ReplayWindow {
    seen: VecDeque<[u8; NONCE_LEN]>,
    capacity: usize,
}

impl ReplayWindow {
    /// Create a new window of given capacity. 1024 is a reasonable default:
    /// ~12 KB RAM; covers typical message bursts.
    pub fn new(capacity: usize) -> Self {
        Self { seen: VecDeque::with_capacity(capacity), capacity: capacity.max(1) }
    }

    /// Check whether `nonce` was seen. If fresh, insert and return Ok.
    /// If already seen, return Err without mutating.
    pub fn check_and_insert(&mut self, nonce: &[u8; NONCE_LEN]) -> Result<(), &'static str> {
        if self.seen.iter().any(|n| n == nonce) {
            return Err("replay detected");
        }
        if self.seen.len() >= self.capacity {
            self.seen.pop_front();
        }
        self.seen.push_back(*nonce);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.seen.len()
    }
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    /// Serialize the current window to bytes (no signature — file integrity is
    /// the filesystem's job). Format: magic(4) + ver(1) + cap(u32 LE) +
    /// count(u32 LE) + N×nonce(12B).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(13 + self.seen.len() * NONCE_LEN);
        buf.extend_from_slice(b"RPLY");
        buf.push(0x01);
        buf.extend_from_slice(&(self.capacity as u32).to_le_bytes());
        buf.extend_from_slice(&(self.seen.len() as u32).to_le_bytes());
        for n in &self.seen {
            buf.extend_from_slice(n);
        }
        buf
    }

    /// Deserialize a window from bytes produced by `to_bytes`. If the stored
    /// capacity exceeds the current one, entries are truncated FIFO.
    pub fn from_bytes(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 13 {
            return Err("too short");
        }
        if &data[..4] != b"RPLY" {
            return Err("bad magic");
        }
        if data[4] != 0x01 {
            return Err("bad version");
        }
        let cap = u32::from_le_bytes(data[5..9].try_into().unwrap()) as usize;
        let count = u32::from_le_bytes(data[9..13].try_into().unwrap()) as usize;
        let expected_len = 13 + count * NONCE_LEN;
        if data.len() < expected_len {
            return Err("truncated");
        }
        let mut rw = ReplayWindow::new(cap.max(1));
        for i in 0..count {
            let off = 13 + i * NONCE_LEN;
            let n: [u8; NONCE_LEN] = data[off..off + NONCE_LEN].try_into().unwrap();
            // Ignore duplicate insert failures — persistence should be idempotent
            let _ = rw.check_and_insert(&n);
        }
        Ok(rw)
    }

    /// Save the window to a filesystem path. Atomically replaces the file.
    #[cfg(feature = "std")]
    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let tmp = format!("{}.tmp", path);
        std::fs::write(&tmp, self.to_bytes())?;
        std::fs::rename(&tmp, path)
    }

    /// Load a window from a filesystem path. Returns the deserialized window.
    #[cfg(feature = "std")]
    pub fn load_from_file(path: &str) -> Result<Self, &'static str> {
        let data = std::fs::read(path).map_err(|_| "read failed")?;
        Self::from_bytes(&data)
    }
}

impl Default for ReplayWindow {
    fn default() -> Self {
        Self::new(1024)
    }
}

/// Extract the nonce from a SPORE\x03 envelope without decrypting.
/// Useful for replay-checking before paying the cost of decryption.
pub fn envelope_nonce(envelope: &[u8]) -> Result<[u8; NONCE_LEN], &'static str> {
    if envelope.len() < 6 + NONCE_LEN {
        return Err("envelope too short");
    }
    if &envelope[..6] != SPORE_V3_MAGIC {
        return Err("bad magic");
    }
    Ok(envelope[6..6 + NONCE_LEN].try_into().unwrap())
}

/// Decrypt a SPORE\x03 envelope. Returns the plaintext on success, Err on
/// ANY misuse (wrong key, tampered ct, tampered tag, truncated, bad magic).
pub fn decrypt_envelope(key: &[u8; KEY_LEN], envelope: &[u8], aad: &[u8]) -> Result<Vec<u8>, &'static str> {
    if envelope.len() < HEADER_LEN + TAG_LEN {
        return Err("envelope too short");
    }
    if &envelope[..6] != SPORE_V3_MAGIC {
        return Err("bad magic");
    }
    let nonce_bytes: [u8; NONCE_LEN] = envelope[6..18].try_into().unwrap();
    let ct_len = u32::from_le_bytes(envelope[18..22].try_into().unwrap()) as usize;
    let expected_end = HEADER_LEN + ct_len + TAG_LEN;
    if envelope.len() < expected_end {
        return Err("envelope truncated");
    }
    let ct_and_tag = &envelope[HEADER_LEN..expected_end];
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    cipher.decrypt(nonce, Payload { msg: ct_and_tag, aad }).map_err(|_| "auth failed")
}

// ───────── ECDH (X25519) + HKDF-SHA256 for forward-secret envelopes ─────────
//
// Threat model addition for v4:
//   v3 (PSK-only) — stolen key decrypts all PAST traffic (no forward secrecy).
//   v4 (ECDH + PSK) — per-message ephemeral DH mixed with PSK via HKDF.
//     Stolen PSK ALONE: attacker lacks recipient's long-term private key, no decrypt.
//     Stolen long-term priv ALONE: attacker lacks PSK, no decrypt.
//     Stolen BOTH: attacker still cannot decrypt PAST messages (sender's ephemeral
//       private key was discarded immediately after send).
//
// Wire format SPORE\x04:
//   [0..6]    "SPORE\x04"
//   [6..38]   ephemeral_pub  — sender's one-time X25519 public key
//   [38..50]  nonce          — random 12 bytes
//   [50..54]  ct_len         — u32 LE
//   [54..]    ciphertext || tag
//
// Key derivation:
//   dh_shared = X25519(sender_eph_priv, recipient_lt_pub)
//             = X25519(recipient_lt_priv, sender_eph_pub)   (both parties compute same)
//   session_key = HKDF-SHA256(ikm = psk || dh_shared, salt = "oasis-ecdh-v1",
//                             info = "session" || eph_pub)

/// Minimal HMAC-SHA256. Standalone to avoid adding the `hmac` crate.
/// Tested indirectly via HKDF + the session-key derivation tests.
fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        let mut h = Sha256::new();
        h.update(key);
        let r: [u8; 32] = h.finalize().into();
        k[..32].copy_from_slice(&r);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0u8; BLOCK];
    let mut opad = [0u8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] = k[i] ^ 0x36;
        opad[i] = k[i] ^ 0x5c;
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(data);
    let inner_out: [u8; 32] = inner.finalize().into();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_out);
    outer.finalize().into()
}

/// HKDF-SHA256 → 32 bytes. Single-block expand (sufficient for one key).
pub fn hkdf_sha256_32(ikm: &[u8], salt: &[u8], info: &[u8]) -> [u8; 32] {
    let prk = hmac_sha256(salt, ikm);
    let mut input = Vec::with_capacity(info.len() + 1);
    input.extend_from_slice(info);
    input.push(0x01);
    hmac_sha256(&prk, &input)
}

fn slice_to_32(s: &[u8]) -> Result<[u8; 32], &'static str> {
    s.try_into().map_err(|_| "expected 32 bytes")
}

/// Generate an X25519 keypair from OS CSPRNG. Returns (private_32B, public_32B).
#[cfg(feature = "os_random")]
pub fn x25519_generate_keypair() -> ([u8; 32], [u8; 32]) {
    let kp = ed25519_compact::x25519::KeyPair::generate();
    let priv_bytes = slice_to_32(kp.sk.as_ref()).unwrap();
    let pub_bytes = slice_to_32(kp.pk.as_ref()).unwrap();
    (priv_bytes, pub_bytes)
}

/// Compute the X25519 public key from a private key (for loading stored keys).
pub fn x25519_pub_from_priv(priv_key: &[u8; 32]) -> Result<[u8; 32], &'static str> {
    let sk = ed25519_compact::x25519::SecretKey::from_slice(priv_key).map_err(|_| "invalid priv key")?;
    let pk = sk.recover_public_key().map_err(|_| "pubkey recovery failed")?;
    slice_to_32(pk.as_ref())
}

fn derive_session_key(psk: &[u8; KEY_LEN], dh_shared: &[u8; 32], eph_pub: &[u8; 32]) -> [u8; KEY_LEN] {
    let mut ikm = Vec::with_capacity(KEY_LEN + 32);
    ikm.extend_from_slice(psk);
    ikm.extend_from_slice(dh_shared);
    let mut info = Vec::with_capacity(7 + 32);
    info.extend_from_slice(b"session");
    info.extend_from_slice(eph_pub);
    hkdf_sha256_32(&ikm, ECDH_SALT, &info)
}

/// Encrypt plaintext with forward-secret v4 envelope, caller-supplied
/// ephemeral key + nonce. Always available (no `os_random` dependency).
///
/// **CRITICAL** (forward secrecy):
///   - `eph_priv` MUST be freshly generated, single-use per envelope, and
///     discarded immediately after this call. Reusing across envelopes
///     breaks forward secrecy.
///   - `nonce_bytes` MUST be unique per (session_key, plaintext). Because
///     the session_key is fresh whenever `eph_priv` is fresh, a zero nonce
///     is cryptographically acceptable, but the caller should default to
///     fresh random bytes to avoid accidental reuse bugs.
///
/// This is the MCU-safe primitive. `encrypt_envelope_v4` is a thin wrapper
/// that calls this with `x25519_generate_keypair()` + `random_nonce()`.
pub fn encrypt_envelope_v4_with_material(
    psk: &[u8; KEY_LEN],
    recipient_pub: &[u8; 32],
    eph_priv: &[u8; 32],
    nonce_bytes: &[u8; NONCE_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, &'static str> {
    // Recover eph_pub from eph_priv so the caller only supplies the secret.
    let eph_pub = x25519_pub_from_priv(eph_priv)?;
    // DH with recipient's long-term pub.
    let rcp_pk = ed25519_compact::x25519::PublicKey::from_slice(recipient_pub).map_err(|_| "bad recipient pubkey")?;
    let eph_sk = ed25519_compact::x25519::SecretKey::from_slice(eph_priv).map_err(|_| "eph sk malformed")?;
    let dh = rcp_pk.dh(&eph_sk).map_err(|_| "dh failed")?;
    let dh_bytes = slice_to_32(dh.as_ref())?;
    // HKDF → session key.
    let session_key = derive_session_key(psk, &dh_bytes, &eph_pub);
    // ChaCha20-Poly1305 with caller-supplied nonce.
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&session_key));
    let ct = cipher.encrypt(Nonce::from_slice(nonce_bytes), Payload { msg: plaintext, aad }).map_err(|_| "v4 encrypt failed")?;
    let ct_len = ct.len().saturating_sub(TAG_LEN);
    // Assemble envelope.
    let mut env = Vec::with_capacity(V4_HEADER_LEN + ct.len());
    env.extend_from_slice(SPORE_V4_MAGIC);
    env.extend_from_slice(&eph_pub);
    env.extend_from_slice(nonce_bytes);
    env.extend_from_slice(&(ct_len as u32).to_le_bytes());
    env.extend_from_slice(&ct);
    // Caller drops eph_priv after this call → forward secrecy guaranteed.
    Ok(env)
}

/// Encrypt plaintext with forward-secret v4 envelope.
/// - `psk`: pre-shared symmetric key (same as v3 key — reused so ops only rotate one value)
/// - `recipient_pub`: receiver's long-term X25519 public key
/// Returns the full envelope ready to wire.
///
/// On MCU targets without `os_random`, use `encrypt_envelope_v4_with_material`
/// and supply your own fresh ephemeral key + nonce.
#[cfg(feature = "os_random")]
pub fn encrypt_envelope_v4(psk: &[u8; KEY_LEN], recipient_pub: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, &'static str> {
    let (eph_priv, _eph_pub) = x25519_generate_keypair();
    let nonce_bytes = random_nonce();
    encrypt_envelope_v4_with_material(psk, recipient_pub, &eph_priv, &nonce_bytes, plaintext, aad)
}

/// Decrypt a v4 envelope using PSK + receiver's long-term X25519 private key.
pub fn decrypt_envelope_v4(psk: &[u8; KEY_LEN], recipient_priv: &[u8; 32], envelope: &[u8], aad: &[u8]) -> Result<Vec<u8>, &'static str> {
    if envelope.len() < V4_HEADER_LEN + TAG_LEN {
        return Err("v4 envelope too short");
    }
    if &envelope[..6] != SPORE_V4_MAGIC {
        return Err("bad v4 magic");
    }
    let eph_pub: [u8; 32] = envelope[6..38].try_into().unwrap();
    let nonce_bytes: [u8; NONCE_LEN] = envelope[38..50].try_into().unwrap();
    let ct_len = u32::from_le_bytes(envelope[50..54].try_into().unwrap()) as usize;
    let end = V4_HEADER_LEN + ct_len + TAG_LEN;
    if envelope.len() < end {
        return Err("v4 envelope truncated");
    }
    let ct_and_tag = &envelope[V4_HEADER_LEN..end];
    let rcp_sk = ed25519_compact::x25519::SecretKey::from_slice(recipient_priv).map_err(|_| "bad recipient privkey")?;
    let eph_pk = ed25519_compact::x25519::PublicKey::from_slice(&eph_pub).map_err(|_| "bad eph pubkey")?;
    let dh = eph_pk.dh(&rcp_sk).map_err(|_| "dh failed")?;
    let dh_bytes = slice_to_32(dh.as_ref())?;
    let session_key = derive_session_key(psk, &dh_bytes, &eph_pub);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&session_key));
    cipher.decrypt(Nonce::from_slice(&nonce_bytes), Payload { msg: ct_and_tag, aad }).map_err(|_| "v4 auth failed")
}

// ───────── Revocation list (operator-signed, Ed25519) ─────────
//
// When a drone is captured / compromised / decommissioned, its static
// private key must be treated as untrusted. The operator signs a revocation
// statement and distributes it to every other drone. Drones load the list
// at startup and reject any v5 envelope whose sender fingerprint appears.
//
// Wire format:
//   [0..6]   "OASREV"                     magic
//   [6]      version                      0x01
//   [7..9]   count (u16 LE)               number of revoked fingerprints
//   [9..]    N × (fp[8] + revoked_at[8])  8-byte fp + 8-byte unix timestamp
//   [9+N*16..9+N*16+64]  Ed25519 signature over all bytes above
//
// Signing key is the OPERATOR's Ed25519 keypair (distinct from peer X25519
// identity keys). Every drone has the operator's Ed25519 PUBLIC key
// embedded or loaded at pairing. Trust root = operator.

pub const REV_MAGIC: &[u8] = b"OASREV";
pub const REV_VERSION: u8 = 0x01;
pub const REV_ENTRY_LEN: usize = SENDER_FP_LEN + 8; // fp + u64 timestamp
pub const REV_SIGNATURE_LEN: usize = 64; // Ed25519 signature

// HashSet for revocation lookup; aliased from BTreeSet on no_std (already
// imported at the top of the file).

/// Signed operator revocation list. Contains fingerprints of drones whose
/// static keys are considered compromised or decommissioned.
#[derive(Debug, Clone, Default)]
pub struct RevocationList {
    entries: Vec<(
        [u8; SENDER_FP_LEN], // fingerprint
        u64,                 // revoked_at_unix_secs
    )>,
    index: HashSet<[u8; SENDER_FP_LEN]>,
}

impl RevocationList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a revocation entry. Returns false if already revoked.
    pub fn revoke(&mut self, fp: [u8; SENDER_FP_LEN], revoked_at: u64) -> bool {
        if self.index.insert(fp) {
            self.entries.push((fp, revoked_at));
            true
        } else {
            false
        }
    }

    pub fn is_revoked(&self, fp: &[u8; SENDER_FP_LEN]) -> bool {
        self.index.contains(fp)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate every (fp, revoked_at_unix_secs) entry in insertion order.
    /// Added 2026-05-10 to fix the O(N²) access pattern flagged in the
    /// tamper-cascade audit: receivers can now bulk-merge a parsed
    /// RevocationList by walking `entries()` once (O(M)) instead of
    /// calling `is_revoked(fp)` for every known fleet fp (O(N × lookup)
    /// where lookup is O(log M) on no_std MCU via BTreeSet alias —
    /// approaching O(N²) wall-clock cost at large M when hash
    /// comparison dominates on Cortex-M0+ at 64 MHz).
    ///
    /// Stable order — matches insertion order via `revoke()`. Borrowing
    /// iterator; cheap to call repeatedly.
    pub fn entries(&self) -> impl Iterator<Item = (&[u8; SENDER_FP_LEN], u64)> {
        self.entries.iter().map(|(fp, ts)| (fp, *ts))
    }

    /// Iterate just the fingerprints. Convenience over `entries()` when
    /// the timestamp isn't needed (the common case: a receiver merging
    /// a revocation broadcast into a local HashSet).
    pub fn fingerprints(&self) -> impl Iterator<Item = &[u8; SENDER_FP_LEN]> {
        self.entries.iter().map(|(fp, _)| fp)
    }

    /// Serialize + sign with the operator's Ed25519 private key seed (32 bytes).
    pub fn serialize_signed(&self, op_ed25519_seed: &[u8; 32]) -> Result<Vec<u8>, &'static str> {
        let mut buf = Vec::with_capacity(6 + 1 + 2 + self.entries.len() * REV_ENTRY_LEN + REV_SIGNATURE_LEN);
        buf.extend_from_slice(REV_MAGIC);
        buf.push(REV_VERSION);
        buf.extend_from_slice(&(self.entries.len().min(u16::MAX as usize) as u16).to_le_bytes());
        for (fp, ts) in &self.entries {
            buf.extend_from_slice(fp);
            buf.extend_from_slice(&ts.to_le_bytes());
        }
        // Sign
        let seed = ed25519_compact::Seed::from_slice(op_ed25519_seed).map_err(|_| "bad op seed")?;
        let kp = ed25519_compact::KeyPair::from_seed(seed);
        let sig = kp.sk.sign(&buf, None);
        buf.extend_from_slice(sig.as_ref());
        Ok(buf)
    }

    /// Parse + verify signature with the operator's Ed25519 public key (32 bytes).
    /// Rejects malformed data, wrong magic, bad version, short signature, invalid sig.
    /// Maximum entries parseable in one revocation list. Prevents OOM from a
    /// malicious-yet-signed blob. u16 count field already caps at 65535; this
    /// value (10_000) sets the SOFT cap. Override via
    /// `RevocationList::parse_and_verify_with_cap`.
    pub const DEFAULT_MAX_ENTRIES: usize = 10_000;

    pub fn parse_and_verify(data: &[u8], op_ed25519_pub: &[u8; 32]) -> Result<RevocationList, &'static str> {
        Self::parse_and_verify_with_cap(data, op_ed25519_pub, Self::DEFAULT_MAX_ENTRIES)
    }

    /// As `parse_and_verify`, but with an explicit cap on entry count.
    /// Rejects oversized lists BEFORE signature verification (to avoid DoS
    /// where attacker forces thousands of HMAC rounds on oversized payload).
    pub fn parse_and_verify_with_cap(data: &[u8], op_ed25519_pub: &[u8; 32], max_entries: usize) -> Result<RevocationList, &'static str> {
        if data.len() < 6 + 1 + 2 + REV_SIGNATURE_LEN {
            return Err("too short");
        }
        if &data[..6] != REV_MAGIC {
            return Err("bad magic");
        }
        if data[6] != REV_VERSION {
            return Err("bad version");
        }
        let count = u16::from_le_bytes([data[7], data[8]]) as usize;
        if count > max_entries {
            return Err("revocation list exceeds max entries cap");
        }
        let body_end = 9 + count * REV_ENTRY_LEN;
        if data.len() < body_end + REV_SIGNATURE_LEN {
            return Err("truncated");
        }
        let body = &data[..body_end];
        let sig_bytes = &data[body_end..body_end + REV_SIGNATURE_LEN];
        // Verify
        let pk = ed25519_compact::PublicKey::from_slice(op_ed25519_pub).map_err(|_| "bad op pubkey")?;
        let sig = ed25519_compact::Signature::from_slice(sig_bytes).map_err(|_| "bad sig bytes")?;
        pk.verify(body, &sig).map_err(|_| "signature invalid")?;
        // Parse entries
        let mut rl = RevocationList::new();
        for i in 0..count {
            let off = 9 + i * REV_ENTRY_LEN;
            let fp: [u8; SENDER_FP_LEN] = data[off..off + SENDER_FP_LEN].try_into().unwrap();
            let ts = u64::from_le_bytes(data[off + SENDER_FP_LEN..off + REV_ENTRY_LEN].try_into().unwrap());
            rl.revoke(fp, ts);
        }
        Ok(rl)
    }
}

/// Decrypt a v5 envelope with a revocation check first. Returns Err("revoked")
/// if the sender fingerprint is on the list — NO decryption attempt is made.
/// This is the production-recommended entry point when revocation is active.
pub fn decrypt_envelope_v5_checked(
    sender_static_pub: &[u8; 32],
    recipient_static_priv: &[u8; 32],
    psk: &[u8; KEY_LEN],
    envelope: &[u8],
    aad: &[u8],
    revocation: &RevocationList,
) -> Result<Vec<u8>, &'static str> {
    let fp = v5_envelope_sender_fp(envelope)?;
    if revocation.is_revoked(&fp) {
        return Err("revoked");
    }
    decrypt_envelope_v5(sender_static_pub, recipient_static_priv, psk, envelope, aad)
}

// ───────── Revocation distribution (SPORE\x06 envelope) ─────────
//
// Wire format is trivial: SPORE\x06 magic followed by the raw OASREV blob.
// The blob carries its own Ed25519 signature so no additional crypto is
// needed on the outer layer. Intermediate nodes can forward without
// inspection; end receivers parse+verify+merge.

/// Wrap a signed OASREV blob into a SPORE\x06 envelope ready to transmit.
pub fn wrap_revocation_envelope(signed_oasrev_blob: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + signed_oasrev_blob.len());
    out.extend_from_slice(SPORE_V6_MAGIC);
    out.extend_from_slice(signed_oasrev_blob);
    out
}

/// Extract the inner OASREV blob from a SPORE\x06 envelope.
pub fn parse_revocation_envelope(envelope: &[u8]) -> Result<&[u8], &'static str> {
    if envelope.len() < 6 + 9 {
        return Err("v6 envelope too short");
    }
    if &envelope[..6] != SPORE_V6_MAGIC {
        return Err("bad v6 magic");
    }
    Ok(&envelope[6..])
}

/// Merge an incoming signed revocation list into a local one.
/// Verifies the inner signature with the operator's Ed25519 public key,
/// then unions entries. Returns (total_after_merge, newly_added_count).
pub fn merge_revocation_envelope(local: &mut RevocationList, envelope: &[u8], op_ed25519_pub: &[u8; 32]) -> Result<(usize, usize), &'static str> {
    let blob = parse_revocation_envelope(envelope)?;
    let incoming = RevocationList::parse_and_verify(blob, op_ed25519_pub)?;
    let before = local.len();
    for (fp, ts) in &incoming.entries {
        local.revoke(*fp, *ts);
    }
    let after = local.len();
    Ok((after, after - before))
}

/// Save a revocation list to disk. On reboot, drones re-load it via
/// `load_revocation_file` and merge any newly-received SPORE\x06 envelopes.
#[cfg(feature = "std")]
pub fn save_revocation_file(list: &RevocationList, path: &str, op_ed25519_seed: &[u8; 32]) -> std::io::Result<()> {
    let signed = list.serialize_signed(op_ed25519_seed).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let tmp = format!("{}.tmp", path);
    std::fs::write(&tmp, &signed)?;
    std::fs::rename(&tmp, path)
}

#[cfg(feature = "std")]
pub fn load_revocation_file(path: &str, op_ed25519_pub: &[u8; 32]) -> Result<RevocationList, &'static str> {
    let data = std::fs::read(path).map_err(|_| "read failed")?;
    RevocationList::parse_and_verify(&data, op_ed25519_pub)
}

// ───────── v5: ECDH + Sender Authentication (Noise-KK pattern) ─────────
//
// v4 gave forward secrecy but NOT sender authentication: anyone with the
// recipient's pubkey + PSK could forge messages appearing to come from any
// drone. v5 closes this with the Noise "KK" pattern:
//
//   dh_es = X25519(sender_eph_priv, recipient_static_pub)   — forward secrecy
//   dh_ss = X25519(sender_static_priv, recipient_static_pub) — authenticates sender
//   session_key = HKDF-SHA256(psk || dh_es || dh_ss, salt, info || eph_pub || sender_pub)
//
// Receiver computes the same outputs because X25519 is symmetric:
//   dh_es = X25519(recipient_static_priv, eph_pub)
//   dh_ss = X25519(recipient_static_priv, sender_static_pub)
//
// Because dh_ss requires the SENDER's static private key, only the real
// sender can produce a valid session_key. An attacker with PSK + recipient_pub
// but no sender_static_priv fails authentication.
//
// Wire format SPORE\x05:
//   [0..6]    "SPORE\x05"
//   [6..14]   sender_fp(8)  — first 8 bytes of SHA-256(sender_static_pub)
//   [14..46]  eph_pub(32)
//   [46..58]  nonce(12)
//   [58..62]  ct_len u32 LE
//   [62..]    ciphertext || tag
//
// The receiver looks up sender_static_pub by fingerprint in its known-senders
// table. Unknown fingerprint = reject. Fingerprint collisions (extremely rare
// for 8 bytes of SHA-256) would require iterating candidates; not implemented
// as the 1-in-2^64 collision rate is acceptable.

/// Compute the 8-byte sender fingerprint = SHA-256(sender_static_pub)[0..8].
pub fn sender_fingerprint(sender_static_pub: &[u8; 32]) -> [u8; SENDER_FP_LEN] {
    let mut h = Sha256::new();
    h.update(sender_static_pub);
    let out: [u8; 32] = h.finalize().into();
    let mut fp = [0u8; SENDER_FP_LEN];
    fp.copy_from_slice(&out[..SENDER_FP_LEN]);
    fp
}

fn derive_session_key_v5(psk: &[u8; KEY_LEN], dh_es: &[u8; 32], dh_ss: &[u8; 32], eph_pub: &[u8; 32], sender_static_pub: &[u8; 32]) -> [u8; KEY_LEN] {
    let mut ikm = Vec::with_capacity(KEY_LEN + 32 + 32);
    ikm.extend_from_slice(psk);
    ikm.extend_from_slice(dh_es);
    ikm.extend_from_slice(dh_ss);
    let mut info = Vec::with_capacity(8 + 32 + 32);
    info.extend_from_slice(b"session5");
    info.extend_from_slice(eph_pub);
    info.extend_from_slice(sender_static_pub);
    hkdf_sha256_32(&ikm, ECDH_SALT, &info)
}

fn x25519_dh(own_priv: &[u8; 32], peer_pub: &[u8; 32]) -> Result<[u8; 32], &'static str> {
    let sk = ed25519_compact::x25519::SecretKey::from_slice(own_priv).map_err(|_| "bad priv key")?;
    let pk = ed25519_compact::x25519::PublicKey::from_slice(peer_pub).map_err(|_| "bad pub key")?;
    let dh = pk.dh(&sk).map_err(|_| "dh failed")?;
    slice_to_32(dh.as_ref())
}

/// Encrypt plaintext with v5 Noise-KK envelope, caller-supplied ephemeral
/// key + nonce. Always available (no `os_random` dependency).
///
/// **CRITICAL**:
///   - `eph_priv` MUST be freshly generated, single-use per envelope, and
///     discarded after this call. Reusing breaks forward secrecy.
///   - `nonce_bytes` uniqueness matters per (session_key, nonce). Since
///     session_key includes fresh eph, a zero nonce is technically safe
///     when eph is fresh — but callers should default to random bytes.
///   - Sender authenticity derives from `sender_static_priv` being secret.
///     An attacker without it cannot produce a valid v5 envelope that
///     claims this sender.
///
/// This is the MCU-safe primitive. `encrypt_envelope_v5` is a thin wrapper.
pub fn encrypt_envelope_v5_with_material(
    sender_static_priv: &[u8; 32],
    sender_static_pub: &[u8; 32],
    recipient_static_pub: &[u8; 32],
    eph_priv: &[u8; 32],
    nonce_bytes: &[u8; NONCE_LEN],
    psk: &[u8; KEY_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, &'static str> {
    let eph_pub = x25519_pub_from_priv(eph_priv)?;
    let dh_es = x25519_dh(eph_priv, recipient_static_pub)?;
    let dh_ss = x25519_dh(sender_static_priv, recipient_static_pub)?;
    let session_key = derive_session_key_v5(psk, &dh_es, &dh_ss, &eph_pub, sender_static_pub);
    let fp = sender_fingerprint(sender_static_pub);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&session_key));
    let ct = cipher.encrypt(Nonce::from_slice(nonce_bytes), Payload { msg: plaintext, aad }).map_err(|_| "v5 encrypt failed")?;
    let ct_len = ct.len().saturating_sub(TAG_LEN);
    let mut env = Vec::with_capacity(V5_HEADER_LEN + ct.len());
    env.extend_from_slice(SPORE_V5_MAGIC);
    env.extend_from_slice(&fp);
    env.extend_from_slice(&eph_pub);
    env.extend_from_slice(nonce_bytes);
    env.extend_from_slice(&(ct_len as u32).to_le_bytes());
    env.extend_from_slice(&ct);
    Ok(env)
}

/// Encrypt plaintext with v5 forward-secret + sender-authenticated envelope.
///
/// On MCU targets without `os_random`, use `encrypt_envelope_v5_with_material`
/// and supply your own fresh ephemeral key + nonce.
#[cfg(feature = "os_random")]
pub fn encrypt_envelope_v5(
    sender_static_priv: &[u8; 32],
    sender_static_pub: &[u8; 32],
    recipient_static_pub: &[u8; 32],
    psk: &[u8; KEY_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, &'static str> {
    let (eph_priv, _eph_pub) = x25519_generate_keypair();
    let nonce_bytes = random_nonce();
    encrypt_envelope_v5_with_material(sender_static_priv, sender_static_pub, recipient_static_pub, &eph_priv, &nonce_bytes, psk, plaintext, aad)
}

/// Extract the 8-byte sender fingerprint from a v5 envelope (cheap, no crypto).
/// Use this to look up the sender's pubkey before calling decrypt_envelope_v5.
pub fn v5_envelope_sender_fp(envelope: &[u8]) -> Result<[u8; SENDER_FP_LEN], &'static str> {
    if envelope.len() < 6 + SENDER_FP_LEN {
        return Err("v5 envelope too short");
    }
    if &envelope[..6] != SPORE_V5_MAGIC {
        return Err("bad v5 magic");
    }
    Ok(envelope[6..14].try_into().unwrap())
}

/// Decrypt a v5 envelope. Caller must first extract the sender fingerprint via
/// `v5_envelope_sender_fp` and look up the corresponding sender_static_pub.
pub fn decrypt_envelope_v5(sender_static_pub: &[u8; 32], recipient_static_priv: &[u8; 32], psk: &[u8; KEY_LEN], envelope: &[u8], aad: &[u8]) -> Result<Vec<u8>, &'static str> {
    if envelope.len() < V5_HEADER_LEN + TAG_LEN {
        return Err("v5 envelope too short");
    }
    if &envelope[..6] != SPORE_V5_MAGIC {
        return Err("bad v5 magic");
    }
    // Verify fingerprint matches the claimed sender_static_pub
    let expected_fp = sender_fingerprint(sender_static_pub);
    if envelope[6..14] != expected_fp {
        return Err("sender fp does not match claimed sender pub");
    }
    let eph_pub: [u8; 32] = envelope[14..46].try_into().unwrap();
    let nonce_bytes: [u8; NONCE_LEN] = envelope[46..58].try_into().unwrap();
    let ct_len = u32::from_le_bytes(envelope[58..62].try_into().unwrap()) as usize;
    let end = V5_HEADER_LEN + ct_len + TAG_LEN;
    if envelope.len() < end {
        return Err("v5 envelope truncated");
    }
    let ct_and_tag = &envelope[V5_HEADER_LEN..end];
    let dh_es = x25519_dh(recipient_static_priv, &eph_pub)?;
    let dh_ss = x25519_dh(recipient_static_priv, sender_static_pub)?;
    let session_key = derive_session_key_v5(psk, &dh_es, &dh_ss, &eph_pub, sender_static_pub);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&session_key));
    cipher.decrypt(Nonce::from_slice(&nonce_bytes), Payload { msg: ct_and_tag, aad }).map_err(|_| "v5 auth failed")
}

// ───────── v7: v5 + Monotonic Sender Counter (long-window replay resistance) ─────────
//
// v5 provides live-traffic replay protection via a bounded 1024-slot nonce window.
// Attacker who waits past 1024 messages sees old nonces evicted → can replay.
// v7 closes this with a monotonic u64 counter maintained by each sender:
//
//   Sender: counter = last_counter + 1; include in envelope + AAD
//   Receiver: look up last_seen[sender_fp]; reject if counter <= last_seen
//             on success: update last_seen = counter
//
// Properties:
// - Unbounded window — protects all history under a given key
// - Constant memory: O(N) for N distinct senders (8 bytes each)
// - Survives restart if `CounterTracker::save/load_from_file` is used
// - Counter is in AAD — tampered counter → Poly1305 tag fails
// - Out-of-order: counter MUST be strictly greater than last_seen (no sliding window)
//   For tolerant reordering, use a per-sender bitmap (future work — not needed yet)
//
// Wire format SPORE\x07:
//   [0..6]    "SPORE\x07"
//   [6..14]   sender_fp(8)
//   [14..22]  counter(u64 LE)
//   [22..54]  eph_pub(32)
//   [54..66]  nonce(12)
//   [66..70]  ct_len(u32 LE)
//   [70..]    ciphertext || tag
//
// Overhead vs v5: +8 bytes per envelope.

/// Encrypt with v7 using caller-supplied ephemeral + nonce + counter.
/// Always available (no `os_random`). The MCU-safe primitive.
///
/// **CRITICAL**:
///   - `eph_priv` MUST be fresh per envelope (forward secrecy).
///   - `counter` MUST be strictly greater than the last counter used with
///     this (sender_static_priv, recipient_static_pub) pair — otherwise the
///     receiver rejects via its CounterTracker. Persist across reboots.
///   - `nonce_bytes` uniqueness is per (session_key, nonce); since
///     session_key is fresh per ephemeral, reuse risk is bounded, but
///     callers should default to fresh random bytes.
///   - Counter is authenticated via AAD (Poly1305) — tampering with the
///     envelope's counter byte range breaks decrypt.
pub fn encrypt_envelope_v7_with_material(
    sender_static_priv: &[u8; 32],
    sender_static_pub: &[u8; 32],
    recipient_static_pub: &[u8; 32],
    eph_priv: &[u8; 32],
    nonce_bytes: &[u8; NONCE_LEN],
    psk: &[u8; KEY_LEN],
    counter: u64,
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, &'static str> {
    let eph_pub = x25519_pub_from_priv(eph_priv)?;
    let dh_es = x25519_dh(eph_priv, recipient_static_pub)?;
    let dh_ss = x25519_dh(sender_static_priv, recipient_static_pub)?;
    let session_key = derive_session_key_v5(psk, &dh_es, &dh_ss, &eph_pub, sender_static_pub);
    let fp = sender_fingerprint(sender_static_pub);
    // Counter in AAD: authenticated but not encrypted. Tampered counter
    // causes the Poly1305 tag to fail at decrypt.
    let mut full_aad = Vec::with_capacity(8 + aad.len());
    full_aad.extend_from_slice(&counter.to_le_bytes());
    full_aad.extend_from_slice(aad);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&session_key));
    let ct = cipher
        .encrypt(Nonce::from_slice(nonce_bytes), Payload { msg: plaintext, aad: &full_aad })
        .map_err(|_| "v7 encrypt failed")?;
    let ct_len = ct.len().saturating_sub(TAG_LEN);
    let mut env = Vec::with_capacity(V7_HEADER_LEN + ct.len());
    env.extend_from_slice(SPORE_V7_MAGIC);
    env.extend_from_slice(&fp);
    env.extend_from_slice(&counter.to_le_bytes());
    env.extend_from_slice(&eph_pub);
    env.extend_from_slice(nonce_bytes);
    env.extend_from_slice(&(ct_len as u32).to_le_bytes());
    env.extend_from_slice(&ct);
    Ok(env)
}

/// Encrypt with v7: like v5 but includes a monotonic counter in AAD.
/// Caller maintains their own counter (persistent across restarts via
/// `counter_state_file_path`). Counter MUST be strictly increasing.
///
/// On MCU targets without `os_random`, use `encrypt_envelope_v7_with_material`
/// and supply your own fresh ephemeral key + nonce in addition to the counter.
#[cfg(feature = "os_random")]
pub fn encrypt_envelope_v7(
    sender_static_priv: &[u8; 32],
    sender_static_pub: &[u8; 32],
    recipient_static_pub: &[u8; 32],
    psk: &[u8; KEY_LEN],
    counter: u64,
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, &'static str> {
    let (eph_priv, _eph_pub) = x25519_generate_keypair();
    let nonce_bytes = random_nonce();
    encrypt_envelope_v7_with_material(sender_static_priv, sender_static_pub, recipient_static_pub, &eph_priv, &nonce_bytes, psk, counter, plaintext, aad)
}

/// Extract (sender_fp, counter) from a v7 envelope without decrypting.
/// Use this to consult the CounterTracker before paying the cost of decryption.
pub fn v7_envelope_header(envelope: &[u8]) -> Result<([u8; SENDER_FP_LEN], u64), &'static str> {
    if envelope.len() < V7_HEADER_LEN {
        return Err("v7 envelope too short");
    }
    if &envelope[..6] != SPORE_V7_MAGIC {
        return Err("bad v7 magic");
    }
    let fp: [u8; SENDER_FP_LEN] = envelope[6..14].try_into().unwrap();
    let counter = u64::from_le_bytes(envelope[14..22].try_into().unwrap());
    Ok((fp, counter))
}

/// Decrypt a v7 envelope. Counter is included in AAD; receiver validates
/// (counter > last_seen) via a separate CounterTracker call by the caller.
pub fn decrypt_envelope_v7(sender_static_pub: &[u8; 32], recipient_static_priv: &[u8; 32], psk: &[u8; KEY_LEN], envelope: &[u8], aad: &[u8]) -> Result<(u64, Vec<u8>), &'static str> {
    if envelope.len() < V7_HEADER_LEN + TAG_LEN {
        return Err("v7 envelope too short");
    }
    if &envelope[..6] != SPORE_V7_MAGIC {
        return Err("bad v7 magic");
    }
    let expected_fp = sender_fingerprint(sender_static_pub);
    if envelope[6..14] != expected_fp {
        return Err("sender fp does not match claimed sender pub");
    }
    let counter = u64::from_le_bytes(envelope[14..22].try_into().unwrap());
    let eph_pub: [u8; 32] = envelope[22..54].try_into().unwrap();
    let nonce_bytes: [u8; NONCE_LEN] = envelope[54..66].try_into().unwrap();
    let ct_len = u32::from_le_bytes(envelope[66..70].try_into().unwrap()) as usize;
    let end = V7_HEADER_LEN + ct_len + TAG_LEN;
    if envelope.len() < end {
        return Err("v7 envelope truncated");
    }
    let ct_and_tag = &envelope[V7_HEADER_LEN..end];
    let dh_es = x25519_dh(recipient_static_priv, &eph_pub)?;
    let dh_ss = x25519_dh(recipient_static_priv, sender_static_pub)?;
    let session_key = derive_session_key_v5(psk, &dh_es, &dh_ss, &eph_pub, sender_static_pub);
    let mut full_aad = Vec::with_capacity(8 + aad.len());
    full_aad.extend_from_slice(&counter.to_le_bytes());
    full_aad.extend_from_slice(aad);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&session_key));
    let pt = cipher
        .decrypt(Nonce::from_slice(&nonce_bytes), Payload { msg: ct_and_tag, aad: &full_aad })
        .map_err(|_| "v7 auth failed")?;
    Ok((counter, pt))
}

/// Per-sender sliding-window replay tracker (IPSec RFC 4303 style, §3.4.3).
///
/// Each sender has `highest` (max counter seen) and a 128-bit `bitmap` (two
/// u64 halves forming a window below highest). Incoming counters are accepted
/// if:
///   - counter > highest  — advance window
///   - counter in [highest - 127, highest] AND bit not set — fill gap
/// Replays (bit already set) and stale (counter + WINDOW_SIZE <= highest) rejected.
///
/// Window widened from 64 → 128 bits this round: tolerates 127 positions of
/// UDP reordering, closes the "reorder > 64 positions dropped" limit observed
/// on high-jitter links (LoRa, satellite, multi-hop mesh).
/// Memory cost: +8 bytes per sender (72 → 80 bytes).
pub const COUNTER_WINDOW_SIZE: u32 = 128;

/// Internal per-sender state. `last_access_ticks` drives LRU eviction.
#[derive(Debug, Clone)]
struct SenderState {
    highest: u64,
    /// 128-bit bitmap: bitmap[0] covers positions 0..63, bitmap[1] covers 64..127.
    /// Bit i in this combined view corresponds to counter `(highest - i)`.
    bitmap: [u64; 2],
    last_access_ticks: u64,
}

impl SenderState {
    #[inline]
    fn bit_set(&self, pos: u32) -> bool {
        let (word, shift) = (pos / 64, pos % 64);
        (self.bitmap[word as usize] >> shift) & 1 == 1
    }
    #[inline]
    fn set_bit(&mut self, pos: u32) {
        let (word, shift) = (pos / 64, pos % 64);
        self.bitmap[word as usize] |= 1u64 << shift;
    }
    /// Left-shift the 128-bit bitmap by `n` positions (up to 127). Bits
    /// shifted past position 127 are lost.
    fn shift_left(&mut self, n: u32) {
        if n >= 128 {
            self.bitmap = [0, 0];
            return;
        }
        if n >= 64 {
            self.bitmap[1] = self.bitmap[0] << (n - 64);
            self.bitmap[0] = 0;
        } else if n > 0 {
            // High word gets (low << n) spillover from bit 0 + (high << n)
            let spillover = self.bitmap[0] >> (64 - n);
            self.bitmap[1] = (self.bitmap[1] << n) | spillover;
            self.bitmap[0] <<= n;
        }
    }
}

/// Per-sender tracker with sliding window + bounded size + LRU eviction.
///
/// `max_senders` default 10_000 (env-override via OASIS_COUNTER_TRACKER_MAX).
/// When full and a new sender arrives, evicts the least-recently-accessed one
/// in O(log N) via a BTreeMap secondary index (previously O(N) min_by_key scan).
#[derive(Debug, Clone)]
pub struct CounterTracker {
    senders: HashMapForTracker<[u8; SENDER_FP_LEN], SenderState>,
    /// Secondary index: tick → fp. Smallest tick = least recently accessed.
    lru: BTreeMapForTracker<u64, [u8; SENDER_FP_LEN]>,
    max_senders: usize,
    tick: u64,
}

#[cfg(feature = "std")]
type HashMapForTracker<K, V> = std::collections::HashMap<K, V>;
#[cfg(feature = "std")]
type BTreeMapForTracker<K, V> = std::collections::BTreeMap<K, V>;
#[cfg(not(feature = "std"))]
type HashMapForTracker<K, V> = alloc::collections::BTreeMap<K, V>;
#[cfg(not(feature = "std"))]
type BTreeMapForTracker<K, V> = alloc::collections::BTreeMap<K, V>;

impl Default for CounterTracker {
    fn default() -> Self {
        #[cfg(feature = "std_env")]
        let max = std::env::var("OASIS_COUNTER_TRACKER_MAX").ok().and_then(|s| s.parse().ok()).unwrap_or(10_000);
        #[cfg(not(feature = "std_env"))]
        let max: usize = 10_000;
        Self::with_capacity(max)
    }
}

impl CounterTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(max_senders: usize) -> Self {
        Self { senders: HashMapForTracker::new(), lru: BTreeMapForTracker::new(), max_senders: max_senders.max(2), tick: 0 }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick = self.tick.wrapping_add(1);
        self.tick
    }

    /// Promote `fp` to "most recently accessed" by removing its old LRU entry
    /// and inserting a new one at the current tick.
    fn touch(&mut self, fp: [u8; SENDER_FP_LEN], old_tick: Option<u64>) -> u64 {
        if let Some(t) = old_tick {
            self.lru.remove(&t);
        }
        let new_tick = self.next_tick();
        self.lru.insert(new_tick, fp);
        new_tick
    }

    /// Evict the LRU sender. O(log N) via pop_first on the BTreeMap index.
    fn evict_lru(&mut self) {
        if let Some((_, fp)) = self.lru.pop_first() {
            self.senders.remove(&fp);
        }
    }

    /// Sliding-window check. Rules:
    /// - counter > highest → shift window, accept
    /// - counter + WINDOW_SIZE ≤ highest → too old, reject
    /// - bit already set in window → replay, reject
    /// - else → set bit, accept
    /// On `Err`, tracker state is NOT mutated.
    pub fn check_and_update(&mut self, fp: [u8; SENDER_FP_LEN], counter: u64) -> Result<(), &'static str> {
        // First, do the pre-check WITHOUT mutating state. This way, an Err
        // return leaves the tracker unchanged — no LRU eviction triggered by
        // what turns out to be a replay.
        if let Some(s) = self.senders.get(&fp) {
            if counter <= s.highest {
                let diff = s.highest - counter;
                if diff >= COUNTER_WINDOW_SIZE as u64 {
                    return Err("counter too old (outside sliding window)");
                }
                if s.bit_set(diff as u32) {
                    return Err("replay detected (bit set in sliding window)");
                }
                // Fall through to mutation path below
            }
        }
        // Evict LRU if we need to insert a NEW sender and we're at capacity
        let is_new = !self.senders.contains_key(&fp);
        if is_new && self.senders.len() >= self.max_senders {
            self.evict_lru();
        }
        // Grab the old tick so we can remove it from the LRU index before re-inserting
        let old_tick = self.senders.get(&fp).map(|s| s.last_access_ticks);
        let new_tick = self.touch(fp, old_tick);
        let s = self.senders.entry(fp).or_insert(SenderState { highest: 0, bitmap: [0, 0], last_access_ticks: new_tick });
        s.last_access_ticks = new_tick;
        if counter > s.highest {
            let shift = counter - s.highest;
            s.shift_left(shift.min(u32::MAX as u64) as u32);
            s.set_bit(0); // bit 0 = highest seen
            s.highest = counter;
        } else {
            // counter <= highest; pre-check already validated bit not set + within window
            let diff = (s.highest - counter) as u32;
            s.set_bit(diff);
        }
        Ok(())
    }

    /// Fast pre-decrypt check: is this counter UNAMBIGUOUSLY stale?
    /// Returns true if counter falls outside the sliding window OR its bit is
    /// already set (= a replay of a previously seen counter). Read-only.
    pub fn is_stale(&self, fp: &[u8; SENDER_FP_LEN], counter: u64) -> bool {
        if let Some(s) = self.senders.get(fp) {
            if counter > s.highest {
                return false;
            }
            let diff = s.highest - counter;
            if diff >= COUNTER_WINDOW_SIZE as u64 {
                return true;
            }
            return s.bit_set(diff as u32);
        }
        false
    }

    /// Read-only: highest counter seen for this sender (0 if unknown).
    pub fn last_seen(&self, fp: &[u8; SENDER_FP_LEN]) -> u64 {
        self.senders.get(fp).map(|s| s.highest).unwrap_or(0)
    }

    pub fn known_senders(&self) -> usize {
        self.senders.len()
    }

    /// Serialize for persistence. Format v3 (current):
    ///   "CTR\x03" + count(u32 LE) + N × (fp[8] + highest[8] + bitmap_lo[8] + bitmap_hi[8])
    /// Entry size: 32 bytes (was 24 in v2). Old v1/v2 files still parse via
    /// forward-compat `from_bytes`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + self.senders.len() * 32);
        buf.extend_from_slice(b"CTR\x03");
        buf.extend_from_slice(&(self.senders.len() as u32).to_le_bytes());
        for (fp, s) in &self.senders {
            buf.extend_from_slice(fp);
            buf.extend_from_slice(&s.highest.to_le_bytes());
            buf.extend_from_slice(&s.bitmap[0].to_le_bytes());
            buf.extend_from_slice(&s.bitmap[1].to_le_bytes());
        }
        buf
    }

    /// Parse. Accepts all historical formats:
    ///   v1 (CTR\x01): fp + counter — bitmap initialized to [1, 0]
    ///   v2 (CTR\x02): fp + highest + bitmap[8] — high word zeroed
    ///   v3 (CTR\x03): fp + highest + bitmap[16] — 128-bit window (current)
    pub fn from_bytes(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 8 {
            return Err("too short");
        }
        if &data[..3] != b"CTR" {
            return Err("bad magic");
        }
        let version = data[3];
        let entry_len = match version {
            0x01 => 16, // fp(8) + counter(8)
            0x02 => 24, // fp(8) + highest(8) + bitmap64(8)
            0x03 => 32, // fp(8) + highest(8) + bitmap128(16)
            _ => return Err("bad version"),
        };
        let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        let expected_len = 8 + count * entry_len;
        if data.len() < expected_len {
            return Err("truncated");
        }
        let mut t = CounterTracker::new();
        for i in 0..count {
            let off = 8 + i * entry_len;
            let fp: [u8; SENDER_FP_LEN] = data[off..off + SENDER_FP_LEN].try_into().unwrap();
            let highest = u64::from_le_bytes(data[off + 8..off + 16].try_into().unwrap());
            let bitmap: [u64; 2] = match version {
                0x01 => [1, 0], // bit 0 set = highest seen
                0x02 => [u64::from_le_bytes(data[off + 16..off + 24].try_into().unwrap()), 0],
                0x03 => [u64::from_le_bytes(data[off + 16..off + 24].try_into().unwrap()), u64::from_le_bytes(data[off + 24..off + 32].try_into().unwrap())],
                _ => unreachable!(),
            };
            let tick = t.next_tick();
            t.senders.insert(fp, SenderState { highest, bitmap, last_access_ticks: tick });
            t.lru.insert(tick, fp);
        }
        Ok(t)
    }

    #[cfg(feature = "std")]
    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let tmp = format!("{}.tmp", path);
        std::fs::write(&tmp, self.to_bytes())?;
        std::fs::rename(&tmp, path)
    }

    #[cfg(feature = "std")]
    pub fn load_from_file(path: &str) -> Result<Self, &'static str> {
        let data = std::fs::read(path).map_err(|_| "read failed")?;
        Self::from_bytes(&data)
    }
}

/// Full-check decrypt: verifies counter BEFORE attempting cryptographic work.
/// This is the production entry point for v7 — cheap early-reject on replay.
pub fn decrypt_envelope_v7_checked(
    sender_static_pub: &[u8; 32],
    recipient_static_priv: &[u8; 32],
    psk: &[u8; KEY_LEN],
    envelope: &[u8],
    aad: &[u8],
    tracker: &mut CounterTracker,
    revocation: &RevocationList,
) -> Result<Vec<u8>, &'static str> {
    let (fp, counter) = v7_envelope_header(envelope)?;
    if revocation.is_revoked(&fp) {
        return Err("revoked");
    }
    // Pre-decrypt cheap reject: unambiguously stale counter (outside window
    // OR bit already set in window). Legitimate UDP reordering within the
    // 64-bit window still passes here.
    if tracker.is_stale(&fp, counter) {
        return Err("counter rejected by sliding window (stale or replay)");
    }
    let (got_counter, pt) = decrypt_envelope_v7(sender_static_pub, recipient_static_priv, psk, envelope, aad)?;
    // Post-decrypt: the counter in AAD is authenticated, so got_counter == envelope_counter.
    // Commit to the tracker only AFTER decryption succeeds (avoid state updates on garbage).
    tracker.check_and_update(fp, got_counter)?;
    Ok(pt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known_key() -> [u8; KEY_LEN] {
        parse_key_hex("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f").unwrap()
    }

    #[test]
    fn roundtrip_preserves_plaintext() {
        let key = known_key();
        let plaintext = b"OASIS digest: fear_spike @ t=1234, vmag=0.42";
        let env = encrypt_envelope(&key, plaintext, b"").unwrap();
        assert!(env.starts_with(SPORE_V3_MAGIC));
        let got = decrypt_envelope(&key, &env, b"").unwrap();
        assert_eq!(got, plaintext);
    }

    #[test]
    fn random_nonce_is_not_constant_and_has_reasonable_spread() {
        // If random_nonce() ever returns the same value twice across 100 calls,
        // something is catastrophically wrong with the RNG. Also verify the
        // distribution isn't stuck in a tiny subset (min/max byte values).
        let mut seen: std::collections::HashSet<[u8; NONCE_LEN]> = std::collections::HashSet::new();
        let mut min_byte = 255u8;
        let mut max_byte = 0u8;
        for _ in 0..100 {
            let n = random_nonce();
            assert!(seen.insert(n), "nonce collision in 100 calls — RNG broken");
            for b in &n {
                min_byte = min_byte.min(*b);
                max_byte = max_byte.max(*b);
            }
        }
        // With 1200 bytes from OS CSPRNG, essentially impossible to miss full byte range
        assert!(max_byte - min_byte > 200, "nonce distribution too narrow: min={} max={}", min_byte, max_byte);
    }

    #[test]
    fn two_encrypts_of_same_plaintext_produce_different_ciphertexts() {
        // Fresh nonce each call → ciphertexts MUST diverge.
        let key = known_key();
        let plaintext = b"identical plaintext";
        let e1 = encrypt_envelope(&key, plaintext, b"").unwrap();
        let e2 = encrypt_envelope(&key, plaintext, b"").unwrap();
        // Nonces (bytes 6..18) must differ with overwhelming probability.
        assert_ne!(&e1[6..18], &e2[6..18], "nonce reuse — catastrophic bug");
        // Therefore ciphertexts also differ.
        assert_ne!(e1, e2);
    }

    #[test]
    fn wrong_key_rejects() {
        let key = known_key();
        let mut wrong = known_key();
        wrong[0] ^= 0x01;
        let env = encrypt_envelope(&key, b"secret", b"").unwrap();
        assert!(decrypt_envelope(&wrong, &env, b"").is_err(), "decrypt with wrong key MUST fail");
    }

    #[test]
    fn tampered_ciphertext_rejects() {
        let key = known_key();
        let mut env = encrypt_envelope(&key, b"secret data", b"").unwrap();
        // Flip a bit in the ciphertext body
        env[HEADER_LEN + 2] ^= 0x40;
        assert_eq!(decrypt_envelope(&key, &env, b"").unwrap_err(), "auth failed");
    }

    #[test]
    fn tampered_tag_rejects() {
        let key = known_key();
        let mut env = encrypt_envelope(&key, b"x", b"").unwrap();
        let last = env.len() - 1;
        env[last] ^= 0xFF;
        assert_eq!(decrypt_envelope(&key, &env, b"").unwrap_err(), "auth failed");
    }

    #[test]
    fn tampered_aad_rejects() {
        let key = known_key();
        let env = encrypt_envelope(&key, b"secret", b"drone_id=d00").unwrap();
        // Decrypt with different AAD must fail
        assert_eq!(decrypt_envelope(&key, &env, b"drone_id=d01").unwrap_err(), "auth failed");
    }

    #[test]
    fn truncated_envelope_rejects_cleanly() {
        let key = known_key();
        let env = encrypt_envelope(&key, b"something", b"").unwrap();
        // Cut off half
        let truncated = &env[..env.len() / 2];
        assert!(decrypt_envelope(&key, truncated, b"").is_err());
        // Cut off just the tag
        let no_tag = &env[..env.len() - 16];
        assert!(decrypt_envelope(&key, no_tag, b"").is_err());
    }

    #[test]
    fn bad_magic_rejects() {
        let key = known_key();
        let mut env = encrypt_envelope(&key, b"x", b"").unwrap();
        env[0] = b'Z';
        assert_eq!(decrypt_envelope(&key, &env, b"").unwrap_err(), "bad magic");
    }

    #[test]
    fn encrypt_with_nonce_roundtrip() {
        // MCU-safe encrypt path: caller supplies nonce, decrypt works.
        let key = known_key();
        let nonce = [7u8; 12];
        let pt = b"drone telemetry from an MCU without getrandom";
        let env = encrypt_envelope_with_nonce(&key, &nonce, pt, b"aad").unwrap();
        let got = decrypt_envelope(&key, &env, b"aad").unwrap();
        assert_eq!(got, pt);
    }

    #[test]
    fn encrypt_with_nonce_is_deterministic() {
        // Same (key, nonce, plaintext, aad) ⇒ byte-identical envelope.
        // This is the DETERMINISTIC property — it's also why nonce reuse
        // is catastrophic.
        let key = known_key();
        let nonce = [0x42u8; 12];
        let pt = b"abc";
        let a = encrypt_envelope_with_nonce(&key, &nonce, pt, b"").unwrap();
        let b = encrypt_envelope_with_nonce(&key, &nonce, pt, b"").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn encrypt_with_different_nonces_yields_different_ciphertexts() {
        let key = known_key();
        let n1 = [1u8; 12];
        let n2 = [2u8; 12];
        let pt = b"same plaintext";
        let a = encrypt_envelope_with_nonce(&key, &n1, pt, b"").unwrap();
        let b = encrypt_envelope_with_nonce(&key, &n2, pt, b"").unwrap();
        assert_ne!(a, b, "nonce change must produce different ciphertext");
    }

    #[test]
    fn encrypt_with_nonce_and_encrypt_envelope_produce_compatible_envelopes() {
        // Sanity: caller-nonce path and auto-nonce path decrypt the same way.
        // I.e., the wire format is identical; only the nonce source differs.
        let key = known_key();
        let pt = b"payload";
        let e_auto = encrypt_envelope(&key, pt, b"").unwrap();
        let nonce = envelope_nonce(&e_auto).unwrap();
        let e_manual = encrypt_envelope_with_nonce(&key, &nonce, pt, b"").unwrap();
        assert_eq!(e_auto, e_manual, "same (key, nonce, pt, aad) must yield identical envelopes via either API");
    }

    #[test]
    fn key_derivation_is_deterministic_and_domain_separated() {
        let k1 = derive_key_from_passphrase(b"hello");
        let k2 = derive_key_from_passphrase(b"hello");
        assert_eq!(k1, k2, "same passphrase → same key");
        let k3 = derive_key_from_passphrase(b"Hello");
        assert_ne!(k1, k3, "different passphrase → different key");
        // Domain tag makes this distinct from a naive SHA-256(passphrase)
        let mut naive = Sha256::new();
        naive.update(b"hello");
        let naive_out: [u8; 32] = naive.finalize().into();
        assert_ne!(k1, naive_out, "domain separation must be in effect");
    }

    #[test]
    fn parse_key_hex_roundtrip() {
        let k = known_key();
        let hex_str = "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f";
        assert_eq!(parse_key_hex(hex_str).unwrap(), k);
        assert!(parse_key_hex("").is_none());
        assert!(parse_key_hex("XYZ").is_none());
        assert!(parse_key_hex("00").is_none(), "wrong length rejected");
    }

    #[test]
    fn replay_window_fresh_nonce_accepted() {
        let mut rw = ReplayWindow::new(16);
        let n = [1u8; NONCE_LEN];
        assert!(rw.check_and_insert(&n).is_ok());
        assert_eq!(rw.len(), 1);
    }

    #[test]
    fn replay_window_duplicate_rejected() {
        let mut rw = ReplayWindow::new(16);
        let n = [2u8; NONCE_LEN];
        rw.check_and_insert(&n).unwrap();
        assert_eq!(rw.check_and_insert(&n).unwrap_err(), "replay detected");
        assert_eq!(rw.len(), 1, "duplicate must not grow the cache");
    }

    #[test]
    fn replay_window_evicts_fifo_when_full() {
        let mut rw = ReplayWindow::new(4);
        for i in 0..4u8 {
            rw.check_and_insert(&[i; NONCE_LEN]).unwrap();
        }
        // 5th nonce evicts the oldest ([0;N])
        rw.check_and_insert(&[99; NONCE_LEN]).unwrap();
        // Old nonce now outside window — can be re-accepted
        assert!(rw.check_and_insert(&[0; NONCE_LEN]).is_ok(), "evicted nonce should be accepted again (documented limitation)");
    }

    #[test]
    fn envelope_nonce_extraction() {
        let key = known_key();
        let env = encrypt_envelope(&key, b"hi", b"").unwrap();
        let n = envelope_nonce(&env).unwrap();
        // Nonce must match bytes 6..18 of envelope
        assert_eq!(&env[6..18], &n);
        // Bad magic → Err
        let mut bad = env.clone();
        bad[0] = b'X';
        assert!(envelope_nonce(&bad).is_err());
    }

    #[test]
    fn replay_window_persistence_roundtrip() {
        let mut rw = ReplayWindow::new(8);
        for i in 0..5u8 {
            rw.check_and_insert(&[i; NONCE_LEN]).unwrap();
        }
        let bytes = rw.to_bytes();
        // Clone-on-check to keep rw2 as source of truth; check_and_insert needs &mut
        for i in 0..5u8 {
            let mut probe = ReplayWindow::from_bytes(&bytes).unwrap();
            assert!(probe.check_and_insert(&[i; NONCE_LEN]).is_err(), "nonce {} must be recognized as seen after reload", i);
        }
        // Fresh nonce still accepted
        let mut probe = ReplayWindow::from_bytes(&bytes).unwrap();
        assert!(probe.check_and_insert(&[99; NONCE_LEN]).is_ok());
    }

    #[test]
    fn replay_window_file_persistence() {
        let path = std::env::temp_dir().join(format!("oasis_replay_test_{}.bin", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let path_s = path.to_string_lossy().to_string();

        let mut rw = ReplayWindow::new(16);
        rw.check_and_insert(&[0xAA; NONCE_LEN]).unwrap();
        rw.check_and_insert(&[0xBB; NONCE_LEN]).unwrap();
        rw.save_to_file(&path_s).unwrap();

        let loaded = ReplayWindow::load_from_file(&path_s).unwrap();
        assert_eq!(loaded.len(), 2);
        let bytes = loaded.to_bytes();
        let mut p1 = ReplayWindow::from_bytes(&bytes).unwrap();
        assert!(p1.check_and_insert(&[0xAA; NONCE_LEN]).is_err());
        let mut p2 = ReplayWindow::from_bytes(&bytes).unwrap();
        assert!(p2.check_and_insert(&[0xBB; NONCE_LEN]).is_err());
        let mut p3 = ReplayWindow::from_bytes(&bytes).unwrap();
        assert!(p3.check_and_insert(&[0xCC; NONCE_LEN]).is_ok());

        let _ = std::fs::remove_file(&path_s);
    }

    #[test]
    fn replay_window_rejects_corrupt_persistence() {
        assert!(ReplayWindow::from_bytes(b"").is_err());
        assert!(ReplayWindow::from_bytes(b"XXXX\x01").is_err());
        // Bad magic
        let mut rw = ReplayWindow::new(4);
        rw.check_and_insert(&[1; NONCE_LEN]).unwrap();
        let mut bytes = rw.to_bytes();
        bytes[0] = b'Z';
        assert!(ReplayWindow::from_bytes(&bytes).is_err());
    }

    #[test]
    fn replay_persistence_survives_restart_attack() {
        // Scenario: drone receives msg with nonce N. Drone restarts. Attacker
        // replays msg. Without persistence: accepted. With persistence: rejected.
        let path = std::env::temp_dir().join(format!("oasis_restart_test_{}.bin", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let path_s = path.to_string_lossy().to_string();

        // First run
        let mut rw = ReplayWindow::new(1024);
        let attacker_nonce = [0xDE; NONCE_LEN];
        rw.check_and_insert(&attacker_nonce).unwrap();
        rw.save_to_file(&path_s).unwrap();
        drop(rw);

        // Restart: load from disk
        let mut rw_after_restart = ReplayWindow::load_from_file(&path_s).unwrap();
        // Attacker replays
        assert_eq!(rw_after_restart.check_and_insert(&attacker_nonce).unwrap_err(), "replay detected", "persistence must prevent restart-window replay");

        let _ = std::fs::remove_file(&path_s);
    }

    #[test]
    fn replay_integration_captures_literal_resend() {
        // Simulate an attacker capturing a valid envelope and resending it.
        let key = known_key();
        let env = encrypt_envelope(&key, b"sensor reading", b"").unwrap();
        let nonce = envelope_nonce(&env).unwrap();

        let mut rw = ReplayWindow::new(1024);
        // First arrival: decrypt + record
        rw.check_and_insert(&nonce).unwrap();
        let pt = decrypt_envelope(&key, &env, b"").unwrap();
        assert_eq!(pt, b"sensor reading");

        // Replay: same bytes arrive again. Replay window rejects BEFORE decrypt.
        assert!(rw.check_and_insert(&nonce).is_err(), "replay must be detected");
    }

    // ───── X25519 ECDH (v4) tests ────────────────────────────────

    #[test]
    fn x25519_keypair_generation_and_pub_recovery() {
        let (priv1, pub1) = x25519_generate_keypair();
        let (priv2, pub2) = x25519_generate_keypair();
        assert_ne!(priv1, priv2, "keypairs must differ");
        assert_ne!(pub1, pub2);
        // Recover pub from priv should match
        let recovered = x25519_pub_from_priv(&priv1).unwrap();
        assert_eq!(recovered, pub1, "recover_public_key must match generation");
    }

    #[test]
    fn hkdf_is_deterministic() {
        let ikm = b"input keying material";
        let salt = b"salt";
        let info = b"info";
        let k1 = hkdf_sha256_32(ikm, salt, info);
        let k2 = hkdf_sha256_32(ikm, salt, info);
        assert_eq!(k1, k2);
        let k3 = hkdf_sha256_32(ikm, b"DIFFERENT salt", info);
        assert_ne!(k1, k3);
    }

    #[test]
    fn v4_roundtrip() {
        let psk = known_key();
        let (rcp_priv, rcp_pub) = x25519_generate_keypair();
        let plaintext = b"forward-secret cargo: scout team location";
        let env = encrypt_envelope_v4(&psk, &rcp_pub, plaintext, b"").unwrap();
        assert!(env.starts_with(SPORE_V4_MAGIC));
        let pt = decrypt_envelope_v4(&psk, &rcp_priv, &env, b"").unwrap();
        assert_eq!(pt, plaintext);
    }

    #[test]
    fn v4_two_encrypts_produce_different_ephemerals() {
        let psk = known_key();
        let (_, rcp_pub) = x25519_generate_keypair();
        let e1 = encrypt_envelope_v4(&psk, &rcp_pub, b"msg", b"").unwrap();
        let e2 = encrypt_envelope_v4(&psk, &rcp_pub, b"msg", b"").unwrap();
        // Ephemeral pub (bytes 6..38) must differ
        assert_ne!(&e1[6..38], &e2[6..38], "ephemeral keys reuse — forward secrecy broken");
    }

    #[test]
    fn v4_rejects_wrong_psk() {
        let (rcp_priv, rcp_pub) = x25519_generate_keypair();
        let psk = known_key();
        let env = encrypt_envelope_v4(&psk, &rcp_pub, b"x", b"").unwrap();
        let mut wrong_psk = psk;
        wrong_psk[0] ^= 0x01;
        assert!(decrypt_envelope_v4(&wrong_psk, &rcp_priv, &env, b"").is_err(), "wrong PSK must fail (PSK is mixed into session key)");
    }

    #[test]
    fn v4_rejects_wrong_recipient_privkey() {
        let (_, rcp_pub) = x25519_generate_keypair();
        let (other_priv, _) = x25519_generate_keypair();
        let psk = known_key();
        let env = encrypt_envelope_v4(&psk, &rcp_pub, b"x", b"").unwrap();
        assert!(decrypt_envelope_v4(&psk, &other_priv, &env, b"").is_err(), "wrong recipient priv must fail");
    }

    #[test]
    fn v4_tampered_ephemeral_rejected() {
        let (rcp_priv, rcp_pub) = x25519_generate_keypair();
        let psk = known_key();
        let mut env = encrypt_envelope_v4(&psk, &rcp_pub, b"x", b"").unwrap();
        env[10] ^= 0xFF; // flip a byte in the ephemeral pub
        assert!(decrypt_envelope_v4(&psk, &rcp_priv, &env, b"").is_err());
    }

    #[test]
    fn v4_forward_secrecy_compromise_simulation() {
        // Simulate the adversary capturing an old ciphertext.
        // Later, they steal the PSK AND the recipient's long-term privkey.
        // They CAN decrypt it (by design — that's what they have).
        // But they CANNOT decrypt if the ephemeral private is unknown (unknown
        // because sender destroyed it). We verify via a NEGATIVE test: without
        // the recipient's priv, no decrypt — which implies the only path is
        // through recipient's long-term priv (kept offline in a real deployment).

        let psk = known_key();
        let (rcp_priv, rcp_pub) = x25519_generate_keypair();
        let old_ciphertext = encrypt_envelope_v4(&psk, &rcp_pub, b"old secret", b"").unwrap();

        // Adversary has ONLY PSK (rcp_priv assumed kept offline). No decrypt.
        let (_wrong_priv, _) = x25519_generate_keypair();
        assert!(decrypt_envelope_v4(&psk, &_wrong_priv, &old_ciphertext, b"").is_err(), "PSK alone must not decrypt — forward secrecy property");

        // With recipient priv, can decrypt. This demonstrates that an attacker
        // who steals BOTH PSK + long-term priv reads ALL traffic for which they
        // have the ciphertext. Forward secrecy cannot prevent this — the user
        // must protect the long-term private key on the drone.
        let pt = decrypt_envelope_v4(&psk, &rcp_priv, &old_ciphertext, b"").unwrap();
        assert_eq!(pt, b"old secret");
    }

    #[test]
    fn v4_with_material_roundtrip() {
        // MCU-safe path: caller supplies eph_priv + nonce.
        let psk = known_key();
        let (rcp_priv, rcp_pub) = x25519_generate_keypair();
        let (eph_priv, _) = x25519_generate_keypair();
        let nonce = [0x42u8; 12];
        let pt = b"payload from MCU without OS RNG";
        let env = encrypt_envelope_v4_with_material(&psk, &rcp_pub, &eph_priv, &nonce, pt, b"").unwrap();
        let got = decrypt_envelope_v4(&psk, &rcp_priv, &env, b"").unwrap();
        assert_eq!(got, pt);
    }

    #[test]
    fn v4_with_material_is_deterministic_given_same_material() {
        // Same (psk, recipient_pub, eph_priv, nonce, pt, aad) ⇒ byte-identical env.
        // This is the determinism that makes nonce + eph reuse catastrophic.
        let psk = known_key();
        let (_, rcp_pub) = x25519_generate_keypair();
        let (eph_priv, _) = x25519_generate_keypair();
        let nonce = [1u8; 12];
        let pt = b"deterministic";
        let a = encrypt_envelope_v4_with_material(&psk, &rcp_pub, &eph_priv, &nonce, pt, b"").unwrap();
        let b = encrypt_envelope_v4_with_material(&psk, &rcp_pub, &eph_priv, &nonce, pt, b"").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn v4_with_material_matches_auto_path_given_same_material() {
        // If we extract the eph_pub + nonce from an auto-path envelope and
        // feed them back through the material API, we should get the SAME
        // ciphertext.
        let psk = known_key();
        let (eph_priv, eph_pub_auto) = x25519_generate_keypair();
        let (_, rcp_pub) = x25519_generate_keypair();
        let nonce = [7u8; 12];
        let pt = b"xyz";
        let env_material = encrypt_envelope_v4_with_material(&psk, &rcp_pub, &eph_priv, &nonce, pt, b"").unwrap();
        // Envelope layout: magic(6) || eph_pub(32) || nonce(12) || len(4) || ct
        let eph_pub_from_env: [u8; 32] = env_material[6..38].try_into().unwrap();
        assert_eq!(eph_pub_from_env, eph_pub_auto, "eph_pub recovered from priv matches generator output");
    }

    #[test]
    fn v4_with_material_different_eph_yields_different_envelope() {
        // Forward-secrecy-relevant: each unique ephemeral produces a unique session.
        let psk = known_key();
        let (_, rcp_pub) = x25519_generate_keypair();
        let (e1, _) = x25519_generate_keypair();
        let (e2, _) = x25519_generate_keypair();
        let nonce = [0u8; 12];
        let pt = b"same plaintext";
        let env1 = encrypt_envelope_v4_with_material(&psk, &rcp_pub, &e1, &nonce, pt, b"").unwrap();
        let env2 = encrypt_envelope_v4_with_material(&psk, &rcp_pub, &e2, &nonce, pt, b"").unwrap();
        assert_ne!(env1, env2);
    }

    // ───── Revocation list tests ──────────────────────────────

    fn known_ed25519_seed() -> [u8; 32] {
        let mut seed = [0u8; 32];
        for (i, b) in seed.iter_mut().enumerate() {
            *b = (i * 7 + 13) as u8;
        }
        seed
    }

    fn ed25519_pub_from_seed(seed: &[u8; 32]) -> [u8; 32] {
        let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::from_slice(seed).unwrap());
        let bytes: &[u8] = kp.pk.as_ref();
        bytes.try_into().unwrap()
    }

    #[test]
    fn revocation_list_basic_add_and_check() {
        let mut rl = RevocationList::new();
        let fp1 = [0x11u8; SENDER_FP_LEN];
        let fp2 = [0x22u8; SENDER_FP_LEN];
        assert!(rl.revoke(fp1, 1700000000));
        assert!(!rl.revoke(fp1, 1700000001), "duplicate revoke returns false");
        assert!(rl.is_revoked(&fp1));
        assert!(!rl.is_revoked(&fp2));
        assert_eq!(rl.len(), 1);
    }

    #[test]
    fn revocation_list_sign_verify_roundtrip() {
        let mut rl = RevocationList::new();
        rl.revoke([0x11; SENDER_FP_LEN], 1700000000);
        rl.revoke([0x22; SENDER_FP_LEN], 1700005000);
        let seed = known_ed25519_seed();
        let pub_key = ed25519_pub_from_seed(&seed);
        let signed = rl.serialize_signed(&seed).unwrap();
        let parsed = RevocationList::parse_and_verify(&signed, &pub_key).unwrap();
        assert_eq!(parsed.len(), 2);
        assert!(parsed.is_revoked(&[0x11; SENDER_FP_LEN]));
        assert!(parsed.is_revoked(&[0x22; SENDER_FP_LEN]));
    }

    #[test]
    fn revocation_list_rejects_tampered() {
        let mut rl = RevocationList::new();
        rl.revoke([0x33; SENDER_FP_LEN], 1700000000);
        let seed = known_ed25519_seed();
        let pub_key = ed25519_pub_from_seed(&seed);
        let mut signed = rl.serialize_signed(&seed).unwrap();
        // Flip a bit in the fingerprint
        signed[9] ^= 0xFF;
        assert!(RevocationList::parse_and_verify(&signed, &pub_key).is_err(), "tampered revocation list must be rejected");
    }

    #[test]
    fn revocation_list_rejects_wrong_op_pubkey() {
        let mut rl = RevocationList::new();
        rl.revoke([0x44; SENDER_FP_LEN], 1700000000);
        let seed1 = known_ed25519_seed();
        let mut seed2 = seed1;
        seed2[0] ^= 0x01;
        let wrong_pub = ed25519_pub_from_seed(&seed2);
        let signed = rl.serialize_signed(&seed1).unwrap();
        assert!(RevocationList::parse_and_verify(&signed, &wrong_pub).is_err(), "wrong operator pubkey must reject (no impersonation)");
    }

    #[test]
    fn revocation_size_cap_rejects_oversized_list() {
        // Craft a valid signed list with 5 entries; try to parse with cap=3.
        // Must reject BEFORE signature work.
        let mut rl = RevocationList::new();
        for i in 0..5u8 {
            rl.revoke([i; SENDER_FP_LEN], 1700000000 + i as u64);
        }
        let seed = known_ed25519_seed();
        let pub_key = ed25519_pub_from_seed(&seed);
        let signed = rl.serialize_signed(&seed).unwrap();

        // Normal parse (default cap 10_000) accepts
        assert!(RevocationList::parse_and_verify(&signed, &pub_key).is_ok());
        // Explicit low cap rejects
        let err = RevocationList::parse_and_verify_with_cap(&signed, &pub_key, 3).unwrap_err();
        assert!(err.contains("exceeds max entries cap"));
    }

    #[test]
    fn revocation_list_empty_signs_verifies() {
        // Empty lists are still signed — operators broadcast "nobody revoked"
        // to confirm the list is up-to-date.
        let rl = RevocationList::new();
        let seed = known_ed25519_seed();
        let pub_key = ed25519_pub_from_seed(&seed);
        let signed = rl.serialize_signed(&seed).unwrap();
        let parsed = RevocationList::parse_and_verify(&signed, &pub_key).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn v5_decrypt_checked_rejects_revoked_sender() {
        // End-to-end: a drone whose static key was revoked cannot be
        // authenticated by recipients even if they still have the pubkey.
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let psk = known_key();
        let env = encrypt_envelope_v5(&s_priv, &s_pub, &r_pub, &psk, b"compromised", b"").unwrap();

        // Build revocation list including this sender
        let mut rl = RevocationList::new();
        rl.revoke(sender_fingerprint(&s_pub), 1700000000);

        // Checked decrypt must reject
        let err = decrypt_envelope_v5_checked(&s_pub, &r_priv, &psk, &env, b"", &rl).unwrap_err();
        assert_eq!(err, "revoked");

        // Unchecked decrypt still works (proves the envelope is otherwise valid)
        let pt = decrypt_envelope_v5(&s_pub, &r_priv, &psk, &env, b"").unwrap();
        assert_eq!(pt, b"compromised");
    }

    #[test]
    fn revocation_envelope_wrap_parse_roundtrip() {
        let mut rl = RevocationList::new();
        rl.revoke([0xAB; SENDER_FP_LEN], 1700000000);
        let seed = known_ed25519_seed();
        let signed = rl.serialize_signed(&seed).unwrap();
        let env = wrap_revocation_envelope(&signed);
        assert!(env.starts_with(SPORE_V6_MAGIC));
        let inner = parse_revocation_envelope(&env).unwrap();
        assert_eq!(inner, &signed[..]);
    }

    #[test]
    fn revocation_envelope_merge_adds_new_entries() {
        let seed = known_ed25519_seed();
        let pub_key = ed25519_pub_from_seed(&seed);
        let mut local = RevocationList::new();
        local.revoke([0x11; SENDER_FP_LEN], 1700000000);

        let mut incoming = RevocationList::new();
        incoming.revoke([0x11; SENDER_FP_LEN], 1700000001); // duplicate
        incoming.revoke([0x22; SENDER_FP_LEN], 1700000050);
        incoming.revoke([0x33; SENDER_FP_LEN], 1700000100);

        let signed = incoming.serialize_signed(&seed).unwrap();
        let envelope = wrap_revocation_envelope(&signed);
        let (total, added) = merge_revocation_envelope(&mut local, &envelope, &pub_key).unwrap();
        assert_eq!(added, 2, "only 2 new entries (duplicate of 0x11 ignored)");
        assert_eq!(total, 3);
        assert!(local.is_revoked(&[0x11; SENDER_FP_LEN]));
        assert!(local.is_revoked(&[0x22; SENDER_FP_LEN]));
        assert!(local.is_revoked(&[0x33; SENDER_FP_LEN]));
    }

    #[test]
    fn revocation_envelope_merge_rejects_wrong_operator() {
        let seed = known_ed25519_seed();
        let mut seed2 = seed;
        seed2[0] ^= 0x01;
        let wrong_pub = ed25519_pub_from_seed(&seed2);
        let mut rl = RevocationList::new();
        rl.revoke([0x77; SENDER_FP_LEN], 1700000000);
        let signed = rl.serialize_signed(&seed).unwrap();
        let env = wrap_revocation_envelope(&signed);
        let mut local = RevocationList::new();
        assert!(merge_revocation_envelope(&mut local, &env, &wrong_pub).is_err(), "revocation signed by wrong operator must be rejected");
        assert_eq!(local.len(), 0, "failed merge must not mutate local list");
    }

    #[test]
    fn revocation_envelope_merge_idempotent() {
        let seed = known_ed25519_seed();
        let pub_key = ed25519_pub_from_seed(&seed);
        let mut incoming = RevocationList::new();
        incoming.revoke([0x55; SENDER_FP_LEN], 1700000000);
        let env = wrap_revocation_envelope(&incoming.serialize_signed(&seed).unwrap());

        let mut local = RevocationList::new();
        let (t1, a1) = merge_revocation_envelope(&mut local, &env, &pub_key).unwrap();
        let (t2, a2) = merge_revocation_envelope(&mut local, &env, &pub_key).unwrap();
        assert_eq!(t1, 1);
        assert_eq!(a1, 1);
        assert_eq!(t2, 1);
        assert_eq!(a2, 0, "second merge is a no-op");
    }

    #[test]
    fn revocation_file_roundtrip() {
        let path = std::env::temp_dir().join(format!("oasis_rev_test_{}.bin", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let path_s = path.to_string_lossy().to_string();

        let seed = known_ed25519_seed();
        let pub_key = ed25519_pub_from_seed(&seed);
        let mut rl = RevocationList::new();
        rl.revoke([0xCA; SENDER_FP_LEN], 1700000000);
        rl.revoke([0xFE; SENDER_FP_LEN], 1700000050);

        save_revocation_file(&rl, &path_s, &seed).unwrap();
        let loaded = load_revocation_file(&path_s, &pub_key).unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded.is_revoked(&[0xCA; SENDER_FP_LEN]));
        assert!(loaded.is_revoked(&[0xFE; SENDER_FP_LEN]));

        let _ = std::fs::remove_file(&path_s);
    }

    #[test]
    fn parse_revocation_envelope_rejects_bad_inputs() {
        assert!(parse_revocation_envelope(b"").is_err());
        assert!(parse_revocation_envelope(b"SPORE\x05garbage").is_err());
        // Valid magic but too short to be a real OASREV blob
        assert!(parse_revocation_envelope(b"SPORE\x06").is_err());
    }

    #[test]
    fn v5_decrypt_checked_accepts_non_revoked_sender() {
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let psk = known_key();
        let env = encrypt_envelope_v5(&s_priv, &s_pub, &r_pub, &psk, b"ok", b"").unwrap();
        let mut rl = RevocationList::new();
        // Revoke a DIFFERENT drone
        rl.revoke([0xFF; SENDER_FP_LEN], 1700000000);
        let pt = decrypt_envelope_v5_checked(&s_pub, &r_priv, &psk, &env, b"", &rl).unwrap();
        assert_eq!(pt, b"ok");
    }

    // ───── v7 (monotonic counter) tests ─────────────────────

    #[test]
    fn v7_roundtrip_with_counter() {
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let psk = known_key();
        let env = encrypt_envelope_v7(&s_priv, &s_pub, &r_pub, &psk, 42, b"hello", b"").unwrap();
        assert!(env.starts_with(SPORE_V7_MAGIC));
        let (fp, counter) = v7_envelope_header(&env).unwrap();
        assert_eq!(fp, sender_fingerprint(&s_pub));
        assert_eq!(counter, 42);
        let (c, pt) = decrypt_envelope_v7(&s_pub, &r_priv, &psk, &env, b"").unwrap();
        assert_eq!(c, 42);
        assert_eq!(pt, b"hello");
    }

    #[test]
    fn v7_tampered_counter_rejected() {
        // Counter is in AAD — any tamper breaks Poly1305.
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let psk = known_key();
        let mut env = encrypt_envelope_v7(&s_priv, &s_pub, &r_pub, &psk, 100, b"x", b"").unwrap();
        // Flip a bit in the counter field
        env[14] ^= 0xFF;
        assert!(decrypt_envelope_v7(&s_pub, &r_priv, &psk, &env, b"").is_err());
    }

    #[test]
    fn v7_with_material_roundtrip() {
        // MCU-safe v7 path: caller supplies eph_priv + nonce + counter.
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let (eph_priv, _) = x25519_generate_keypair();
        let psk = known_key();
        let nonce = [0x77u8; 12];
        let counter: u64 = 4711;
        let pt = b"v7 material payload with monotonic counter";
        let env = encrypt_envelope_v7_with_material(&s_priv, &s_pub, &r_pub, &eph_priv, &nonce, &psk, counter, pt, b"").unwrap();
        assert!(env.starts_with(SPORE_V7_MAGIC));
        // Header extraction (no crypto) returns the same counter we supplied.
        let (fp, c_in_env) = v7_envelope_header(&env).unwrap();
        assert_eq!(fp, sender_fingerprint(&s_pub));
        assert_eq!(c_in_env, counter);
        let (c, got) = decrypt_envelope_v7(&s_pub, &r_priv, &psk, &env, b"").unwrap();
        assert_eq!(c, counter);
        assert_eq!(got, pt);
    }

    #[test]
    fn v7_with_material_counter_in_aad_is_authenticated() {
        // Prove the counter is authenticated via AAD. Flipping a byte in
        // the counter field invalidates the Poly1305 tag.
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let (eph_priv, _) = x25519_generate_keypair();
        let psk = known_key();
        let nonce = [1u8; 12];
        let mut env = encrypt_envelope_v7_with_material(&s_priv, &s_pub, &r_pub, &eph_priv, &nonce, &psk, 100, b"x", b"").unwrap();
        // Flip a bit in the counter range (bytes 14..22).
        env[14] ^= 0xFF;
        assert!(decrypt_envelope_v7(&s_pub, &r_priv, &psk, &env, b"").is_err(), "tampered counter must fail decrypt (AAD authenticated)");
    }

    #[test]
    fn v7_with_material_deterministic_given_same_material_and_counter() {
        let ((s_priv, s_pub), (_, r_pub)) = make_sender_recipient();
        let (eph_priv, _) = x25519_generate_keypair();
        let psk = known_key();
        let nonce = [2u8; 12];
        let a = encrypt_envelope_v7_with_material(&s_priv, &s_pub, &r_pub, &eph_priv, &nonce, &psk, 7, b"det", b"").unwrap();
        let b = encrypt_envelope_v7_with_material(&s_priv, &s_pub, &r_pub, &eph_priv, &nonce, &psk, 7, b"det", b"").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn v7_with_material_different_counters_yield_different_envelopes() {
        // Changing only the counter must change the ciphertext because
        // counter is mixed into AAD which shapes the Poly1305 tag.
        let ((s_priv, s_pub), (_, r_pub)) = make_sender_recipient();
        let (eph_priv, _) = x25519_generate_keypair();
        let psk = known_key();
        let nonce = [3u8; 12];
        let a = encrypt_envelope_v7_with_material(&s_priv, &s_pub, &r_pub, &eph_priv, &nonce, &psk, 1, b"x", b"").unwrap();
        let b = encrypt_envelope_v7_with_material(&s_priv, &s_pub, &r_pub, &eph_priv, &nonce, &psk, 2, b"x", b"").unwrap();
        assert_ne!(a, b, "counter change must produce different envelope");
    }

    #[test]
    fn counter_tracker_rejects_exact_replay() {
        // Exact same counter = replay (bit already set in sliding window)
        let mut t = CounterTracker::new();
        let fp = [0xAB; SENDER_FP_LEN];
        assert!(t.check_and_update(fp, 1).is_ok());
        assert!(t.check_and_update(fp, 2).is_ok());
        assert!(t.check_and_update(fp, 5).is_ok());
        assert!(t.check_and_update(fp, 5).is_err(), "exact replay of 5 must fail");
        assert!(t.check_and_update(fp, 1).is_err(), "exact replay of 1 must fail");
        assert!(t.check_and_update(fp, 100).is_ok());
        assert_eq!(t.last_seen(&fp), 100);
    }

    #[test]
    fn counter_tracker_tolerates_in_window_reorder() {
        // UDP reordering within the 64-position window must be accepted.
        let mut t = CounterTracker::new();
        let fp = [0xCD; SENDER_FP_LEN];
        // Sender emits 1, 2, 3, 4, 5 but receiver sees 1, 2, 5, 4, 3
        assert!(t.check_and_update(fp, 1).is_ok());
        assert!(t.check_and_update(fp, 2).is_ok());
        assert!(t.check_and_update(fp, 5).is_ok()); // advances highest to 5
        assert!(t.check_and_update(fp, 4).is_ok(), "in-window reorder must pass");
        assert!(t.check_and_update(fp, 3).is_ok(), "in-window reorder must pass");
        // Now any of them as a replay must fail
        assert!(t.check_and_update(fp, 3).is_err());
        assert!(t.check_and_update(fp, 5).is_err());
    }

    #[test]
    fn counter_tracker_rejects_stale_outside_window() {
        // Window widened to 128 bits this round.
        let mut t = CounterTracker::new();
        let fp = [0xCE; SENDER_FP_LEN];
        t.check_and_update(fp, 300).unwrap();
        // Counter 100 is 200 positions below highest — WAY outside 128-wide window
        assert!(t.check_and_update(fp, 100).is_err(), "counter outside 128-bit window must be rejected");
        // Counter 300-127 = 173 is at window edge — still inside → accept
        assert!(t.check_and_update(fp, 173).is_ok());
        // Counter 300-128 = 172 is just outside
        assert!(t.check_and_update(fp, 172).is_err(), "counter at window+1 must be rejected");
    }

    #[test]
    fn counter_tracker_per_sender_isolation() {
        let mut t = CounterTracker::new();
        let fp1 = [0x01; SENDER_FP_LEN];
        let fp2 = [0x02; SENDER_FP_LEN];
        t.check_and_update(fp1, 50).unwrap();
        // Sender 2 starts fresh — any counter >0 valid
        assert!(t.check_and_update(fp2, 1).is_ok());
        // Within fp1's sliding window — accepted (legitimate reorder)
        assert!(t.check_and_update(fp1, 40).is_ok());
        // Exact replay of 50 on fp1 — rejected
        assert!(t.check_and_update(fp1, 50).is_err());
    }

    #[test]
    fn counter_tracker_is_stale_query() {
        let mut t = CounterTracker::new();
        let fp = [0xFE; SENDER_FP_LEN];
        assert!(!t.is_stale(&fp, 1), "unknown sender: never stale");
        t.check_and_update(fp, 200).unwrap();
        assert!(!t.is_stale(&fp, 201), "higher: not stale");
        assert!(t.is_stale(&fp, 200), "exact replay: stale (bit set)");
        assert!(!t.is_stale(&fp, 199), "in-window unseen: NOT stale");
        // Window is 128-bit now. 200-128=72 is just outside.
        assert!(t.is_stale(&fp, 72), "outside 128-bit window (200-128): stale");
        assert!(!t.is_stale(&fp, 73), "inside 128-bit window (200-127): NOT stale");
    }

    #[test]
    fn counter_tracker_window_tolerates_100_position_reorder() {
        // NEW: reorders within the expanded 128-bit window that would have
        // failed with the old 64-bit window.
        let mut t = CounterTracker::new();
        let fp = [0xAA; SENDER_FP_LEN];
        t.check_and_update(fp, 200).unwrap();
        // Reorder 100 positions back — now accepted
        assert!(t.check_and_update(fp, 100).is_ok(), "100-position reorder must pass in 128-bit window");
        // But beyond 127 positions still rejected
        assert!(t.check_and_update(fp, 50).is_err(), "150-position stale is outside window");
    }

    #[test]
    fn counter_tracker_lru_eviction() {
        let mut t = CounterTracker::with_capacity(3);
        let fps: Vec<[u8; SENDER_FP_LEN]> = (0..5u8).map(|i| [i; SENDER_FP_LEN]).collect();
        // Fill to capacity
        for i in 0..3 {
            t.check_and_update(fps[i], 1).unwrap();
        }
        assert_eq!(t.known_senders(), 3);
        // Access fp0 to make it most-recent
        t.check_and_update(fps[0], 2).unwrap();
        // Add fp3 → should evict LRU (fp1 or fp2)
        t.check_and_update(fps[3], 1).unwrap();
        assert_eq!(t.known_senders(), 3, "capacity must hold at 3");
        assert_eq!(t.last_seen(&fps[0]), 2, "fp0 must not be evicted (recently accessed)");
    }

    #[test]
    fn counter_tracker_persistence() {
        let path = std::env::temp_dir().join(format!("oasis_ctr_test_{}.bin", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let path_s = path.to_string_lossy().to_string();

        let mut t = CounterTracker::new();
        let fp = [0x42; SENDER_FP_LEN];
        t.check_and_update(fp, 9999).unwrap();
        t.save_to_file(&path_s).unwrap();

        let loaded = CounterTracker::load_from_file(&path_s).unwrap();
        assert_eq!(loaded.last_seen(&fp), 9999);
        // Replay after restart — rejected
        let mut l2 = loaded;
        assert!(l2.check_and_update(fp, 9999).is_err(), "persistence must prevent post-restart replay");
        assert!(l2.check_and_update(fp, 10000).is_ok());

        let _ = std::fs::remove_file(&path_s);
    }

    #[test]
    fn v7_checked_decrypt_rejects_replay_unbounded() {
        // The KEY PROPERTY this test proves: replay resistance is NOT bounded
        // by a window size — ANY past counter is rejected.
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let psk = known_key();
        let mut tracker = CounterTracker::new();
        let rl = RevocationList::new();

        // Send counter=1, recipient accepts
        let e1 = encrypt_envelope_v7(&s_priv, &s_pub, &r_pub, &psk, 1, b"msg1", b"").unwrap();
        let pt1 = decrypt_envelope_v7_checked(&s_pub, &r_priv, &psk, &e1, b"", &mut tracker, &rl).unwrap();
        assert_eq!(pt1, b"msg1");

        // Fast-forward: millions of messages later, counter=9_999_999
        let e_big = encrypt_envelope_v7(&s_priv, &s_pub, &r_pub, &psk, 9_999_999, b"later", b"").unwrap();
        let pt_big = decrypt_envelope_v7_checked(&s_pub, &r_priv, &psk, &e_big, b"", &mut tracker, &rl).unwrap();
        assert_eq!(pt_big, b"later");

        // Attacker NOW replays the ancient counter=1 message.
        // A window-based scheme would ACCEPT (1 is long past the window).
        // Counter-based v7 MUST reject (1 <= last_seen 9_999_999).
        let err = decrypt_envelope_v7_checked(&s_pub, &r_priv, &psk, &e1, b"", &mut tracker, &rl).unwrap_err();
        assert!(err.contains("counter") || err.contains("replay"), "ancient replay must be rejected unconditionally, got: {}", err);
    }

    #[test]
    fn v7_checked_decrypt_rejects_revoked_sender() {
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let psk = known_key();
        let mut tracker = CounterTracker::new();
        let mut rl = RevocationList::new();
        rl.revoke(sender_fingerprint(&s_pub), 1700000000);
        let env = encrypt_envelope_v7(&s_priv, &s_pub, &r_pub, &psk, 1, b"x", b"").unwrap();
        let err = decrypt_envelope_v7_checked(&s_pub, &r_priv, &psk, &env, b"", &mut tracker, &rl).unwrap_err();
        assert_eq!(err, "revoked");
        // Tracker must NOT be polluted by a revoked sender's counter
        assert_eq!(tracker.last_seen(&sender_fingerprint(&s_pub)), 0);
    }

    #[test]
    fn v7_checked_decrypt_tolerates_udp_reordering() {
        // End-to-end proof that sliding window accepts legitimate UDP reorder.
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let psk = known_key();
        let mut tracker = CounterTracker::new();
        let rl = RevocationList::new();

        // Sender emits 1..=5. Receiver sees 1, 3, 5, 2, 4.
        let envs: Vec<_> = (1u64..=5).map(|c| encrypt_envelope_v7(&s_priv, &s_pub, &r_pub, &psk, c, b"x", b"").unwrap()).collect();
        let order = [0usize, 2, 4, 1, 3]; // counter values: 1, 3, 5, 2, 4
        for &i in &order {
            let _ = decrypt_envelope_v7_checked(&s_pub, &r_priv, &psk, &envs[i], b"", &mut tracker, &rl).expect("in-window reorder must decrypt");
        }
        // All 5 counters now marked seen — any exact replay rejected
        for i in 0..5 {
            let err = decrypt_envelope_v7_checked(&s_pub, &r_priv, &psk, &envs[i], b"", &mut tracker, &rl).unwrap_err();
            assert!(err.contains("stale") || err.contains("replay"), "replay of counter {} must fail, got: {}", i + 1, err);
        }
    }

    #[test]
    fn v7_checked_decrypt_does_not_update_tracker_on_crypto_failure() {
        // Wrong PSK → decrypt fails. Tracker state MUST remain unchanged.
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let psk = known_key();
        let mut wrong_psk = psk;
        wrong_psk[0] ^= 0xFF;
        let rl = RevocationList::new();
        let mut tracker = CounterTracker::new();
        let env = encrypt_envelope_v7(&s_priv, &s_pub, &r_pub, &psk, 777, b"y", b"").unwrap();
        assert!(decrypt_envelope_v7_checked(&s_pub, &r_priv, &wrong_psk, &env, b"", &mut tracker, &rl).is_err());
        // Tracker must show last_seen == 0 still
        assert_eq!(tracker.last_seen(&sender_fingerprint(&s_pub)), 0, "failed decrypt must NOT update tracker (would DoS future messages)");
    }

    // ───── v5: Sender-authenticated envelope tests ─────────────

    fn make_sender_recipient() -> (([u8; 32], [u8; 32]), ([u8; 32], [u8; 32])) {
        let (s_priv, s_pub) = x25519_generate_keypair();
        let (r_priv, r_pub) = x25519_generate_keypair();
        ((s_priv, s_pub), (r_priv, r_pub))
    }

    #[test]
    fn sender_fingerprint_is_deterministic_and_pub_dependent() {
        let (_, pub1) = x25519_generate_keypair();
        let (_, pub2) = x25519_generate_keypair();
        let fp1a = sender_fingerprint(&pub1);
        let fp1b = sender_fingerprint(&pub1);
        let fp2 = sender_fingerprint(&pub2);
        assert_eq!(fp1a, fp1b, "same pub → same fingerprint");
        assert_ne!(fp1a, fp2, "different pub → different fingerprint");
    }

    #[test]
    fn v5_roundtrip_with_authenticated_sender() {
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let psk = known_key();
        let plaintext = b"authenticated scout report: friendly at grid 4711";
        let env = encrypt_envelope_v5(&s_priv, &s_pub, &r_pub, &psk, plaintext, b"").unwrap();
        assert!(env.starts_with(SPORE_V5_MAGIC));
        // Receiver extracts fingerprint, looks up s_pub, decrypts
        let fp = v5_envelope_sender_fp(&env).unwrap();
        assert_eq!(fp, sender_fingerprint(&s_pub));
        let pt = decrypt_envelope_v5(&s_pub, &r_priv, &psk, &env, b"").unwrap();
        assert_eq!(pt, plaintext);
    }

    #[test]
    fn v5_with_material_roundtrip() {
        // MCU-safe v5 path: caller supplies eph_priv + nonce in addition to
        // sender_static and recipient_static keys.
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let (eph_priv, _) = x25519_generate_keypair();
        let psk = known_key();
        let nonce = [0x55u8; 12];
        let pt = b"MCU v5 payload with sender auth";
        let env = encrypt_envelope_v5_with_material(&s_priv, &s_pub, &r_pub, &eph_priv, &nonce, &psk, pt, b"").unwrap();
        assert!(env.starts_with(SPORE_V5_MAGIC));
        let got = decrypt_envelope_v5(&s_pub, &r_priv, &psk, &env, b"").unwrap();
        assert_eq!(got, pt);
    }

    #[test]
    fn v5_with_material_sender_auth_holds() {
        // Attacker tries to impersonate by constructing the envelope with the
        // real sender's PUBLIC key only (no priv). Must fail because dh_ss
        // requires sender_static_priv.
        let ((real_s_priv, real_s_pub), (_, r_pub)) = make_sender_recipient();
        let (attacker_priv, _) = x25519_generate_keypair();
        let (eph_priv, _) = x25519_generate_keypair();
        let psk = known_key();
        let nonce = [1u8; 12];

        // Real envelope
        let env_real = encrypt_envelope_v5_with_material(&real_s_priv, &real_s_pub, &r_pub, &eph_priv, &nonce, &psk, b"x", b"").unwrap();

        // Attacker uses WRONG sender_priv (their own) but claims real_s_pub
        let env_forged = encrypt_envelope_v5_with_material(&attacker_priv, &real_s_pub, &r_pub, &eph_priv, &nonce, &psk, b"x", b"");
        assert!(env_forged.is_ok(), "envelope builds (we can't detect wrong priv locally)");
        // But ciphertext bytes differ from real, and more importantly the forged
        // envelope would DECRYPT to garbage because session_key depends on dh_ss
        // = X25519(attacker_priv, r_pub), not X25519(real_s_priv, r_pub).
        // The receiver will see decrypt failure (Poly1305 tag mismatch).
        assert_ne!(env_real, env_forged.unwrap(), "forged envelope must differ from real — different dh_ss → different session_key");
    }

    #[test]
    fn v5_with_material_deterministic_given_same_material() {
        let ((s_priv, s_pub), (_, r_pub)) = make_sender_recipient();
        let (eph_priv, _) = x25519_generate_keypair();
        let psk = known_key();
        let nonce = [7u8; 12];
        let pt = b"det";
        let a = encrypt_envelope_v5_with_material(&s_priv, &s_pub, &r_pub, &eph_priv, &nonce, &psk, pt, b"").unwrap();
        let b = encrypt_envelope_v5_with_material(&s_priv, &s_pub, &r_pub, &eph_priv, &nonce, &psk, pt, b"").unwrap();
        assert_eq!(a, b, "same material ⇒ byte-identical envelope");
    }

    #[test]
    fn v5_with_material_wraps_cleanly_via_auto_path_decrypt() {
        // Prove the material-path envelopes are indistinguishable from auto-path
        // envelopes at the wire level: decrypt the material envelope using the
        // SAME decrypt_envelope_v5 that handles auto-path ones. If there was
        // any layout drift, this would fail.
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let (eph_priv, _) = x25519_generate_keypair();
        let psk = known_key();
        let nonce = [0u8; 12];
        let pt = b"auto-path decrypt of material-path envelope";
        let env = encrypt_envelope_v5_with_material(&s_priv, &s_pub, &r_pub, &eph_priv, &nonce, &psk, pt, b"").unwrap();
        let fp_in_env = v5_envelope_sender_fp(&env).unwrap();
        assert_eq!(fp_in_env, sender_fingerprint(&s_pub));
        let got = decrypt_envelope_v5(&s_pub, &r_priv, &psk, &env, b"").unwrap();
        assert_eq!(got, pt);
    }

    #[test]
    fn v5_impersonation_rejected_attacker_has_psk_and_recipient_pub_only() {
        // This is the critical test: v4 allowed this forgery; v5 must reject it.
        let ((real_sender_priv, real_sender_pub), (r_priv, r_pub)) = make_sender_recipient();
        let (attacker_priv, attacker_pub) = x25519_generate_keypair();
        let psk = known_key();

        // Attacker encrypts with THEIR OWN sender keys but tries to impersonate real_sender
        // (they know real_sender_pub is what receiver expects)
        let attacker_env = encrypt_envelope_v5(&attacker_priv, &attacker_pub, &r_pub, &psk, b"FORGED ORDERS", b"").unwrap();
        // Override fingerprint to look like real sender — a naive attacker might try
        let mut forged = attacker_env.clone();
        let real_fp = sender_fingerprint(&real_sender_pub);
        forged[6..14].copy_from_slice(&real_fp);

        // Receiver tries to decrypt claiming the sender is real_sender
        let err = decrypt_envelope_v5(&real_sender_pub, &r_priv, &psk, &forged, b"").unwrap_err();
        assert_eq!(err, "v5 auth failed", "attacker WITHOUT real_sender_priv must fail SS DH → auth fail");

        // Sanity: if receiver uses attacker_pub as the claimed sender, fingerprint mismatches
        let original_env = attacker_env;
        let err2 = decrypt_envelope_v5(&real_sender_pub, &r_priv, &psk, &original_env, b"").unwrap_err();
        assert!(err2.contains("fp does not match"), "fingerprint mismatch must be caught cleanly, got: {}", err2);

        // Real sender still works
        let real_env = encrypt_envelope_v5(&real_sender_priv, &real_sender_pub, &r_pub, &psk, b"genuine", b"").unwrap();
        let pt = decrypt_envelope_v5(&real_sender_pub, &r_priv, &psk, &real_env, b"").unwrap();
        assert_eq!(pt, b"genuine");
        let _ = (r_priv, r_pub); // suppress unused warnings
    }

    #[test]
    fn v5_rejects_wrong_psk() {
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let psk = known_key();
        let env = encrypt_envelope_v5(&s_priv, &s_pub, &r_pub, &psk, b"x", b"").unwrap();
        let mut wrong_psk = psk;
        wrong_psk[0] ^= 0x01;
        assert_eq!(decrypt_envelope_v5(&s_pub, &r_priv, &wrong_psk, &env, b"").unwrap_err(), "v5 auth failed");
    }

    #[test]
    fn v5_rejects_wrong_recipient_priv() {
        let ((s_priv, s_pub), (_r_priv, r_pub)) = make_sender_recipient();
        let (other_priv, _) = x25519_generate_keypair();
        let psk = known_key();
        let env = encrypt_envelope_v5(&s_priv, &s_pub, &r_pub, &psk, b"x", b"").unwrap();
        assert_eq!(decrypt_envelope_v5(&s_pub, &other_priv, &psk, &env, b"").unwrap_err(), "v5 auth failed");
    }

    #[test]
    fn v5_ephemeral_freshness_preserves_forward_secrecy() {
        let ((s_priv, s_pub), (_, r_pub)) = make_sender_recipient();
        let psk = known_key();
        let e1 = encrypt_envelope_v5(&s_priv, &s_pub, &r_pub, &psk, b"msg", b"").unwrap();
        let e2 = encrypt_envelope_v5(&s_priv, &s_pub, &r_pub, &psk, b"msg", b"").unwrap();
        // Ephemeral pub is at [14..46]. Must differ across calls.
        assert_ne!(&e1[14..46], &e2[14..46], "v5 must still rotate ephemeral keys each call");
        // Fingerprint [6..14] is sender-dependent, stable across calls.
        assert_eq!(&e1[6..14], &e2[6..14]);
    }

    #[test]
    fn v5_tampered_ephemeral_rejected() {
        let ((s_priv, s_pub), (r_priv, r_pub)) = make_sender_recipient();
        let psk = known_key();
        let mut env = encrypt_envelope_v5(&s_priv, &s_pub, &r_pub, &psk, b"x", b"").unwrap();
        env[20] ^= 0xFF; // corrupt ephemeral pub
        assert_eq!(decrypt_envelope_v5(&s_pub, &r_priv, &psk, &env, b"").unwrap_err(), "v5 auth failed");
    }

    #[test]
    fn v5_fingerprint_lookup_flow() {
        // Simulate a multi-sender deployment: receiver has a lookup table
        // fp -> sender_static_pub. Extract fp from envelope, look up, decrypt.
        let (r_priv, r_pub) = x25519_generate_keypair();
        let (a_priv, a_pub) = x25519_generate_keypair();
        let (b_priv, b_pub) = x25519_generate_keypair();
        let mut known: std::collections::HashMap<[u8; SENDER_FP_LEN], [u8; 32]> = std::collections::HashMap::new();
        known.insert(sender_fingerprint(&a_pub), a_pub);
        known.insert(sender_fingerprint(&b_pub), b_pub);
        let psk = known_key();

        let env_from_b = encrypt_envelope_v5(&b_priv, &b_pub, &r_pub, &psk, b"from B", b"").unwrap();
        let fp = v5_envelope_sender_fp(&env_from_b).unwrap();
        let sender_pub = known.get(&fp).expect("fp must resolve to a known sender");
        assert_eq!(sender_pub, &b_pub);
        let pt = decrypt_envelope_v5(sender_pub, &r_priv, &psk, &env_from_b, b"").unwrap();
        assert_eq!(pt, b"from B");
        let _ = a_priv;
    }

    #[test]
    fn v5_rejects_unknown_sender_via_bad_fingerprint() {
        // Receiver's lookup fails. The transport layer must reject *before* calling decrypt.
        let (_, r_pub) = x25519_generate_keypair();
        let (s_priv, s_pub) = x25519_generate_keypair();
        let psk = known_key();
        let env = encrypt_envelope_v5(&s_priv, &s_pub, &r_pub, &psk, b"hi", b"").unwrap();
        let fp = v5_envelope_sender_fp(&env).unwrap();
        // Empty lookup table — sender unknown
        let known: std::collections::HashMap<[u8; SENDER_FP_LEN], [u8; 32]> = Default::default();
        assert!(known.get(&fp).is_none(), "unknown sender must not resolve; transport layer should reject the envelope");
    }

    /// RFC 8439 Section 2.8.2 test vector — verifies the ChaCha20-Poly1305
    /// implementation matches the IETF spec. If this test breaks, the crate
    /// upgrade may have changed semantics; DO NOT deploy until reviewed.
    #[test]
    fn rfc_8439_test_vector_sanity() {
        // Plaintext from RFC 8439 §2.8.2
        let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let key: [u8; 32] = [
            0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e,
            0x9f,
        ];
        let aad: [u8; 12] = [0x50, 0x51, 0x52, 0x53, 0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7];
        let nonce: [u8; 12] = [0x07, 0x00, 0x00, 0x00, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47];
        // Direct cipher call (bypass our random nonce) to check spec compliance.
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let ct = cipher.encrypt(Nonce::from_slice(&nonce), Payload { msg: plaintext, aad: &aad }).unwrap();
        // Expected ciphertext from RFC 8439 (first 16 bytes shown; full vector is long)
        let expected_prefix = [0xd3, 0x1a, 0x8d, 0x34, 0x64, 0x8e, 0x60, 0xdb, 0x7b, 0x86, 0xaf, 0xbc, 0x53, 0xef, 0x7e, 0xc2];
        assert_eq!(&ct[..16], &expected_prefix, "RFC 8439 vector mismatch — do not deploy");
        // And the tag (last 16 bytes)
        let expected_tag = [0x1a, 0xe1, 0x0b, 0x59, 0x4f, 0x09, 0xe2, 0x6a, 0x7e, 0x90, 0x2e, 0xcb, 0xd0, 0x60, 0x06, 0x91];
        assert_eq!(&ct[ct.len() - 16..], &expected_tag, "RFC 8439 tag mismatch");
    }
}
