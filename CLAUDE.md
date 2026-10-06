# OASIS — Calibrated Product Definition

**Calibrated 2026-04-22 (session 2)**: updated after a long session that
closed three gaps — MCU port, A/B vs ROS 2, and insider-resistant mesh
signing. Every perf number below now has K=10 statistical bands.
Aspirational framing has been removed. When a claim here conflicts with
an older doc, this file wins.

**Recalibrated 2026-06-02 (pure-Rust migration)**: the repo is now
**exclusively Rust**. Removed the TypeScript `kernel/` (~27k LOC), the
root TS demos, the npm toolchain (`package.json`/`tsconfig`/`vitest`),
`node_modules/`, `dist/`, all 28 Python harnesses, ~300 MB of root
`*.zip` snapshots, and 3 undocumented `oasis_grid_demo*` bins. Git
history was initialized — it previously had **zero commits**, so the
older "preserved in git for archaeology" claims were *false*; all
removed content now lives in baseline commit `d8681c5`. The 4 std crates
are a Cargo **workspace** (`resolver = "2"`); the 3 MCU crates are
*excluded* so `mesh_bloom_mcu` feature-unification can no longer shrink
the host Bloom (64 KiB → 2 KiB). Re-measured headline numbers (commands
reproduce them): **455 lib tests** (`cargo test --workspace --release`; 444
before mesh v0B, +11 `v0b_*`),
**120 Kani proof harnesses** (`grep -rE 'kani::proof' oasis-rt/src | wc -l`;
113 before v0B, +7; full CBMC pass *not* re-run — slow under WSL), **30 modules**,
**19 `[[bin]]`**, **32 `src/*.rs`** files (largest module now `spore_crypto.rs`
2583 L; `mesh.rs` was split 2026-06-02 — its tests + Kani proofs moved to
`src/mesh/{tests,kani_proofs}.rs`; the protocol core is **1509 L** after mesh v0B
was added 2026-10-06). ⚠️ The per-file line counts in the tree
below are stale snapshots (drift +20 % to +115 %); the authoritative
source is `wc -l oasis-rt/src/*.rs`.

---

## What OASIS IS (two faces, both validated)

### Face 1 — Drone autonomy + comms kernel (Rust)
- Runs on top of PX4 via MAVLink v2
- 278 KB Rust kernel + spore P2P crypto stack (v3→v7)
- Validated end-to-end on PX4 SITL + jmavsim (4-waypoint mission, altitude 1.94m vs 2.0m target)
- 3 Kani R14 safety proofs verified in WSL
- **Production target**: robotics / drone swarms

### Face 2 — Android "nervous system" daemon (Rust)
- Single binary (`main.rs`, ~600 LOC) running in Termux on Android
- 11 bio-inspired mechanisms operating on real phone sensors (accelerometer, gyroscope, barometer, light, microphone)
- Longest run *claimed*: 3h23 on Samsung S23 FE (121 290 ticks, zero crash) — ⚠️ session logs are NOT archived in this repo, so it is not reproducible from the tree (see IOT_READINESS.md)
- **Research target**: embedded adaptive AI experiments

Both faces share the `oasis-rt` crate. No TS runtime is deployed.

---

## What OASIS IS NOT

- Not a PX4 replacement — it's a layer ABOVE PX4
- Not flight-certified / audited externally
- Not a chatbot, SaaS, or web framework
- Not production-ready — pre-1.0
- No longer ships any TypeScript — the `kernel/` reference spec (~27k LOC) was removed 2026-06-02 and exists only in git history (baseline `d8681c5`). The runtime is exclusively Rust.
- Not using Webots any more (PX4 SITL replaced it 20+ rounds ago)

---

## Repo layout (post-calibration)

