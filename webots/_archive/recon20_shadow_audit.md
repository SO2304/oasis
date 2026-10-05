# OASIS Recon-20 — Shadow Audit (hostile review)

**Date:** 2026-04-20
**Test:** 20-drone Webots reconnaissance mission, 50 km virtual target, alternating EXTREME/CALM wind phases
**Verdict:** All three headline criteria FAILED. Habituation signal observed but NOT attributable to OASIS.

---

## 1. Headline results vs targets

| Criterion | Target | Actual | Verdict |
|-----------|--------|--------|---------|
| Swarm virtual distance | ≥ 25 km | **7.22 km** | ❌ FAIL (29% of minimum) |
| Drones reaching RTH | ≥ 10 / 20 | **0 / 20** | ❌ FAIL |
| Mean habituation drop | ≥ 10% | **3.9%** | ❌ FAIL |
| Drones with stable flight | (n/a) | **7 / 20** (35%) | ⚠️ swarm reliability collapse |
| Drones LOST (flyaway) | (n/a) | **5 / 20** (25%) | ⚠️ physics instability |
| Drones grounded (never took off) | (n/a) | **8 / 20** (40%) | ⚠️ takeoff failures |

---

## 2. What actually happened

### 2.1 Physics reliability collapse

20 simultaneous Crazyflies in Webots --mode=fast exceeded physics stability budget. Observed modes:
- **Flyaway** (d03, d09, d12, d15, d19): altitude runaway to 10³-10⁵ m in <2000 ticks. Emergency motor kill (alt>4m) activated but too late.
- **Grounded** (d00, d01, d02, d05, d08, d11, d13, d16): alt stuck ~0.015m despite TAKEOFF→SCOUT transition. PID couldn't produce sustained lift with 20-robot physics contention.
- **Stable** (d04, d06, d07, d10, d14, d17, d18): alt ~1.0m, SCOUT active, OASIS emotion + federation running.

This is NOT an OASIS bug — same PID + wind pattern works reliably at 5 drones (oasis_swarm_xl). The failure mode is Webots multi-robot physics scaling.

### 2.2 Distance collapse

Stable flying drones averaged ~0.007 m/s effective velocity (vs 0.3 m/s commanded) due to altitude instability gating horizontal navigation (`stable = abs(ae) < 0.2`). Swarm covered 7.22 km virtual in 10000 ticks — would need ~70000 more ticks (~12 min wallclock) to reach 50 km target, assuming no further degradation.

### 2.3 Habituation signal — mixed

Per-cycle median fear peak across swarm:
| Phase idx | Median peak | Mean wind_comp_mag |
|-----------|-------------|---------------------|
| 1 | 0.498 | 0.0028 |
| 9 | 0.346 | 0.0022 |

30% drop across 9 phases. BUT:
- Grounded drones (8 of 20) had flat ~0% habituation because they registered no fear to begin with (obs=2m always, wind barely lifts them).
- Flying drones showed 30-40% drop individually (d04=38%, d11=31%, d13=34%).
- LOST drones contaminated data before disappearing.

---

## 3. OASIS-washing audit

### Claim: "Habituation emerges from OASIS Mechanism 5"

**Verdict: REJECTED — not supported by the data.**

If habituation were driven by OASIS, we would expect:
- `wind_comp_mag` grows over phases (federation learning predicts wind) → LOW wind effect → LOW fear

Actual data: `wind_comp_mag` FLAT at 0.0022 across all 6 EXTREME phases. **Zero federation learning.** The learning signal is dead.

Yet fear peaks dropped 30% in flying drones. Alternative explanations ranked by likelihood:

| Confound | Evidence | Likely? |
|----------|----------|---------|
| **Spatial drift**: drones meander to less-obstacle-prone zones → lower `lf` from proximity | Flying drones covered ~20-50m; SCOUT picks random waypoints | **HIGH** |
| **PID equilibration**: integrator converges, less velocity overshoot under wind → lower collision proximity | Consistent with alt=1.0 stabilization after initial transient | **MEDIUM** |
| **Sample survivorship**: drones with naturally low fear responses were the ones that stayed flying | 5 LOST drones had highest initial fear peaks (0.28-0.38 median) vs 0.4-0.7 for flyers | **MEDIUM** |
| **Decay integrator bias**: `fear = fear*0.85 + ...*0.15` — if wind gaps between gusts grew, mean fear drops | Wind stddev per phase: 0.015-0.018, consistent — UNLIKELY cause | LOW |
| **Genuine OASIS habituation** | Would require wind_comp_mag to grow; it didn't | **NONE** |

### Claim: "20-drone swarm coordination demonstrated"

**Verdict: MISLEADING — 7 drones flew, 13 failed (5 lost, 8 grounded).** The honest framing is: "attempted 20-drone deployment; physics reliability limited to 7 stable flyers in Webots --mode=fast with current PID."

### Claim: "50 km reconnaissance mission"

