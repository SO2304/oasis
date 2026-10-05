# OASIS — Strict audit: MCU build PASSES (0 errors, real `cargo build`)

**2026-04-22.** Climax of the no_std push. `cargo build --target
thumbv7em-none-eabi --lib --no-default-features --features mesh_bloom_mcu`
**finishes successfully**. Not just `check` — actual build, .o files
produced. Zero errors. Zero host regression.

---

## 1. Verified commands

```
$ cargo test --lib --release
test result: ok. 415 passed; 0 failed; 0 ignored; 0 measured

$ cargo build --release --bins
    Finished `release` profile [optimized] target(s) in 43.49s
(17/17 bins built)

$ cargo check --target thumbv7em-none-eabi --lib --no-default-features --features mesh_bloom_mcu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.11s
(0 errors, 36 warnings — all unused-variable / dead-code, none functional)

$ cargo build --target thumbv7em-none-eabi --lib --no-default-features --features mesh_bloom_mcu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.05s
(0 errors, real .o files produced for ARMv7E-M)
```

## 2. Trajectory

| Round | MCU errors |
|---|---:|
| Session start (5 hours ago) | 5 827+ |
| Dep-layer fixes | 792 |
| `spore` module gate | 665 |
| Material variants + encrypt gates | ~670 |
| no_std pragma | 295 |
| Option A (alloc prelude pass) | 206 |
| **Option B + libm + final residuals (this round)** | **0** |

**5 827+ → 0** in one continuous session of measured small steps.

## 3. What this round did

### Lib.rs gate (high-leverage)
Gated 5 modules behind `#[cfg(feature = "std")]` because they fundamentally need std (UDP/fs/thread/std::io):
- `spore`, `federation`, `nerve`, `spinal`, `mavlink_min`, `transport`

MCU users build their own transport/MAVLink/sysfs and call the OASIS primitives (mesh, spore_crypto, topics, services, actions, transforms, timers, parameters, hyper_state, emotion, synapse, dreams, morpho, branching, efference, hal, world_model, audio, tension, vitality, reflex, vec) directly.

### Added `libm` dep + `fmath::F64Ext` trait
On std, `f64::cos`/`sin`/`sqrt`/`exp`/`ln`/`atan2`/`powi`/`abs`/`floor`/`ceil`/`round`/`powf` are inherent methods. On no_std they don't resolve. The `F64Ext` extension trait, gated `#[cfg(not(feature = "std"))]`, exposes the SAME method names backed by `libm::*`. Modules that need it `use crate::fmath::F64Ext;` — no other code change needed because the trait method takes precedence over… nothing (no inherent methods on no_std).

11 modules updated to import the trait: transforms, vec, audio, branching, emotion, mesh, hyper_state, synapse, world_model, efference, reflex.

### Constants: `std::f64::consts` → `core::f64::consts`
Same module, both available. Trivially fixed in audio.rs (PI), branching.rs (TAU).

### CounterTracker conditional types
`std::collections::HashMap` and `std::collections::BTreeMap` direct refs in `CounterTracker` swapped to type aliases `HashMapForTracker`/`BTreeMapForTracker` that are `cfg`-conditional.

### `BTreeSet::with_capacity` → `BTreeSet::new`
`BTreeSet` has no `with_capacity` (BTreeSet is tree-based, not array-backed). Conditional construction in `MeshRouter::new` and `with_config`.

### Light residuals
- `dreams.rs`: `std::cmp::Ordering` → `core::cmp::Ordering`
- `synapse.rs`: `std::array::from_fn` → `core::array::from_fn`
- `tension.rs`: added `alloc::vec::Vec` import
- `morpho.rs`: derived `PartialOrd, Ord` on `Role` enum + swapped HashMap→BTreeMap alias
- `emotion.rs`: gated `save_pain`/`load_pain` behind std, gated env-var reads behind std_env

### spore_crypto.rs surgery
- Replaced `std::collections::VecDeque` with conditional alias
- Replaced `std::collections::HashSet` with conditional alias (`BTreeSet as HashSet` on no_std)
- Gated `save_to_file` / `load_from_file` / `save_revocation_file` / `load_revocation_file` behind `std`
- Gated env-var reads behind `std_env`