```
oasis/
├── oasis-rt/                 Production Rust kernel
│   ├── src/
│   │   ├── lib.rs            crate root, 30 modules (incl. mesh, topics, nav, services, actions, transforms, timers, parameters, fmath, + ROS-2-equivalent rclcpp primitives)
│   │   ├── main.rs    604L   Android Termux daemon (Face 2)
│   │   ├── vec.rs     190L   128D vector algebra, zero-alloc
│   │   ├── hyper_state.rs 436L  M2 entropy + R14 + 3 Kani proofs
│   │   ├── tension.rs 202L   M1 vector tension field
│   │   ├── synapse.rs 366L   M7 Hebbian / STDP
│   │   ├── emotion.rs 512L   M5 5 emotions + pain memory
│   │   ├── reflex.rs   93L   M9 reflex arc
│   │   ├── federation.rs 883L M11 Ed25519-signed mesh
│   │   ├── morpho.rs  248L   M6 specialisation (EXPERIMENTAL)
│   │   ├── efference.rs 235L M3 efference copy (EXPERIMENTAL)
│   │   ├── dreams.rs  208L   M8 dream consolidation (EXPERIMENTAL)
│   │   ├── branching.rs 158L M4 temporal branching (EXPERIMENTAL)
│   │   ├── world_model.rs 239L M10 non-Euclidean pressure (EXPERIMENTAL)
│   │   ├── audio.rs   221L   RMS, FFT, pitch, no ML
│   │   ├── spore.rs  1232L   P2P transport + rate limiter + listener
│   │   ├── mesh.rs    1099L   multi-hop mesh v8/v9/v0A + Bloom dedup (tests + 46 Kani proofs → src/mesh/)
│   │   ├── spore_crypto.rs 2583L  ChaCha20-Poly1305 AEAD + X25519 ECDH + revocation + counter tracker
│   │   ├── mavlink_min.rs 1420L  MAVLink v2 parser/signer/replay
│   │   ├── hal.rs     288L   KillSwitch + physics constraints
│   │   ├── nerve.rs   450L   24 afferent + 8 efferent dims
│   │   ├── spinal.rs  495L   5-platform auto-discovery (LOGGED ONLY)
│   │   ├── vitality.rs 202L  Graceful degradation states
│   │   └── transport.rs 298L Transport trait + LoRa frame stub
│   └── src/bin/              18 production binaries (see below)
├── kernel/                   TS reference spec (ARCHIVED — 27k LOC, unmaintained)
├── webots/                   Current shadow audit docs (8 files)
│   └── _archive/             40+ older audit snapshots
├── oasis-rt/_archive_bins/   17 legacy test/validation binaries
└── _archive/                 Old planning docs
```

---

## Production binaries (18, post-hygiene-round)

| Binary | Role |
|---|---|
| **main** (via `cargo run --release`) | Android Termux daemon — Face 2 |
| **drone_bridge** | OASIS kernel ↔ MAVLink JSON bridge |
| **mavlink_adapter** | PX4 adapter, OFFBOARD, PARAM_SET, waypoint loop |
| **spore_send** | P2P digest transmitter (v1 / v2 fragmented / QR) |
| **spore_recv_v2** | Listener with FEC reassembly + v7 decrypt |
| **spore_revoke** | Operator revocation-list publisher |
| **oasis_keygen** | X25519 identity generator (v5+v7 pairing) |
| **oasis_fingerprint** | Pairing verification (exit-coded) |
| **oasis_autodetect** | Zero-driver hardware detection (sysfs + sensor enum) |
| **udp_loss_proxy** | MITM loss simulator (uniform + Gilbert-Elliott burst) |
| **sim_200_drones** | 200-drone mesh real-time sim (84.8% dedup drops) |
| **sim_scenarios** | 3-scenario validation (multi-vendor, connectivity, R14 attack) |
| **bench_r14_latency** | R14 decision latency bench (p50 = 331 ns Linux) |
| **bench_spore_loss** | In-memory loss tolerance bench (N=200 internal) |
| **bench_mechanisms_soak** | Long-running mechanism soak |
| **bench_mesh** | Mesh routing throughput (K=10 banded) |
| **bench_mesh_signed** | v8/v9/v0A mesh bench (K=10 banded) |
| **bench_full_stack** | 5-pipeline integration bench (K=10 banded) |
| **bench_payload_sweep** | Payload-size sweep with OLD-vs-NEW buffer-backed comparison |

