# OASIS vs MAVLink / STANAG 4586 — Comparative Benchmark

**Scope**: paper-based + measurement-based comparison. Where I have hard numbers from v5 logs, I cite them. Where I use published performance data for MAVLink/STANAG, I cite the source paradigm (not specific claims beyond public docs).

---

## 1. State representation size

Per "one slice of drone state":

| System | Message | Size | Content |
|--------|---------|------|---------|
| MAVLink 2 | `GLOBAL_POSITION_INT` | 28 bytes | lat, lon, alt, vel xyz, heading (integers) |
| MAVLink 2 | `HEARTBEAT` | 9 bytes | type, autopilot, base_mode, custom_mode, system_status, mavlink_version |
| MAVLink 2 | `ATTITUDE` | 28 bytes | time_boot, roll, pitch, yaw, rates |
| STANAG 4586 | Level 2 position report | ~70 bytes | UAV ID, position, attitude, timestamp |
| **OASIS** | One federation digest | ~60 bytes | axis vector + strength + sign + entropy |
| **OASIS** | One pain memory entry | ~220 bytes | 128D sparse pos + intensity + tick |
| **OASIS** | Full pain buffer (128 entries) | **28 172 bytes** (real file) | complete "experiential memory" |
| **OASIS** | Full fed memory | **59 bytes** (real file) | one collective digest |

**Verdict**: OASIS digest (60 B) is comparable to MAVLink position (28 B) or slightly larger. OASIS full pain buffer (28 KB) is much larger than a single MAVLink message but comparable to one MAVLink log file snippet.

**Honest conclusion**: OASIS is NOT bandwidth-optimized. For pure position/state exchange, MAVLink wins on size. OASIS's size advantage (if any) is in CONTENT — the buffer includes learned priors, not just current state.

---

## 2. Transfer / sync pattern

| System | Offline operation | Reconnect cost | Loss tolerance |
|--------|-------------------|----------------|----------------|
| MAVLink | Log to dataflash; full replay on reconnect | high (minutes for long logs) | low (sequenced messages assume link) |
| STANAG 4586 | Similar; LRU+buffer scheme | high | medium |
| **OASIS** | Keeps running, accumulates pain+fed state offline | low (merge_foreign reads one file) | high (trust-gated merge tolerates missing peers) |

**Verdict**: OASIS's federation architecture is designed for DDIL (denied, disconnected, intermittent, limited) operation. MAVLink is designed for continuous link.

**Honest caveat**: OASIS still uses the FILESYSTEM for transfer. No radio/network transport built in. This means "DDIL operation" is theoretical — in practice you need something (LoRa? Bluetooth? QR scan?) to move the files between nodes. Until that's built, the DDIL claim is architectural only.

---

## 3. Safety / explainability

| System | Safety output | Auditability |
|--------|---------------|--------------|
| MAVLink | STATUS_TEXT (severity + string) | Strings are unstructured |
| STANAG 4586 | Status Level 2/3 messages | structured but ambient-only |
| PX4 | Failsafe modes (discrete enum) | documented but rule-pile |
| **OASIS** | R14 BLOCK stderr line: `mode=X signal=Y thr=Z level=W (blocks=N)` | Structured, per-tick, traceable to code-level invariants |

**OASIS unique**: every R14 block emits a line that tells you EXACTLY why ("entropy 0.97 exceeded threshold 0.95 at vitality level Healthy"). No MAVLink equivalent that ties state to a named invariant.

**Shadow audit**: OASIS logs go to stderr, not to any structured protocol. To be certification-ready, they would need:
- JSON lines format
- Event numbering / seq
- Cryptographic signing
- Separation from noise

Currently OASIS is ahead of MAVLink on explainability POTENTIAL, but behind on STANDARDIZATION of that potential.

---

## 4. Cross-platform portability

| System | Phone | Drone | Microcontroller |
|--------|-------|-------|-----------------|
| MAVLink | yes (QGroundControl, Mission Planner) | yes (PX4, AP) | yes (mavgen in C) |
| STANAG 4586 | usually ground-station only | level 2/3 as VSM | rare |
| ROS 2 | heavy (requires rclcpp/py) | heavy | impractical (<16 MB RAM) |
| **OASIS** | yes (Termux/Android) | subprocess (Webots+Rust) | NOT YET TESTED (no_std partial) |

**Verdict**: same binary across phone/drone is architecturally possible but only phone+sim validated. Microcontroller claim is aspirational.

---

## 5. Bandwidth per drone per second

Rough scaling analysis at 20 drones × 10 Hz update:

