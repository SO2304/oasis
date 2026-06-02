# OASIS Architecture — one-page integration view

**Audience**: architect or integration engineer at a grid operator or
IoT-edge deployment. Assumes familiarity with MQTT, Modbus, SCADA,
and generic embedded-Linux / MCU topology.

---

## Where OASIS sits in your stack

```
  ┌──────────────────────────────────────────────────────────────┐
  │  SCADA / DMS / HMI                                           │
  │  (existing — GE, Siemens Spectrum, ABB Ability)              │
  └──────────────┬───────────────────────────────────────────────┘
                 │ IEC 61850, DNP3, OPC UA (you keep)
  ┌──────────────┴───────────────────────────────────────────────┐
  │  Concentrator / gateway                                      │
  │  (existing — Moxa, Belden, RAD)                              │
  └──────────────┬───────────────────────────────────────────────┘
                 │ Modbus TCP / MQTT / REST   (you keep)
  ┌──────────────┴───────────────────────────────────────────────┐
  │            OASIS LAYER (new) — 5 coordinated gates           │
  │                                                              │
  │  Every command/telemetry traverses ALL 5 layers, top-down.   │
  │  Any one of them can refuse. Layers are independent in       │
  │  failure mode but coordinated in execution.                  │
  │                                                              │
  │  L1  M11 mesh signing      Ed25519 per-envelope, Bloom dedup │
  │      catches: insider spoof, replay, tamper                  │
  │  L2  R14 entropy gate      Refuses action when world unclear │
  │      catches: high-uncertainty SCADA commands                │
  │  L3  M9 reflex arc         Sub-µs sigma-outlier detection    │
  │      fires:   emergency stop, immediate protective response  │
  │  L4  M10 pressure field    Non-Euclidean nav AWAY from danger│
  │      forces:  trajectory escapes hazard, finds goal          │
  │  L5  Vitality + KillSwitch Graceful degradation, last halt   │
  │      catches: sensor loss, geofence breach, hardware fault   │
  │                                                              │
  │  Other bio-mechanisms (M1 tension, M5 emotion, M7 synaptic,  │
  │  M8 dreams) sit alongside as optional autonomy primitives.   │
  │  The 5 layers above are the gate-chain; others are tools.    │
  └──────────────┬───────────────────────────────────────────────┘
                 │ UART / SPI / UDP / LoRa / Ethernet
  ┌──────────────┴───────────────────────────────────────────────┐
  │  Edge node (MCU or Linux)                                    │
  │  - Cortex-M0+ / M4F / M7 (RP2040, STM32F4, STM32H7)          │
  │  - Linux SBC (Raspberry Pi, BeagleBone)                      │
  │  - Sensor + actuator + radio                                 │
  └──────────────────────────────────────────────────────────────┘
```

## What OASIS replaces / does not replace

**Replaces**:
- Your custom per-hop mesh crypto (if any) — OASIS gives you Ed25519
  per-envelope signing with insider-resistance out of the box.
- Per-vendor proprietary sensor-mesh protocols — OASIS is open-wire,
  vendor-neutral.
- Rolling your own replay-protection, dedup, TTL — OASIS provides
  these baked into the mesh router.

**Does NOT replace**:
- Your SCADA / DMS / HMI.
- Your IEC 61850 / DNP3 / OPC UA application protocols (OASIS sits
  below them and carries them as opaque payload).
- Your physical radios, modems, or fiber links.
- Your network management (NMS, SNMP). OASIS is at the transport layer.

## Integration effort — typical

| Target | Effort | Why |
|---|---|---|
| Existing Linux gateway/concentrator | 3-5 days | Link `oasis-rt` as Rust lib, wrap your existing transport |
| STM32F4/F7 MCU-based sensor | 5-10 days | Port `oasis-rt` (no_std ready), integrate radio driver |
| Cortex-M0+ MCU (RP2040) | 5-10 days | Heap budget 64-192 KiB, Bloom shrunk via feature flag |

## Deployable artifacts

- `oasis-rt` — the core kernel (Rust crate, ~278 KB compiled on MCU)
- `oasis-lora-transport` — LoRa radio transport layer (Rust crate)
- Reference integrations (forthcoming): PX4 / ArduPilot (UAS), Mosquitto
  bridge (MQTT gateway), OpenPLC adapter (PLC edge).

## What you will need to supply

1. **Identity provisioning** — each node needs an Ed25519 seed. In
   development we use fixed seeds; in production the seed should live
   in an SE (ATECC608B / OPTIGA Trust M / NXP SE050). That integration
   is ~1 week per chip.
2. **Radio or transport** — UART, SPI (→ SX1262 LoRa), UDP, Ethernet.
   OASIS abstracts this via a `Transport` trait.
3. **Payload schema** — OASIS carries opaque bytes. You still own the
   sensor data schema (CBOR, Protobuf, ASN.1, raw IEC 61850 GOOSE).
4. **Key-revocation policy** — how you decide a node is compromised and
   broadcast the revocation is operational policy. OASIS provides
   the signed-revocation envelope primitive.

## Performance (measured)

| Operation | x86_64 K=10 median | Cortex-M0+ 125 MHz |
|---|---:|---:|
| Ed25519 sign (full origin_wrap, 125 B env) | 274 µs | 387 ms |
| Ed25519 verify (router process) | 141 µs | 206 ms |
| Replay rejection (Bloom dedup) | 122 µs | sub-µs (no crypto) |
| Tamper rejection (bit-flip in sig) | 15 µs | ~206 ms (constant-time verify still runs) |
| R14 entropy gate | ~250 ns host | ≤ 1 µs (below timer resolution) |

MCU numbers are K=10 on RP2040 under cycle-accurate Wokwi simulation.
Half-spread 0.0002 % — see [capabilities.md](capabilities.md).

## Security posture — honest summary

| Property | Coverage |
|---|---|
| Packet confidentiality | ChaCha20-Poly1305 AEAD (spore v3) |
| Forward secrecy | X25519 ECDH ephemeral key (spore v4) |
| Sender authentication | Noise-KK dual-DH (spore v5) |
| Revocation | Ed25519-signed envelopes (spore v6) |
| Replay resistance | Monotonic counter + 128-bit window (spore v7) |
| Mesh multi-hop | Flooding + TTL + Bloom dedup (spore v8) |
| External-attacker MAC | HMAC-SHA256-8 on mesh header (spore v9) |
| **Insider-resistant per-node sig** | **Ed25519 over (msg_id || origin_fp) (spore v0A / mesh_v10)** |

For threats outside this coverage, see [capabilities.md](capabilities.md)
→ "Known gaps".

## Next step — see other one-pagers

- [capabilities.md](capabilities.md) — what's proven vs what's aspirational
- [grid_use_cases.md](grid_use_cases.md) — 3 scenarios for Elia/HEDGE-IoT
- [SHADOW_AUDIT.md](SHADOW_AUDIT.md) — skeptic-first gap review

---

**Contact**: Souhayb Rharrab — souhaybrharrab@gmail.com — +32 486 32 64 46
