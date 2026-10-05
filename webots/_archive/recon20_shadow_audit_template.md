# OASIS Recon-20 — Shadow Audit (template, fill after run)

Hostile review of the 20-drone reconnaissance test. Goal: catch over-claims,
methodological flaws, OASIS-washing, and confounds before publication.

## 1. Mission scope honesty

| Claim | Reality | Verdict |
|-------|---------|---------|
| "20 drones" | Literal: 20 Crazyflies in Webots | ✅ true |
| "50 km traversal" | Virtual, scaled 1 actual m = 10 virtual m. Real distance = 5 km swarm aggregate | ⚠️ DOCUMENTED but reads as inflation in headline |
| "Reconnaissance + RTH" | Random walk + return-to-home waypoint | ⚠️ "Reconnaissance" is generous for random walk; should say "exploration" |
| "Extreme/calm conditions" | Wind ±0.15 m/s for EXTREME, ±0.01 for CALM | ✅ honest range |
| "Habituation via OASIS" | <FILL: M5 emotion + wind_comp learning> | <FILL after results> |

## 2. Methodological audit

- [ ] Is the EXTREME stimulus IDENTICAL across phases? (Same RNG seed?)
  - Current: each drone has its own Wind RNG seeded at ctor. Wind in phase N differs
    from phase 1 → confound: maybe phase N had genuinely milder gusts.
  - **Mitigation**: report wind_x stddev per phase to confirm uniformity.

- [ ] Does the test isolate OASIS from PID effects?
  - PID integrator + wind disturbance can produce smoother trajectories over time
    purely from PID equilibration → less obstacle proximity → less fear.
  - **Mitigation**: report wind_comp magnitude growth (the actual learning signal).

- [ ] Sample size: 20 drones × 6 EXTREME phases = 120 data points. Adequate?
  - For repeated-measures: yes, sufficient for detecting >10% effect.

- [ ] Are crashed/stuck drones excluded?
  - Current analysis treats all drones equally even if landed early or stuck on ground.
  - **Mitigation**: report state distribution; add filter for landed-too-early.

## 3. OASIS-washing checks

| Mechanism claimed | Code path | Honest? |
|-------------------|-----------|---------|
| M5 Emotional gain | `fear = fear*0.85 + max(lf, ff*0.7)*0.15` | ✅ matches Rust EmotionalState pattern (decay + new) |
| M11 Federation | `broadcast()` + `receive()` JSON file relay | ⚠️ NOT real OASIS Rust kernel — just file-based digest sharing in Python |
| M9 Reflex | `if rf<0.25: dvx -= ...` | ⚠️ hardcoded reflex thresholds, not AdaptiveReflex's calibrated 2σ |

**Verdict**: This test runs an OASIS-INSPIRED Python controller, not the full Rust
kernel through `drone_bridge.exe`. It demonstrates the architectural pattern works at
20-drone scale, but does NOT validate the Rust runtime under the same conditions.

## 4. Confounds for habituation result

If habituation drop > 10%:
- C1: wind_comp learned → lower drift → lower lf — TRUE OASIS effect
- C2: PID integrator equilibrated → smoother → lower lf — PID effect, not OASIS
- C3: drones drifted to obstacle-free zones via random walk — coincidence
- C4: drones LANDED early during later phases — selection bias
- C5: wind RNG happened to be milder in late phases — pure noise

To attribute to OASIS, must show:
- wind_comp_mag grows over time (✓ logged)
- AND fear peaks decrease (✓ measured)
- AND wind stddev is similar across phases (must verify)
- AND drones are ALIVE in late phases (must verify)

## 5. Per-claim verdict (FILL after results)

| Claim made | Evidence | Verdict |
|------------|----------|---------|
| 20-drone swarm coordination | <FILL> | ? |
| 50 km virtual coverage | <FILL> | ? |
| Habituation emerges | <FILL> | ? |
| RTH success | <FILL> | ? |
| Federation observed | <FILL> | ? |

## 6. Recommended honest framing

(FILL after results)

## 7. What this test does NOT prove

- Does NOT validate Rust drone_bridge.exe under load
- Does NOT validate against literal 50 km outdoor flight
- Does NOT compare against a non-OASIS baseline (no PID-only swarm of 20)
- Does NOT measure power consumption, latency, or wallclock fitness for real BVLOS
