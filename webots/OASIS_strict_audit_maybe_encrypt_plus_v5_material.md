# OASIS — Strict audit: `maybe_encrypt` gate + v5 material variant

**2026-04-22.** Two items, both directly requested by the user based
on my own prior audit recommendations. Both shipped. Detailed numbers
below.

---

## 1. Item 1: `spore::maybe_encrypt` gate (~10 min)

### What I actually did
Instead of gating the single function, I gated **the entire `spore`
module** behind `std_env` at `lib.rs`:

```rust
// `spore` is transport-layer: uses std::net::UdpSocket, std::fs, std::thread.
// Host-only by nature — gated behind std_env so MCU builds can opt out.
#[cfg(feature = "std_env")]
pub mod spore;
```

### Why module-level, not function-level
`spore.rs` uses `std::net::UdpSocket`, `std::fs`, `std::thread`,
`std::sync` — the entire module is std-only. Gating just `maybe_encrypt`
would leave dozens of other std-dependent functions still unbuildable
on MCU. Gating the module is the cleaner boundary.

### Impact
- **Host build: unchanged** (`std_env` is in default features).
- **MCU cross-compile errors: 785 → 665 (−120 errors)** from one line of `#[cfg]`.
- The "1 remaining function error" (`spore::maybe_encrypt` calling
  ungated `encrypt_envelope`) is now entirely absent from MCU compile.

## 2. Item 2: `encrypt_envelope_v5_with_material` (~1 h)

### New always-available primitive

```rust
pub fn encrypt_envelope_v5_with_material(
    sender_static_priv:    &[u8; 32],
    sender_static_pub:     &[u8; 32],
    recipient_static_pub:  &[u8; 32],
    eph_priv:              &[u8; 32],     // caller-supplied
    nonce_bytes:           &[u8; NONCE_LEN], // caller-supplied
    psk:                   &[u8; KEY_LEN],
    plaintext:             &[u8],
    aad:                   &[u8],
) -> Result<Vec<u8>, &'static str>
```

Noise-KK with:
- `dh_es` = X25519(eph_priv, recipient_static_pub) — forward secrecy
- `dh_ss` = X25519(sender_static_priv, recipient_static_pub) — sender auth
- Session key = HKDF over (psk, dh_es, dh_ss, eph_pub, sender_static_pub)
- ChaCha20-Poly1305 with caller-supplied nonce

### Reshuffle
- `encrypt_envelope_v5` → thin wrapper: `x25519_generate_keypair()` +
  `random_nonce()` + the new primitive. Still `#[cfg(feature = "os_random")]`.

### Mirror-image of prior v3 + v4 material work
Same pattern as `encrypt_envelope_with_nonce` (v3) and
`encrypt_envelope_v4_with_material`. Consistent surface:
- v3: (nonce)
- v4: (eph_priv, nonce)
- v5: (eph_priv, nonce) — sender keys already required
- v7: **still not covered** — monotonic counter complicates the flow

## 3. Tests added (4 new v5 material tests)

| Test | What it proves |
|---|---|
| `v5_with_material_roundtrip` | encrypt-with-material → decrypt_v5 round-trip works end-to-end |
| `v5_with_material_sender_auth_holds` | attacker using wrong sender_priv but claiming real sender_pub produces DIFFERENT ciphertext (session_key diverges via dh_ss) |
| `v5_with_material_deterministic_given_same_material` | reuse → identical envelope (reuse hazard documented) |
| `v5_with_material_wraps_cleanly_via_auto_path_decrypt` | material envelopes decrypt via the existing `decrypt_envelope_v5` — wire format identical |

The `sender_auth_holds` test is the important one: it exercises the
property that justifies v5 over v4. Noise-KK authenticates the sender
because the receiver's session_key computation requires
`dh_ss = X25519(sender_pub, recipient_priv)` to match the sender's
`dh_ss = X25519(sender_priv, recipient_pub)`. Both only match if the
claimed `sender_pub` is paired with the actual `sender_priv` held by
the sender. An attacker without the private key cannot produce a
ciphertext the receiver will decrypt successfully.

## 4. Scoreboard

| Metric | Pre | Post |
|---|---|---|
| Lib tests (host) | 407 | **411** (+4) |
| Kani proofs | 67 | 67 |
| Cargo features | 3 | 3 |
| MCU cross-compile errors | 785 | **669** (−116) |
| MCU function/macro errors (non-prelude) | 1 | **0** |
| MCU encrypt-path availability | v3 + v4 | **v3 + v4 + v5** |

**All 669 remaining MCU errors are pure std-prelude** (Vec/Result/Option/
None/String/vec!/format!/matches!/unreachable!). Top breakdown:

