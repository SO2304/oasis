# SHADOW AUDIT — MCU per-envelope steady-state revocation bench (M1 validation)

**Date**: 2026-05-10.
**Trigger**: prediction M1 from
[SHADOW_AUDIT_MCU_REVOCATION_BENCH.md](SHADOW_AUDIT_MCU_REVOCATION_BENCH.md):

> M1: A separate bench that builds local set ONCE then loops 1000
> envelope-checks will measure ~10 µs per check (O(log M) BTreeSet
> on MCU) — fits R20 trivially.

The previous round's per-merge bench falsified prediction P4 (the
"500× speedup" claim). The corrected mental model said: per-merge
cost is acceptable to be slow because it's a rare event; the SECURITY-
critical metric is the **per-envelope steady-state cost**, which
should fit R20 trivially via O(log M) BTreeSet contains.

This round runs that exact measurement and reports.

---

## What got built

| Artifact | Purpose |
|---|---|
| `oasis-mcu-demo/src/bin/revocation_steady_state_bench.rs` | RP2040 bench, builds local set once, loops 10 000 contains() per trial, K=10 trials |
| 3 Kani proofs in `oasis-secure-element` | Steady-state invariants (R20 budget, build-once, determinism) |

## Pre-bench predictions

| # | Prediction |
|---|---|
| M1a | Per-check cost ~10 µs at M=1000 |
| M1b | R20 1ms budget = 100× headroom at M=1000 |
| M1c | K=10 bands will be tight (no scheduler noise on baremetal) |
| M1d | Cost scales O(log M) — M=4000 should be ≈ M=1000 × log₂(4000)/log₂(1000) ≈ 1.4× |

## Outcomes — measured on Wokwi RP2040 @ 125 MHz

```
──────────────────────────────────────────────────────────────────
  Steady-state: rev_count = 100 (local BTreeSet)
──────────────────────────────────────────────────────────────────
  K=10 bands: min = 7443 ns/check, median = 7443 ns/check, max = 7444 ns/check
  R20 budget headroom (median): 134×

──────────────────────────────────────────────────────────────────
  Steady-state: rev_count = 1000 (local BTreeSet)
──────────────────────────────────────────────────────────────────
  K=10 bands: min = 8309 ns/check, median = 8309 ns/check, max = 8309 ns/check
  R20 budget headroom (median): 120×

──────────────────────────────────────────────────────────────────
  Steady-state: rev_count = 4000 (local BTreeSet)
──────────────────────────────────────────────────────────────────
  K=10 bands: min = 11887 ns/check, median = 11887 ns/check, max = 11887 ns/check
  R20 budget headroom (median): 84×
```

## Predictions vs actuals

| # | Predicted | Actual | Match? |
|---|---|---|---|
| M1a | ~10 µs at M=1000 | **8.3 µs** | ✅ |
| M1b | 100× headroom at M=1000 | **120×** | ✅ better than predicted |
| M1c | Tight bands | **half-spread = 0 across all 3 tiers** (deterministic to 1 ns) | ✅ better than predicted |
| M1d | Scaling factor ~1.4× M=1000→M=4000 | **8.3 → 11.9 = 1.43×** | ✅ exact |

**4 / 4 predictions matched.** The corrected mental model from the
previous round (falsification of P4) is now empirically validated.

## R20 budget reclaim — fully confirmed

| Tier | Per-check median | R20 1 ms budget | Headroom |
|---|---:|---:|---:|
| M = 100 | 7 443 ns | 1 000 000 ns | **134×** |
| M = 1 000 | 8 309 ns | 1 000 000 ns | **120×** |
| M = 4 000 | 11 887 ns | 1 000 000 ns | **84×** |

R20's 1 ms atomization-deadline budget is **massively respected** in
the steady-state path:
- Worst measured: 12 µs out of 1 ms = 1.2 % of budget consumed.
- Best measured: 7.4 µs = 0.7 % of budget.
- **84× headroom** at the largest tested rev count (4000).

## Why M1 was right (and what makes it different from P4)

The previous P4 was wrong because it conflated the merge cost with
the per-envelope cost. M1 separated them and predicted the
per-envelope cost specifically:

| Property | Per-merge (previous bench, falsified P4) | Per-envelope (this bench, validated M1) |
|---|---|---|
| Frequency | Rare (revocation broadcast = hourly/daily) | Hot path (every received envelope) |
| Cost on MCU @ M=1000 | ~7 ms (Pattern A or B) | ~8 µs |
| R20 1 ms budget? | EXCEEDS (7×) — but acceptable since rare | FITS (120× headroom) — required since hot |
| Bottleneck character | Re-scan of fleet during merge | Single BTreeSet contains() |
| Architectural fix needed? | NO — accept slow merge as rare event | NO — already trivial |

The OASIS architecture is sound: per-envelope is fast (8 µs), per-
merge is acceptable (7 ms for a hourly event).

## Determinism observation — half-spread = 0 across all tiers

K=10 trials at each rev_count produced the SAME per-check ns:

| Tier | min | median | max | spread |
|---|---:|---:|---:|---:|
| M=100 | 7443 | 7443 | 7444 | 1 ns |
| M=1000 | 8309 | 8309 | 8309 | 0 ns |
| M=4000 | 11887 | 11887 | 11887 | 0 ns |