All 17 removed binaries preserved at `oasis-rt/_archive_bins/` for git archaeology.

---

## Bio-inspired mechanisms — status honnête

Out of 11 mechanisms, 6 are validated on real hardware (PROVEN), 5 are cabled into the daemon but never validated in long-session real-world conditions (EXPERIMENTAL). No mechanism has been removed.

| # | Mechanism | Status | Evidence |
|---|---|---|---|
| M1 | Tensorial agent communication | **PROVEN** | 9 Rust tests + test_real T1 on phone |
| M2 | HyperState + R14 entropy gate | **PROVEN** | 9 tests + 3 Kani proofs + 243 ns latency |
| M3 | Efference copy | EXPERIMENTAL | 6 tests, bench OK, motors not tested |
| M4 | Temporal branching | EXPERIMENTAL | 4 tests, cabled, no long run |
| M5 | Emotional gain modulation | **PROVEN** | 7 tests + 1h30 real run (1100 ticks) |
| M6 | Morphogenesis | EXPERIMENTAL | 6 tests, cabled |
| M7 | Hebbian / STDP | **PROVEN** | 9 tests + 7 phone tests + cycle proof |
| M8 | Dream consolidation | EXPERIMENTAL | 5 tests, blocked in live sessions |
| M9 | Reflex arc | **PROVEN** | 3 tests + test_motor v6 T4 |
| M10 | Non-Euclidean world model | EXPERIMENTAL | 7 tests, cabled |
| M11 | Federated resonance | **PROVEN** | 6 tests + 5 phone tests + PC↔Phone |

---

## Spore comms stack — full wire-format versioning

| Version | Purpose | Threats covered |
|---|---|---|
| `SPORE\x01` | v1 UDP multicast (single packet) | baseline |
| `SPORE\x02` | v2 fragmented + XOR-parity FEC | lossy radio (bench: 78% @ 30% loss, +FEC+repeat=2) |
| `SPORE\x03` | v3 ChaCha20-Poly1305 AEAD (PSK) | confidentiality + integrity + downgrade reject |
| `SPORE\x04` | v4 ECDH forward secrecy (ephemeral DH + PSK) | past-traffic decryption |
| `SPORE\x05` | v5 Noise-KK (dual DH) | **sender authentication** |
| `SPORE\x06` | v6 signed revocation envelope | compromised drone eviction |
| `SPORE\x07` | v7 v5 + monotonic counter + 128-bit sliding window | unbounded replay resistance + UDP reorder tolerance (127 positions) |
| `SPORE\x08` | v8 multi-hop mesh envelope (unsigned) | flooding + TTL + Bloom dedup |
| `SPORE\x09` | v9 mesh + HMAC-SHA256-8 over (magic\|\|msg_id\|\|origin_fp) | **external-attacker resistance on mesh header** (shared MAC key) |
| `SPORE\x0A` | v0A mesh + Ed25519 per-node signature | **insider resistance** — compromised node with PSK still can't forge other senders |
| `SPORE\x0B` | v0B mesh + Ed25519 over `"OASIS-MESH-v0B"‖network_id‖origin_fp‖counter‖payload_len‖SHA-256(payload)` + persisted 128-bit counter window + relay-enforced revocation | **payload forgery, message suppression, post-reboot replay, cross-network replay, revoked origins** — all rejected at the first hop. v0A signed only 22 header bytes, so payload swaps kept a valid signature (12/50 accepted on silicon) and poisoned dedup to suppress the real message. ⚠️ `ttl`/`hops` stay unsigned (they mutate per hop) — range-checked only; sender-side counter lease **not yet implemented** (liveness gap, see `docs/MESH_V0B_SPEC.md` §5) |

