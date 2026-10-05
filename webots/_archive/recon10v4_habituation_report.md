# OASIS Recon-10 v4 — Habituation Report (3 stimulus mechanisms)

## Setup

- **Phone brain loaded**: `OASIS_PHONE_BRAIN=c:/dev/oasis/phone_brain/` → 128 pain memories + 0 federation digests.
- **Fault injection**: d01/d04/d07 get `sonar_alive=false` first 8s of each EXTREME phase.
- **Moving hazards**: 2 Crazyflies (hz0, hz1) orbit origin r=2.5m, alt=1.5m during EXTREME.
- **Wind**: ±0.05 EXTREME / ±0.005 CALM.

## Swarm classification

- FLY: 7
- GROUND: 2
- LOST: 1
- Phone brain loaded on: 10/10 drones (confirmed in logs)

## Mission + safety signals

- Total loops (swarm): **677**
- Total R14 blocks (swarm): **3400**

## Habituation — FLYING drones only (7)

- R14 block median: phase 1 → last = **+0.0%** (target: ≥10% decrease)
- Entropy median: phase 1 → last = **+0.1%** (target: ≥5% decrease)
- **R14 RATE drop (first-half vs second-half per drone)**: mean **-5.9%** across 2 drones with R14 activity

### Per-EXTREME-cycle stats (median across flyers)

| Phase idx | N | R14 est | Entropy mean | Loops +Δ | Abort ticks |
|-----------|---|---------|--------------|----------|-------------|
| 1 | 7 | 0 | 0.8391 | 8 | 0 |
| 3 | 7 | 0 | 0.8376 | 8 | 0 |
| 5 | 7 | 0 | 0.8524 | 9 | 0 |
| 7 | 7 | 0 | 0.878 | 8 | 0 |
| 9 | 7 | 0 | 0.8377 | 6 | 0 |
| 11 | 7 | 0 | 0.8393 | 6 | 0 |
| 13 | 7 | 0 | 0.8385 | 3 | 0 |

### Per-drone outcome + R14 rate change

| Drone | Class | Takeoff@ | PB | Loops | Entropy | Total R14 | 1st-half | 2nd-half | R14 Δ% |
|-------|-------|----------|----|-------|---------|-----------|----------|----------|--------|
| d00 | FLY | 251 | 128 | 140 | 0.846 | 0 | 0 | 0 | +0.0 |
| d01 | FLY | 251 | 128 | 116 | 0.639 | 0 | 0 | 0 | +0.0 |
| d02 | FLY | 251 | 128 | 90 | 0.806 | 0 | 0 | 0 | +0.0 |
| d03 | LOST | 267 | 128 | 0 | 0.910 | 0 | 0 | 0 | +0.0 |
| d04 | FLY | 267 | 128 | 128 | 0.881 | 1700 | 850 | 900 | -5.9 |
| d05 | FLY | 267 | 128 | 20 | 0.916 | 0 | 0 | 0 | +0.0 |
| d06 | FLY | 388 | 128 | 143 | 0.839 | 0 | 0 | 0 | +0.0 |
| d07 | FLY | 388 | 128 | 40 | 0.888 | 1700 | 850 | 900 | -5.9 |
| d08 | GROUND | 0 | 128 | 0 | 0.000 | 0 | 0 | 0 | +0.0 |
| d09 | GROUND | 0 | 128 | 0 | 0.000 | 0 | 0 | 0 | +0.0 |

## Verdict

- Physics (≥6/10 FLY): **PASS** (7/10)
- Habituation R14 (≥10% decrease): **FAIL** (+0.0%)
- Habituation entropy (≥5% decrease): **FAIL** (+0.1%)
- Mission activity (≥100 loops): **PASS** (677)