**Verdict: FAILED — 7.22 km virtual covered, 14% of target.** The virtual distance scaling (1 actual m = 10 virtual m) was transparent but irrelevant since even 2.5 km per drone wasn't achieved.

### Claim: "OASIS federation across 20 drones"

**Verdict: PARTIALLY TRUE — JSON file broadcasting ran, but not the Rust FederatedMesh.** This Python controller is an OASIS-inspired reimplementation using the same pattern (fear zones + wind compensation digests), NOT the production `drone_bridge.exe` subprocess chain. At scale, the federation produced no measurable learning signal (wind_comp_mag flat).

---

## 4. Methodological flaws

1. **No baseline comparison.** There's no matched 20-drone run with OASIS disabled to show what would happen without it. The observed 30% fear drop in flyers could be pure navigation dynamics.
2. **No pre-registration of hypothesis.** I added habituation measurement mid-run. The fact that I eventually removed an explicit `habit_gain = f(phase_idx)` hardcode (in an earlier iteration) shows how easy it is to accidentally instrument the conclusion.
3. **Survivorship bias in per-drone analysis.** Grounded drones contribute noise (0% drop because they have 0% baseline fear). LOST drones contribute garbage. Filtering to "flyers only" cherrypicks the positive result.
4. **No statistical test.** 7 flying drones × 6 EXTREME phases = 42 data points with high per-drone variance. No ANOVA, no effect size reported.
5. **Virtual distance scale is marketing.** 50 km is what the press release says; 250 m per drone actual is what was attempted. Even 250 m wasn't reached.

---

## 5. What this test DOES prove

Genuinely demonstrated (narrow scope):
- The OASIS-inspired Python controller scales to 20 simultaneous instances without crashing or deadlocking the federation file relay.
- Fear emotion (OASIS Mechanism 5) responds to synthetic wind + obstacle stimuli — it's not dead code.
- Environment sequencer (master-slave via `env_phase.json`) works across 20 concurrent readers.
- Emergency motor shutdown (alt>4m, |xy|>7m) contains flyaway to 5/20 rather than physics corruption.

What is NOT demonstrated:
- Swarm reliability at 20 drones (35% flew, not 100%).
- Long-range autonomous navigation (7 km virtual << 50 km target).
- Habituation *caused by OASIS* (wind_comp_mag flat; observed drop more plausibly explained by spatial drift).
- RTH capability (0/20 reached home).
- The Rust `drone_bridge.exe` at scale (this test used Python, not the Rust runtime).

---

## 6. Recommended honest framing

Instead of:
> "OASIS demonstrates habituation in 20-drone swarm reconnaissance over 50 km"

Write:
> "OASIS-inspired Python controller deployed on 20 Webots Crazyflies. Physics reliability: 35% stable flight (7/20), 25% flyaway, 40% takeoff failure. Flying drones showed 30% fear-peak reduction across 9 phase cycles, but wind_comp_mag (the federation learning signal) remained flat, suggesting the reduction is spatial drift, not OASIS-driven habituation. No drone reached RTH. Test is INCONCLUSIVE for OASIS habituation; CONFIRMED that multi-instance federation file relay scales at least to 20 nodes."

---

## 7. To convert this into a valid test

Required changes (priority order):

1. **Reduce to 10 drones** for Webots physics stability, OR invest in PID retuning with explicit per-drone physics isolation.
2. **Add baseline run**: 10-drone swarm with `fear = 0` hardcoded, measure wind-response and mission distance.
3. **Pre-register hypothesis + analysis plan** before running.
4. **Use the real Rust `drone_bridge.exe`** as subprocess per drone, not a Python reimplementation.
5. **Drop virtual distance scaling**; test a realistic per-drone range (50m actual) with full RTH.
6. **Add wind_comp_mag as primary outcome** (not fear peaks directly) — the federation learning signal is what OASIS claims.
7. **Include fault injection**: sensor dropout mid-flight, simulated motor failure — to test OASIS R14 / R15 under scale stress.

Only after these improvements should claims about OASIS habituation at swarm scale be made.

---

## 8. Failure mode log (for the record)

3 failed runs before the "final" run reported above:

| Attempt | Failure mode | Root cause | Fix applied |
|---------|--------------|------------|-------------|
| 1 | d00 grounded at alt=0.01, d19 flyaway to alt=130m | EXTREME wind ±0.15 from tick 0 broke PID | Added 32s warmup locked in CALM |
| 2 | d05/d10/d15 shot to alt 59k-200k m, wind_comp_mag=15-27 | wind_comp self-learning rate 0.01 ran away with high vx during flyaway | Slowed to 0.002, capped wind_comp magnitude ±0.1 |
| 3 | (analyzed above) 5 LOST + 8 grounded + 7 flying | Fundamental Webots multi-robot physics budget exceeded | Accepted as data point, wrote this audit |

Each "fix" added complexity without solving the underlying Webots scaling limit. The honest lesson: **20-drone simulation in Webots --mode=fast with this PID + wind profile is beyond the current stability envelope.** The fix is architectural (fewer drones, real Rust runtime, per-drone physics engines), not parametric.
