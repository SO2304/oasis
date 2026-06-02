# OASIS — Strict audit: `bench_mechanisms_soak` banded (deferral closed)

**2026-04-22.** The deferred item from 4 prior audits — banding
`bench_mechanisms_soak` — finally shipped. **Every single-shot bench
in the repo is now banded.** Bonus: because the bench uses seeded
LCG inputs, the K=10 pattern ALSO acts as a cross-run determinism
tripwire. Pred 4 was wrong in the good direction — spread is
tighter than predicted.

---

## 1. Pre-audit predictions vs actual

| Pred | Predicted | Actual | Accuracy |
|---|---|---|---|
| 1 | Soak is a long-running single pass, K=10 expensive | Each run ~170 ms, K=10 total ~1.7s | ✅ cheap |
| 2 | Output shape per-mechanism or aggregate | Mix: invariant status + aggregate tick rate | ✅ adapted |
| 3 | ≥30 min code work | ~45 min (extraction of `run_soak` fn) | ✅ |
| 4 | ±20-40% spread on some metric | **±7.2% Windows, ±11.8% Linux** | ❌ **way better than predicted** |
| 5 | One calibration correction | No correction — old "~58k Hz" tick rate holds | ⚠️ no surprise this time |
| 6 | 45-60 min total | ~45 min | ✅ |

**Pred 4 was miscalibrated in OASIS's favour**: I expected mechanism
state-machine soak to be noisy. Because inputs are deterministic, only
the TIMING varies, and on modern CPUs the 10k-tick loop is stable
within ±10%. **The determinism of the bench itself is the design win.**

## 2. Measurements — same bench, both platforms, K=10

```
Windows:
  elapsed median:     0.174s  (0.163-0.188s) ±7.2%
  tick rate median:   57 615 Hz  (53 227-61 445 Hz)
  determinism check:  all 10 runs produced identical functional output
  violations:         0

Linux (WSL2):
  elapsed median:     0.167s  (0.160-0.199s) ±11.8%
  tick rate median:   59 929 Hz  (50 289-62 668 Hz)
  determinism check:  all 10 runs produced identical functional output
  violations:         0
```

### Cross-platform consistency (functional)

Identical across Windows ↔ Linux (bit-exact where floats involved):

```
dream fires:    19
reflex fires:    0
branch calls:  200
final entropy: 0.280
final fear:    0.000
final pain:    0.025
roles:         Stem=0 Nav=2 Sent=0 Work=2 Scout=4 Heal=0
```

Every one of the 11 bio-mechanisms produces the **same output** at
tick 10 000 on Linux and Windows. This is a real design win —
**OASIS's kernel is bit-deterministic across platforms** given the
same seeded input.

## 3. The bonus find: determinism tripwire

The K=10 pattern as implemented does a `SoakOutcome` equality check
between all 10 runs. If any run diverges, the process exits with
code 2 and prints both outcomes. This catches:

- Accidental reliance on unstable `HashMap` iteration order
- System-clock-dependent behaviour
- Uninitialised memory reads via Rust UB (would never happen in safe
  Rust, but defensively useful)
- Any floating-point non-determinism (none observed — the bench uses
  `f64::to_bits` for equality)

**The check passes today.** Future OASIS changes that accidentally
introduce non-determinism in the hot loop will trigger this.

## 4. What shipped (hygiene only)

| File | Change |
|---|---|
| `bench_mechanisms_soak.rs` | Extracted `run_soak()` helper, K=10 loop, `SoakOutcome` equality check, median/min/max + tick-rate band reporting |

**Zero OASIS library changes.** 427/427 host tests still pass. Same
18 bins build. Pure measurement + determinism hygiene.

## 5. Scoreboard — the "no single-shot bench" claim is now honest

