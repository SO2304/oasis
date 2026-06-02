# OASIS Recon-10 v5 — Habituation Report (3 stimulus mechanisms)

## Setup

- **Phone brain loaded**: `OASIS_PHONE_BRAIN=c:/dev/oasis/phone_brain/` → 128 pain memories + 0 federation digests.
- **Fault injection**: d01/d04/d07 get `sonar_alive=false` first 8s of each EXTREME phase.
- **Moving hazards**: 2 Crazyflies (hz0, hz1) orbit origin r=2.5m, alt=1.5m during EXTREME.
- **Wind**: ±0.05 EXTREME / ±0.005 CALM.

## Swarm classification

- FLY: 5
- GROUND: 4
- LOST: 1
- Phone brain loaded on: 10/10 drones (confirmed in logs)

## Mission + safety signals

- Total loops (swarm): **3456**
- Total R14 blocks (swarm): **32900**

## Habituation — FLYING drones only (5)

- R14 block median: phase 1 → last = **-1.9%** (target: ≥10% decrease)
- Entropy median: phase 1 → last = **+0.1%** (target: ≥5% decrease)
- **R14 RATE drop (first-half vs second-half per drone)**: mean **-0.3%** across 3 drones with R14 activity

### Per-EXTREME-cycle stats (median across flyers)

| Phase idx | N | R14 est | Entropy mean | Loops +Δ | Abort ticks |
|-----------|---|---------|--------------|----------|-------------|
| 1 | 5 | 106 | 0.8387 | 9 | 0 |
| 3 | 5 | 107 | 0.8379 | 9 | 0 |
| 5 | 5 | 108 | 0.8724 | 3 | 0 |
| 7 | 5 | 108 | 0.8784 | 4 | 0 |
| 9 | 5 | 108 | 0.8724 | 3 | 0 |
| 11 | 5 | 107 | 0.8838 | 3 | 0 |
| 13 | 5 | 109 | 0.8848 | 3 | 0 |
| 15 | 5 | 106 | 0.8747 | 3 | 0 |
| 17 | 5 | 107 | 0.87 | 3 | 0 |
| 19 | 5 | 108 | 0.8728 | 2 | 0 |
| 21 | 5 | 107 | 0.8805 | 2 | 0 |
| 23 | 5 | 107 | 0.8711 | 4 | 0 |
| 25 | 5 | 108 | 0.8757 | 9 | 0 |
| 27 | 5 | 107 | 0.8715 | 6 | 0 |
| 29 | 5 | 107 | 0.8373 | 9 | 0 |
| 31 | 5 | 107 | 0.8361 | 9 | 0 |
| 33 | 5 | 108 | 0.8376 | 8 | 0 |
| 35 | 5 | 106 | 0.837 | 9 | 0 |
| 37 | 5 | 107 | 0.8362 | 9 | 0 |
| 39 | 5 | 107 | 0.8391 | 9 | 0 |
| 41 | 5 | 107 | 0.838 | 5 | 0 |
| 43 | 5 | 105 | 0.8384 | 9 | 0 |
| 45 | 5 | 106 | 0.8384 | 9 | 0 |
| 47 | 5 | 108 | 0.8377 | 9 | 0 |
| 49 | 5 | 108 | 0.8383 | 9 | 0 |
| 51 | 5 | 108 | 0.8382 | 8 | 0 |
| 53 | 5 | 106 | 0.8378 | 8 | 0 |
| 55 | 5 | 106 | 0.8385 | 9 | 0 |
| 57 | 5 | 108 | 0.8387 | 7 | 0 |
| 59 | 5 | 107 | 0.8369 | 9 | 0 |
| 61 | 5 | 108 | 0.8382 | 8 | 0 |
| 63 | 5 | 109 | 0.8371 | 8 | 0 |
| 65 | 5 | 107 | 0.8378 | 9 | 0 |
| 67 | 5 | 107 | 0.837 | 9 | 0 |
| 69 | 5 | 108 | 0.8376 | 8 | 0 |
| 71 | 5 | 108 | 0.8375 | 9 | 0 |
| 73 | 5 | 107 | 0.8384 | 8 | 0 |
| 75 | 5 | 108 | 0.8364 | 7 | 0 |
| 77 | 5 | 107 | 0.838 | 6 | 0 |
| 79 | 5 | 108 | 0.8383 | 7 | 0 |
| 81 | 5 | 108 | 0.8376 | 9 | 0 |
| 83 | 5 | 108 | 0.8386 | 9 | 0 |
| 85 | 5 | 108 | 0.8374 | 6 | 0 |
| 87 | 5 | 108 | 0.838 | 9 | 0 |
| 89 | 5 | 107 | 0.8373 | 6 | 0 |
| 91 | 5 | 108 | 0.8377 | 9 | 0 |

### Per-drone outcome + R14 rate change

| Drone | Class | Takeoff@ | PB | Loops | Entropy | Total R14 | 1st-half | 2nd-half | R14 Δ% |
|-------|-------|----------|----|-------|---------|-----------|----------|----------|--------|
| d00 | GROUND | 251 | 128 | 335 | 0.835 | 0 | 0 | 0 | +0.0 |
| d01 | FLY | 251 | 128 | 691 | 0.820 | 10300 | 5150 | 5200 | -1.0 |
| d02 | FLY | 251 | 128 | 475 | 0.709 | 0 | 0 | 0 | +0.0 |
| d03 | GROUND | 267 | 128 | 0 | 0.890 | 0 | 0 | 0 | +0.0 |
| d04 | FLY | 267 | 128 | 781 | 0.893 | 11250 | 5650 | 5650 | +0.0 |
| d05 | LOST | 267 | 128 | 1 | 0.939 | 0 | 0 | 0 | +0.0 |
| d06 | FLY | 388 | 128 | 904 | 0.838 | 0 | 0 | 0 | +0.0 |
| d07 | FLY | 388 | 128 | 269 | 0.854 | 11350 | 5700 | 5700 | +0.0 |
| d08 | GROUND | 0 | 128 | 0 | 0.000 | 0 | 0 | 0 | +0.0 |
| d09 | GROUND | 0 | 128 | 0 | 0.000 | 0 | 0 | 0 | +0.0 |

## Verdict

- Physics (≥6/10 FLY): **FAIL** (5/10)
- Habituation R14 (≥10% decrease): **FAIL** (-1.9%)
- Habituation entropy (≥5% decrease): **FAIL** (+0.1%)
- Mission activity (≥100 loops): **PASS** (3456)