| System | Per-drone BW | Swarm BW | Notes |
|--------|--------------|----------|-------|
| MAVLink (minimal) | ~250 B/s | 5 KB/s | HEARTBEAT + GLOBAL_POSITION_INT + ATTITUDE |
| MAVLink (full) | ~2 KB/s | 40 KB/s | with sensor streams, servo outputs, etc. |
| STANAG 4586 L2 | ~700 B/s | 14 KB/s | position + attitude + identity |
| **OASIS** (fed emit every 100t = 3.2s) | **~20 B/s** | **400 B/s** | 60 B digest / 3.2s |
| **OASIS** (per-tick fed emit) | ~1.9 KB/s | 37 KB/s | 60 B × 31 Hz |

**Verdict**: OASIS at its default rate (emit every 100 ticks) is VERY low bandwidth — an order of magnitude less than MAVLink. This is because OASIS emits digests on events, not continuous streams.

**Honest caveat**: low BW = less information. MAVLink at 10 Hz lets ground station draw smooth tracks; OASIS at ~0.3 Hz digest emission is event-driven only. Different trade-off for different use cases.

**REAL potential win for OASIS**: in BW-constrained environments (LoRa 10 kbps, HF radio, satcom), OASIS's 400 B/s swarm-total fits easily; MAVLink's 40 KB/s does not.

---

## 6. What MAVLink/STANAG do that OASIS doesn't

Honest list:
- Parameter management (MAV_CMD_PARAM_SET, etc.)
- Mission planning protocol (waypoints upload/download)
- Sensor calibration commands
- Manual control inputs
- Ground-station tooling (QGroundControl, Mission Planner)
- Authentication (MAVLink 2 signing)
- Video stream headers (MAVLink CAMERA messages)
- Certification (STANAG 4586 is NATO-approved)
- Large-community tooling

**OASIS does essentially none of these**. It's a cognition+safety layer, not a command-and-control protocol. For a real deployment, OASIS would ride ON TOP OF MAVLink, not replace it.

---

## 7. Where OASIS could win a benchmark

Three measurable comparisons where OASIS could show real advantage:

### 7.1 Bandwidth under DDIL conditions
**Test**: 20 drones, link available 30% of time, rest disconnected.
**Metric**: total bits transmitted + information retained.
**Expected result**: OASIS digests (event-driven) use less BW and retain more (because they accumulate offline); MAVLink logs fill, replay costs mount.

### 7.2 Reconnection cost after partition
**Test**: drone disconnected 10 minutes, then reconnected.
**Metric**: time to full sync between drone and ground.
**Expected result**: OASIS `merge_foreign` is O(digests) ~60 B × 20 = 1.2 KB; MAVLink log replay is O(log size) ~MB scale.

### 7.3 Safety audit-log completeness
**Test**: inject 100 random faults, record all safety decisions.
**Metric**: fraction of decisions explainable from logs alone.
**Expected result**: OASIS 100% (every R14 block logged with signal+threshold+level); MAVLink less (STATUS_TEXT not structured).

**None of these benchmarks have been run.** All are hypothesis-level predictions based on architecture.

---

## 8. Honest final comparison

| Dimension | OASIS | MAVLink | STANAG 4586 |
|-----------|-------|---------|--------------|
| Continuous telemetry | ⚠️ not optimized | ✅ design purpose | ✅ |
| DDIL operation | ✅ architecturally (untested transport) | ⚠️ log replay only | ⚠️ similar |
| State per message | ⚠️ larger | ✅ small | ⚠️ medium |
| Safety explainability | ✅ structured per-tick | ⚠️ STATUS_TEXT only | ⚠️ status msgs |
| Cross-platform | ⚠️ partial (phone+sim) | ✅ broad | ⚠️ ground+UAV |
| Community/tooling | ❌ one project | ✅ huge | ⚠️ NATO only |
| Certification | ❌ pre-1.0 | ⚠️ supplier-dependent | ✅ NATO-approved |
| Cognitive layer (learning, priors) | ⚠️ partial | ❌ absent | ❌ absent |
| Binary size | ✅ 278 KB | - | - |

**Conclusion**: OASIS complements MAVLink/STANAG rather than competing. Realistic architecture: MAVLink for continuous telemetry + ground-station control, OASIS for on-board cognition + safety + offline state persistence. Not one-or-the-other.

---

## 9. Measured vs aspirational — honest separator

**Actually measured in v5 logs:**
- 278 KB binary (`ls -la drone_bridge.exe`)
- 28 KB pain buffer file (disk)
- 60 B single digest (disk)
- 10-drone parallel operation, 30-min continuous
- 32 900 R14 BLOCK events with structured format
- 6952 digest emissions, 0 received (peer-name bug until v6)

**Aspirational / paper-based:**
- "runs on microcontrollers" — no MCU test done
- "DDIL advantage" — no packet-loss test done
- "reconnection cost lower than MAVLink" — no comparative test done
- "safety-audit complete" — no formal certification done

Anyone citing the aspirational row without the "not yet measured" caveat would be OASIS-washing.
