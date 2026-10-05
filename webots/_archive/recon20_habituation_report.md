# OASIS Recon-20 — Habituation & Mission Report

## Mission summary

- Drones analyzed: **20**
- Total virtual distance covered: **7.22 km** (target: 50.00 km)
- Drones that reached RTH: **0 / 20**
- Drones that LANDED: **0 / 20**

## Habituation (Mechanism 5 verification)

Hypothesis: fear peak in EXTREME phase N decreases relative to phase 1 baseline.

- Mean habituation drop (first-3 vs last-3 EXTREME peaks, per drone): **3.9%**
- Drones showing >10% drop: **6 / 20**

### Per-cycle fear peak + confound controls (across swarm)

| Phase idx | N drones | Median peak | Mean peak | Min | Max | Med wind_std | Med wind_comp_mag |
|-----------|----------|-------------|-----------|-----|-----|--------------|--------------------|
| 1 | 20 | 0.498 | 0.507 | 0.157 | 0.83 | 0.0152 | 0.0028 |
| 3 | 20 | 0.432 | 0.501 | 0.219 | 0.764 | 0.016 | 0.0022 |
| 5 | 20 | 0.436 | 0.515 | 0.258 | 0.928 | 0.0171 | 0.0022 |
| 7 | 20 | 0.417 | 0.479 | 0.209 | 0.764 | 0.0169 | 0.0022 |
| 9 | 20 | 0.346 | 0.45 | 0.207 | 0.764 | 0.0159 | 0.0022 |
| 11 | 20 | 0.425 | 0.477 | 0.235 | 0.764 | 0.0178 | 0.0022 |

*If `Med wind_std` is similar across phases AND `Med wind_comp_mag` grows AND peak drops, habituation is OASIS-attributable.*

### Per-drone summary

| Drone | Final | Dist actual (m) | Dist virtual (km) | Landed | EXTREME phases | First-3 med | Last-3 med | Habit drop (%) |
|-------|-------|-----------------|-------------------|--------|----------------|-------------|------------|----------------|
| d00 | SCOUT | 4.6 | 0.046 | no | 6 | 0.312 | 0.323 | -3.6 |
| d01 | SCOUT | 5.5 | 0.055 | no | 6 | 0.254 | 0.387 | -52.4 |
| d02 | SCOUT | 3.2 | 0.032 | no | 6 | 0.31 | 0.334 | -7.7 |
| d03 | LOST | 19.7 | 0.197 | no | 6 | 0.383 | 0.325 | 15.2 |
| d04 | SCOUT | 49.0 | 0.49 | no | 6 | 0.683 | 0.423 | 38.0 |
| d05 | SCOUT | 7.0 | 0.07 | no | 6 | 0.342 | 0.362 | -6.0 |
| d06 | SCOUT | 16.3 | 0.163 | no | 6 | 0.747 | 0.747 | 0.0 |
| d07 | SCOUT | 16.2 | 0.162 | no | 6 | 0.548 | 0.548 | 0.0 |
| d08 | SCOUT | 6.2 | 0.062 | no | 6 | 0.738 | 0.738 | 0.0 |
| d09 | LOST | 68.1 | 0.681 | no | 6 | 0.274 | 0.288 | -5.2 |
| d10 | SCOUT | 3.2 | 0.032 | no | 6 | 0.764 | 0.764 | 0.0 |
| d11 | SCOUT | 26.5 | 0.265 | no | 6 | 0.407 | 0.282 | 30.8 |
| d12 | LOST | 65.9 | 0.659 | no | 6 | 0.443 | 0.269 | 39.4 |
| d13 | SCOUT | 12.9 | 0.13 | no | 6 | 0.421 | 0.278 | 34.0 |
| d14 | SCOUT | 2.6 | 0.026 | no | 6 | 0.627 | 0.627 | 0.0 |
| d15 | LOST | 371.6 | 3.716 | no | 6 | 0.375 | 0.261 | 30.4 |
| d16 | SCOUT | 1.5 | 0.015 | no | 6 | 0.638 | 0.638 | -0.0 |
| d17 | SCOUT | 1.7 | 0.017 | no | 6 | 0.608 | 0.608 | 0.0 |
| d18 | SCOUT | 12.7 | 0.127 | no | 6 | 0.739 | 0.739 | 0.0 |
| d19 | LOST | 27.9 | 0.279 | no | 6 | 0.279 | 0.377 | -35.4 |

## Verdict

- Habituation criterion (mean drop >= 10%): **FAIL** (3.9%)
- Distance criterion (swarm cumulative >= 25 km virtual): **FAIL** (7.22 km)
- RTH criterion (>= 10 drones reached RTH): **FAIL** (0 drones)