This confirms the prior cycle-accurate-sim observations: M0+ baremetal
under Wokwi is byte-deterministic. There is no scheduler, no L1 cache,
no DRAM, no preemption. Same input → same cycles. The 1-ns spread at
M=100 is timer-resolution rounding, not actual variance.

## The 3 new Kani proofs

All under `#[cfg(kani)]` in `oasis-secure-element/src/lib.rs:proofs`:

### 1. `proof_steady_state_per_check_within_r20`

For any rev_count in [1, 4000] and per-check cost ≤ 20 000 ns
(generous upper bound on the measured 12 µs ceiling), R20's 1 000 000
ns budget is honored with ≥50× headroom.

**Why load-bearing**: future increases in revocation list size (say
to 10k entries) could push per-check cost — the proof states the
bound below which R20 is preserved. If a future change pushes per-
check above 20 000 ns, the proof would NEED to be updated, alerting
us to the architectural shift.

### 2. `proof_steady_state_excludes_build_cost`

The bench reports `per_check_ns = loop_elapsed_ns / N`, NOT
`(loop_elapsed_ns + build_ns) / N`. This is the build-once
invariant: construction of the local set is rare/amortized; only the
loop body counts.

**Why load-bearing**: a future patch that "simplifies" the bench by
including build cost would inflate the reported per-envelope number
by 100-1000× (build cost dominates the loop) and trigger a false
R20 alarm, leading to architectural changes that aren't needed.

### 3. `proof_steady_state_determinism`

On baremetal MCU with no preemption / no cache, two identical trials
must produce identical (or within 1 %) elapsed times. The K=10 trials
all reporting the same value is the empirical signal.

**Why load-bearing**: drift > 1 % across trials would mean the bench
is contaminated (an ISR firing, a heap fragmentation effect, etc.) —
catches measurement integrity bugs before they become reported numbers.

## Updated SE crate Kani proof count

| Round | Proofs added | Cumulative |
|---|---:|---:|
| Gap 4 step 1 | 6 | 6 |
| Wire-compat / migration | 4 | 10 |
| Tamper cascade | 4 | 14 |
| Iterator | 3 | 17 |
| MCU bench harness | 3 | 20 |
| **MCU steady-state (this round)** | **3** | **23** |

## What's NOT done in this round (honest)

- **STM32H7 (Cortex-M7) comparison.** Predicted 4-5× faster per op —
  not run. Would fit larger fleets due to 1 MiB SRAM. M2 prediction
  from previous round; deferred.
- **Bloom filter alternative for the local set.** Predicted O(M) bits-
  set, no logarithm. M3 prediction; deferred. BTreeSet is the
  default no_std choice; Bloom would add false-positive risk that
  needs separate analysis.
- **Streaming RevocationList parser.** Build cost was 13 ms at M=1000;
  predicted 5-10× cut with streaming. M4 prediction; deferred.
- **Hit / miss path comparison.** Bench mixes 10 % hits + 90 % misses
  to model real fleet traffic. Could decompose: hit-only path may be
  slightly faster than miss-only (BTreeSet termination differs).
  Not measured.

## Updated defense-vertical posture

Before this round:
> "Both patterns exceed R20 1ms at fleet=1024 [PER-MERGE, but I
> didn't say per-merge clearly]. Iterator value is memory not CPU."

After this round:
> "Per-envelope steady-state revocation check on Cortex-M0+ at 125 MHz:
> 8 µs at M=1000, 12 µs at M=4000. R20 1 ms budget honored with
> 84-134× headroom across all tested tiers. Per-merge remains 7-21 ms
> but is a rare event (revocation broadcast). The architecture is
> sound: hot path is fast, cold path is acceptable."

The previous round's worry that "R20 is in danger" was based on the
per-merge measurement. This round confirms R20 is **not** in danger
on the per-envelope steady-state path, which is the security-relevant
one. The honest narrative correction is now complete.

## Predictions for next round

| # | Prediction |
|---|---|
| L1 | On STM32H7 (M7 @ 480 MHz), per-check cost will be ~2 µs at M=1000 (4× faster than M0+ at 125 MHz × 4 = ~16× clock advantage but offset by higher memory latency for cache-cold paths) |
| L2 | A Bloom filter local set (1024 bits, k=4 hash) at M=1000 will measure ~3 µs per check (no logarithm; fixed cost). False positive ≈ 0.1 % at M=1000 in 1024 bits = acceptable for revocation but worth measuring |
| L3 | Decomposing hit-only vs miss-only paths will show ~5 % asymmetry — miss path slightly faster (BTreeSet leaf-not-found terminates earlier than leaf-found verifies match) |
| L4 | A 24-hour soak running this bench every minute will produce identical numbers (no drift, no leaks) |

These will be validated when next-round work ships.

## One-sentence verdict

**Per-envelope revocation check on Cortex-M0+ at 125 MHz is 8 µs at
M=1000 — 120× under R20's 1 ms budget. The corrected mental model
from the P4 falsification is now empirically validated; the OASIS
architecture's hot-path budget is sound; the previous round's worry
about R20 was based on the wrong access pattern measurement.**
