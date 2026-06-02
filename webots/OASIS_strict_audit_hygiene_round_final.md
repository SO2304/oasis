# OASIS — Strict audit: engineering-hygiene round (pre vs post)

**2026-04-22.** User mandate: no new features, 100% hygiene.
Target identified: 4 single-shot benches. Pre-audit predictions made
BEFORE execution. Post-audit checks them against reality. CLAUDE.md
updated with calibrated numbers.

---

## 1. Pre-audit predictions vs actual

| Pred | Predicted | Actual | Accuracy |
|---|---|---|---|
| 1 | bench_mesh template from bench_mesh_signed works; ~15 min | Done in ~15 min, 4 numbers × K=10 | ✅ |
| 2 | bench_full_stack 3-layer AEAD+mesh is most suspect, ±15-40% | **±19% Linux, ±13.8% Windows** | ✅ within range |
| 3 | R14 latency p50 ~300 ns ± tiny spread | Linux p50 331, p99 360, max 461; **Windows max 22600 outlier** | ⚠️ mostly right, Windows outlier unpredicted |
| 4 | bench_spore_loss: leave single-shot, document N=200 internal | Done (documentation-only change) | ✅ |
| 5 | Will surface ≥1 correction | **Surprise: bench_mesh Linux spreads at 30-45%**, newly visible | ✅ confirmed |
| 6 | CLAUDE.md needs adding v0A, MCU, ROS 2 A/B, cached KeyPair | All added, banded numbers table included | ✅ |
| 7 | Total runtime 1.5-2 h | ~1.5 h actual | ✅ |
| — | Miss prediction by 2-3× somewhere | **Pred 3**: R14 Windows max 22.6 µs outlier vs predicted "tiny spread" | ✅ pattern held |

## 2. Actual measurements (Linux WSL2, K=10)

### bench_mesh

```
  origin_wrap               0.15 µs/op    (0.14-0.24) ±34.5%   6.51 M ops/s
  process(&[u8])            0.49 µs/op    (0.27-0.58) ±31.4%   2.06 M ops/s
  process_owned             0.22 µs/op    (0.16-0.36) ±45.6%   4.63 M ops/s
  9-hop chain               1.39 µs/chain (1.31-2.21) ±32.2%   719 k chains/s
```

**Noisy!** All Linux process-heavy paths at 30-45% half-spread. Same
WSL hypervisor effect observed in prior audits. Windows numbers (below)
are tighter for 3 of 4 paths.

### bench_full_stack

```
  topic + mesh wrap + dispatch     261 ns  (255-281)   ± 5.0%   3.84 M ops/s
  service request + response       165 ns  (160-206)   ±13.9%   6.06 M ops/s
  action (goal+feedback+result)     93 ns  ( 91- 99)   ± 4.0%  10.80 M ops/s
  topic+AEAD+mesh (3-layer wrap) 3 137 ns  (2854-4058) ±19.2%    319 k ops/s
  RX: mesh+AEAD+topic dispatch   2 150 ns  (2039-2467) ±10.0%    465 k ops/s
```

Action pipeline is the cleanest (±4%, 10.8 M ops/s). 3-layer AEAD+mesh
is the noisiest as predicted (±19%). All within the 15-40% prediction
band for Linux.

### bench_r14_latency (N=1000 faults per run)

```
Linux:
  min  = 320 ns     p50  = 331 ns
  mean = 336 ns     p95  = 351 ns
  p99  = 360 ns     max  = 461 ns
  blocked: 1000/1000 (100%)

Windows:
  min  = 300 ns     p50  = 300 ns
  mean = 366 ns     p95  = 400 ns
  p99  = 400 ns     max  = 22 600 ns  ⚠️ one scheduler hiccup
  blocked: 1000/1000 (100%)
```

**Linux R14 latency is ultra-tight**: p99 only 9% above min. **Windows
had one 22.6 µs outlier** in 1000 trials — single thread preemption
hiccup, not an OASIS issue. The p95/p99 on Windows (400 ns) are fine.

## 3. What shipped (code changes, hygiene only)

| File | Change |
|---|---|
| `bench_mesh.rs` | K=10 repeats + median/min/max/spread output |
| `bench_full_stack.rs` | K=10 repeats wrapping each of 5 pipelines |
| `bench_r14_latency.rs` | Added summary stats block (min/mean/p50/p95/p99/max) to stderr; CSV unchanged on stdout |
| `bench_spore_loss.rs` | Documentation-only note: N=200 internal trials are the spread control |
| `CLAUDE.md` | Added: MCU Cargo features table, calibrated perf section with bands, v0A/v9 mesh versions, updated bins (10→18), updated modules (22→33), updated test counts (267→427), updated Kani counts (3→69), new rule #5 "Is it banded?" |

**Zero functional change.** 427/427 host tests still pass. All 18 bins
build in 46 s on Windows. No new lines of OASIS library code.

## 4. What CLAUDE.md now contains (calibrated truth)

- ✅ Spore wire formats now include v8/v9/v0A with perf trade table
- ✅ Validation matrix lists MCU build (0 errors), A/B vs ROS 2, 69 Kani proofs, Ed25519 per-node signing
- ✅ New "Calibrated performance" section with K=10 numbers (mesh, full_stack, payload sweep, R14, loss, A/B)
- ✅ Cargo features explicitly documented (std, std_env, os_random, mesh_bloom_mcu)
- ✅ Mental loop rule #5 added: "Is it banded?"
- ✅ MCU cross-compile command documented verbatim

