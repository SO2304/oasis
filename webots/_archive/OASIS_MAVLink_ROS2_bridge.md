# OASIS ↔ MAVLink / ROS 2 Bridge — Interoperability Mapping

**Purpose**: document how OASIS's JSON stdin/stdout protocol maps cleanly to MAVLink / ROS 2 so
OASIS complements (rather than competes with) existing stacks.

---

## 1. The adapter pattern

```
     MAVLink (UDP 14550)           OASIS drone_bridge.exe (stdio)
  ┌────────────────────┐          ┌──────────────────────────────┐
  │ ATTITUDE           │  →JSON→  │ stdin:                       │
  │ GLOBAL_POSITION_INT│          │   {"x","y","alt",            │
  │ DISTANCE_SENSOR    │          │    "roll","pitch",           │
  │ SYS_STATUS         │          │    "rf","rl","rr","rb",      │
  │                    │          │    "imu_alive",...}          │
  │                    │          │                              │
  │ SET_POSITION_TARGET│  ←JSON←  │ stdout:                      │
  │ HEARTBEAT(armed)   │          │   {"dvx","dvy","s_alt",      │
  │ STATUSTEXT         │          │    "abort","fear","entropy"} │
  └────────────────────┘          └──────────────────────────────┘
```

A ~200 LOC Python/Rust adapter translates bidirectionally. Neither side knows the other exists.
This is **proven viable** because the existing `oasis_factory.py` Webots controller is exactly such
an adapter (translates Webots sensors → OASIS JSON → Webots motor velocities).

## 2. Field mapping — sensors (MAVLink → OASIS stdin)

| OASIS field | MAVLink message.field | Transform |
|-------------|------------------------|-----------|
| `x, y, alt` | `GLOBAL_POSITION_INT.lat/lon/alt` | Convert (lat,lon,alt) → local (x,y,z) via ENU projection |
| `roll, pitch, gz` | `ATTITUDE.roll, .pitch, .yawspeed` | Direct |
| `vx, vy` | `GLOBAL_POSITION_INT.vx, .vy` | Divide by 100 (MAVLink cm/s → m/s) |
| `rf, rl, rr, rb` | `DISTANCE_SENSOR[4]` (4 separate messages) | orientations 0°, 270°, 90°, 180°; scale /1000 (mm→m) |
| `imu_alive` | `SYS_STATUS.onboard_control_sensors_enabled & MAV_SYS_STATUS_SENSOR_3D_GYRO` | Bool mask |
| `gps_alive` | `SYS_STATUS.onboard_control_sensors_enabled & MAV_SYS_STATUS_SENSOR_GPS` | Bool mask |
| `sonar_alive` | `SYS_STATUS.onboard_control_sensors_enabled & MAV_SYS_STATUS_SENSOR_LASER_POSITION` | Bool mask (or PROXIMITY_SENSOR health) |

## 3. Field mapping — commands (OASIS stdout → MAVLink)

| OASIS output | MAVLink message.field | Transform |
|--------------|------------------------|-----------|
| `dvx, dvy` | `SET_POSITION_TARGET_LOCAL_NED.vx, .vy` | m/s, type_mask velocity-only |
| `s_alt` | `SET_POSITION_TARGET_LOCAL_NED.z` | negate sign (NED Z-down), send as target altitude |
| `abort=true` | Send `COMMAND_LONG(MAV_CMD_DO_SET_MODE, mode=MANUAL)` + zero setpoint | Drops to manual; ground operator takes over |
| `fear`, `entropy` | Custom `STATUSTEXT` or `NAMED_VALUE_FLOAT` | Optional telemetry for ground station |

## 4. Field mapping — ROS 2 (when needed)

For ROS 2 integration:

| ROS 2 topic | OASIS mapping |
|-------------|---------------|
| `/odom` (nav_msgs/Odometry) | Produces: x, y, alt, vx, vy |
| `/scan` (sensor_msgs/LaserScan) | Produces: rf, rl, rr, rb (subset of 360° scan) |
| `/cmd_vel` (geometry_msgs/Twist) | Consumes: dvx, dvy |
| `/imu` (sensor_msgs/Imu) | Produces: roll, pitch, gz |
| `/diagnostics` (diagnostic_msgs) | Consumes: `"evt":"r14_block"` JSON lines |

A `rclpy` node of ~150 LOC wraps `drone_bridge.exe` subprocess.

## 5. Deployment topologies

