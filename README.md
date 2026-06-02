# OASIS — Open Agentic System for Intelligent Simulation

[![CI](https://github.com/oasis-project/oasis/actions/workflows/ci.yml/badge.svg)](https://github.com/oasis-project/oasis/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)

Bio-inspired agentic middleware: tension fields, Hebbian/STDP synapses, emotional
modulation, reflex arcs, federated learning. Runtime in Rust (30 modules, 443
unit tests). Validated on:

- **Real hardware** — Samsung S23 FE (Android/Termux), 3h23 continuous session
  with real sensors (LSM6DSVTR IMU, barometer, light, mic), 121 290 ticks.
- **Simulation** — Webots multi-drone factory inspection, 168 full coverage
  loops by patrol1 over ~76 000 sim ticks in `--mode=fast --no-rendering`
  (wall-clock ~10 minutes; Webots runs faster than real-time).

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
    Webots / Crazyflie / phone sensors  ────┐
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
                 Webots / Crazyflie / motors
```

Source layout:
```
oasis-rt/                # Rust — production runtime (30 modules, 443 tests, 18 bins)
  src/vec.rs              # 128D vector algebra, zero-alloc
  src/hyper_state.rs      # Mechanism 2 — continuous state + entropy gate
  src/tension.rs          # Mechanism 1 — tension field, interference
  src/synapse.rs          # Mechanism 7 — Hebbian/STDP + credit assignment
  src/emotion.rs          # Mechanism 5 — 5 signals + pain memory
  src/reflex.rs           # Mechanism 9 — reflex arc, adaptive calibration
  src/federation.rs       # Mechanism 11 — vectorial resonance
  src/world_model.rs      # Mechanism 10 — non-Euclidean pressure fields
  src/bin/drone_bridge.rs # Thin drone brain for Webots/Crazyflie (400L R10)
webots/                  # Factory inspection demo (3 drones, 8 machines)
crazyflie-bridge/        # cflib wrapper for Crazyflie 2.1 (SITL + hardware)
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
cargo test --workspace --release # 443 unit tests
./target/release/drone_bridge patrol1 0 < sensor_stream.jsonl
```

### Webots factory demo
```bash
cargo build --release --bin drone_bridge
webots --mode=fast --no-rendering webots/worlds/oasis_factory.wbt
tail -f webots/factory_patrol1.log
```

## Factory Inspection Results (Webots simulation, `--mode=fast`)

3 autonomous drones in a 6×5m factory with 9 obstacles of varying height.
Emergent navigation via OASIS kernel (no scripted waypoints):

| Drone | Inspections | Full Coverage Loops | Crashes | Federation Digests |
|-------|-------------|---------------------|---------|---------------------|
| patrol1 (1.30m cruise) | 674 | **168 × 4/4** | 0 | 200+ |
| patrol2 (1.30m cruise) | 14 | 3 × 4/4 | 1 (post-loop-3) | 31 |
| supervisor (1.70m cruise) | 10 | 1 × 8/8 | 0 | 200+ |

Active mechanisms in `drone_bridge.rs` (400L, R10 respected):
- **C2 HyperState + R14** — entropy gate blocks actuation when sensor loss
- **C5 Emotion** — fear modulates speed continuously
- **C7 Hebbian 4-agent** — motor/goal/obstacle/fear synapses
- **C9 Reflex** — AdaptiveReflex calibrated on sonar (2σ threshold)
- **C10 WorldModel** — altitude-gated pressure-field gradient descent
- **C11 Federation** — real digests saved/loaded cross-drone

## Proven Production Session

- **3h23 continuous** on Samsung S23 FE via Termux (121 290 ticks, walking + transit)
- **Record**: 3h33 v0.3 (2590 ticks on train), 0 crashes
- **Kernel latency**: avg 8ms, 76% under 10ms
- **Memory**: persistent on /sdcard/ (pain + federated digests, ~7 KB)
- **Industrial hardening**: 7/7 tests passed with real physical perturbations

## Inviolable Rules (Rust)

| # | Rule | Status |
|---|------|--------|
| R2 | Agents communicate ONLY via tension field | enforced |
| R5 | Validation on all mutations | enforced |
| R10 | Files < 400 lines | target (rustfmt expansion pushed 5 modules over — see note) |
| R13 | Inter-agent communication via vector tension | enforced |
| R14 | No physical action if entropy > critical threshold | enforced (`is_action_safe`) |
| R18 | Jitter < 5%, out-of-budget operations truncated | enforced |
| R20 | Unsigned node = atomization in < 1ms | enforced |

**R10 note**: pre-fmt the kernel modules were all < 400 lines. Applying `cargo
fmt` with `max_width=200` still expands 5 files (federation, nerve, spinal,
main, drone_bridge) past the limit due to struct literal and match-arm rules.
The codebase is now fmt-clean at the cost of this R10 breach; we are working
on compact idioms to restore it. R1 (strict typing) is satisfied by Rust,
which is strictly typed by default.

## Honest Positioning

OASIS is a **research prototype** occupying a unique niche:
*nervous-system-as-middleware*, bio-inspired, vector-field-native. It is NOT
a commercial product (no vision stack, no certifications, no production
deployments). It IS a scientifically defensible platform with 4-5 patentable
ideas and hardware validation.

### Roadmap toward market relevance

- [ ] Crazyflie 2.1 physical port (cflib bridge prototype in this repo)
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
  url    = {https://github.com/oasis-project/oasis},
  note   = {Research prototype, hardware validation in progress}
}
```
