# OASIS Capabilities — what works, what doesn't, as of commit HEAD

**Audience**: engineer doing due diligence. Wants to know what's real
vs aspirational. No marketing tolerance.

Every ✅ below links to captured evidence (test output, commit hash, or
shadow audit document). Every ⚠️ / ❌ names what's missing.

---

## What works today (measured on real silicon models)

### Core mesh kernel

| Capability | Status | Evidence |
|---|---|---|
| Mesh routing (TTL, multi-hop, Bloom dedup) | ✅ | 428 host tests, 77 Kani proofs, 3-STM32F4 chain verified in Renode |
| Ed25519 per-envelope mesh signing (v0A / mesh_v10) | ✅ | 10 unit tests, byte-exact sig_head across 3 MCUs |
| HMAC-SHA256-8 external-attacker MAC (v9) | ✅ | measured 431 ns sign / 551 ns verify host x86_64 |
| ChaCha20-Poly1305 AEAD (v3) | ✅ | RFC 8439 vector byte-match test |
| X25519 ECDH forward secrecy (v4) | ✅ | roundtrip tests |
| Noise-KK sender auth (v5) | ✅ | tests |
| Signed revocation envelopes (v6) | ✅ | tests |
| Monotonic counter + replay window (v7) | ✅ | 128-bit sliding window, 127-reorder-tolerance tests |

### Runtime footprint

| Target | Status | Evidence |
|---|---|---|
| x86_64 Linux / Windows / macOS host | ✅ | 428/428 tests pass |
| Cortex-M0+ (RP2040, thumbv6m-none-eabi) | ✅ | Wokwi cycle-accurate sim, 11 TRL-6 rounds |
| Cortex-M4F (STM32F4, thumbv7em-none-eabihf) | ✅ | Renode multi-MCU sim, 3-hop chain verified |
| Cortex-M7 (STM32H7) | ✅ compile | not yet run-tested, but same no_std feature set |

### Five-layer coordinated defense (lab-pack v2 demo)

| Layer | Mechanism | Demonstrated by |
|---|---|---|
| L1 | M11 mesh sign + dedup | scenarios 1-3 of demo (insider, replay, tamper) |
| L2 | R14 entropy gate | scenario 4 (refuses on uncertainty > 0.5) |
| L3 | M9 reflex arc | scenario 5 (sigma-outlier on transformer vibration) |
| L4 | M10 pressure-field navigation | scenario 6 (escapes hazard, approaches goal) |
| L5 | Vitality + KillSwitch | scenarios 7-8 (sensor loss + geofence breach) |

