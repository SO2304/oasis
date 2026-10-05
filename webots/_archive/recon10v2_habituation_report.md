# OASIS Recon-10 — Habituation Report (REAL Rust kernel via drone_bridge.exe)

## Setup

- 10 drones, each running `drone_bridge.exe` subprocess (22-module Rust kernel, 130 unit tests)
- Fear signal: `EmotionalState::fear` (Mechanism 5) from Rust, not Python
- Entropy signal: HyperState Shannon (Mechanism 2) from Rust
- Wind: injected Python-side onto dvx/dvy before PID (CALM ±0.01, EXTREME ±0.08)

## Swarm classification

- FLY (stable): **6 / 10**
- GROUND (never took off): 2
- LOST (alt > 4 m): 2
- ABORT (bridge triggered abort majority of run): 0

## Mission outcomes

- Total inspected targets (swarm sum): **5**
- Total full coverage loops: **736**

## Habituation — FLYING drones only

- Drones with >3 EXTREME phases: 6
- Mean habituation drop (first-3 vs last-3 per drone): **100.0%**
- Swarm median habituation drop (phase 1 vs last): **100.0%**
- Drones with >10% drop: 2 / 6

### Per-cycle fear peak (FLYING drones)

| Phase idx | N | Median | Mean | Min | Max |
|-----------|---|--------|------|-----|-----|
| 1 | 6 | 0.004 | 0.009 | 0.0 | 0.026 |
| 3 | 6 | 0.0 | 0.01 | 0.0 | 0.03 |
| 5 | 6 | 0.0 | 0.008 | 0.0 | 0.029 |
| 7 | 6 | 0.0 | 0.009 | 0.0 | 0.027 |
| 9 | 6 | 0.012 | 0.014 | 0.0 | 0.031 |
| 11 | 6 | 0.0 | 0.01 | 0.0 | 0.036 |
| 13 | 6 | 0.0 | 0.0 | 0.0 | 0.0 |
| 15 | 6 | 0.0 | 0.0 | 0.0 | 0.0 |

### Per-drone summary

| Drone | Class | Inspected | Loops | Final entropy | EXTREME phases | First-3 peak | Last-3 peak | Drop % |
|-------|-------|-----------|-------|---------------|----------------|--------------|-------------|--------|
| d00 | LOST | 0 | 37 | 0.862 | 8 | 0.035 | 0.035 | 0.0 |
| d01 | FLY | 0 | 162 | 0.837 | 8 | 0.021 | 0.0 | 100.0 |
| d02 | FLY | 1 | 123 | 0.794 | 8 | 0.0 | 0.0 | 0.0 |
| d03 | LOST | 1 | 33 | 0.835 | 8 | 0.0 | 0.0 | 0.0 |
| d04 | FLY | 1 | 149 | 0.891 | 8 | 0.0 | 0.0 | 0.0 |
| d05 | GROUND | 1 | 5 | 0.85 | 8 | 0.044 | 0.044 | 0.0 |
| d06 | FLY | 0 | 168 | 0.84 | 8 | 0.029 | 0.0 | 100.0 |
| d07 | FLY | 1 | 59 | 0.883 | 8 | 0.0 | 0.0 | 0.0 |
| d08 | GROUND | 0 | 0 | 0.0 | 8 | 0.0 | 0.0 | 0.0 |
| d09 | FLY | 0 | 0 | 0.0 | 8 | 0.0 | 0.0 | 0.0 |

## Verdict

- Physics reliability (≥6/10 FLY): **PASS** (6/10)
- Habituation (drop ≥10% in flyers OR swarm cycle drop ≥10%): **PASS**
- Mission progress (≥20 target inspections): **FAIL** (5)