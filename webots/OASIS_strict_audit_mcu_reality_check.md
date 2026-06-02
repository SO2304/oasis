# OASIS — Strict audit: MCU cross-compile reality check

**2026-04-22.** I claimed last round that the `mesh_bloom_mcu` feature
"unblocks MCU / embedded targets." I even self-critiqued by warning
"I did not actually cross-compile." This round I tried. **The claim
was misleading.** This audit documents what actually breaks and walks
back the prior characterization.

---

## 1. What I attempted

```
rustup target add thumbv7em-none-eabi            # Cortex-M4F
cargo check --target thumbv7em-none-eabi \
            --lib                                \
            --no-default-features                \
            --features mesh_bloom_mcu
```

## 2. What actually happened

**The check fails.** Two separate blockers in the dep tree, neither
of which the MCU feature flag addresses:

### Blocker 1: `getrandom` (4 errors)
```
error: target is not supported. You may need to define a custom backend
       see: https://docs.rs/getrandom/0.3.4/#custom-backend
error[E0463]: can't find crate for `std`
error[E0425]: cannot find function `fill_inner` in module `backends`
```

`getrandom` v0.3+ requires an explicit target selection. For `thumbv7em`,
the caller must provide a custom backend (e.g., hardware RNG on the
MCU). OASIS's `Cargo.toml` has:

```toml
getrandom = "0.2"
```

— no backend specified, no way to specify one without user code.

### Blocker 2: `serde_core` (5 827 errors)
Every error is a variant of "can't find crate for std" — because OASIS's
`Cargo.toml` has:

```toml
serde = { version = "1", features = ["derive"] }
```

No `default-features = false`. Serde pulls `std` unconditionally. To
fix, the dep line must be:

```toml
serde = { version = "1", default-features = false, features = ["derive", "alloc"] }
```

### Blocker 3 (not reached, but present): the OASIS code itself

`cargo check` aborts at the dep layer, so oasis-rt's own modules never
compile. If the dep issues were fixed, further breaks are certain:

- `lib.rs` has no `#![cfg_attr(not(feature = "std"), no_std)]` pragma.
- Many modules use `std::collections::{HashMap, HashSet, VecDeque}`,
  `std::thread`, `std::fs`, `std::time::Instant`, `std::env`.
- `main.rs` is an Android daemon — std-only by design, but it's excluded
  from `--lib` builds, so it doesn't break the library path.

## 3. What this means for last round's claim

### The misleading statement I made
> "MCU / embedded targets can now use the mesh layer by opting into
> the smaller Bloom."

### What is actually true
The `mesh_bloom_mcu` feature shrinks the Bloom filter's RAM footprint
from 64 KiB to 2 KiB, **conditional on the crate compiling for the
target in the first place**. The crate does not compile for
`thumbv7em-none-eabi`. Therefore the feature flag is **cosmetic for
its stated purpose** at this point in time.

It is still useful on x86_64 / aarch64 hosts where an operator chooses
to minimize RAM (e.g., running 1000 in-process routers in a sim). It
does not unblock embedded deployment.

### I should have caught this earlier
The prior audit's "Not measured: real MCU cross-compile" was an implicit
admission that the claim was unverified. I went ahead and presented the
feature as unblocking MCU anyway. That was wrong.

## 4. Scoreboard

| Metric | Pre | Post |
|---|---|---|
| Lib tests | 399 | 399 |
| Kani proofs | 67 | 67 |
| Cargo features | 2 | 2 |
| MCU build status | claimed unblocked | **confirmed blocked** |

## 5. What honest MCU support actually requires

Rough scope estimate (not done this round):

| Step | Effort | Risk |
|---|---|---|
| Fix `serde` dep to `default-features = false, features = ["derive", "alloc"]` | 5 min | low |
| Gate `serde_json` behind an `std` feature (used by drone_bridge) | 30 min | low |
| Switch `getrandom` to provide user-configurable backend / make optional | 2 h | medium |
| Add `#![cfg_attr(not(feature = "std"), no_std)]` + `extern crate alloc` | 15 min | low |
| Gate every `std::` usage behind `#[cfg(feature = "std")]` across 30+ modules | **2-3 days** | **medium-high** |
| Split `lib.rs` into std vs core modules (some are hopeless for no_std: `main.rs`, `federation.rs` Ed25519 signing) | 1 day | medium |
| Actually boot on a real Cortex-M board | 1 day | high |

**Realistic total: 3–5 days focused work.** Not a one-round task.

## 6. What I did NOT do (but could have)

- Fix the serde dep declaration (5 min). Would reduce errors from 5 827
  to whatever comes next. I deliberately did not, because a partial fix
  would produce a misleading "progress" story without actually unblocking
  anything. **Better to have zero compile success than fake partial
  success.**
- Open an issue in an issue tracker. OASIS has no tracker; I wrote it
  here instead.
- Remove `getrandom` as a dep entirely (only used for AEAD nonce in
  spore_crypto; could be user-provided). This would be a surgical
  fix but changes crypto API surface. Out of scope.

## 7. What this audit asks the reader to do

Treat any prior claim of "MCU support" in OASIS docs as **aspirational
until cross-compile is demonstrated**. I will update the prior audit
file with a link to this one so the retraction is visible.

The `mesh_bloom_mcu` feature still exists and is tested on x86_64. Its
purpose will become real once the above 3–5 days of no_std porting
happens, not before.

## 8. Self-critique

Three-strike pattern now observed:

1. Strict audit claimed Bloom FPR = 0.3 %. Bench showed 61 %. **Off 14×.**
2. Strict audit claimed MCU support unblocked. Cross-compile fails before
   oasis-rt's own code is reached. **Unverified claim.**
3. (Not yet disproven: Kani proofs count as verification of correctness.
   Prior rigor-breakdown partially addressed this, but many pure-helper
   proofs do NOT verify the composed state machines that use them.)

Common cause: I report things that "should" work given assumptions, instead
of things I've actually run. Going forward: if an audit claims X, the
same round should contain the exact command and its output that
demonstrates X, or the claim should explicitly be marked "unverified."

## 9. Concrete change to make next round

Two options, pick one:

### Option A — actually fix the first blocker (serde + getrandom)
Small scope. 2 hours. Outcome: either we see oasis-rt's own code
compile errors (productive, informative) or we discover more dep-level
issues. Either way we learn something real.

### Option B — drop the MCU feature + retract the claim
Remove `mesh_bloom_mcu` from `Cargo.toml`. Delete the `BLOOM_WORDS`
conditional in `mesh.rs`. Mark the last audit doc as retracted.
Accept that OASIS is a host-side library and move on.

I lean toward **Option A** — we've committed to the MCU direction
across two rounds now, and the fix is cheap enough to try. But I
won't claim success until `cargo check --target thumbv7em-none-eabi`
produces output I actually post.

## 10. Non-coverage disclosures

- I only tried ONE MCU target (thumbv7em-none-eabi, Cortex-M4F).
  Behavior on thumbv6m, thumbv8m, or other targets not tested.
- I did not try `cargo build` (just `check`). Linking may add more
  failures via panic handler, alloc hooks, etc.
- I did not even attempt `aarch64-unknown-none`, `riscv32i-unknown-none-elf`,
  or any other no_std target.
- I did not try to use `alloc`-only builds (some crates have `alloc`
  features that might work).
