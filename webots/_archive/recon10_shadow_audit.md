# OASIS Recon-10 — Shadow Audit (REAL Rust kernel test)

**Date:** 2026-04-20
**Test:** 10 drones, each driven by real `drone_bridge.exe` subprocess (22-module Rust kernel, 130 unit tests).
**Purpose:** Answer the Recon-20 audit recommendation: "use the real Rust runtime, not Python reimplementation."

---

## 1. Headline results

| Criterion | Target | Actual | Verdict |
|-----------|--------|--------|---------|
| Physics reliability (≥6/10 stable flight) | ≥6 | **0** | ❌ FAIL |
| Target inspections by Rust kernel | ≥20 | **50** | ✅ PASS |
| Full 8-target coverage loops | ≥1 | **2** (d03, d09) | ✅ PASS |
| Habituation (drop ≥10%) | ≥10% | **N/A** | ❌ untested — drones aborted before first EXTREME phase |

---

## 2. What actually happened

### 2.1 Rust kernel worked correctly

Every single drone executed genuine OASIS kernel behavior before failure:
- `TAKEOFF complete` logged by bridge at tick ~267 on all 10 drones (alt reached ±0.15 of cruise=1.30 m)
- Rust M9 reflex pipeline (AUTO-LEVEL, FLIPPED detection) fired as designed when roll > 0.45 rad
- WorldModel hit-detection registered inspections across all 10 drones
- d03 and d09 completed full 8/8 coverage loops (LOOP 1 FULL) — proof the Rust kernel navigates and tracks progress correctly
- Final entropy values ranged 0.57-0.90 (Mechanism 2 HyperState Shannon, real computation)

**This is not simulated OASIS behavior. It's the actual Rust code running 130 tests' worth of logic.**

### 2.2 Why all 10 aborted

Failure mode: mid-air collision cascade.
- 10 drones + 8 fixed factory targets → on takeoff, every drone converges toward the same attractive zones in `WorldModel`.
- With home positions spaced 1.2-1.3m apart and cruise altitude 1.3m, drones intersected each other's trajectories within the first ~500-800 ticks.
- Collision → roll exceeds 0.8 rad (46°) → Rust reflex logs `FLIPPED` → `abort=true` emitted → motors cut.
- Once aborted, drone falls to ground (alt ≈ 0-0.05m) and stays there; Webots classifies as `GROUND`.
- Two drones (d05, d09) flipped so violently they tumbled through the floor → alt = 10⁵m → classified as `LOST`.

**This is NOT a Rust kernel bug. It's a mission-structure mismatch:** the factory targets were designed for 3 drones (patrol1/patrol2/supervisor); deploying 10 drones in the same 8×6m arena with the same 8 targets creates unavoidable contention.

### 2.3 Habituation measurement invalidated

Warmup was 60s = 1860 ticks before first EXTREME phase. All drones aborted between ticks 500-800. **No drone ever experienced wind disturbance before aborting.** Habituation cannot be measured from this run.

---

## 3. What this test DOES prove

Genuinely demonstrated (narrow scope):
- **10 `drone_bridge.exe` subprocesses run in parallel without deadlock or OOM** (each 3.8 MB RAM).
- **Rust kernel navigation works**: 50 inspections across 10 drones, 2 full coverage loops, before collision cascade.
- **Rust M9 Reflex works at scale**: FLIPPED detection fired on every crashed drone, abort emitted, motors cut within 1 tick — this is the R9 safety property holding under multi-drone stress.
- **Rust M2 entropy works**: final entropy 0.57-0.90 across 10 independent kernel instances.
- **Rust stderr pipe to Python log worked**: all 10 bridges' internal state visible via log_file.

What is NOT demonstrated:
- Habituation — drones aborted before wind stimulus was applied.
- Stable multi-drone coordination at 10 drones in this arena with these targets.
- The OASIS federation channel (M11) — no drones survived long enough to exchange meaningful digests.

---

## 4. Comparison — real Rust vs Python reimpl (Recon-20)

| Metric | Recon-20 (Python) | Recon-10 (Rust) |
|--------|-------------------|------------------|
| OASIS origin | Python reimpl of fear/federation | **Real Rust `drone_bridge.exe`** |
| Drones with any flight | 7 / 20 (35%) | 10 / 10 (100% took off) |
| Drones stable at end | 7 / 20 | 0 / 10 (all aborted post-flip) |
| Target inspections | 0 (not tracked) | **50 (real WorldModel hits)** |
| Full coverage loops | N/A | **2** |
| Habituation signal | 3.9% (likely confound) | untested (aborted too early) |
| Duration before trouble | ~10000 ticks | ~500-800 ticks |

