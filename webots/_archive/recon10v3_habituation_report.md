# OASIS Recon-10 v3 — Habituation Report (3 stimulus mechanisms)

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

- Total loops (swarm): **737**
- Total R14 blocks (swarm): **4300**

## Habituation — FLYING drones only (5)

- R14 block median: phase 1 → last = **+0.0%** (target: ≥10% decrease)
- Entropy median: phase 1 → last = **+0.3%** (target: ≥5% decrease)
- **R14 RATE drop (first-half vs second-half per drone)**: mean **-4.7%** across 2 drones with R14 activity

### Per-EXTREME-cycle stats (median across flyers)

| Phase idx | N | R14 est | Entropy mean | Loops +Δ | Abort ticks |
|-----------|---|---------|--------------|----------|-------------|
| 1 | 5 | 0 | 0.8389 | 9 | 0 |
| 3 | 5 | 0 | 0.8375 | 9 | 0 |
| 5 | 5 | 0 | 0.8337 | 9 | 0 |
| 7 | 5 | 0 | 0.8339 | 9 | 0 |
| 9 | 5 | 0 | 0.8317 | 9 | 0 |
| 11 | 5 | 0 | 0.8336 | 8 | 0 |
| 13 | 5 | 0 | 0.8338 | 8 | 0 |
| 15 | 5 | 0 | 0.8382 | 5 | 0 |
| 17 | 5 | 0 | 0.8361 | 4 | 0 |

### Per-drone outcome + R14 rate change

| Drone | Class | Takeoff@ | PB | Loops | Entropy | Total R14 | 1st-half | 2nd-half | R14 Δ% |
|-------|-------|----------|----|-------|---------|-----------|----------|----------|--------|
| d00 | FLY | 251 | 128 | 172 | 0.851 | 0 | 0 | 0 | +0.0 |
| d01 | FLY | 251 | 128 | 186 | 0.802 | 2100 | 1050 | 1100 | -4.8 |
| d02 | FLY | 251 | 128 | 94 | 0.804 | 0 | 0 | 0 | +0.0 |
| d03 | LOST | 267 | 128 | 4 | 0.871 | 0 | 0 | 0 | +0.0 |
| d04 | GROUND | 267 | 128 | 18 | 0.890 | 0 | 0 | 0 | +0.0 |
| d05 | GROUND | 267 | 128 | 9 | 0.824 | 0 | 0 | 0 | +0.0 |
| d06 | FLY | 388 | 128 | 192 | 0.831 | 0 | 0 | 0 | +0.0 |
| d07 | FLY | 388 | 128 | 62 | 0.875 | 2200 | 1100 | 1150 | -4.5 |
| d08 | GROUND | 0 | 128 | 0 | 0.000 | 0 | 0 | 0 | +0.0 |
| d09 | GROUND | 0 | 128 | 0 | 0.000 | 0 | 0 | 0 | +0.0 |

## Verdict

- Physics (≥6/10 FLY): **FAIL** (5/10)
- Habituation R14 (≥10% decrease): **FAIL** (+0.0%)
- Habituation entropy (≥5% decrease): **FAIL** (+0.3%)
- Mission activity (≥100 loops): **PASS** (737)