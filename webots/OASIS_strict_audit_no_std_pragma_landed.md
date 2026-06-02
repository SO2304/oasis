# OASIS — Strict audit: `#![no_std]` pragma landed, per-module work remains

**2026-04-22.** One atomic change (pragma + `extern crate alloc` + new
`std` default feature). MCU cross-compile error count dropped from
**674 → 295** (−379, over half) with zero host regression.

---

## 1. What shipped

### `lib.rs`
```rust
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
```

### `Cargo.toml`
```toml
[features]
default = ["std", "std_env", "os_random"]
std = []
```

**3 lines in `lib.rs`, 3 lines in `Cargo.toml`.** That's the entire
code change this round.

## 2. Measurement

### Host build and tests
- `cargo build --lib --release` → finished in 5.1 s, no errors.
- `cargo test --lib --release` → **415/415 pass**, same as previous round.

### MCU cross-compile
- Pre: 674 errors.
- Post: **295 errors.** 379-error drop.

### Remaining error breakdown (295)

| Error class | Count | Root cause |
|---|---:|---|
| `E0425` (unresolved value) | 135 | Vec / String / Box not in prelude; per-module `use alloc::...;` needed |
| `E0433` (unresolved module `std`) | 130 | Direct `std::collections::*`, `std::thread`, `std::fs`, `std::time::Instant`, `std::net::*` usage |
| `vec!` / `format!` macros | 27 | Need `use alloc::{vec, format};` per module |
| `E0432` (unresolved import) | 2 | Minor |
| other | 1 | Build summary line |

### Per-module cost ranking (errors attributed to file)

| Module | Errors | Main cause |
|---|---:|---|
| `spore_crypto.rs` | 55 | `std::collections::VecDeque`, `std::fs` (CounterTracker save/load) |
| `mavlink_min.rs` | 34 | `std::io::Read/Write`, time types |
| `nerve.rs` | 32 | `serde_json::Value` in an alloc-only context + potentially `std::env` |
| `transport.rs` | 31 | `std::net::UdpSocket`, `std::fs` for file transport |
| `spinal.rs` | 21 | `std::fs` for /sys reads |
| `federation.rs` | 20 | alloc prelude + Ed25519 state |
| `mesh.rs` | 13 | `std::collections::{HashSet, VecDeque}` (swap to `alloc::collections::BTreeSet` / `alloc::collections::VecDeque`) |
| `emotion.rs` | 13 | Vec/String |
| `services.rs` | 12 | Vec + HashMap (swap to BTreeMap) |
| `actions.rs` | 10 | Vec |
| `transforms.rs` | 9 | BTreeMap is already used; Vec/String missing |
| others (topics, parameters, morpho, etc.) | ~40 | Vec/String prelude |

## 3. Scoreboard

| Metric | Pre | Post |
|---|---|---|
| Lib tests (host) | 415 | **415** (zero regression) |
| Kani proofs | 67 | 67 |
| Cargo features | 3 | **4** (+`std`) |
| MCU errors total | 674 | **295** |
| MCU errors, pragma-fixable in isolation | 674 | **0** |
| MCU errors needing per-module `use alloc::...` | — | 162 |
| MCU errors needing per-module `#[cfg(feature="std")]` gate or rewrite | — | 133 |

## 4. What this round closes

- ✅ **The single highest-leverage MCU change is done.** `lib.rs` is
  no_std-aware. Any sub-module that's already `alloc`-compatible
  would compile fine under `--no-default-features`; the ones that
  don't are clearly identified above.
- ✅ **Zero host regression.** The `std` feature in `default` means
  the host build is byte-identical to before.
- ✅ **The `std` feature is now a documented boundary.** Downstream
  users can reason about which parts of OASIS are MCU-safe by reading
  the feature flag.

## 5. What this round does NOT do

### 🔴 The remaining 295 errors are per-module work
Each one of the top-10 modules needs:
- `use alloc::{vec::Vec, string::String, vec, format};` at the top, AND/OR
- `#[cfg(feature = "std")]` gating for functions that use
  `std::collections::*`, `std::net::*`, `std::fs`, `std::time::Instant`,
  `std::thread`.

This is the ~1 day of focused work I've been estimating. I did NOT
attempt it in this round deliberately. The change that WAS made is
atomic and correct; a partial per-module pass would be neither.

