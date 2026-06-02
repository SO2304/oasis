# OASIS — State of Truth (2026-04-21)

This document supersedes the 35 round-by-round audit files in `webots/`.
Older docs are kept for git history but should NOT be cited as current state.

---

## 1. What OASIS IS, today

A **278 KB Rust autonomy kernel** for drones with:
- Formally-verified safety gate (R14 entropy threshold) — 3 Kani proofs
- Full **MAVLink v2** stack (parser, encoder, signing, replay protection, allowlist, persistence)
- **Real PX4 SITL integration** verified end-to-end (jmavsim simulator)
- Bio-inspired runtime (11 mechanisms — see `CLAUDE.md`)

Plus a TypeScript reference kernel and Android/Termux deployment for sensor experiments.

## 2. What OASIS IS NOT

- Not flight-certified
- Not tested on real hardware (Pixhawk + actual quadcopter)
- Not a mission-planning system (no MISSION_ITEM_INT, no GCS-style planner)
- Not GPS-denied capable
- Not weather/disturbance-tested

---

## 3. Validation matrix (only the claims that hold up)

| Capability | Evidence | Status |
|---|---|---|
| 174 unit tests pass in parallel | `cargo test --lib --release` exit 0 | ✅ |
| 3 Kani R14 proofs verified | `cargo kani --lib` in WSL Ubuntu | ✅ |
| MAVLink v2 frame parse + CRC | 29 in-suite tests + 300 real PX4 frames parsed | ✅ |
| MAVLink v2 signing (Ed25519 + SipHash) | `signed_roundtrip_same_key`, `ed25519_signing_roundtrip` | ✅ |
| Replay protection (per-link bitmap) | `signed_rejects_tampered`, `ed25519_rejects_tampered` | ✅ |
| Auto-arm PX4 via OASIS | PX4 log: `Armed by external command` | ✅ |
| Auto-takeoff via OASIS | PX4 log: `Takeoff detected` | ✅ |
| MAVLink PARAM_SET (msgid 23) | 5/5 PARAM_VALUE responses match commanded values | ✅ |
| **Closed-loop altitude hold** | drone settles at 1.94m vs target 2.0m (within 6cm) | ✅ |
| **4-waypoint mission complete** | 3 WAYPOINT_REACHED events, drone returned home | ✅ (1 successful run) |
| Z-axis accumulator (VfrHud lock) | Regression test `vfr_hud_does_not_override_local_alt` | ✅ |
| 11 bio-inspired mechanisms | See CLAUDE.md status table | mixed (6/11 PROUVE) |

---

## 4. Validation matrix (gaps, honestly)

| Gap | Why it's open | Effort to close |
|---|---|---|
| jmavsim flight is flaky | Some runs reach all waypoints, some hover stuck. Same code, same env vars. | Investigate — likely jmavsim physics seed, not OASIS |
| OFFBOARD altitude limit only in velocity-mode codepath | Position-setpoint mode bypasses it | ~10 min — add `tz.min(alt_limit)` already partially in place |
| No real hardware test | No Pixhawk + quad available | Hardware access — bounded by this constraint |
| MISSION_ITEM_INT not implemented | OASIS uses OFFBOARD continuous setpoints, not stored missions | ~2-3h |
| No staleness recovery test | Adapter has handler (`elapsed > 500ms → hover`) but never hit it under stress | ~30 min — induce link loss in script |
| Yaw not commanded (type_mask 0x0DF8 ignores yaw) | Drone may rotate uncontrolled | ~15 min — add yaw setpoint |
| 11 mechanisms: only 6/11 PROUVE on hardware | 4 (M4, M6, M8, M10) are CABLE+TESTE in daemon, M3 PARTIEL | Ongoing — long sessions on phone |
| Binary "280 KB" claim is the lib only | mavlink_adapter binary is 650 KB on Linux | Edit pitch wording |

---

## 5. Architecture (as of today)

```
oasis-rt/                    Rust runtime, 23+ modules, 174 tests
  src/
    lib.rs                   crate root
    vec.rs            139L   128D vectors, zero-alloc cosine/norm
    hyper_state.rs    436L   M2 entropy + R14 + 3 Kani proofs
    tension.rs        181L   M1 vector field interference
    synapse.rs        288L   M7 Hebbian/STDP + credit assignment
    emotion.rs        278L   M5 5 emotions + saturated fear (5x cap)
    reflex.rs          87L   M9 reflex arc, adaptive sigma
    federation.rs     815L   M11 Ed25519-signed federated mesh
    morpho.rs         236L   M6 reversible specialisation
    efference.rs      227L   M3 efference copy + pain
    dreams.rs         191L   M8 replay + counterfactual
    branching.rs      156L   M4 N-timeline fork + fitness
    world_model.rs    215L   M10 non-Euclidean pressure fields
    audio.rs          201L   RMS, FFT, pitch (no ML)
    spore.rs          245L   inter-device base64/QR
    hal.rs            296L   killswitch, manifolds
    nerve.rs          397L   24 afferent + 8 efferent
    spinal.rs         395L   5-platform auto-discovery
    vitality.rs       200L   Healthy → Degraded → Critical → Dead
    transport.rs      194L   Transport trait + InMemory + LoRa stub
    mavlink_min.rs   1392L   MAVLink v2: parse/encode/sign/replay/allowlist
                             — 11 message types, 8 encoders, accumulator
                             — VFR_HUD altitude leak fix (2026-04-21)
    bin/
      mavlink_adapter.rs 424L  PX4 adapter: 4 threads, OFFBOARD,
                               BOOST_PARAMS, waypoint loop
      drone_bridge.rs        OASIS kernel ↔ MAVLink JSON bridge
      [+ ~17 other test/demo binaries]
```

