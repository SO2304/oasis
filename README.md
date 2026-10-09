# OASIS — Open Agentic System for Intelligent Simulation

[![CI](https://github.com/SO2304/oasis/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/SO2304/oasis/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)

Bio-inspired agentic middleware: tension fields, Hebbian/STDP synapses, emotional
modulation, reflex arcs, federated learning. Runtime in Rust (49 modules, 672 lib tests,
716 passing across the workspace, 196 Kani proof harnesses of which 182 are verified on
CI). `tools/check_claims.sh` re-derives every one of those numbers from the tree.
Validated on:

- **Real hardware (claimed)** — Samsung S23 FE (Android/Termux), 3h23 continuous
  session with real sensors (LSM6DSVTR IMU, barometer, light, mic), 121 290 ticks.
  Note: session logs are not archived in this repo (not reproducible from the tree;
  see IOT_READINESS.md).
- **Simulation** — PX4 SITL + jmavsim over MAVLink v2: OASIS auto-armed PX4,
  triggered takeoff, and flew a 4-waypoint mission with closed-loop altitude
  hold (1.94 m vs 2.0 m target, ±6 cm). One successful end-to-end run; no real
  Pixhawk hardware yet. (Earlier Webots factory runs are kept as legacy results
  below — the Webots simulator was replaced by PX4 SITL.)

Mechanisms 1, 2, 5, 7, 9, 11 are **proven** on real hardware. Mechanisms 4, 6, 8, 10
are **wired and tested** in shorter runs. Mechanism 3 is **partial** (sensors yes,
DC motor actuators untested). See [status table below](#11-bio-inspired-mechanisms-honest-status).

## What OASIS is NOT

- Not a web app, not a SaaS, not a chatbot
- Not a framework, not a library, not a middleware for microservices
- Not a theoretical project — every mechanism is proven on real hardware

## What OASIS IS

An **artificial nervous system** running on real hardware (Android phones,
microcontrollers, robots) with real sensors (IMU, barometer, light, mic).
Agents do not send messages — they emit **vector forces** into a shared
tension field. Learning is biological (Hebbian/STDP), emotions modulate
behavior, reflexes short-circuit deliberation, and experience is shared via
federated resonance.

## 11 Bio-inspired Mechanisms (honest status)

| # | Mechanism | Status |
|---|-------|--------|
| 1 | Tensorial Agent Communication | PROVEN (9 Rust tests + phone) |
| 2 | HyperState + R14 Entropy Gate | PROVEN (9 tests + real trauma data) |
| 3 | Efference Copy (proprioception) | PARTIAL (6 tests + sensor; DC motors untested) |
| 4 | Temporal Branching | WIRED+TESTED (4 tests + phone 3/4) |
| 5 | Emotional Gain Modulation | PROVEN (long-run 1h30, habituation confirmed) |
| 6 | Agent Morphogenesis | WIRED+TESTED (6/6 Rust + 4/4 phone) |
| 7 | Hebbian Synaptic Network (STDP) | PROVEN (9/9 Rust + 7/7 phone) |
| 8 | Dream Consolidation | WIRED+TESTED (5 tests + phone 3/4) |
| 9 | Reflex Arc (<1ms adaptive) | PROVEN (3 tests + industrial T4) |
| 10 | Non-Euclidean World Model | WIRED+TESTED (7 tests + phone 4/4) |
| 11 | Federated Synaptic Resonance | PROVEN (6/6 Rust + 5/5 phone + cross-device) |

## Architecture

```
    PX4 SITL (MAVLink v2) / phone sensors ───┐
                                             │  (JSON sensor stream)
                                             ▼
  ┌─────────────────────────────────────────────────────┐
  │  OASIS Rust kernel  (oasis-rt/src/bin/drone_bridge) │
  │  ┌──────────────────────────────────────────────┐   │
  │  │ HyperState (M2) ── entropy gate R14          │   │
  │  │ WorldModel (M10) ── pressure-field gradient  │   │
  │  │ EmotionalState (M5) ── fear modulation       │   │
  │  │ SynapticNetwork (M7) ── Hebbian/STDP 4-agent │   │
  │  │ AdaptiveReflex (M9) ── calibrated 2σ         │   │
  │  │ FederatedMesh (M11) ── cross-drone digests   │   │
  │  └──────────────────────────────────────────────┘   │
  └─────────────────────────────────────────────────────┘
                                             │  (JSON motor commands)
                                             ▼
                 PX4 / MAVLink actuators
```

Source layout:
```
oasis-rt/                # Rust — production runtime (49 modules, 672 lib tests, 28 bins)
  src/vec.rs              # 128D vector algebra, zero-alloc
  src/hyper_state.rs      # Mechanism 2 — continuous state + entropy gate
  src/tension.rs          # Mechanism 1 — tension field, interference
  src/synapse.rs          # Mechanism 7 — Hebbian/STDP + credit assignment
  src/emotion.rs          # Mechanism 5 — 5 signals + pain memory
  src/reflex.rs           # Mechanism 9 — reflex arc, adaptive calibration
  src/federation.rs       # Mechanism 11 — vectorial resonance
  src/world_model.rs      # Mechanism 10 — non-Euclidean pressure fields
  src/bin/drone_bridge.rs    # OASIS kernel <-> MAVLink JSON bridge
  src/bin/mavlink_adapter.rs # PX4 adapter: OFFBOARD, PARAM_SET, waypoint loop
webots/                  # Legacy Webots audit docs (simulator no longer used)
```

## Quickstart

### Docker
```bash
docker build -f oasis-rt/Dockerfile -t oasis-rt .
docker run --rm oasis-rt
```

### Native (cargo)
```bash
cd oasis-rt
cargo build --release
cargo test --workspace --release # 716 pass, 2 doc-test blocks marked `ignore`
./target/release/drone_bridge patrol1 0 < sensor_stream.jsonl
```

### PX4 SITL (MAVLink)
```bash
cargo build --release --bin mavlink_adapter
# with PX4 SITL + jmavsim running and exposing MAVLink on udp:14540
./target/release/mavlink_adapter   # auto-arm -> takeoff -> 4-waypoint OFFBOARD loop
```

## Validation Results

### PX4 SITL (current drone path)

OASIS drives PX4 over MAVLink v2 end-to-end: PX4 logged `Armed by external
command` → `Takeoff detected` → 3× `WAYPOINT_REACHED`, then held altitude
closed-loop at 1.94 m vs a 2.0 m target (±6 cm). One successful end-to-end run;
**no real Pixhawk hardware tested yet.**

### Webots factory inspection (legacy — simulator since replaced by PX4 SITL)

3 autonomous drones in a 6×5m factory with 9 obstacles of varying height.
Emergent navigation via OASIS kernel (no scripted waypoints):

| Drone | Inspections | Full Coverage Loops | Crashes | Federation Digests |
|-------|-------------|---------------------|---------|---------------------|
| patrol1 (1.30m cruise) | 674 | **168 × 4/4** | 0 | 200+ |
| patrol2 (1.30m cruise) | 14 | 3 × 4/4 | 1 (post-loop-3) | 31 |
| supervisor (1.70m cruise) | 10 | 1 × 8/8 | 0 | 200+ |

Active mechanisms in `drone_bridge.rs`:
- **C2 HyperState + R14** — entropy gate blocks actuation when sensor loss
- **C5 Emotion** — fear modulates speed continuously
- **C7 Hebbian 4-agent** — motor/goal/obstacle/fear synapses
- **C9 Reflex** — AdaptiveReflex calibrated on sonar (2σ threshold)
- **C10 WorldModel** — altitude-gated pressure-field gradient descent
- **C11 Federation** — real digests saved/loaded cross-drone

## Proven Production Session

- **3h23 continuous** on Samsung S23 FE via Termux (121 290 ticks, walking + transit) — logs not archived in-repo
- **Record**: 3h33 v0.3 (2590 ticks on train), 0 crashes
- **Kernel latency**: avg 8ms, 76% under 10ms
- **Memory**: persistent on /sdcard/ (pain + federated digests, ~7 KB)
- **Industrial hardening**: 7/7 tests passed with real physical perturbations

## Inviolable Rules (Rust)

| # | Rule | Status |
|---|------|--------|
| R2 | Agents communicate ONLY via tension field | enforced |
| R5 | Validation on all mutations | enforced |
| R10 | Files < 400 lines | fmt-clean; >400 L limited to sanctioned crypto/protocol + daemon modules (see note) |
| R13 | Inter-agent communication via vector tension | enforced |
| R14 | No physical action if entropy > critical threshold | enforced (`is_action_safe`) — ⚠️ **not applicable on a Linux host**: the TCP gateway has no sensor to read, so it passes the named `R14_NOT_APPLICABLE_ON_HOST`, which means *not applicable*, not *verified safe* |
| R18 | Jitter < 5%, out-of-budget operations truncated | enforced |
| R20 | Unsigned node = atomization in < 1ms | enforced |

**R10 note**: the codebase is fmt-clean — `cargo fmt --check` passes under
`oasis-rt/rustfmt.toml`. The files over 400 L are the cohesive crypto/protocol
modules (`spore_crypto`, `mavlink_min`, `spore`, `federation`, `mesh`) and the
Termux daemon (`main`), tracked as sanctioned R10 exceptions in CLAUDE.md;
`mesh.rs` had its tests + Kani proofs split into `src/mesh/` to shrink its core.
R1 (strict typing) is satisfied by Rust, which is strictly typed by default.

## Honest Positioning

OASIS is a **research prototype** occupying a unique niche:
*nervous-system-as-middleware*, bio-inspired, vector-field-native. It is NOT
a commercial product (no vision stack, no certifications, no production
deployments). It IS a scientifically defensible platform with 4-5 patentable
ideas and hardware validation.

### Roadmap toward market relevance

- [ ] Crazyflie 2.1 physical port (planned; prior Python cflib bridge removed in the pure-Rust migration)
- [ ] OAK-D vision integration (addresses #1 market gap)
- [ ] ROS2 / Crazyswarm2 node wrapper
- [ ] External quantitative benchmark vs Nav2 on standardized scenarios
- [ ] Scale test: 20-50 drones in simulation
- [ ] DO-178C awareness + deterministic execution paths

## License

MIT — see [LICENSE](./LICENSE).

## Contributing

Issues and PRs welcome. Baseline expectations:
- R10: no file > 400 lines
- R1: strict typing (Rust is strictly typed by default; no `any` equivalent)
- New features require hardware or ruthless simulation tests

## Citation

A preprint / arXiv entry will be published after the first real Crazyflie
physical flight with OASIS kernel onboard. Until then, cite this repository:

```
@software{oasis2026,
  title  = {OASIS: Bio-Inspired Agentic Middleware},
  author = {OASIS Project contributors},
  year   = {2026},
  url    = {https://github.com/SO2304/oasis},
  note   = {Research prototype, hardware validation in progress}
}
```