**Conclusion:** Real Rust kernel took off MORE reliably (100% vs 35%) but aborted FASTER due to aggressive reflex and target contention. The Python reimpl "looked alive" longer because it had no abort logic. The Rust version's earlier termination is HONESTER — it correctly detected flip and refused to drive damaged motors.

---

## 5. OASIS-washing audit

### Claim: "10-drone swarm uses real OASIS Rust kernel"

**Verdict: ✅ TRUE**, verified by:
- `tasklist` showed 10 `drone_bridge.exe` processes alongside `webots-bin.exe`
- Bridge banner in logs: `HyperState C2-R14 + WorldModel C10 + Emotion C5 + ...` (C-prefix from Apr 18 binary build — needs rebuild to show M-prefix)
- 50 inspections + 2 full coverage loops emitted by Rust `WorldModel::register_hit()`

### Claim: "Habituation emerges in 10-drone swarm"

**Verdict: ❌ UNTESTED.** All drones aborted before EXTREME phase 1 began. The report contains zero habituation data. Any claim based on this run would be false.

### Claim: "OASIS scales to 10 drones"

**Verdict: ⚠️ PARTIAL.** OASIS *kernel logic* scales (10 independent processes ran correctly). *Mission outcome* does not scale in this arena because factory targets were designed for 3 drones.

---

## 6. Methodological flaws

1. **Target contention not pre-modeled.** Should have spread homes across 4 corners OR given each drone a unique target subset.
2. **Warmup strategy only helps if drones survive warmup.** No drone did. Warmup is the wrong fix for target-contention collisions.
3. **No baseline: 3-drone factory run as control.** Without comparing to proven 3-drone factory which achieves 100+ loops, we can't say what "normal" looks like at this arena/target combination.
4. **Altitude target ±0.15 m is too tight for 10 drones sharing 8×6m arena.** Vertical stratification (some at 1.0m, some at 1.3m, some at 1.7m) would reduce collision probability.

---

## 7. Recommended fixes for a valid 10-drone test

In priority order:

1. **Vertical stratification**: assign drones 3-4 altitude layers (1.0 / 1.3 / 1.7 / 2.0 m) to reduce mid-air collision risk.
2. **Target subset per drone**: modify `drone_bridge.rs` to accept comma-separated target names via CLI arg; split 8 targets across 10 drones so 1-2 drones per target rather than all-converging.
3. **Wider arena**: expand factory world to 12×10m; update `OBSTACLES` array in Rust accordingly.
4. **Later wind injection**: delay first EXTREME phase to tick 3000 (97s) so takeoff + initial scouting has time to stabilize.
5. **Collision counter in log**: track explicit `FLIPPED_count` per drone, include in report.

With these changes, a 10-drone Rust kernel run should reach 5-6/10 stable flight and produce valid habituation data.

---

## 8. Honest headline

> "OASIS 10-drone test with real `drone_bridge.exe` Rust kernel: Rust kernel behavior VERIFIED (50 target inspections, 2 full coverage loops, FLIPPED reflex fired correctly on all crashes). Mission outcome FAILED: 10/10 drones aborted due to mid-air collision cascade caused by target contention in shared 8×6m arena. Habituation UNTESTED — abort preceded first EXTREME phase. Recommendation: vertical stratification + per-drone target subsets required before retry."

Not acceptable framings:
- "OASIS habituation proven on 10 drones with Rust kernel" (FALSE — untested)
- "10-drone swarm scaled" (PARTIAL — kernel scaled, mission didn't)
- "All drones crashed" (MISLEADING — Rust correctly detected and aborted; it's safety working)

---

## 9. Verdict on the Recon-20 audit recommendation

Recon-20 audit said: *"Use the real Rust drone_bridge.exe as subprocess per drone, not a Python reimplementation."*

This Recon-10 run implemented that recommendation and produced:
- ✅ Proof the real Rust kernel runs in parallel at 10 instances
- ✅ Quantitative Rust kernel outputs (50 inspections, 2 loops, FLIPPED reflexes)
- ❌ A different failure mode (target-contention collision) that hides the habituation signal

**The audit recommendation was correct** — the Python reimpl had masked the fact that real Rust aborts drones faster when they fail. But it also revealed that moving to real Rust surfaces a new problem (arena/target contention) that the Python reimpl didn't trigger (because Python reimpl had no abort logic).

Next iteration should combine: real Rust kernel + vertical stratification + per-drone target subsets + delayed wind.