Threat model: **19 of 23 threats covered in-protocol** (remaining: disk
wipe, operator key compromise, rogue pairing, post-quantum, traffic
analysis; v0A closed 2 from the prior "17 of 23" count — insider
forge + mesh-header spoof).

Performance trade across the 3 mesh variants (Linux WSL, K=10 medians):

| | origin_wrap | process (verify) | use when |
|---|---:|---:|---|
| v8 unsigned | 123 ns ±3.5% | 201 ns ±99% ⚠️ | trusted local bus |
| v9 HMAC-SHA256-8 | 431 ns ±7% | 551 ns ±26% | external-attacker model |
| v0A Ed25519 (cached kp) | 250 814 ns ±9% | 134 935 ns ±19% | insider-resistant auth — ~580× v9 cost, use for low-frequency authority broadcasts only |
| v0B Ed25519 + SHA-256(payload) | 251 596 ns ±25% | 125 585 ns ±1.3% | payload/counter/network-bound + persisted anti-replay + revocation. **≈ v0A + 1 %** (the SHA-256 is noise next to Ed25519, flat to 1 KB) — same "low-frequency authenticated broadcast" guidance, but this is the one to use when relays are untrusted. `bench_mesh_v0b`, K=10, 15 B |

---

## Validation matrix (what holds up under scrutiny)

