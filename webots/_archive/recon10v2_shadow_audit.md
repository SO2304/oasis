# OASIS Recon-10 v2 — Shadow Audit (post-fixes)

**Date:** 2026-04-20
**Fixes applied:** (1) vertical stratification (4 altitude layers), (2) per-drone target subsets via new `drone_bridge.exe` CLI arg, (3) arena widened to 12×10m.
**Goal:** test whether these fixes turn the failed Recon-10 v1 into a valid habituation test.

---

## 1. Executive summary

| Metric | v1 (pre-fix) | v2 (post-fix) | Verdict |
|--------|--------------|---------------|---------|
| Drones stable at end | 0 / 10 | **6 / 10** | ✅ **Physics fix WORKED** |
| Target inspections (swarm-sum) | 50 (then aborted) | 5 (point-in-time) | ⚠️ metric unreliable — see §4 |
| Full coverage loops | 2 | **736** | ✅ **368× improvement** |
| EXTREME phases observed | 0 (aborted before warmup) | **8 phases** | ✅ |
| Habituation signal measurable | N/A (no data) | ⚠️ 100% "drop" — likely ARTIFACT | ❌ |

**Bottom line:** The three fixes successfully created a stable multi-drone environment — the Rust kernel now scales to 10 drones. But habituation STILL can't be measured because the wind stimulus is too weak to trigger meaningful Rust `EmotionalState::fear` signal.

---

## 2. What the three fixes achieved

### 2.1 Vertical stratification — strong positive
- 4 altitude layers: 1.0m (3 drones), 1.3m (3), 1.7m (2), 2.0m (2)
- Passed via `OASIS_CRUISE` env var to each `drone_bridge.exe` subprocess
- Eliminated same-altitude crossings → 0 FLIPPED events during cruise (v1 had 10/10 flip)

### 2.2 Per-drone target subsets — strong positive
- New `drone_bridge.exe` CLI arg 3: comma-separated target names (e.g. `cnc,rack1`)
- Catalog-filtered so the Rust `WorldModel` only adds zones for assigned targets
- 10 drones × 2-3 targets each, each target covered by 2-3 drones
- Result: 736 full coverage loops across 6 flyers vs 2 loops in v1

### 2.3 12×10m arena — modest positive
- Widened from 8×6m to 12×10m (wall positions only; OBSTACLES array unchanged)
- Combined with `drone_bridge.rs` divergence envelope loosened to ±5.5/±4.5
- Gave drones 2× the horizontal play area for takeoff + cruise

---

## 3. Verified Rust kernel behavior at scale

| OASIS Mechanism | Evidence in v2 logs | Verdict |
|-----------------|---------------------|---------|
| M2 HyperState entropy | Per-drone final entropy 0.79-0.89 (computed by Rust Shannon) | ✅ running |
| M5 Emotion fear | Peaks 0.0-0.044 observed; signal too weak for habituation | ⚠️ active but under-stimulated |
| M9 AdaptiveReflex | Takeoff reflex (TAKEOFF complete at T~267) fired on all 10 drones | ✅ running |
| M10 WorldModel | Per-drone target subsets produced 736 cumulative loops | ✅ running |
| M11 Federation | Running (bridges spawn FederatedMesh) — no cross-drone digests observed in this run | ⚠️ present but quiet |

**Conclusion:** The real Rust kernel genuinely drives 10 parallel drones with per-drone missions. This is a solid scaling demonstration.

---

## 4. Why habituation test STILL failed

### 4.1 Fear signal is obstacle-driven, not wind-driven

The Rust `EmotionalState::fear` in `drone_bridge.rs` is updated from `AdaptiveReflex` (obstacle proximity) and pain memory, not directly from input velocity commands. My wind injection (`dvx_final = dvx_cmd + wx`) adds perturbation to the motor command AFTER the Rust kernel has computed its fear. The Rust kernel sees sensor readings (GPS, sonar, IMU) but doesn't see the wind directly.

For wind to trigger fear, it must:
1. Push the drone close to an obstacle → sonar reading drops → fear spikes

With wind ±0.08 m/s and obstacles well-distanced in a 12×10m arena, drones rarely got close enough to trigger fear.

**Fear peaks observed: 0.021, 0.029, 0.044 — barely above noise floor.**

### 4.2 "100% habituation drop" is an artifact

Per-cycle median fear peaks across 6 flying drones:

