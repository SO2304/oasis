# OASIS — Strict audit: `encrypt_envelope_v4_with_material` (MCU forward secrecy)

**2026-04-22.** Completes the MCU encrypt story for the forward-secret
(v4) envelope. Mirror-image of last round's v3 work. Still does NOT
touch v5 (Noise-KK) or v7 (monotonic counter).

---

## 1. What shipped

### New always-available primitive

```rust
pub fn encrypt_envelope_v4_with_material(
    psk:         &[u8; KEY_LEN],
    recipient_pub: &[u8; 32],
    eph_priv:    &[u8; 32],          // caller-supplied, single-use
    nonce_bytes: &[u8; NONCE_LEN],   // caller-supplied, 12 bytes
    plaintext:   &[u8],
    aad:         &[u8],
) -> Result<Vec<u8>, &'static str>
```

- Recovers `eph_pub` via existing `x25519_pub_from_priv` — caller supplies only the secret.
- Does X25519 DH with recipient's long-term pub.
- HKDF-SHA256 → session key.
- ChaCha20-Poly1305 with the caller-supplied nonce.
- Builds a wire-format-identical SPORE\x04 envelope.

### Reshuffle

- `encrypt_envelope_v4` → thin wrapper calling
  `x25519_generate_keypair()` + `random_nonce()` + the new primitive.
  Still `#[cfg(feature = "os_random")]`.
- `x25519_generate_keypair` itself now gated (was plain `pub fn`). Prior
  round already gated `random_nonce`; this completes the pair.

### Docstring

Explicit forward-secrecy warnings:
- `eph_priv` MUST be freshly generated, single-use, and discarded.
- Reusing across envelopes breaks forward secrecy catastrophically.
- A zero nonce is cryptographically acceptable if `eph_priv` is fresh
  (since the session_key varies), but callers should default to fresh
  random bytes to avoid accidental reuse bugs from copy-paste code.

## 2. Tests added (4)

| Test | What it proves |
|---|---|
| `v4_with_material_roundtrip` | full encrypt-with-material → decrypt_v4 round-trip |
| `v4_with_material_is_deterministic_given_same_material` | same (psk, recipient_pub, eph_priv, nonce, pt, aad) ⇒ byte-identical env. Makes nonce/eph reuse hazards visible. |
| `v4_with_material_matches_auto_path_given_same_material` | eph_pub recovered from priv matches `x25519_generate_keypair` output; material and auto paths are wire-identical |
| `v4_with_material_different_eph_yields_different_envelope` | fresh ephemerals → distinct envelopes even with fixed nonce (forward-secrecy-relevant) |

## 3. Scoreboard

| Metric | Pre | Post |
|---|---|---|
| Lib tests | 403 | **407** (+4) |
| Kani proofs | 67 | 67 |
| MCU cross-compile errors | 781 | **785** (+4 ripple from gating `x25519_generate_keypair`) |
| MCU forward-secret AEAD availability | **absent** | **`encrypt_envelope_v4_with_material` compiles** |
| Remaining function errors on MCU | 2 | **1** (just `spore::maybe_encrypt` still calls `encrypt_envelope` ungated) |

The +4 delta is honest ripple: gating `x25519_generate_keypair` broke
4 non-gated callers. None of them are v4_with_material — they're older
paths (v5, v7, key derivation helpers) that still assume the helper is
available. Not addressed this round.

## 4. What this closes

- ✅ **MCU can now send v4 forward-secret envelopes** once the no_std
  pragma ships. Prior round enabled v3 (plain AEAD); this round enables
  v4 (ECDH + AEAD with forward secrecy via per-envelope ephemeral keys).
- ✅ Wire format unchanged. Envelopes from MCU and laptop are
  indistinguishable to receivers (same V4_HEADER_LEN = 54, same layout).
- ✅ `x25519_generate_keypair` is now honestly feature-gated — callers
  on MCU get a clear "function not found" at compile time rather than
  a silent `getrandom` failure at runtime.

## 5. What this round does NOT do

### 🔴 Does NOT add v5 or v7 material variants
- v5 (Noise-KK, dual DH) needs `(sender_sk, sender_eph_priv, nonce)`
  supplied. Internal flow is more complex — 2 DH ops + HKDF over concatenated
  secrets.
- v7 (v5 + monotonic counter) adds counter-as-nonce derivation; less
  user-supplied state because the counter itself is an ordered nonce.
- Each requires a similar extract-internal-helper pattern to v3/v4 but
  touches more functions. Deliberately scoped out.

