# OASIS — IoT Readiness Assessment

**Date:** 2026-06-02 · **Type:** read-only deep audit (7-dimension, multi-agent) · **Scope:** the post-pure-Rust-migration repo (11 commits).

> One-line verdict: **the Rust code is good; the IoT story is lab-grade.** OASIS
> is a research-quality kernel with real cryptography, strong formal verification
> (113 Kani proofs, 443 tests), and clean architecture — but **every constraint
> that *defines* IoT is simulated, stubbed, or missing**: no real LoRa radio
> driver, no power management, zero real-silicon validation, and the one
> real-hardware claim (a 3h23 Android run) has no artifact committed to the repo.
> **Not IoT-deployable today.** The good news: most gaps are cheap to close
> (~$5–80 of hardware + hours/days), because the architecture is sound.

A note on method: the audit agents initially over-labelled compile-clean and
Renode/Wokwi-simulation results as "hardware-proven". Those were corrected — **no
capability below has run on real silicon.** "Compiles for an MCU target" and
"runs in a cycle-accurate simulator" are *not* "works on a device".

---

## Status matrix (corrected)

| Dimension | Readiness | Why |
|---|---|---|
| MCU / embedded | prototype (SIM only) | builds + Wokwi/Renode run; **0 boots on real silicon** |
| **LoRa / wireless (the IoT WAN link)** | **STUB** | `Sx1262Driver::init()` returns `"not implemented"`; 12-step init is commented out |
| Mesh on constrained nodes | prototype (SIM only) | dedup/TTL logic real; sim is *topology*, not *radio* (ignores airtime) |
| Security / HW root-of-trust | STUB / unvalidated | primitives solid; `ATECC608B` driver is a contract, no I2C; keys live in RAM |
| **Resource & power** | **MISSING** | 0 hits for sleep/WFI/idle/duty/power across 159 `.rs` files |
| Edge safety / real-time | prototype (SIM only) | R14/vitality proven in sim; watchdog is a stub; RT is soft, jitter ±4–54% |
| Production / fleet ops | MISSING | no OTA, no scale provisioning, no cloud/gateway, no remote telemetry |

---

## The honest ledger — real vs sim vs stub vs missing