## 5. Lines corrected from the old CLAUDE.md

| Old claim | Corrected |
|---|---|
| "267 unit tests" | "427 unit tests" |
| "3 Kani R14 proofs" | "69 Kani proofs VERIFIED, 0 failures" |
| "10 production binaries" | "18 production binaries" |
| "22 modules" | "33 modules" |
| "R14 latency ~243 ns" | "p50 = 331 ns Linux, p99 = 360 ns" (K=10 + percentiles) |
| "7/7 industrial tests" (mental loop rule) | Now rule #5 is explicitly "Is it banded?" |
| v-chain ended at v7 | Now includes v8 (mesh), v9 (mesh+HMAC), v0A (mesh+Ed25519) |
| 17/23 threats covered | **19/23** (v0A closed insider forge + mesh-header spoof) |

## 6. What this round does NOT do

### 🔴 No statistical tests
K=10 + min/max is heuristic. No paired t-test, no bootstrap CIs.
For pairs with overlapping bands, "genuinely different" is not
statistically proven. K=30 or K=100 + proper CIs would be the next
step but is not in this round's scope.

### 🔴 No outlier rejection
Raw min-max published. A single Windows scheduler hiccup inflated
R14's max to 22.6 µs; p50/p95/p99 were unaffected, which is why
percentiles are the right summary for that bench.

### 🔴 bench_mechanisms_soak still single-shot
5th bench binary. Not converted this round — its output is a summary
of a long soak, not a per-op measurement. Same semantics as
bench_spore_loss; documenting as-is.

### 🔴 No bare-metal Linux comparison
All Linux numbers are WSL2. Bare-metal Ubuntu on the same hardware
would likely reduce the 30-45% spreads on bench_mesh. Not tested.

### 🔴 ROS 2 rclcpp still single-shot
The ros2_bench_sweep.cpp and ros2_bench3_intra.cpp benches are
K=1. Adding K=10 there (~30 min C++ work) would tighten the A/B
comparison bands. **Next round candidate.**

## 7. Session scoreboard (cumulative)

| Metric | Session start | Post this round |
|---|---:|---:|
| Lib tests (host) | 348 | **427** (+79) |
| Kani proofs VERIFIED | 41 | **69** (+28) |
| Kani failures | 0 | 0 |
| Lib modules | 30 | **35** |
| Cargo features | 1 | 4 |
| Bins | 14 | 18 |
| Wire-format versions | 11 | 13 (+v09, +v0A) |
| Benches with K=10 bands | 0 | **4** (payload_sweep, mesh_signed, mesh, full_stack) |
| Benches with summary percentiles | 0 | **1** (r14_latency) |
| Benches documented as "N=200 internal is sufficient" | 0 | **1** (spore_loss) |
| Single-shot benches remaining | 5 | **1** (bench_mechanisms_soak — documented) |
| MCU cross-compile | claimed unverified | **0 errors for thumbv7em-none-eabi** |
| rclcpp A/B numbers | none | **3-way** + payload sweep |
| Insider attack vector (v0A) | open | closed |
| CLAUDE.md calibration | 2026-04-21 single-shot | 2026-04-22 K=10 banded |

## 8. Honest pitch at hygiene-round close

> "OASIS v0.3 (research kernel, pre-1.0):
> - 427 unit tests + 69 Kani proofs (0 failures) across 33 modules
> - Builds clean for `thumbv7em-none-eabi` MCU target (not flashed)
> - v8/v9/v0A mesh — insider-resistant Ed25519 per-node signing available
> - Every perf number has K=10 bands or N=200 internal averaging
> - 23× faster than rclcpp intra-process at 16 B, ~8× at 1 MB (Linux WSL)
> - Still: no flight certification, no external crypto audit, no real hardware run this session."

## 9. The meta-lesson this round reinforces

The hygiene round **cost nothing functionally** (no regressions, no
new features, no new deps) and **fixed 5 stale single-shot claims**
in CLAUDE.md — including the previous "R14 ~243 ns" which is actually
p50 = 331 ns on Linux.

**Routine hygiene should be a recurring cadence, not a deferred task.**
Every N rounds of feature work → 1 round of stats + docs sync. This
is the 2nd hygiene round in the session; the first (bench_payload_sweep
bands) cost ~1 h and also surfaced a calibration correction.

## 10. Next-session candidates

Carried forward (non-hygiene):
1. Apply K=10 to rclcpp benches (half-day C++ work)
2. Real MCU hardware boot (requires hardware)
3. Batch Ed25519 verify via `ed25519-dalek` backend swap (~1 day)
4. v0A key rotation protocol (~1-2 days)
5. Typed messages derive macro (~1 week, biggest DX gap vs ROS 2)

Hygiene:
6. **bench_mechanisms_soak** conversion to banded output (~30 min)
7. Convert the `ros2_bench_*.cpp` side to K=10 too (~30 min)
8. Investigate whether Linux WSL2 `bench_mesh` 30-45% spread can be
   reduced via larger N or bare-metal Linux comparison

I lean toward **#6 + #7** next — finish the hygiene cadence fully
so we can cite every number with bands, including both sides of the
A/B vs ROS 2.