| Bench | K | Spread observed |
|---|---:|---:|
| bench_payload_sweep | 10 | ±3-12% |
| bench_mesh_signed | 10 | ±2-99% (v8 process WSL outlier) |
| bench_mesh | 10 | ±31-46% Linux / ±5-37% Windows |
| bench_full_stack | 10 | ±4-19% |
| bench_r14_latency | N=1000 samples (percentile report) | p99-p50 / p50 ≈ 10% |
| **bench_mechanisms_soak** | **10** | **±7-12%** |
| bench_spore_loss | N=200 internal trials | documented |

**7 of 7 benchmarks have explicit variance disclosure.** No
single-shot perf claims remain in the repo.

## 6. The 4-deferral correction — kept the commitment this time

Prior 4 rounds each flagged "bench_mechanisms_soak — ~30 min, deferred
again." This round: 45 min actual, shipped.

The deferral ended up being **useful delay**, because all prior
hygiene rounds established the `K=10 + median + min/max + spread`
pattern, so this round's code was mostly copy-adapt with one new idea
(the determinism equality check via `SoakOutcome` struct).

**Lesson:** some deferrals are genuine discipline (focus on higher-
value work first). Some are avoidance (never getting to the smaller
item). This one was the first — applied the right pattern after
refining it 5 times elsewhere.

## 7. What this round does NOT do

### 🔴 No new functional test coverage
The soak bench already existed; this round only added timing bands +
determinism check. Did not add new mechanism-level invariants to
check.

### 🔴 No statistical tests on the timing band
Still K=10 + median/min/max heuristic. The ±7.2% Windows spread
suggests Windows might actually be quieter than WSL2 Linux for this
workload — but that's eyeballed, not CI-tested.

### 🔴 Linux WSL2 ±11.8% noise — hypervisor suspected
Same pattern as previous benches: Linux-under-WSL is 50% noisier than
Windows. Bare-metal Linux likely to tighten.

### 🔴 CLAUDE.md not updated
The "58k Hz" tick rate was already approximately correct in prior
claims. No recalibration needed. If a future regression changes the
tick rate materially, THIS audit is the baseline.

## 8. Calibration history at the hygiene inflection

| Round | Prediction hit rate |
|---|---|
| Session-seal v0A | 1 miss (870× vs 582×) |
| v0A KeyPair cache | 1 miss (3× vs 1.48×) |
| bench_payload_sweep bands | 2 hits |
| bench_mesh_signed bands | 1 correction surfaced |
| bench_mesh + full_stack bands | 1 surprise (30-45% WSL Linux spread) |
| A/B both-sides banded | **big miss** (1 MB rclcpp drifted 55%, not 10-15%) |
| **bench_mechanisms_soak** (this round) | **predicted 20-40% spread; actual 7-12%** — miss in OASIS's favour |

Pattern holding: predictions improve when I have prior banded data
(e.g., after this round I can reasonably predict bench_*_soak at
~10% spread for deterministic workloads). Blind first-time predictions
still miss by 2-5×.

## 9. End state after 3 hygiene rounds

- 7/7 benches with explicit variance disclosure
- CLAUDE.md has banded A/B numbers both sides (rclcpp + OASIS)
- "No single-shot bench" mental-loop rule is honest
- Determinism tripwire on the main mechanism soak — regression guard
- 427/427 host tests, 69 Kani proofs, MCU cross-compile clean

The hygiene push is materially complete. Remaining pure-hygiene items
are minor (bare-metal Linux comparison requires new hardware;
statistical tests require more K).

## 10. Next-step candidates (post-hygiene)

Hygiene (tail):
1. Apply trimmed-mean instead of raw min-max to reduce outlier
   influence (~30 min one-time helper update).
2. Investigate the WSL2 Linux v8 process ±99% spread once (it's the
   only genuinely noisy measurement in the benches).

Features (if the user unlocks from hygiene):
3. v0A key rotation protocol
4. Batch Ed25519 verify
5. Real MCU hardware boot
6. Typed messages derive macro (biggest DX gap vs ROS 2)
7. Third face: a formally proven scheduler layer atop the 7/7 rclcpp
   primitives

I don't have a strong recommendation — hygiene is clean, and the
user has the floor on what feature direction matters next.
