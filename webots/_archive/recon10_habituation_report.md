# OASIS Recon-10 — Habituation Report (REAL Rust kernel via drone_bridge.exe)

## Setup

- 10 drones, each running `drone_bridge.exe` subprocess (22-module Rust kernel, 130 unit tests)
- Fear signal: `EmotionalState::fear` (Mechanism 5) from Rust, not Python
- Entropy signal: HyperState Shannon (Mechanism 2) from Rust
- Wind: injected Python-side onto dvx/dvy before PID (CALM ±0.01, EXTREME ±0.08)

## Swarm classification

- FLY (stable): **0 / 10**
- GROUND (never took off): 8
- LOST (alt > 4 m): 2
- ABORT (bridge triggered abort majority of run): 0

## Mission outcomes

- Total inspected targets (swarm sum): **50**
- Total full coverage loops: **2**

## Habituation — FLYING drones only

- Drones with >3 EXTREME phases: 0
- Mean habituation drop (first-3 vs last-3 per drone): **0.0%**
- Swarm median habituation drop (phase 1 vs last): **0.0%**
- Drones with >10% drop: 0 / 0

### Per-cycle fear peak (FLYING drones)

| Phase idx | N | Median | Mean | Min | Max |
|-----------|---|--------|------|-----|-----|

### Per-drone summary

| Drone | Class | Inspected | Loops | Final entropy | EXTREME phases | First-3 peak | Last-3 peak | Drop % |
|-------|-------|-----------|-------|---------------|----------------|--------------|-------------|--------|
| d00 | GROUND | 2 | 0 | 0.886 | 8 | 0.006 | 0.006 | 0.0 |
| d01 | GROUND | 7 | 0 | 0.757 | 8 | 0.045 | 0.045 | 0.0 |
| d02 | GROUND | 4 | 0 | 0.798 | 8 | 0.122 | 0.122 | 0.0 |
| d03 | GROUND | 5 | 1 | 0.873 | 8 | 0.05 | 0.05 | 0.0 |
| d04 | GROUND | 5 | 0 | 0.8 | 8 | 0.001 | 0.001 | 0.0 |
| d05 | LOST | 7 | 0 | 0.57 | 8 | 0.0 | 0.0 | 0.0 |
| d06 | GROUND | 3 | 0 | 0.902 | 8 | 0.003 | 0.003 | 0.0 |
| d07 | GROUND | 7 | 0 | 0.901 | 8 | 0.01 | 0.01 | 0.0 |
| d08 | GROUND | 4 | 0 | 0.795 | 8 | 0.12 | 0.12 | 0.0 |
| d09 | LOST | 6 | 1 | 0.882 | 8 | 0.054 | 0.054 | 0.0 |

## Verdict

- Physics reliability (≥6/10 FLY): **FAIL** (0/10)
- Habituation (drop ≥10% in flyers OR swarm cycle drop ≥10%): **FAIL**
- Mission progress (≥20 target inspections): **PASS** (50)