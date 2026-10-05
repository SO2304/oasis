# OASIS — Strict audit: v7 material variant + complete encrypt suite

**2026-04-22.** Last material variant shipped. The AEAD encrypt API
surface is now mirrored for MCU (no OS RNG) across **all four envelope
versions**: v3, v4, v5, v7.

---

## 1. What shipped

### New always-available primitive

```rust
pub fn encrypt_envelope_v7_with_material(
    sender_static_priv:    &[u8; 32],
    sender_static_pub:     &[u8; 32],
    recipient_static_pub:  &[u8; 32],
    eph_priv:              &[u8; 32],
    nonce_bytes:           &[u8; NONCE_LEN],
    psk:                   &[u8; KEY_LEN],
    counter:               u64,
    plaintext:             &[u8],
    aad:                   &[u8],
) -> Result<Vec<u8>, &'static str>
```

v5 Noise-KK + monotonic counter authenticated via AAD. Same invariants
as the auto-path `encrypt_envelope_v7`:
- Counter in Poly1305 AAD → tampering detected.
- Fresh `eph_priv` per envelope → forward secrecy.
- Wire format byte-identical to the auto path.

### Reshuffle
- `encrypt_envelope_v7` → thin wrapper: `x25519_generate_keypair()` +
  `random_nonce()` + the new primitive. Still `#[cfg(feature = "os_random")]`.

### Docstring
Explicit warnings covering all three fresh-material invariants: eph,
counter monotonicity (with cross-reboot persistence reminder), and
nonce uniqueness.

## 2. Tests added (4 new)

| Test | What it proves |
|---|---|
| `v7_with_material_roundtrip` | encrypt-with-material → decrypt_v7 returns (counter, plaintext) correctly |
| `v7_with_material_counter_in_aad_is_authenticated` | tampering byte 14 of envelope (counter field) causes decrypt failure — **AAD authentication holds through the material path** |
| `v7_with_material_deterministic_given_same_material_and_counter` | same inputs ⇒ byte-identical envelope |
| `v7_with_material_different_counters_yield_different_envelopes` | counter change (1 → 2) produces different envelope even with identical everything else |

## 3. Scoreboard

| Metric | Pre | Post |
|---|---|---|
| Lib tests (host) | 411 | **415** (+4) |
| Kani proofs | 67 | 67 |
| MCU cross-compile errors | 669 | **674** (+5 ripple) |
| MCU function/macro errors | 0 | **0** |
| MCU encrypt-path coverage | v3 + v4 + v5 | **v3 + v4 + v5 + v7** |

## 4. Complete material-variant suite — completeness scoreboard

| Version | Auto API (needs `os_random`) | Material API (MCU-safe) |
|---|---|---|
| v3 plain AEAD | `encrypt_envelope` | **`encrypt_envelope_with_nonce`** |
| v4 forward-secret ECDH | `encrypt_envelope_v4` | **`encrypt_envelope_v4_with_material`** |
| v5 Noise-KK sender-auth | `encrypt_envelope_v5` | **`encrypt_envelope_v5_with_material`** |
| v7 v5 + monotonic counter | `encrypt_envelope_v7` | **`encrypt_envelope_v7_with_material`** |

All 4 versions have a material variant. MCU users can emit every
envelope type once the no_std pragma lands. No more "MCU can decrypt
but can't encrypt" class of limitations.

Material-variant arity progression (forced by the crypto itself):

| Version | Caller supplies |
|---|---|
| v3 | nonce |
| v4 | eph_priv, nonce |
| v5 | eph_priv, nonce |
| v7 | eph_priv, nonce, counter |

## 5. What this round closes

- ✅ **v7 MCU encrypt is no longer a gap.**
- ✅ **Material-variant suite is now complete** for all OASIS encrypted
  envelope versions. The user's original session concern — "OASIS should
  be MCU-friendly" — is resolved at the API level.
- ✅ **Counter-in-AAD authentication proven through the material path.**
  The `v7_with_material_counter_in_aad_is_authenticated` test
  exercises exactly the replay-resistance property v7 adds to v5.

## 6. What this round does NOT do

### 🔴 The no_std pragma is still not added
All 674 remaining MCU errors are pure std-prelude:
```
 41 cannot find value `None`
 17 cannot find macro `vec`
 10 cannot find macro `format`
  1 cannot find macro `unreachable`
  1 cannot find macro `matches`
```