| Capability | Evidence | Status |
|---|---|---|
| **455 unit tests pass in parallel** (444 + 11 mesh v0B) | `cargo test --workspace --release` | ✅ |
| **120 Kani proof harnesses** (53 in mesh) | `cargo kani --lib` (WSL) — full CBMC pass NOT re-run 2026-06-02 (slow under WSL); 4/4 sampled passed | ⚠️ count verified, end-to-end pass not reproduced |
| MAVLink v2 CRC + signing + replay | 29 in-suite tests + 300 real PX4 frames | ✅ |
| Ed25519 federation + signing | `ed25519_signing_roundtrip` + tamper rejection | ✅ |
| **Ed25519 per-node mesh signing (v0A)** | 10 tests inc. `v10_spoofed_origin_fp_rejected` | ✅ |
| **Mesh v0B — payload-bound, fresh, domain-separated (`SPORE\x0B`)** | 11 `v0b_*` tests (one per attack: exhaustive bit-flip of every signed byte, content-swap suppression, forged origin, immediate replay, post-reboot replay, post-Bloom-reset replay, foreign network, revoked origin, ttl inflation, invalid-then-valid, reorder-accept-once) + 7 Kani proofs (parser totality; preimage binds payload-digest/counter/network) + `bench_mesh_v0b`. Builds `no_std` `thumbv6m`. **Silicon (3× RP2040, stamp `d285ef4`)**: 0/150 pre-CRC bit-flips accepted (v0A: 12/50), suppression defeated, **T8 reboot-replay rejected across a real USB power-cut** from the flash-restored counter window, on-chip cost +1 %. See `docs/MESH_V0B_SPEC.md`, `docs/SECURITY_COMPARISON.md`, `evidence/silicon/2026-10-06/REPORT_MESH_V0B.md`. ⚠️ Sender-side counter lease NOT implemented → an origin that loses power is locked out of relays that persisted its counter (liveness, not security). | ✅ |
| Auto-arm PX4 via OASIS | PX4 log: `Armed by external command` | ✅ |
| Auto-takeoff via OASIS | PX4 log: `Takeoff detected` | ✅ |
| 4-waypoint mission complete | 3 WAYPOINT_REACHED events in adapter log | ✅ (1 successful run) |
| Altitude hold closed-loop | 1.94 m vs 2.0 m target (±6 cm) | ✅ |
| Spore v7 loss + FEC real UDP | bench + loss proxy: 98% @ 30% uniform, 82-90% @ 30% burst | ✅ |
| RFC 8439 ChaCha20-Poly1305 vector | test vector matches byte-for-byte | ✅ |
| All 11 mechanisms compile + test | 455 Rust tests across 30 modules | ✅ |
| Android daemon 3h+ run | session_v0_5 on S23 FE, 121 290 ticks | ⚠️ claimed; logs not in repo |
| **MCU cross-compile** (`thumbv7em-none-eabi`) | `cargo build --target thumbv7em-none-eabi --lib --no-default-features --features mesh_bloom_mcu --release` | ✅ 0 errors |
| **A/B vs ROS 2 Jazzy** (Linux intra-process, K=10 medians) | OASIS 241 ns vs rclcpp intra 5 624 ns vs rclcpp DDS 52 411 ns at 16 B — 23–217× faster | ✅ measured |
| 7/7 rclcpp core primitives matched | pub/sub + services + actions + tf2 + timer + parameters + params-events (8/8 incl. tf) | ✅ |
| Real hardware test (Pixhawk + quad) | **none** | ❌ |
| External crypto audit | **none** | ❌ |
| Real MCU hardware boot | full T0–T6 suite (ChaCha20-Poly1305 RFC 8439, X25519 RFC 7748, Ed25519 sign/verify/tamper, R14 gate, mesh v8/v9/v0A incl. forge-reject, on-silicon timing) **PASS 3× on THREE RP2040 boards** (9/9 runs green). ⚠️ An independent review found 3 **test-quality** defects in the 2026-10-04 run — T0 echoed a hard-coded clock (never measured), T5-v9's tamper drop was a dedup false-positive (MAC verifier never reached), T4 had no negative control. All three **corrected and re-verified on silicon 2026-10-06** (3 boards × 3 runs, 9/9 green): T0 now measures the core clock (±1 %: 124.9986 MHz vs 125 MHz, per-board variance), T5-v9 asserts `Drop("bad mesh mac")` on a fresh router, T4 proves `blocked=50/50 && nominal_false_blocks=0/50`. The OASIS crypto/R14/mesh logic was **not** changed — only the tests. Plus a **wired-UART A→B→C v0A mesh relay** with per-hop Ed25519 verification (`uart_mesh.rs`, §10). ⚠️ Not over-the-air — **no LoRa radio**; no energy/secure-element/flight; T8 flash-persistence not done. See `evidence/silicon/2026-10-06/REPORT.md` (corrected) + `evidence/silicon/2026-10-04/REPORT.md` §13 (erratum). | ✅ |

---

## Calibrated performance (post-hygiene round, 2026-04-22)

All numbers K=10 medians with (min-max) ranges. Linux WSL2 Ubuntu 24.04
on a laptop. Windows numbers in source comments. **±Nn%** = half-spread
as percentage of median.

### Mesh layer (bench_mesh_signed, bench_mesh)

```
origin_wrap (16 B payload):
  v8 unsigned:          123 ns ± 3.5%     (8.0 M ops/s)
  v9 HMAC-SHA256-8:     431 ns ± 6.9%     (2.3 M ops/s)
  v0A Ed25519 sign:     250 814 ns ± 9.4% (4.0 k ops/s)

process() forward:
  v8:                   201 ns ±99%  ⚠️  [WSL hypervisor noise]
  v9 HMAC verify:       551 ns ±26%       (1.8 M ops/s)
  v0A Ed25519 verify:   134 935 ns ±19%   (7.4 k ops/s)
```

### Full-stack pipelines (bench_full_stack)

```
  topic + mesh wrap + dispatch:         261 ns ± 5.0%  (3.84 M ops/s)
  service request + response:           165 ns ±14%    (6.06 M ops/s)
  action (goal+feedback+result):         93 ns ± 4.0%  (10.8 M ops/s)
  topic + AEAD + mesh (3-layer wrap):  3 137 ns ±19%   (319 k ops/s)
  RX: mesh + AEAD + topic dispatch:    2 150 ns ±10%   (465 k ops/s)
```

