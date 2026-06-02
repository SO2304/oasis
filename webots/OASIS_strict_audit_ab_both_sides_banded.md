# OASIS — Strict audit: A/B banded both sides (the correction)

**2026-04-22.** Continued hygiene: applied K=10 repeats to the rclcpp
side of the A/B comparison. Result: **the 1 MB "8× faster" claim
tightens to "3.75× faster"** because rclcpp's prior K=1 number was
a slow outlier. Honest correction now in CLAUDE.md.

---

## 1. Pre-audit predictions vs actual

| Pred | Predicted | Actual | Accuracy |
|---|---|---|---|
| 1 | rclcpp medians drift ±10-15% vs K=1 | Drifted **DOWN 14-55%** | ❌ **3× larger than predicted** — rclcpp K=1 was slower than median |
| 2 | DDS highest spread ±15-30% | ±37.4% | ⚠️ spread exceeded upper bound |
| 3 | 1 MB intra lowest spread (±5%) | **±54%** ⚠️ | ❌ completely wrong — 1 MB has HIGHEST spread |
| 4 | "23× gap" holds ±10% | 28× at 16 B (+22%), **3.75× at 1 MB (−55%)** | ❌ wildly off at 1 MB |
| 5 | ~60 min | ~45 min actual | ✅ |
| 6 | One unexpected finding | **1 MB rclcpp median is 55% faster than K=1** | ✅ confirmed |

**Pattern continues: my predictions keep being wrong in the direction
that doesn't favour OASIS's prior claims.** At 1 MB, rclcpp is much
faster than I'd quoted, not slower. Correction in both directions this
time — I've been miscalibrated BOTH ways at different times this session.

## 2. Banded rclcpp measurements (Linux WSL2, K=10)

### ros2_bench_sweep (intra-process)

```
       payload    median      (min-max)          ±spread      ops/s
        16 B       4 562 ns  (4 339-4 942)       ± 6.6%      219 209
        1 KB       4 951 ns  (4 329-5 512)       ±11.9%      201 994
       64 KB       5 175 ns  (3 195-6 716)       ±34.0%      193 245
        1 MB     183 340 ns  (127 133-325 996)   ±54.2% ⚠️    5 454
```

### ros2_bench2 (DDS default)

```
Run 1-10: 42241, 20724, 28176, 54553, 51987, 42312, 45242, 51235, 32094, 46216
Median: 45 243 ns/op  (20 724-54 554)  ±37.4%  (22 103 ops/s)
```

## 3. Before/after comparison

| | K=1 single-shot | K=10 median | Drift |
|---|---:|---:|---:|
| rclcpp intra 16 B | 5 624 ns | 4 562 ns | **−19%** |
| rclcpp intra 1 KB | 7 067 ns | 4 951 ns | **−30%** |
| rclcpp intra 64 KB | 8 000 ns | 5 175 ns | **−35%** |
| rclcpp intra 1 MB | 406 645 ns | 183 340 ns | **−55%** |
| rclcpp DDS 16 B | 52 411 ns | 45 243 ns | **−14%** |

**Every rclcpp number drifted lower under K=10.** Single-shot runs
systematically overestimated cost. 1 MB was the worst case — 55% off.

## 4. Updated ratios (both sides banded)

| Payload | OASIS (K=10 median) | rclcpp intra (K=10 median) | Ratio |
|---:|---:|---:|---:|
| 16 B | 162 ns ±12% | 4 562 ns ±6.6% | **28× faster** |
| 1 KB | 247 ns ±8% | 4 951 ns ±12% | **20× faster** |
| 64 KB | 2 813 ns ±9% | 5 175 ns ±34% | **1.8× faster** |
| 1 MB | 48 819 ns ±8% | 183 340 ns ±54% | **3.75× faster** |

At 16 B the gap widened (23× → 28×). At 1 MB it tightened (8× → 3.75×).
**OASIS still wins at every payload size**, but the magnitudes are
honest now.

## 5. What shipped (hygiene only)

| File | Change |
|---|---|
| `c:\tmp\ros2_bench_sweep.cpp` | Rewritten for K=10 + median/min/max output |
| `c:\tmp\ros2_bench2.cpp` | Rewritten for K=10 (full init cycle per run), prints median |
| `CLAUDE.md` A/B section | Banded both sides; ratios updated; 1 MB corrected 7.85× → 3.75× |

