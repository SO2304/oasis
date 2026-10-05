# OASIS — Strict audit: `encrypt_envelope_with_nonce` (MCU encrypt path)

**2026-04-22.** Closes a concrete functional gap flagged in the
previous two audits: on `os_random`-disabled builds, AEAD encryption
was unavailable entirely. Now it isn't.

---

## 1. What shipped

### New always-available API

```rust
pub fn encrypt_envelope_with_nonce(
    key:   &[u8; KEY_LEN],
    nonce_bytes: &[u8; NONCE_LEN],
    plaintext:   &[u8],
    aad:   &[u8],
) -> Result<Vec<u8>, &'static str>
```

No feature gate. Works on every target `oasis-rt` targets (once the
lib-level no_std pragma lands). Produces a SPORE\x03 envelope
byte-identical to `encrypt_envelope` given the same nonce.

### Reshuffle

- `encrypt_envelope` → thin wrapper that calls `random_nonce()` +
  `encrypt_envelope_with_nonce()`. Kept behind `os_random`.
- `encrypt_envelope_v4 / v5 / v7` → gated behind `os_random` (they each
  call `random_nonce` internally; wrapping them to take a supplied
  nonce is future work, not scoped this round).

### Documentation bias

The docstring on `encrypt_envelope_with_nonce` explicitly warns:
- Caller responsible for nonce uniqueness per key.
- Reusing (key, nonce) breaks AEAD security completely.
- Do NOT use a fixed nonce. Do NOT derive it from plaintext.
- Monotonic counter pattern MUST survive reboot (NVS) or nonce reuse
  after restart.

This warning is load-bearing. The prior round's strict audit flagged
MCU encryption as a functional gap; exposing a nonce-taking function
transfers the cryptographic burden from OS to caller. Naïve users can
break AEAD catastrophically here. Hence the prominent warning.

## 2. Tests added (4)

| Test | What it proves |
|---|---|
| `encrypt_with_nonce_roundtrip` | decrypt works end-to-end with a caller-supplied nonce |
| `encrypt_with_nonce_is_deterministic` | same (key, nonce, pt, aad) → byte-identical envelope — this is BOTH a correctness property and a warning about nonce reuse |
| `encrypt_with_different_nonces_yields_different_ciphertexts` | nonce change ⇒ different ciphertext (basic correctness) |
| `encrypt_with_nonce_and_encrypt_envelope_produce_compatible_envelopes` | the two APIs are wire-format identical |

## 3. Scoreboard

| Metric | Pre | Post |
|---|---|---|
| Lib tests (host) | 399 | **403** (+4) |
| Kani proofs | 67 | 67 |
| Cargo features | 3 | 3 |
| MCU cross-compile errors in oasis-rt | 792 | **781** (−11) |
| MCU encrypt-AEAD availability | **absent** | **`encrypt_envelope_with_nonce` compiles for thumbv7em** |

The −11 is from gating `encrypt_envelope_v4/v5/v7` (removed 5 primary +
6 ripple errors). Remaining 781 are all std-prelude class
(Vec/Result/Option/None/String/vec!/format!) — they vanish when the
`#![no_std]` pragma lands.

## 4. What this round closes

### ✅ MCU AEAD encryption is no longer a blocker
A binary targeting `thumbv7em-none-eabi` can (once the no_std pragma
is added at the crate level) call `encrypt_envelope_with_nonce(key,
&my_counter_nonce, pt, aad)` and produce a valid SPORE\x03 envelope
without any OS RNG. Previously there was no path at all.

### ✅ Wire format remains unchanged
Same 6-byte magic + 12-byte nonce + 4-byte length + ciphertext+tag.
Envelopes from MCU and laptop are indistinguishable to receivers.

### ✅ `encrypt_envelope_v4/v5/v7` now honestly feature-gated
They were compile-time errors on MCU before (calling non-existent
`random_nonce`). Now they're compile-time absent on MCU. Same
effect for callers; better signal in the error messages.