No function-not-found errors. The API surface is complete. Single
remaining step is the `#![cfg_attr(...)]` pragma + `extern crate alloc`
+ per-module std-gating. Estimated ~1 day because every module that
uses `std::` needs auditing.

### 🔴 +5 ripple from gating `encrypt_envelope_v7`
Same pattern as prior material rounds. Callers of `encrypt_envelope_v7`
(likely in v7 helper functions, counter tracker integration, or
internal tests) are now breaking. Didn't chase down this round.

### 🔴 Not measured
- v7 material vs v7 auto-path performance delta.
- MCU hardware boot.
- `cargo build` (only `check`).

### 🔴 No Kani proofs
ChaCha20, Poly1305, HMAC, X25519 all bit-blast SAT. v7's additional
AAD-counter authentication isn't Kani-provable. Unit tests are the
primary validation.

## 7. Ripple accounting across the MCU trajectory

| Round | Δ MCU errors | Note |
|---|---:|---|
| Session start | — | 5 827+ (dep layer blocked) |
| serde + getrandom dep fixes | → 792 | dep layer cleared |
| Gate v4/v5/v7 encrypt_envelope | → 781 | minus 11 |
| v4_with_material | → 785 | +4 ripple |
| spore module gate | → 665 | **−120** (big win) |
| v5_with_material | → 669 | +4 ripple |
| **v7_with_material (this round)** | **→ 674** | +5 ripple |

Every individual round adds functional capability. Some rounds have
ripple from gating; they're paid back by subsequent material variants
or by the no_std push that will eliminate all std-prelude errors at once.

## 8. Honest disclosure on v7 material

v7 asks the caller for TEN pieces of input (sender keys ×2, recipient
pub, eph_priv, nonce, psk, counter, plaintext, aad). That's ergonomically
heavy. The docstring lists three DIFFERENT correctness invariants the
caller must respect:

1. **Fresh `eph_priv`** — single-use, discard after call.
2. **Strictly increasing `counter`** — for this (sender, recipient)
   pair. Must persist across reboots (`tx_counter`-like pattern in
   MeshRouter already covers mesh-layer version; this is the
   AEAD-layer version).
3. **Unique `nonce_bytes`** — per (session_key, nonce). Fresh eph
   bounds risk, but callers should still default to random.

Failure modes:
- Counter goes backwards → receiver's CounterTracker rejects → silent drop.
- Counter wraps u64 (impossible in any realistic deployment) → same.
- eph reuse → forward secrecy broken for those envelopes.
- nonce reuse WITH eph reuse → keystream XOR leaked, plaintext recoverable.

The API is a footgun. So are v4, v5 material variants. All documented.
The alternative is to not expose these at all on MCU — which means
OASIS is host-only. The trade is: powerful primitives with clear
warnings, or no primitives at all.

## 9. Cumulative v7 test coverage

Before: 3 v7 tests (roundtrip, tampered counter, + tracker integration).
After: 7 v7 tests — material roundtrip, material counter AAD authenticity,
material determinism, counter sensitivity.

## 10. Non-coverage disclosures

- No fuzz corpus for v7 envelopes.
- No RFC vector (v7 is an OASIS composition).
- No benchmark.
- `cargo build` not attempted.
- No hardware boot.
- Counter persistence integration across actual binaries (drone_bridge,
  etc.) still relies on `CounterTracker::save/load_from_file` which
  requires std::fs — not MCU-safe. The `encrypt_envelope_v7_with_material`
  primitive is MCU-safe but callers must bring their own counter
  persistence (NVS, EEPROM, etc.).

## 11. Next-step candidates

The remaining roadmap, unchanged except v7 now crossed off:

1. **`#![no_std]` pragma push** at lib.rs + `extern crate alloc` +
   per-module std audit. **~1 day focused.** This is the single
   highest-leverage change left. Will drop MCU errors 674 → small
   single digits in one push.
2. **Per-node Ed25519 mesh signatures** (insider attack closure) —
   unchanged from session start.
3. **Typed messages derive macro** — biggest DX gap vs ROS 2.
4. **200-drone sim re-run with v9 + 64 KiB Bloom** — validation.
5. **Real MCU hardware boot** — depends on #1.

I lean toward **#1** (no_std push) next. Every session has claimed
progress toward MCU; the pragma is the climax that closes the loop.
I will NOT claim it's a quick fix — it's a day of focused work —
but the next round could reasonably deliver a substantial chunk if
not all.