**Zero OASIS library changes.** Same 427/427 tests pass. Same 18 bins
build. This round is purely correcting an external-measurement claim.

## 6. What this round does NOT do

### 🔴 ros2_bench3_intra.cpp not banded
Redundant with ros2_bench_sweep which covers 16 B intra at K=10.
The standalone bench was a one-point measurement; skipped as
low-value.

### 🔴 bench_mechanisms_soak still single-shot
Last OASIS bench without bands. ~30 min conversion; deferred again.
The pattern of deferring this specific item is now 4 rounds long.

### 🔴 No statistical tests
Still just min/max + median. For "is OASIS-vs-rclcpp 1 MB genuinely
3.75× or could it be 2-5×?" — formally unclear. The 95% CIs on both
sides likely overlap at the band edges.

### 🔴 No bare-metal Linux rclcpp
WSL2 Linux hypervisor variance likely contributes to rclcpp's ±37-54%
spreads. A bare-metal Ubuntu box would probably tighten those.

### 🔴 No Windows rclcpp
I have no way to run ROS 2 Jazzy on Windows. Cross-platform
comparison stays Linux-only.

## 7. Calibration history

Running tally of my prediction misses this session:

| Prediction | Actual | Direction | Magnitude |
|---|---|---|---|
| Bloom FPR at 20k: 0.3% | 4.3% theoretical, 61% observed (probe-feedback) | pessimistic | 14× |
| MCU pragma: 1 day | 1 hour | **pessimistic** | 8× |
| v0A sign: 30-100 µs | 370 µs | optimistic | 3-12× |
| Buffer-backed refactor 1 MB: "close the gap" | reverse by 60× | **pessimistic** | huge |
| KeyPair cache: 3× speedup | 1.48× | optimistic | 2× |
| rclcpp band drift ±10-15% | 14-55% | pessimistic | 3× |
| rclcpp gap holds ±10% | 1 MB: ±55% shift | optimistic | 5× |

**Net:** 4 optimistic, 3 pessimistic. Variance in both directions.
Lesson still holds: **measure before predicting magnitudes**.

## 8. Scoreboard (post-hygiene-continuation)

| Metric | Prior round | Now |
|---|---:|---:|
| Lib tests | 427 | 427 (unchanged) |
| Kani proofs | 69 | 69 |
| Benches with K=10 on OASIS side | 4 | 4 |
| Benches with K=10 on rclcpp side | 0 | **2** (+sweep, +DDS) |
| CLAUDE.md A/B numbers | K=1 for rclcpp, K=10 for OASIS | **K=10 both sides** |
| Corrected claims this round | n/a | **5** (all 4 sweep points + 16 B DDS) |

## 9. The updated calibrated pitch

Prior:
> "OASIS 7.85× faster than rclcpp intra-process at 1 MB"

Corrected:
> "OASIS **3.75× faster** than rclcpp intra-process at 1 MB (both K=10
>  medians, OASIS ±8%, rclcpp ±54%). Above 64 KB the performance
>  advantage narrows toward parity as zero-copy sharing of the unique_ptr
>  starts dominating rclcpp's per-call overhead."

At small payloads (16 B, 1 KB) the OASIS advantage is decisive
(20-28×) because per-call overhead dominates both stacks and OASIS's
is much smaller. Above 64 KB, rclcpp's zero-copy semantics close the
gap; the refactored OASIS `origin_wrap_with` pulls ahead but only
modestly.

## 10. Next-step candidates

Hygiene:
1. **bench_mechanisms_soak** (~30 min) — the one remaining single-shot
   OASIS bench. 4th round in a row I've deferred this.
2. Bare-metal Linux comparison to isolate WSL2 hypervisor from rclcpp
   variance. Hardware-dependent; not actionable without a second box.

Features (not this session's mandate):
3. v0A key rotation, batch verify, typed messages derive macro, real
   MCU hardware boot.

I should ship **#1 next round** to keep the "no single-shot bench"
mental-loop rule honest. The deferral has been flagged 4 rounds in a
row — at some point continuing to defer it undermines the discipline.