## 4. Verification

| Check | Pre | Post |
|---|---|---|
| `cargo test --lib --release` | 415/415 | **415/415** (zero regression) |
| `cargo build --release --bins` | 17/17 | **17/17** (43.5 s, zero regression) |
| `cargo check --target thumbv7em-none-eabi …` | 295 errors | **0 errors** |
| `cargo build --target thumbv7em-none-eabi …` | not attempted | **passes** (13 s, .o files produced) |
| Kani proofs | 67 | 67 |

## 5. What MCU users get

Available on `thumbv7em-none-eabi` with `--no-default-features --features mesh_bloom_mcu`:

| Module | Purpose |
|---|---|
| `mesh` | Multi-hop mesh routing + Bloom dedup + v9 signed envelopes |
| `spore_crypto` | All 4 envelope versions via `*_with_material` primitives (v3/v4/v5/v7) |
| `topics` | Pub/sub with BTreeMap-backed router |
| `services` | RPC services |
| `actions` | Long-running actions |
| `transforms` | TF2-equivalent SE(2) (uses libm trig) |
| `timers` | Tick-driven timer registry |
| `parameters` | Typed parameter server |
| `hyper_state`, `emotion`, `synapse`, `dreams`, `morpho`, `branching`, `efference`, `world_model`, `tension`, `vitality`, `audio`, `reflex`, `hal` (partial), `vec` | All bio-inspired mechanisms |

Not available on MCU (gated behind `std`):
- `spore` (transport — UDP/fs)
- `federation` (Ed25519 propagation)
- `nerve` (serde_json + env)
- `spinal` (/sys reads)
- `mavlink_min` (std::io)
- `transport` (UdpSocket + thread)
- `hal::KillSwitch` (std::sync::Mutex) — `clamp_command` and other compute-only HAL items still available

## 6. Honest disclosures (the asterisks)

### 🔴 No actual MCU hardware boot
The build completes; .o files for ARMv7E-M are produced. **I have not flashed this onto a real Cortex-M4 chip.** Linker errors are still possible if a downstream binary integrator's runtime conflicts. The library compiles; whether it links into a flashable image depends on the user's board support crate (e.g., `cortex-m-rt`).

### 🔴 No MCU-side test coverage
Host has 415 tests. MCU has zero — `cargo test` cannot run on an embedded target without a test harness like `defmt-test` and a debug probe. The 415 host tests cover the algorithms; behavioral correctness on MCU **assumes the std and no_std code paths are equivalent**. Where they aren't (BTreeMap vs HashMap iteration order, libm vs std math precision), unit tests don't catch divergence.