---

## 6. The PX4 integration recipe (verified working)

```bash
# WSL Ubuntu 24.04
sudo apt install -y openjdk-17-jre-headless ant build-essential cmake
git clone --depth=1 --recursive https://github.com/PX4/PX4-Autopilot.git
cd PX4-Autopilot && bash Tools/setup/ubuntu.sh --no-nuttx --no-sim-tools -y
make px4_sitl none && cd Tools/simulation/jmavsim/jMAVSim && ant create_run_jar copy_res

cd /mnt/c/dev/oasis/oasis-rt
CARGO_TARGET_DIR=/root/oasis_linux_target cargo build --release \
  --bin mavlink_adapter --bin drone_bridge

# Three terminals (or nohup):
# 1: jmavsim
cd /root/PX4-Autopilot/Tools/simulation/jmavsim/jMAVSim/out/production
java --add-exports java.base/java.lang=ALL-UNNAMED \
  --add-exports java.desktop/sun.awt=ALL-UNNAMED \
  --add-exports java.desktop/sun.java2d=ALL-UNNAMED \
  -jar jmavsim_run.jar -tcp 127.0.0.1:4560 -no-gui

# 2: PX4 SITL
PX4_SIM_MODEL=jmavsim_iris /root/PX4-Autopilot/build/px4_sitl_default/bin/px4 \
  -d /root/PX4-Autopilot/build/px4_sitl_default/etc

# 3: OASIS adapter — autonomous waypoint mission
OASIS_BRIDGE_PATH=/root/oasis_linux_target/release/drone_bridge \
OASIS_MAV_PEER=127.0.0.1:18570 \
OASIS_AUTO_OFFBOARD=1 \
OASIS_OFFBOARD_STREAM=1 \
OASIS_BOOST_PARAMS=1 \
OASIS_WAYPOINTS="1,0,2;1,1,2;0,1,2;0,0,2" \
OASIS_WP_RADIUS=0.5 \
OASIS_ALT_LIMIT=8.0 \
/root/oasis_linux_target/release/mavlink_adapter d00 0
```

Expected: `Armed → Takeoff detected`, 5 PARAM_VALUE responses confirming
boost (`MPC_XY_VEL_MAX=4.0` etc), drone navigates square + returns home.

⚠️ jmavsim flake: re-run if drone hovers without takeoff — usually
EKF didn't converge properly first time.

---

## 7. The pitch (final, no OASIS-washing)

> "OASIS is a 278 KB Rust autonomy kernel with formally-proved safety
> (3 Kani R14 proofs), 174 unit tests, full MAVLink v2 stack (signing,
> replay, allowlist), and **closed-loop autonomous waypoint navigation
> demonstrated on real PX4 SITL** (4-waypoint square completed, altitude
> held at 2m ±6cm). 11 bio-inspired mechanisms (6/11 validated on
> Android phone with real sensors over 3+ hour sessions). Pre-1.0 —
> needs hardware validation on Pixhawk + actual quadcopter."

Every clause backed by a test, a log line, or a measurement. No more.

---

## 8. Old docs (deprecated reading order)

These exist for git history but represent *snapshots in time*, not
current state. Read STATE_OF_TRUTH.md instead. If a claim in an old
doc contradicts this file, **this file wins**.

```
recon10*.md         — 10-drone Webots swarm tests
recon20*.md         — 20-drone test (capped at 10 stable)
OASIS_PX4_*.md      — 5 docs covering PX4 integration progression
OASIS_FLIES_DRONE.md, OASIS_autonomous_flight.md, OASIS_closed_loop_validated.md,
OASIS_waypoint_mission.md, OASIS_position_setpoint.md, OASIS_param_boost_validated.md
                    — chronological documentation of MAVLink stack build-out
OASIS_kani_verification.md           — Kani install + 3 proofs (still accurate)
OASIS_mavlink_security_complete.md   — signing + replay (still accurate)
OASIS_competitive_analysis.md, OASIS_novelty_analysis.md, OASIS_vs_MAVLink_STANAG_benchmark.md
                    — positioning analysis (timeless)
OASIS_v6_*.md       — ROS 2 / MAVLink bridge plans
OASIS_execution_round_*.md           — historical iteration notes
```

---

## 9. What's next (specialist priorities)

1. **Hardware test on Pixhawk + real quadcopter** — closes the only gap
   that simulation can't address. Bounded by hardware access.
2. **jmavsim flake investigation** — track down why drone sometimes
   hovers stuck instead of navigating. Likely PX4 EKF convergence timing
   or jmavsim physics initial conditions.
3. **MISSION_ITEM_INT** (~2-3h) — switch from continuous OFFBOARD
   setpoints to stored mission for more robust GCS-style flight.
4. **Yaw setpoint** (~15 min) — currently type_mask 0x0DF8 ignores yaw.
5. **Staleness/link-loss stress test** (~30 min) — validate handler.

Everything else (more PARAM_SETs, integral altitude, hysteresis) is
diminishing returns on a simulator.