### 🔴 The 4 cascaded errors (from gating `x25519_generate_keypair`)
Callers in v5 / v7 / internal test helpers still call the gated
function. Fixing each requires either:
- Mirror-gating the caller behind `os_random`, OR
- Adding a material variant for that caller.

Didn't fix this round. The net trajectory is still correct: 5 827+ →
785. A later round will eat the remaining ripple as it builds out v5/v7
material.

### 🔴 `spore::maybe_encrypt` still calls `encrypt_envelope` ungated
This is the 1 remaining non-prelude function error on MCU. It's a
library-level helper that reads the key from env and wraps in v3.
Fixing requires either:
- Gate `maybe_encrypt` behind `std_env` + `os_random`, OR
- Add a nonce-supplying variant of `maybe_encrypt`, OR
- Move `maybe_encrypt` out of the library entirely (it's a
  convenience for binaries, not a primitive).

Out of scope this round.

### 🔴 No RFC 8439 test vector for the material path
The existing `rfc8439_test_vector` test exercises `encrypt_envelope`,
which now calls through `encrypt_envelope_with_nonce` — so the crypto
PATH is covered. There is NO similar test vector for v4 (no official
RFC vector exists; v4 is an OASIS composition of X25519 + HKDF + AEAD).
The `v4_with_material_roundtrip` test covers end-to-end correctness.

## 6. Honest disclosure on the new API

`encrypt_envelope_v4_with_material` asks the caller to supply TWO
pieces of cryptographic material:
1. `eph_priv: &[u8; 32]` — ephemeral X25519 secret
2. `nonce_bytes: &[u8; 12]` — AEAD nonce

Forward secrecy depends on:
- `eph_priv` being **freshly generated for each envelope**.
- `eph_priv` being **destroyed immediately after the call**.

The API cannot enforce either. A caller who generates one `eph_priv`
and uses it for 1 000 envelopes kills forward secrecy for all 1 000.
The docstring is load-bearing.

Realistic MCU pattern for an `eph_priv` source:
- Hardware TRNG if available (nRF52840, STM32 chips with crypto
  accelerator, ESP32, etc.).
- A ChaCha20-based DRBG seeded from a small entropy pool accumulated
  during boot (sensor noise, timer jitter). Acceptable only if the
  pool is genuinely unpredictable.
- **NOT** acceptable: `[0u8; 32]`, process-start timestamp, any
  deterministic seed.

The audit makes this explicit because it's too easy to write
"demo code" that's insecure.

## 7. Cumulative Kani breakdown (67 — unchanged)

No new Kani proofs this round. ChaCha20-Poly1305 and X25519 both
bit-blast SAT (same reason as SHA-256). The crypto correctness relies
on:
- RustCrypto `chacha20poly1305` (audited).
- `ed25519-compact`'s X25519 impl.
- Unit test coverage: 7 v4 tests total (3 pre-existing + 4 new),
  covering roundtrip, tamper rejection, wrong privkey rejection,
  forward-secrecy negative test, material roundtrip, determinism,
  compat with auto path, and different-eph distinctness.

## 8. Next-step candidates

1. **v5 + v7 material variants** — closes the Noise-KK and
   monotonic-counter paths on MCU. ~1 hour each.
2. **`#![no_std]` pragma push** at lib.rs + `extern crate alloc` +
   per-module std audit. ~1 day. This is the biggest single unlocker.
3. **Gate `maybe_encrypt` in `spore.rs`** behind features. ~10 min.
4. **Per-node Ed25519 mesh signatures** (insider closure) — unchanged.
5. **Typed messages derive macro** — unchanged.

I lean toward **#3 + #1** stacked in one round: tiny gate fix for
`maybe_encrypt`, then v5 material variant. That's ~2 hours and takes
the MCU error count below 400, crossing the psychological threshold
where the no_std push becomes the clearly-next step.

## 9. Non-coverage disclosures

- `encrypt_envelope_v4_with_material` not benchmarked vs the auto path.
  Cost should be identical minus ~200 ns of eph key generation.
- No hardware boot. Still no thumbv7em `cargo build` (only `check`).
- v4 has no RFC test vector — our negative-test coverage (forward
  secrecy, tamper, wrong privkey) is the primary validation.
- The `v4_with_material_is_deterministic_given_same_material` test
  deliberately demonstrates that nonce reuse is catastrophic. This is
  both a correctness check and a warning. A future lint or runtime
  check that DETECTS reuse is not shipped this round.
