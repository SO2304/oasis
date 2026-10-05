# OASIS — Strict audit: MCU dep-layer cleared, crate-level work remains

**2026-04-22.** Smallest honest increment on the MCU port. Two
deps fixed; cross-compile errors dropped from 5 827+ (dep layer) to
**792 (all in oasis-rt itself)**. The nature of the blocker shifted
from "third-party crate won't compile" to "our library isn't annotated
as no_std."

---

## 1. What shipped

### Cargo.toml changes

```diff
- serde = { version = "1", features = ["derive"] }
- serde_json = "1"
- getrandom = "0.2"
- ed25519-compact = { ..., features = ["x25519", "random", "std"] }
+ serde = { version = "1", default-features = false, features = ["derive", "alloc"] }
+ serde_json = { version = "1", default-features = false, features = ["alloc"] }
+ getrandom = { version = "0.2", optional = true }
+ ed25519-compact = { ..., features = ["x25519"] }

[features]
- default = ["std_env"]
+ default = ["std_env", "os_random"]
+ os_random = ["dep:getrandom", "ed25519-compact/random", "ed25519-compact/std"]
```

### `spore_crypto.rs` cfg gates

- `random_nonce()` and `encrypt_envelope()` are now `#[cfg(feature = "os_random")]`.
- `decrypt_envelope()` remains always-available — decryption doesn't need an RNG.
- Documented the implication: MCU builds without `os_random` can decrypt but cannot emit new AEAD envelopes until we expose a user-injectable RNG.

## 2. Host regression surface: ZERO

```
cargo test --lib --release
test result: ok. 399 passed; 0 failed; 0 ignored; 0 measured
```

The host build is unchanged. All 399 tests pass. Encryption round-trip
tests still pass because `os_random` is in `default` and pulls
`getrandom` + `ed25519-compact/random,std`.

## 3. Cross-compile error count progression

```
round 1 (before any fixes):  5 827+  blocked at serde_core
round 2 (this round):            792  blocked in oasis-rt itself
```

**Dep layer is clean.** `cargo check --target thumbv7em-none-eabi
--lib --no-default-features --features mesh_bloom_mcu` now fails only
in our own code.

### Error-kind distribution of the remaining 792

```
  116  cannot find type `Vec`
   67  cannot find type `Result`
   52  cannot find type `Option`
   45  cannot find value `None`
   39  cannot find type `String`
   29  cannot find attribute `derive`    ← std prelude macro
   20  cannot find macro `vec`
   13  cannot find trait `Default`
   12  cannot find macro `format`
    3  cannot find function `random_nonce`   ← expected, feature-gated
    2  cannot find trait `Clone`
    1  cannot find type `Box`
    1  cannot find trait `Send`
    1  cannot find function `encrypt_envelope` ← expected, feature-gated
    ...
```

**Every Vec/Result/Option/None/String/vec!/format! error is the same
root cause**: `lib.rs` has no `#![cfg_attr(not(feature = "std"), no_std)]`
pragma and no `extern crate alloc`. For a std target these come from
the implicit prelude; for a no_std target they must be explicitly
imported.

Fixing this requires:
1. `#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]` at `lib.rs`.
2. `#[cfg(not(feature = "std"))] extern crate alloc;`
3. Gate every `std::` import (several hundred call sites across 30+ modules) behind `#[cfg(feature = "std")]` or swap to `core::` / `alloc::` equivalents.
4. Gate modules that are std-only (e.g., `federation.rs` uses `std::thread`, `spinal.rs` uses `std::fs`) behind `#[cfg(feature = "std")]`.

The 3 `random_nonce` and 1 `encrypt_envelope` errors are expected and
correct — they're test code referencing feature-gated items without
mirror-gating themselves. A proper fix will add cfg-matching to the
call sites.

## 4. Scoreboard

| Metric | Pre | Post |
|---|---|---|
| Lib tests (host) | 399 | 399 |
| Kani proofs | 67 | 67 |
| Cargo features | 2 | **3** (+os_random) |
| Cross-compile errors (dep layer) | 5 827+ | **0** |
| Cross-compile errors (oasis-rt itself) | — | **792** (all "no std prelude") |
| Lines of Cargo.toml change | 0 | 7 |
| Lines of source change | 0 | 6 (two cfg gates) |