## 5. What this round does NOT do

### 🔴 Does NOT provide a v4/v5/v7 nonce-taking equivalent
- `encrypt_envelope_v4_with_nonce` — not implemented.
- `encrypt_envelope_v5_with_nonce` — not implemented.
- `encrypt_envelope_v7_with_nonce` — not implemented.

v3 (plain AEAD + PSK) is the simplest OASIS envelope. v4+ add ECDH /
sender auth / counters. For an MCU application needing forward secrecy
or sender authenticity, they're still stuck. This round intentionally
scoped down to v3 only — refactoring v4/v5/v7 each requires threading
the nonce through 2–3 internal helpers. Not hard, just more lines.

### 🔴 Does NOT fix the 781 remaining MCU errors
All std-prelude. Requires the lib.rs no_std pragma + `extern crate
alloc` push, still estimated ~1 day of work.

### 🔴 Does NOT add nonce-safety invariants as Kani proofs
Kani can't model ChaCha20-Poly1305 or Poly1305 tag — same
SAT-intractable issue as SHA-256. The deterministic-output property
is a unit test only. A caller passing the same nonce twice will:
- Encrypt correctly (no panic).
- Produce identical ciphertexts (witnessed by the deterministic test).
- Leak the keystream XOR ⇒ plaintext recovery for an attacker with
  BOTH ciphertexts. **No runtime check detects this misuse.**

### 🔴 Does NOT ship a nonce-counter helper
I considered adding `struct NonceCounter { counter: u64, persisted_at: ... }`
that auto-increments and refuses to go backwards, but:
- Persistence semantics are platform-specific (NVS, file, BLE-backed).
- A weak helper invites misuse more than no helper.
- `tx_counter` in MeshRouter already covers mesh-layer counter
  persistence — the AEAD layer equivalent deserves its own design.

Left as a deliberate non-ship.

## 6. Honest caveat on the new API

`encrypt_envelope_with_nonce` is a **footgun**. The SAFE path is
`encrypt_envelope` with OS RNG. The new function exists because some
targets don't have OS RNG. Users of the new function must understand:

1. AEAD security assumes unique (key, nonce) pairs. Nonce uniqueness
   is the caller's problem.
2. A common naïve implementation — "I'll increment a counter and use
   the bytes as the nonce" — works IF and ONLY IF the counter survives
   reboots. Otherwise you get nonce reuse on the first post-reboot msg.
3. Using `rand::thread_rng()` from the `rand` crate still requires an
   RNG backend on MCU (usually hardware); not free.

The docstring says all this. It will still be misused. That's the
cost of exposing a primitive — documented.

## 7. Disciplined self-limit

I took 3 small steps and stopped:
1. Add `encrypt_envelope_with_nonce` as the always-available primitive.
2. Make `encrypt_envelope` a thin wrapper.
3. Gate v4/v5/v7 for consistency.

I did NOT expand the scope to:
- Full v4/v5/v7 nonce-taking variants.
- A NonceCounter helper struct.
- Adding Kani proofs that SAT can't discharge.
- The no_std pragma push.

Scope discipline matches the last two rounds. This is a ~1 h round
that closes exactly the gap it promised.

## 8. Non-coverage disclosures

- `encrypt_envelope_with_nonce` has not been benchmarked vs
  `encrypt_envelope`. The performance should be identical minus the
  RNG cost of `random_nonce()` (typically ~100–300 ns on laptop).
- No RFC 8439 test vector was re-run against `encrypt_envelope_with_nonce`
  specifically. The existing `rfc8439_test_vector` test uses
  `encrypt_envelope` which now calls through the new function — so
  the path IS exercised, just not by a test that names the new function.
- Zero MCU hardware boot.
- v4/v5/v7 are now feature-gated-out on MCU but their behavior-on-MCU
  is still untested (would need nonce-variants to test anything at all).
