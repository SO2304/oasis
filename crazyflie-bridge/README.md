# OASIS → Crazyflie Bridge

Hardware-in-the-loop wrapper that connects the OASIS Rust kernel (`drone_bridge.exe`)
to a **Crazyflie 2.1** micro-quadrotor (Bitcraze) — the de facto open-hardware
platform for swarm research (ICRA/IROS papers, 49-drone Crazyswarm demo, etc.).

This **targets closing the gap** between "168 Webots loops" and "real flight
loops" — the physical-flight validation has not yet happened. This repo
contains the protocol wrapper only.

## What this is

- Python subprocess wrapper (~180L) using `cflib` (official Crazyflie Python lib)
- Reads CF state estimate + IMU + multi-ranger deck → JSON sensor packet
- Spawns `drone_bridge.exe` (the validated 10/10 OASIS kernel)
- Pipes sensors in, pipes motor commands out
- Applies commands via `send_hover_setpoint()` (world-frame velocity + altitude)

## What this is NOT

- NOT yet tested on physical hardware (author has no Crazyflie in hand)
- NOT tested against crazyflie-firmware SITL (planned, not validated)
- NOT a replacement for cfclient / Crazyswarm2 — just a OASIS-kernel frontend

**Honest status: PROTOTYPE adapter. Code compiles conceptually; wire-level
validation pending first real flight.**

## Prerequisites

### Software
```bash
pip install cflib
# Build the OASIS kernel
cd ../oasis-rt && cargo build --release --bin drone_bridge
```

### Hardware (for real flight)
- Crazyflie 2.1 (base, ~$220)
- Flow deck v2 (optical-flow positioning, ~$50) — OR Lighthouse positioning
- Multi-ranger deck (4× VL53L1X sonars, ~$80) — enables OASIS sonar reflex
- Crazyradio PA 2.0 (USB dongle, ~$35)
- **Total: ~$385 for a minimum viable OASIS-powered drone**

### Hardware (for SITL, no purchase needed)
```bash
git clone https://github.com/bitcraze/crazyflie-firmware
cd crazyflie-firmware && make sitl_defconfig && make sitl -j4
./build/sitl/cf2_sitl   # starts UDP server on 127.0.0.1:19850
```

## Usage

```bash
# SITL (no hardware):
python oasis_cf_bridge.py --uri udp://127.0.0.1:19850 --name patrol1 --duration 60

# Physical Crazyflie over Crazyradio:
python oasis_cf_bridge.py --uri radio://0/80/2M/E7E7E7E7E7 --name patrol1 --duration 60
```

Log is written to `./factory_patrol1.log` by default (same format as Webots
demo — full OASIS kernel telemetry with fear%, entropy, synapses, digests).

## Protocol mapping

| OASIS sensor | Crazyflie cflib log variable |
|--------------|------------------------------|
| x, y, alt    | stateEstimate.x / .y / .z |
| vx, vy       | stateEstimate.vx / .vy |
| roll, pitch  | stabilizer.roll / .pitch (deg→rad) |
| gz           | gyro.z (deg/s→rad/s) |
| rf, rb, rl, rr | range.front / .back / .left / .right (multi-ranger deck) |

| OASIS motor cmd | Crazyflie action |
|-----------------|------------------|
| dvx, dvy (m/s)  | send_hover_setpoint body vx, vy |
| s_alt (m)       | send_hover_setpoint z target |
| abort=true      | send_stop_setpoint |

## Why Crazyflie?

The Crazyflie 2.1 is the most academically-cited open hardware quadrotor
(ICRA/IROS/RSS papers), runs a permissive-license firmware, has a mature
Python bindings layer (cflib), and supports swarm scaling via Crazyswarm2
ROS2 stack. It is the lowest-friction path from "runs in Webots" to "runs
in the air".

## Known gaps to close before first flight

1. Validate kernel dvx/dvy magnitudes (capped ±0.30 m/s) match safe CF envelope
2. Tune R14 entropy threshold for real sensor noise (Webots is clean, CF is not)
3. Add failsafe: if kernel latency >50ms, fall back to `send_stop_setpoint`
4. Verify multi-ranger deck data rate (50ms period may be too slow for sonar reflex)
5. Battery telemetry → emotion.record_pain (bio-inspired low-battery fear)
6. Ground test with propellers OFF first (safe motor command sanity check)

## Next steps toward real 10/10

- [ ] Run in crazyflie-firmware SITL, verify protocol end-to-end
- [ ] Acquire Crazyflie 2.1 hardware, ground test
- [ ] First tethered flight, validate takeoff + hover
- [ ] First free flight with 1 target
- [ ] Multi-drone swarm (2-3 CFs with OASIS federation)
- [ ] Benchmark vs Crazyswarm2 stock controller on same mission