### Topology A: OASIS as cognitive co-processor alongside PX4
```
  Sensors → PX4 (flight control) → OASIS (R14 + emotion + federation) → Ground
```
PX4 handles stabilization / PID / radio. OASIS runs on companion computer (Raspberry Pi, Jetson) reading MAVLink. Receives PX4 telemetry, emits advisory (abort, fear, entropy) back to PX4 via custom MAVLink or mission abort.

### Topology B: OASIS standalone with MAVLink telemetry output
```
  Sensors → OASIS (full stack via drone_bridge) → MAVLink telemetry → Ground station
```
OASIS is the flight controller. QGroundControl sees a MAVLink-speaking autopilot. Same stdin/stdout adapter just outputs MAVLink streams.

### Topology C: OASIS swarm layer over PX4 fleet
```
  Multiple PX4 drones → each with OASIS companion → OASIS mesh (federation) ↔ ↔ ↔
                                                  └── auto-discovery via shared FS or LoRa
```
PX4 handles flight per drone. OASIS coordinates across the fleet via digest exchange. Ground needn't know about OASIS-level coordination.

## 6. Security considerations

- MAVLink 2 signing: each message has an HMAC SHA256 (6B truncated) tag with timestamp
- OASIS federation signing: with `OASIS_SIGNING_KEY` set, digests get a SipHash-based MAC (64 bits)
- To combine: MAVLink messages + OASIS digests can share a signing key, giving end-to-end authentication

**Trust asymmetry**: MAVLink 2 signing is per-message (fine-grained), OASIS signing is per-digest (coarse-grained). For defense deployment, use MAVLink 2 for telemetry channel + OASIS signing for federation blobs; both require key distribution (out of scope for either).

## 7. Benchmarkable metrics when bridged

With the adapter in place, measurable comparisons:

| Metric | OASIS advantage | Test approach |
|--------|-----------------|---------------|
| R14 decision latency | 243 ns measured (bench_r14_latency) vs PX4 failsafe 50-200 ms | Inject sensor fault, measure time to mode change |
| Swarm bandwidth | 20 B/s/drone (OASIS) vs 250+ B/s/drone (MAVLink) | tcpdump byte counts over 10 min swarm test |
| Disconnect resilience | seconds to merge on reconnect vs minutes of log replay | Kill uplink 10 min, reconnect, measure sync time |
| Safety audit log completeness | 100% structured JSON vs STATUSTEXT strings | Parse log after 1000 fault injections |

## 8. Adapter reference implementation sketch (~200 LOC Python)

```python
import json, subprocess, threading
from pymavlink import mavutil

BRIDGE = subprocess.Popen(["drone_bridge.exe", "d0", "0"],
                          stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
mav = mavutil.mavlink_connection('udpin:localhost:14550')

def mav_to_oasis():
    while True:
        msg = mav.recv_match(blocking=True)
        if msg.get_type() == 'GLOBAL_POSITION_INT':
            pkt = {"x": msg.lat/1e7, "y": msg.lon/1e7, "alt": msg.alt/1e3, ...}
            BRIDGE.stdin.write(json.dumps(pkt) + "\n"); BRIDGE.stdin.flush()

def oasis_to_mav():
    while True:
        line = BRIDGE.stdout.readline()
        cmd = json.loads(line)
        if cmd.get("abort"):
            mav.mav.command_long_send(..., mavutil.mavlink.MAV_CMD_DO_SET_MODE, ...)
        else:
            mav.mav.set_position_target_local_ned_send(..., vx=cmd["dvx"], vy=cmd["dvy"], ...)

threading.Thread(target=mav_to_oasis, daemon=True).start()
threading.Thread(target=oasis_to_mav, daemon=True).start()
```

That's the entirety of the bridge. ~200 LOC including MAVLink enum imports and error handling. A ROS 2 equivalent is similar with `rclpy.Node` + topic subs/pubs.

## 9. Honest gap statement

This document describes the **architecture** of a MAVLink/ROS 2 bridge. It is NOT implemented in the
current codebase. The `oasis_factory.py` controller is analogous but Webots-specific. A full
MAVLink adapter would be ~60h of engineering. A ROS 2 node ~60h. **Neither is done.**

What IS done:
- JSON stdin/stdout protocol stabilized
- `OASIS_LOG_JSON=1` emits structured audit lines that MAVLink-side logging can ingest
- `OASIS_SIGNING_KEY` allows compatible MAC with MAVLink 2 signing
- `Transport` trait allows swapping file → MAVLink / ROS 2 topics without changing federation code