```
 41 cannot find value `None`
 17 cannot find macro `vec`
 10 cannot find macro `format`
  1 cannot find macro `unreachable`
  1 cannot find macro `matches`
```

Zero function-not-found errors. The library API surface is entirely
MCU-reachable — only the std-prelude pragma is missing at `lib.rs`.

## 5. What this round closes

- ✅ **1 remaining non-prelude function error on MCU is gone.**
- ✅ **v5 Noise-KK sender-authenticated encryption is now reachable on MCU** (once the no_std pragma lands). Prior rounds enabled v3 (plain AEAD) and v4 (forward-secret AEAD). This round closes v5 (sender auth + forward secrecy).
- ✅ **−116 MCU errors** in one round — the largest single-round drop since session start.
- ✅ **Every remaining MCU error is a known class** (std-prelude). No mystery errors.

## 6. What this round does NOT do

### 🔴 v7 still has no material variant
v7 adds a monotonic u64 counter to the v5 construction (for unbounded
replay protection). Adding a material variant requires threading (counter, counter_bytes_as_aad) through the flow. Not hard, not done.

Estimated effort: 30-45 min.

### 🔴 `spore` module is completely absent on MCU — not a partial port
The whole transport module is gated out on `--no-default-features`.
MCU users must construct their own UDP/LoRa/whatever transport and
call `spore_crypto::encrypt_envelope_*_with_material` directly. The
convenience helpers in `spore.rs` (rate limiting, reassembly, listener
loop) are lost on MCU. That's an intentional trade — for now.

### 🔴 `lib.rs` still lacks the no_std pragma
The remaining 669 errors all vanish when:
1. `#![cfg_attr(not(feature = "std"), no_std)]` is added at top of `lib.rs`.
2. `extern crate alloc;` is added.
3. Each module that uses `std::collections::*`, `std::thread`, etc. is
   gated.

This is the single highest-leverage change left for the MCU port. Estimated
~1 day of work because it requires per-module audit.

### 🔴 Not tested
- `cargo build` (only `check`).
- Real MCU hardware.
- `v5_with_material` decrypt on MCU (no build binary to run there).
- Any performance measurement of v5_with_material vs v5 auto-path.

## 7. Ripple accounting

- Item 1 (`spore` gate): −120 errors.
- Item 2 (v5 material + reshuffle of `encrypt_envelope_v5`): +4 ripple
  because `encrypt_envelope_v5` is gated, and callers (internal tests,
  `spore::maybe_encrypt` — now irrelevant since spore is gated) shifted.
- Net: −116 errors.

The +4 ripple is the honest cost of gating another function. Same
pattern as v4 round. Future v7 material variant will close the remaining
`x25519_generate_keypair` + `random_nonce` ripple.

## 8. Honest framing of the new v5 primitive

`encrypt_envelope_v5_with_material` asks for SEVEN pieces of caller input:
1. `sender_static_priv` — long-term sender identity (persisted)
2. `sender_static_pub` — derivable from priv, supplied for efficiency
3. `recipient_static_pub` — peer's pubkey (trust anchor)
4. `eph_priv` — **fresh per envelope**, forward secrecy depends on it
5. `nonce_bytes` — unique per (session_key, message); fresh-eph makes zero nonce safe but callers should use fresh random
6. `psk` — pre-shared key
7. `(plaintext, aad)`

Misuse patterns the API cannot prevent:
- Reusing `eph_priv` → forward secrecy broken.
- Using a deterministic `eph_priv` derivation (e.g., HKDF from counter)
  might be acceptable IF the counter is monotonic and persistent,
  otherwise breaks.
- Stealing `sender_static_priv` → attacker can forge any sender's
  envelopes forever until revocation.

Docstring warns. Audit repeats. Real safety requires the caller to
understand what they're doing.

## 9. Cumulative Kani breakdown (67 — unchanged)

No new Kani proofs. v5 ECDH + sender auth properties depend on X25519
and HMAC-SHA256 — both SAT-intractable under CBMC. Unit tests are
the primary validation. The 4 new tests (9 v5 tests total now)
cover roundtrip, sender auth, determinism, and wire-format compat.

## 10. Non-coverage disclosures

- No RFC test vector for v5 (Noise-KK is not a ratified IETF spec;
  our construction is defined in-repo).
- No benchmark of v5 material vs auto path.
- `x25519_pub_from_priv` is called inside `encrypt_envelope_v5_with_material`
  to recover eph_pub — that's one X25519 scalar mult ≈ 20-30 µs on laptop.
  On MCU this may be 10-50× slower. Not measured.
- No hardware boot. Still no `cargo build` for MCU (only `check`).
- `sender_auth_holds` test is a negative/indirect test. A stronger
  direct test would require implementing a full "attacker builds a
  forged envelope → receiver rejects on decrypt" integration. Deferred.