### Buffer-backed builder win (bench_payload_sweep)

```
                    OLD path           NEW path (origin_wrap_with)
    16 B:           263 ns ± 3%        162 ns ±12%         1.6× faster
    1 KB:           390 ns ±3%         247 ns ± 8%         1.6× faster
   64 KB:         5 749 ns ±3%       2 813 ns ± 9%         2.0× faster
    1 MB:     3 555 289 ns ±3%      48 819 ns ± 8%       72.8× faster  ⚡
```

### R14 entropy gate (bench_r14_latency)

```
Linux:  min 320 ns, p50 331 ns, p95 351 ns, p99 360 ns  — ultra-clean
Windows: p50 300 ns, p99 400 ns, max 22 600 ns (scheduler hiccup outlier)
R14 gate blocked 1000/1000 faults above signal threshold 0.95.
```

### Loss tolerance (bench_spore_loss, N=200 trials internal)

```
                      0%     10%    20%    30%    40%    50%
v2 no-FEC,  rep=1    100%   56%    35%    20%     9%     5%
v2 +FEC,    rep=1    100%   87%    62%    38%    24%    14%
v2 +FEC,    rep=2    100%   91%    81%    78%    67%    40%
v2 +FEC,    rep=3    100%   91%    90%    80%    80%    68%
```

### A/B vs ROS 2 Jazzy (Linux WSL2 intra-process, BOTH sides K=10 banded)

```
                            OASIS NEW             rclcpp intra-process    rclcpp DDS          rclpy
                            (bench_payload_sweep) (ros2_bench_sweep)      (ros2_bench2)       (single-shot)
16 B pub/sub loop:          162 ns ±12%           4 562 ns  ± 6.6%        45 243 ns  ±37%     908 765 ns
1 KB:                       247 ns ± 8%           4 951 ns  ±12%          (not measured)
64 KB:                      2 813 ns ± 9%         5 175 ns  ±34%          (not measured)
1 MB:                       48 819 ns ± 8%        183 340 ns ±54%         (not measured)
```

Ratios (K=10 medians both sides):

| Payload | OASIS vs rclcpp intra | OASIS vs rclcpp DDS |
|---:|---:|---:|
| 16 B | **28× faster** | **279× faster** |
| 1 KB | 20× faster | — |
| 64 KB | 1.8× faster | — |
| 1 MB | **3.75× faster** | — |

**Correction from prior single-shot numbers**: rclcpp medians came in
**28-55% lower than prior K=1 measurements** — OASIS's 1 MB
"7.85× faster" claim tightens to **3.75× under K=10 medians** because
rclcpp's 1 MB single-shot 406 µs was actually a slow outlier (true
median 183 µs ±54%). At 16 B the gap widened slightly (23×→28×) because
both stacks' K=10 medians were faster than their single-shot numbers.

**Honest spread:** rclcpp 1 MB at ±54% spread means single-run
rclcpp-intra-1-MB claims are unreliable. ±37% on the DDS path for the
same reason — DDS discovery + executor variance dominates.

**Architectural truth unchanged:** OASIS wins at every measured
payload size via function-call dispatch; rclcpp's cost is executor
+ waitable machinery (intra) or + DDS roundtrip (DDS default).

---

## Inviolable rules (same as before — unchanged)