| Phase | Median peak | Interpretation |
|-------|-------------|----------------|
| 1     | 0.004       | tiny random triggers |
| 3-7   | 0.0         | no collision events |
| 9     | 0.012       | one drone briefly near obstacle |
| 11-15 | 0.0         | drift equilibrium, no triggers |

The "100% drop from phase 1 to phase 15" = 0.004 → 0.0. This is noise dying out, not biological habituation. The stimulus (wind) never produced sustained fear in the first place.

### 4.3 Per-drone peaks confirm

- d01: first-3 peak 0.021 → last-3 peak 0.0 (flagged as 100% drop)
- d06: first-3 peak 0.029 → last-3 peak 0.0 (flagged as 100% drop)
- d02, d04, d07: all zero throughout
- d09: literally 0 loops, 0 entropy (drone never did anything useful despite being classified FLY)

---

## 5. What's needed for valid habituation

The test fixes the swarm scaling problem but exposed a deeper issue: **wind is not a fear stimulus in the Rust kernel**. To measure habituation via OASIS M5, I would need one of:

1. **Moving obstacles** (not in this test setup): spawn a Webots object that drifts near drones during EXTREME phases. This would trigger sonar-proximity fear repeatedly, and habituation could be measured as fear_peak decrease across repeated exposure.

2. **Simulated sensor dropout** (existing mechanism in bridge via `imu_alive`/`gps_alive`/`sonar_alive` flags): toggle these during EXTREME phases. The Rust `Vitality` + `EmotionalState` would spike, and we'd watch recovery patterns.

3. **Intensify wind to actually destabilize** (risky, may recreate v1 flyaways).

4. **Use `pool_push_test` digests**: inject peer fear signals via `FederatedMesh` during EXTREME phases. Drones would receive "phantom fear" from simulated peers.

None of these were implemented in v2. The test validates physics scaling only.

---

## 6. OASIS-washing audit

### Claim: "10-drone OASIS swarm with real Rust kernel scales"
**Verdict: ✅ TRUE.** 6/10 stable flight, 736 full coverage loops, 8 phase cycles, 22-module Rust kernel running per drone. This is a genuine multi-drone OASIS demonstration.

### Claim: "Habituation demonstrated at 10-drone scale"
**Verdict: ❌ FALSE.** Fear peaks barely exceeded 0.03 on a 0.0-5.0 scale. "100% drop" is noise, not adaptation. The test shows no habituation signal because no sustained fear stimulus was applied.

### Claim: "v2 fixes solved the problems"
**Verdict: ⚠️ HALF-TRUE.** Fixed swarm scaling (0→6 flyers, 2→736 loops). Did NOT fix habituation measurement, because the stimulus model was wrong, not the drone count.

---

## 7. Honest headline

> "OASIS Recon-10 v2 with real Rust `drone_bridge.exe` and 3 audit-recommended fixes: achieved **60% stable flight at 10-drone scale** and **736 full coverage loops** — a 368× mission-progress improvement over v1. Rust kernel verified working in parallel (M2 entropy 0.79-0.89 per drone, M9 reflex, M10 WorldModel tracking). **Habituation remains untested**: the wind perturbation model does not reach the Rust EmotionalState fear channel, so fear peaks stayed at 0.0-0.04 noise level across 8 EXTREME phases. The v2 fixes solved the physics problem but exposed a stimulus-model problem that requires moving obstacles or sensor fault injection to resolve."

---

## 8. What this audit-driven iteration proves

1. **Shadow audits work**: the Recon-20 audit identified a real issue (Python reimpl vs real Rust). The Recon-10 v1 audit identified the next issue (arena/target contention). V2 addresses both, and the next audit identifies the remaining issue (stimulus model).
2. **Each iteration is honest about what it fixes and what it doesn't**. No claim is made that habituation works just because scaling works.
3. **Comparison of v1 → v2 shows the ROI of the fixes**, not a standalone headline.

---

## 9. Next iteration, if pursued

Priority: implement moving-obstacle stimulus during EXTREME phases in the Webots world or via controller trajectory modification. Then re-run with identical 10-drone v2 setup and measure fear_peak per EXTREME cycle. If M5 habituation is real, peaks should decrease 10-30% over 6+ cycles.

Without that stimulus upgrade, adding more drones or more phases won't produce habituation data — it'll just produce more loop counts.