**Genuinely real & strong (keep, don't re-litigate):**
- Cryptographic primitives: ChaCha20-Poly1305 (RFC 8439 byte-exact vector), X25519 ECDH, Ed25519 — no home-grown crypto.
- Formal verification: 113 Kani proofs + 443 unit tests green; invariants (quorum monotonicity, revocation idempotence, mesh-signature scope, EMI false-accept bound) proven.
- `no_std` cross-compile clean for `thumbv6m-none-eabi` (RP2040) and `thumbv7em-none-eabihf` (STM32F4); mesh Bloom shrinks 64 KiB → 2 KiB correctly.
- Mesh dedup logic (84.8% dup-drop in sim), TTL flood control, Bloom auto-reset — implemented and tested.
- Clean `LoRaRadio`/`Transport` trait boundary — a real driver drops in without rework.
- **Honest stubs**: SX1262 and ATECC608B explicitly return "not implemented" rather than faking success.

**Simulation only (do NOT cite as hardware validation):**
- All MCU execution = Wokwi (RP2040) + Renode (STM32F4), cycle-accurate but not silicon.
- All benchmarks = x86/WSL. R14 ≈ 331 ns on x86 → ~8 µs on M0+; Ed25519 verify ≈ 127 µs on x86 → **~206 ms on M0+ (Renode), ≈1500× slower**.
- `sim_200_drones` = network-topology sim (no airtime, no collisions). At real LoRa airtime (~41 ms/pkt SF7), 200 nodes × 2 msg/s ≈ **16.4 s of airtime per second — physically impossible**. The "70% reach" is optimistic for real radio.

**Stub / missing (the blockers):** real SX1262 driver, duty-cycle enforcement, power management, brownout-safe persistence, HW secure element, OTA, scale provisioning, cloud/gateway, remote telemetry, real-silicon + real-flight validation.

---

## Blockers, ranked (pre-mortem: most-likely-to-kill first)

| # | Blocker | Impact | Effort (est.) |
|---|---|---|---|
| 1 | **LoRa radio is a stub** | the headline IoT WAN link doesn't exist; "IoT mesh" is sim-only | ~$40–80 + 2–3 h (`sx126x-rs`) |
| 2 | **No power management** | 100% CPU, LoRa 100% RX, v0A signing 250 mW on a 200 mW budget → battery nodes impossible | design + weeks |
| 3 | **Duty-cycle not enforced** | transmitting outdoors in EU868/US915 is **illegal** without airtime throttling | days |
| 4 | **No brownout-safe state** | `tx_counter` resets to 0 on power loss → **replay attack** on restart (`main.rs` never calls `set_tx_counter()`) | hours (cheap, security-critical) |
| 5 | **Zero real-silicon validation** | all Wokwi/Renode/x86; 3h23 phone claim has **no committed artifact** | ~$5–25 boards + days |
| 6 | **No OTA / provisioning / cloud / telemetry** | cannot operate or update a fleet | weeks–months |
| 7 | **Ed25519 per-hop infeasible on M0+** (206 ms) | signed mesh viable only for low-freq authority broadcasts; code doesn't enforce the split | days |
| 8 | **Secure element stubbed** | no HW root-of-trust; keys in RAM | ~$5 chip + days |
| 9 | **Watchdog is a stub** | daemon zombies on `VitalityLevel::Dead`; no auto-restart | hours |

---

## Roadmap

### Phase 0 — cheap in-repo fixes (days, no new hardware)
- **Restore `tx_counter` on boot** (Blocker #4) — persist before use (fsync), call restore in `main.rs` startup; add a power-loss replay test. *(Security-critical, easy.)*
- **Per-origin rate limiting in the mesh layer** — current limiter is per-IP, inert for single-medium LoRa.
- **Watchdog**: break the loop + `exit(1)` on `kill.is_killed()`; add systemd `WatchdogSec`/MCU WDT hook.
- **Bake Bloom auto-reset default** (1200 inserts MCU / 40k host) into daemon init.
- **Document the v8/v9/v0A signing-cost split** (authority-only Ed25519) so nobody enables per-hop v0A on an M0+.
- **Resolve the 3h23 claim**: commit the `session_v0_5` artifact, or reframe it as "claimed, artifact not archived".

### Phase 1 — real radio + power (weeks, ~$80)
- Real `Sx1262Driver<SPI,…>` via `sx126x-rs`/`lora-phy`; loopback + range/PER curves with an RTL-SDR.
- `AirtimeCalculator` + `DutyCycleThrottler` (EU868 1% / US915) wired into `send_envelope()`.
- Power: CPU `WFI`/sleep between ticks, interrupt-driven RX, duty budgeting; measure idle/active draw.

### Phase 2 — silicon validation (weeks, ~$30)
- Flash RP2040 + STM32F4; verify boot/UART vs simulator logs byte-for-byte; measure real flash/RAM/power and stack headroom.
- Real ATECC608B I2C driver (Microchip CryptoAuthLib reference) for HW-backed keys.
- Real Pixhawk + LoRa flight test (auto-arm → takeoff → 4-waypoint → R14 failsafe).

### Phase 3 — fleet ops + certification (months)
- OTA (signed manifest + atomic A/B + rollback), zero-touch provisioning, cloud/gateway + store-and-forward backhaul, remote telemetry/observability dashboard.
- External crypto audit (Trail of Bits/Cure53), radio certification (FCC/CE), FAA/EASA path.

---

*Generated from a 7-dimension read-only audit. Numbers marked "x86" or "sim"
are not hardware measurements. This file is an assessment + roadmap, not a
claim of capability.*