## 5. What this round closes

- ✅ Dep tree no longer blocks MCU port.
- ✅ `os_random` feature gives MCU users a documented opt-out path
  (at the cost of losing `encrypt_envelope` until a user-supplied
  RNG hook is added).
- ✅ Zero host regression.
- ✅ Every subsequent cross-compile attempt will see oasis-rt's OWN
  errors, not dep errors — actionable.

## 6. What this round does NOT close

### 🔴 oasis-rt itself is still std-only
792 errors remain, all in our code. The `mesh_bloom_mcu` feature
documented in the code comment is still cosmetic for embedded use
until the no_std pragma is added and modules are audited.

### 🔴 No new user-facing API for MCU encryption
The `os_random` cfg gate removes `encrypt_envelope` on MCU. There is
NO replacement — no `encrypt_envelope_with_nonce(key, plaintext, aad,
nonce)` function exposed. MCU users can decrypt, can run mesh dedup,
can sign with v9 MAC (HMAC-SHA256 doesn't need RNG), but cannot emit
AEAD. This is a real functional gap disclosed in the docstring.

### 🔴 The v0.3 → v0.2 getrandom mismatch
`cargo tree` showed `ed25519-compact` was actually pulling **getrandom
v0.3.4**, while we declared `getrandom = "0.2"`. With `random` now
feature-gated out of ed25519-compact on MCU, this dual-version problem
is dormant, but on host both versions are still in the build graph.
Duplicate deps are a minor hygiene issue, not functional — didn't
address this round.

### 🔴 Not attempted
- `cargo check --target thumbv6m-none-eabi` (Cortex-M0, smaller).
- `cargo check --target riscv32i-unknown-none-elf` (RISC-V).
- `cargo build` (vs `check`) — linker errors may reveal more.
- Real MCU hardware boot.
- The `chacha20poly1305` crate's no_std compatibility — it's currently
  compiling on thumbv7em because it already has `default-features = false`
  in our Cargo.toml. But if we disable `os_random`, we still pull it —
  its actual no_std behavior is unverified beyond "the compiler didn't
  complain this round."

## 7. Disciplined self-critique

I resisted the temptation to "just add `#![no_std]` and fix the 792
errors in one big push." That would have been:
- a 2–3 hour slog across 30+ files
- zero tests (they're std-only for serial_test harness reasons)
- high chance of introducing subtle regressions to the host build
- audit-unfriendly (one enormous "this change enables no_std"
  commit with no intermediate checkpoint)

Instead, this round delivers:
- **two small, isolated Cargo.toml changes**
- **two cfg gates** in one source file
- **zero host regression**
- a documented state where the next person (me or otherwise) can
  pick up at "add the no_std pragma to lib.rs, see what happens"

## 8. Next-step candidates

1. **Add `#![cfg_attr(...)] no_std` to lib.rs + `extern crate alloc`**,
   then see which modules survive. Estimated 1 day of cfg-gating work
   with per-module test runs.
2. **Expose user-injectable RNG hook for MCU encryption** (removes the
   `os_random` functional gap). ~1 h.
3. **Per-node Ed25519 mesh signatures** (insider attack closure) —
   unchanged from prior audits.
4. **Typed messages derive macro** — unchanged.

I recommend **#2** next — small, closes a real gap, and doesn't require
the giant no_std surgery. The no_std pragma push should be its own
full round when I'm ready to take on 1 day of work.

## 9. Non-coverage disclosures

- Only thumbv7em tested.
- `cargo build` not attempted (only `check`).
- No hardware boot.
- The `chacha20poly1305` crate's internal no_std posture is not audited
  beyond "it compiled."
- Duplicate getrandom versions (v0.2 + v0.3 in host build) not
  reconciled.
- `os_random`-disabled builds do not have a working encrypt path —
  this is a real functional limitation, not just a technicality.