### 🔴 Iteration-order divergence remains undetected
`HashMap` (std) iterates in insertion-order (Rust's default randomized seed). `BTreeMap` (no_std alias) iterates in sorted-key order. Any code that depends on iteration order behaves differently on MCU vs host. **Unit tests would not catch this** because they only run on host.

Specific concerns:
- `topics::TopicRouter::dispatch` — multiple subscribers fire in alphabetical order on MCU
- `services::ServiceRouter` — same
- `transforms::TransformTree::lookup` — frame chain walk order may differ
- `morpho::count_roles` — Role iteration sorted

None of these were observed broken in any test. They MIGHT be broken on real hardware.

### 🔴 libm vs std::f64 precision
For `cos`/`sin`/`sqrt`/`exp`/`ln`, libm and std::f64 are **mostly bit-equivalent** but not guaranteed. Edge cases (very small inputs near subnormal, very large near overflow) may diverge by 1-2 ULPs. For OASIS use cases (drone control, navigation — never working at f64 precision limits), this is irrelevant. But it IS a divergence from "MCU and host produce identical results."

### 🔴 `powi(n: i32)` on libm via `pow(self, n as f64)`
Marginally slower and slightly less precise than std::f64::powi. For small integer powers in hot loops, this is a real (small) cost. Not measured.

### 🔴 36 warnings remain
All "unused variable" or "dead code" — none functional. Most are from cfg-gated code paths where the compiler sees one path inactive and warns about the other's variables.

### 🔴 `cargo build --target thumbv7em-none-eabi` is `dev` profile
I didn't try `--release`. Should work but unverified.

### 🔴 No FLASH/RAM size estimate
Real MCU users care about code size. With the Bloom 64 KiB → 2 KiB swap (`mesh_bloom_mcu`), one router is ~6-10 KiB total. The compiled library's text + rodata size on ARM is probably 50-150 KiB but **I did not run `cargo size` or equivalent**.

### 🔴 Single MCU target
Only thumbv7em-none-eabi (Cortex-M4F). thumbv6m (Cortex-M0/M0+, no FPU), thumbv7m (Cortex-M3), thumbv8m, riscv32* — all untested.

## 7. Cumulative scoreboard

| Metric | Session start | Session end |
|---|---:|---:|
| Lib tests (host) | 348 | **415** (+67) |
| Kani proofs VERIFIED | 41 | **67** (+26) |
| Kani failures | 0 | 0 |
| Lib modules | 30 | **35** (+5: timers, parameters, transforms, fmath, services) |
| Cargo features | 1 | **4** (`std`, `std_env`, `os_random`, `mesh_bloom_mcu`) |
| Bins | 14 | **17** (+3: bench_full_stack, bench_mesh_signed, default `oasis-rt` from main.rs) |
| MCU compile status | "not attempted, claimed" | **builds clean for thumbv7em-none-eabi** |
| Lines of Cargo.toml change in session | — | ~15 |
| Lines of `lib.rs` change | — | ~25 |
| Modules touched for no_std | — | ~25 |

## 8. The actually new artifact: `src/fmath.rs`

```rust
// On std: trait is not even compiled (inherent methods take precedence)
// On no_std: trait fills the f64 method gap via libm.
#[cfg(not(feature = "std"))]
pub trait F64Ext {
    fn cos(self) -> f64;
    fn sin(self) -> f64;
    fn atan2(self, other: f64) -> f64;
    fn sqrt(self) -> f64;
    fn exp(self) -> f64;
    fn ln(self) -> f64;
    fn powi(self, n: i32) -> f64;
    fn abs(self) -> f64;
    fn floor(self) -> f64;
    fn ceil(self) -> f64;
    fn round(self) -> f64;
    fn powf(self, n: f64) -> f64;
}
```

This pattern is now reusable for any future cross-target math need.

## 9. Self-critique: what discipline DID and didn't do

**Did well:**
- Atomic, measurable steps. Each round had a clear before/after error count.
- Public retraction when a claim turned out false (MCU support unverified).
- The "audit pattern" forced me to confront divergent claims (FPR off 14×).
- Final round delivered the climax cleanly: 295 → 0 in one push, broken into ~7 conceptual sub-steps.

**Didn't do:**
- Real MCU hardware. Still pure laptop work.
- `cargo size` for FLASH/RAM disclosure.
- A Linux verification of the host changes.
- Any actual MCU runtime test. Compiling ≠ running.
- Cross-architecture (ARM Cortex-M0, RISC-V) verification.
- `cargo build --release` for the MCU target (only `dev` profile tested).

## 10. What this UNLOCKS

- OASIS can credibly claim "no_std-compatible". Not "MCU-deployed", but no_std-compatible at the source level.
- Future MCU-specific work has a clean foundation: add a `cortex-m-rt`-using bin in a separate crate, wire transport, flash, run.
- The fmath pattern + module-gating pattern are reusable when the next batch of std deps gets added.

## 11. Next-step candidates (post-MCU)

The MCU port is now a dischargeable claim. Returning to features:

1. **Per-node Ed25519 mesh signatures** — closes insider attack vector
2. **Typed messages derive macro** — biggest DX gap vs ROS 2
3. **200-drone sim re-run with v9 + 64 KiB Bloom** — validation of recent changes
4. **`oasis_topic` CLI** (echo/list/pub/call) — ROS 2 ergonomic parity
5. **3D transforms (SE(3))** — tf2 quaternion equivalent
6. **Real MCU hardware boot** (would close one of the disclosed gaps)