### 🔴 Not all modules are equally fixable
- **Fixable with `use alloc::...`**: `mesh.rs`, `emotion.rs`,
  `actions.rs`, `transforms.rs`, `parameters.rs`, `topics.rs`,
  `services.rs`, `morpho.rs`, `efference.rs`. ~100 errors.
- **Needs `#[cfg(feature = "std")]` gating on specific functions**:
  `spore_crypto.rs` (CounterTracker uses `std::fs`),
  `mavlink_min.rs` (Read/Write traits), `spinal.rs` (/sys reads),
  `transport.rs` (UdpSocket + file I/O).
- **Needs deeper refactor**: `nerve.rs` uses `serde_json::Value` in
  a way that may or may not survive no_std. Unclear without diving in.

### 🔴 `std::collections::HashMap` is not in alloc
Several modules use `HashMap<K, V>`. `alloc::collections::BTreeMap`
exists; swapping requires API re-examination because BTreeMap requires
`Ord` on keys (HashMap only requires `Hash + Eq`). Where keys are
strings/u64/integers, BTreeMap is fine. For more complex keys,
`hashbrown` crate is the no_std standard swap — that's a new dep.

### 🔴 Not tested
- `cargo build` (only `check` — linker may surface more issues).
- Real MCU hardware.
- Linux / macOS / ARM64 with or without the new feature.
- Cross-version behavior (the `std` feature's addition may break
  downstream consumers that depended on certain features being ON
  by default — though `default = ["std", ...]` should be backward-compatible).

## 6. Trajectory across the session

| Round | MCU errors |
|---|---:|
| Session start (pre-dep-fixes) | 5 827+ |
| Dep-layer fixes (serde, getrandom) | 792 |
| `spore` module gate | 665 |
| v4/v5/v7 material + encrypt gates | ~670 |
| **no_std pragma (this round)** | **295** |
| Projected after per-module cleanup | ≲ 20 |

Over one session: **5 827+ → 295 errors = 95 % reduction** in MCU
cross-compile blockers. Each reduction paid for by a small, measured
change with documented caveats.

## 7. What the next round could do

Two reasonable scopes:

### Option A — fix ALL pure-alloc modules in one round
Add `use alloc::{...}` prelude at the top of `mesh.rs`, `emotion.rs`,
`actions.rs`, `transforms.rs`, `parameters.rs`, `topics.rs`,
`services.rs`, `morpho.rs`, `efference.rs`. Swap `HashMap` → `BTreeMap`
where possible, or add `hashbrown` dep. Estimated 2-3 hours. Would
take MCU errors to ~130.

### Option B — gate the std-heavy modules
Add `#[cfg(feature = "std")]` on functions/items using
`std::fs`/`std::net`/`std::thread` in `spore_crypto.rs`,
`mavlink_min.rs`, `spinal.rs`, `transport.rs`. Estimated 3-4 hours.
Would take MCU errors to ~80.

### Option C — do both in one long round
Might finally hit zero errors. But 5-7 hours. Audit discipline suggests
Option A first, measure, then Option B.

## 8. Honest disclosures

- I did NOT try to fix any per-module error this round despite knowing
  how. The discipline is to ship atomic, measurable changes. The pragma
  is one atomic change; the per-module audit is another.
- The `std` feature is a new default. Any downstream crate pinning
  exact feature set may have to update. In practice, no downstream
  crate exists yet (OASIS is not published).
- `cargo test` was only the `--lib` test suite (415 tests). I did NOT
  re-run any bin or integration tests. The host bin targets all still
  link against the default features which include `std`, so they should
  still build — but I didn't verify.
- The `std` feature itself is currently EMPTY (just a marker). It
  doesn't gate any code YET because the per-module pass is where those
  `#[cfg(feature = "std")]` annotations will land.
- I did not add any Kani proof or unit test for the feature flag
  behavior — the next round's per-module pass will include tests
  where appropriate.

## 9. Non-coverage

- ✅ Bin build check run after writing this audit: `cargo build
  --release --bins` → all 17 bins link cleanly in 43.12 s. No
  regression in the binary targets.
- No integration test coverage of `--no-default-features` for any
  downstream consumer.
- `std` feature is a boolean; no finer granularity (e.g., separate
  `std_net`, `std_fs`). That may become a requirement when the
  per-module work surfaces modules that need std::net but not std::fs.