| # | Rule |
|---|---|
| R1 | Strict typing, no `any` |
| R2 | Inter-agent comms only via tension field (never direct calls) |
| R5 | Validation on all mutations |
| R9 | No physical agent without safety sandbox active |
| R10 | Files < 400 lines (exceptions: spore.rs, spore_crypto.rs, mavlink_min.rs, federation.rs, mesh.rs — cohesive crypto/protocol modules; mesh.rs tests + Kani proofs were split into src/mesh/ to shrink the core 3049→1099 L, now **1509 L** after mesh v0B — tests live in src/mesh/tests.rs 1090 L, proofs in src/mesh/kani_proofs.rs 1108 L) |
| R13 | Inter-agent comms via vectorial tension |
| R14 | No physical action if entropy > critical threshold |
| R15 | Sensor loss = entropy spike + actuation freeze |
| R16 | Intent/reality divergence > threshold = massive entropy |
| R18 | Jitter < 5 %, budget-over operations truncated |
| R20 | Unsigned node = atomization < 1 ms |

---

## Mental loop (specialist discipline)

1. **Is it proven?** — ruthless test or it doesn't exist
2. **Does it break?** — 455 Rust tests + 120 Kani proof harnesses must pass before and after
3. **Is it bounded?** — fear ≤ 5×, entropy [0,1], latency < 1 ms, lux < 100 000
4. **Is it honest?** — every mechanism explicitly PROVEN vs EXPERIMENTAL
5. **Is it banded?** — **no single-shot bench number in the repo**. K=10 median ± half-spread or equivalent (Spore loss bench uses N=200 internal trials). Single-number claims are suspect.
6. **Is it resilient?** — 7/7 industrial tests, vitality-based graceful degradation, zero-config on 5 platforms
7. **Is it alive?** — the daemon does not die; it suffers, adapts, continues
8. **Cross-platform?** — Windows, Linux WSL, and MCU (`thumbv7em-none-eabi` cargo build passes; actual boot untested).

---

## Cargo features (4)

| Feature | Default | Purpose |
|---|---|---|
| `std` | on | Enables std-dependent modules (spore, federation, nerve, spinal, transport, mavlink_min, hal::KillSwitch). Off ⇒ no_std build for MCU. |
| `std_env` | on | env var config helpers (disable on no_std / sandbox) |
| `os_random` | on | OS RNG (getrandom). Off ⇒ MCU; caller supplies nonce via `*_with_material` / `*_with_nonce` AEAD APIs |
| `mesh_bloom_mcu` | off | Shrinks mesh Bloom 64 KiB → 2 KiB per router. 1% FPR threshold drops from ~52k to ~1.6k inserts. |

MCU build:
```
cargo build --target thumbv7em-none-eabi --lib --no-default-features --features mesh_bloom_mcu --release
```
Zero errors as of 2026-04-22. Binary flash/RAM size NOT measured (requires
linking against `cortex-m-rt` runtime).

---

## What was removed this round (abandoned)

**Rust**:
- 17 validation-phase binaries moved to `oasis-rt/_archive_bins/` and dropped from `Cargo.toml`
  (test_hebbian, test_federation, pc_bridge, pc_replay, test_real, test_cross, test_predictive, pc_bidir, spore_listen, spore_qr_test, test_motor, test_claims, test_collective, test_spinal_nerve, test_propagate, test_claim3, mavlink_sniff)
- Rationale: their work is done; preserved in git for archaeology.

**Docs**:
- 40 superseded audit/validation docs moved to `webots/_archive/`
- 6 obsolete root planning docs moved to `_archive/` (PLAN-CLAIMS-*, PLAN-TEST-*, DEMO-MESH-*, RAPPORT-OASIS, skills.md, PROJECT_MAP.md)
- `STATE_OF_TRUTH.md` + last 7 spore crypto audit docs kept as current truth

**TS kernel**:
- Kept on disk for spec reference, but flagged ARCHIVED (unmaintained)
- 1 known failing test (`integration-final.test.ts`); not a release blocker for the Rust path

Git history was initialized 2026-06-02 (baseline commit `d8681c5`) — the repo
previously had zero commits, so earlier "git history" claims were false. The
removed TypeScript, Python, demos, and archives are preserved in that baseline.
The Cargo workspace (`oasis-rt` + 3 std crates) and the 3 excluded MCU crates
now reflect only active Rust.