All 5 layers run together in the same process / same Rust kernel /
same MCU. They are independent in failure mode (one layer's bug
doesn't disable the others) but coordinated at runtime — every
command/telemetry passes through the full chain top-down.

### Cryptographic integrity (against 6 attack scenarios)

| Attack | Result |
|---|---|
| Insider node captured, signs valid but spoofs origin_fp | ✅ rejected |
| Replay of a valid envelope | ✅ rejected (Bloom dedup) |
| Bit-flip tamper in Ed25519 signature | ✅ rejected |
| Wrong-pubkey-in-registry | ✅ rejected |
| Unknown-sender | ✅ rejected |
| Downgrade v0A → v9 on v0A-only router | ✅ rejected |

### Measured performance

| Operation | x86_64 K=10 median | Cortex-M0+ 125 MHz |
|---|---:|---:|
| Ed25519 sign (full origin_wrap) | 274 µs | 387 ms |
| Ed25519 verify | 141 µs | 206 063 µs (K=10, half-spread 0.0002 %) |
| HMAC sign | 431 ns | ≈ 400 µs estimated |
| R14 entropy gate (action safety) | ≤ 350 ns | < 1 µs |
| Constant-time verify (accept vs reject) | — | 206 062 vs 206 056 µs (0.003 % variance) |

### Anti-jam (simulated, byte-layer)

| Scenario | Delivery |
|---|---:|
| No jam baseline | 100 % |
| Narrowband jammer, 1 channel (no FHSS) | 0 % |
| Narrowband jammer, 8-channel FHSS | 90 % measured (87.5 % theoretical) |
| Narrowband jammer, 32-channel FHSS | 96.3 % |
| Sweep jammer, 8-channel FHSS | 86-90 % (not meaningfully worse than static) |
| Follower jammer, latency < 0.8 × packet | **0 % — FHSS defeated** |
| Follower jammer, latency ≥ 0.8 × packet | 100 % |
| Short-packet defense (10 ms) vs 20 ms follower | 100 % |

Wideband barrage jammer: **not mitigated** — requires DSSS or physical
displacement, outside OASIS software layer.

---

## What's scoped but not built

| Capability | Status | What it takes |
|---|---|---|
| Real SX1262 LoRa driver | ⚠️ stub | `sx126x-rs` crate integration, 2-3 days |
| Real radio outdoor range test | ⚠️ pending hardware | ~$40 of 2 Pi Pico + SX1262 HATs, 1 afternoon |
| Secure-element integration (ATECC608B, SE050) | ⚠️ not started | ~1 week per chip family |
| Secure boot chain | ⚠️ not started | chip-specific, 1-3 weeks |
| Tamper-switch → SE key wipe | ⚠️ not started | 2 weeks for full flow |
| STANAG 4586 adapter | ⚠️ not started | 2-4 months or partner |
| LoRaWAN compatibility | ⚠️ not applicable | OASIS is P2P mesh, not server-mediated |

---

## What's NOT on the roadmap (honest)

| Non-goal | Why |
|---|---|
| Replace your SCADA | Different layer, different market |
| Replace your IEC 61850 / DNP3 stack | OASIS carries their bytes as payload |
| Full vertical UAS stack (Anduril-style) | Not our ambition; we're a component |
| Post-quantum signatures | Ed25519 is classical; PQ migration is an industry-wide problem, not ours alone |
| Satellite / SATCOM native support | Can be used as transport but no specialized waveform work |
| Windows kernel driver | Linux-side userspace only for gateways |
| PLC / IEC 61131-3 runtime | Out of scope; adapters possible |

---

## Known gaps (read this before deploying anything)

### No external cryptographic audit yet

Self-audited. Protocol + implementation reviewed by author. **Zero
independent review.** For production-critical deployment, recommend
budget of €60-180k for a Trail of Bits / NCC Group / Cure53 / Doyensec
scope engagement. OASIS team will support the engagement.

### No real-hardware field test yet

All mesh / crypto / anti-jam claims are sim-validated at **byte-layer**.
Cycle-accurate RP2040 sim + Renode STM32F4 sim are very high fidelity
but they're still simulators. First real-hardware RF demo is a ~$40 +
1-afternoon deliverable away; not yet executed.

### No certification

No CE marking claim. No UL. No IEC 62443 (industrial cybersecurity)
compliance claim. No IEC 61850 certification. No ISO 27001 SMS. These
are deployment-context decisions your org makes; OASIS as a component
supports compliance efforts but does not itself carry certifications.

### Cryptographic-algorithm status

- Ed25519: classical secure, constant-time on M0+ verified (≤0.003 %
  variance accept-vs-reject).
- ChaCha20-Poly1305: RFC 8439 compliant.
- SHA-256 / SHA-512: RustCrypto implementations, unaudited in-context
  but widely used.
- **No post-quantum migration path yet.** Not urgent but worth noting.

### Operational gaps

- Key rotation at scale not packaged (the primitives exist; the
  operational tooling does not).
- Time synchronization across nodes (required for some anti-jam
  defenses) not built.
- Monitoring / telemetry of the mesh itself is minimal (you'd wire it
  through your existing NMS).

---

## Recommended evaluation flow for a lab

### Day 1 (today)

- Run `run_demo.bat` — confirm the 4 scenarios PASS.
- Read this document and [SHADOW_AUDIT.md](SHADOW_AUDIT.md).
- Decide whether to go further.

### Day 2-5

- Request source access under NDA.
- Build from source on your development host. `cargo test --release`
  should produce 428+ passing tests.
- Run Renode multi-MCU demo if you have the tool; else we can
  screen-share the run.

### Day 6-10

- Port one of your existing sensor payload formats into a v0A envelope.
- Wire one of your actual radios (LoRa, Wi-Sun, whatever) into the
  `Transport` trait.
- End-to-end run: your sensor → OASIS sign → your radio → receiver →
  OASIS verify → your SCADA.

### Day 11-30

- Integrate on one substation / sensor / edge node in a staging lab.
- Run a 72-hour soak test.
- Inject 3 of your own adversary scenarios, measure detection rate.
- Decide whether to fund a pilot deployment.

---

## The single most important sentence on this page

OASIS today is a **technically credible prototype at TRL 6 against
simulators** with a clearly-scoped path to TRL 7 (real hardware + field
test). It is **not production-certified**. Deploying to a critical
grid asset without a full external audit and field validation would
be premature. Deploying to a pilot / R&D / testbed environment is
exactly what the current maturity level fits.

---

**Contact**: Souhayb Rharrab — souhaybrharrab@gmail.com — +32 486 32 64 46
