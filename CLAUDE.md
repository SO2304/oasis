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
removed content now lives in baseline commit `d8681c5`. The **5** std crates
are a Cargo **workspace** (the fifth, `oasis-test-plc`, is this PC playing the device) (`resolver = "2"`); the 3 MCU crates are
*excluded* so `mesh_bloom_mcu` feature-unification can no longer shrink
the host Bloom (64 KiB → 2 KiB). Re-measured headline numbers (commands
reproduce them): **663 `oasis-rt` lib tests, 706 across the workspace**
(`cargo test -p oasis-rt --release --lib` / `cargo test --workspace --release`;
re-measured 2026-10-08, and **counted, not added up** — `cargo test -p oasis-rt --release
--lib -- --list | grep -c ': test$'` gives 663 across **49 test modules**, the largest being
`spore_crypto` 82, `mesh` 68, `actuation` 32, `spore` 31, `mavlink_min` 30. ⚠️ This line used
to carry a phase-by-phase addition chain; it summed to **621** and the validation matrix's
version to **624**, against a measured **623**. Both chains are gone rather than patched,
because per-phase deltas cannot be re-derived without a worktree and a test run at each
phase commit, and a chain nobody can check is worse than no chain),
**195 Kani proof harnesses** (`grep -rE 'kani::proof' oasis-rt/src | wc -l`;
53 mesh incl. v0A/v0B, 3 pre-filter v0C, 15 actuation gate + parts G/H/J, 6 part I journal, 4 part K TimeView, 5 C9 quorum, 4 B1 MAVLink carrier, 2 Modbus TCP, 5 Modbus TCP reads, 5 OSE1 confidentiality, 7 authority + fragment, 7 enrollment + ownership, 3 firmware, 3 Modbus gateway, 6 revocation + sender lease, and 67 across the 19 older mechanism and rclcpp-primitive modules; **counted per file, not by addition** — the incremental list this line used to carry summed to 179 and the per-module list below to 115, both against a measured 182). **First full-suite run 2026-10-07** (`evidence/kani/2026-10-07/full/`): **130 verified, 10 undetermined (`CBMC failed`, out of memory — NOT refutations), 12 never reached** before the OOM killer stopped the run on a 3.3 GB WSL. No counterexample **in that run**; the remaining 22 are **unknown, not verified**, and need ≥ 16 GB or the CI job. ⚠️ **« aucun contre-exemple » n'est plus vrai de l'historique** : `main` a réfuté deux harnais mesh — `proof_ad_reset_count_predictable` (le disjoint `|| actual_resets == 0` admettait un écart > 1) et `proof_af_combined_pattern_no_dashboard_alert` (seuil non borné, `threshold * 1000` débordait) — et cinq autres expiraient à 600 s. Les correctifs sont entrés par la fusion de `main` le 2026-10-08 (`a3e4111`, `8572913`, `68efc66`). ⚠️ That run covered the **152** harnesses that existed then; the **26** added by Phase 2 parts G/H/I/J/K and by C9 were verified separately — 14/14 (`…/2026-10-07/hardening/`), 7/7 (`…/2026-10-08/jk/`) and 12/12 (`…/2026-10-08/c9/`, which re-ran the 7 J/K harnesses because `REASON_COUNT` changed) and 4/4 (`…/2026-10-08/b1/`) — so no full-suite pass over all 189 exists yet — the sweep below covered 175 of the 182 that existed before Modbus TCP was merged, so the 2 `mbtcp_*` harnesses were verified on their own branch (`evidence/kani/2026-10-07/modbus_tcp/`, 2/2 with failing negative controls) and not in that sweep — but a **sequential sweep of 2026-10-08** (`evidence/kani/2026-10-08/sweep/`, stamp `fb1db2c`) verified **175 of the 182, 0 refuted, 0 undetermined**, one harness at a time, in ~70 min on the same 3 381 MB: the OOM of the 2026-10-07 parallel run came from the **parallelism**, not the harnesses. The **7 heavy** ones are still not attempted locally, so "182/182" remains false. **CI-sharded by `oasis-rt/kani_shards.sh`** (merged from `main` 2026-10-08): `--check` passes over all **182**, each in exactly one shard, so the full suite is runnable on CI even though it is not runnable on this 3.3 GB WSL. On `main`'s 149 harnesses the CI run of 2026-10-06 gave **123 of the first 129 verified, 6 not verified**, the other 20 verified individually. **The 182 of this branch have never been run as one suite, on CI or locally.** **49 modules**,
**25 `[[bin]]`**, **50 `src/*.rs`** files (35 before Phase 1.1; this file said 32, stale) (largest module now `spore_crypto.rs`
2512 L; `mesh.rs` was split 2026-06-02 — its tests + Kani proofs moved to
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
│   │   └── transport.rs 298L Transport trait + LoRa frame builder
│   (the real radio lives in the oasis-lora-transport crate: 6 files, incl. a
│    434-line SX1262 embedded-hal 1.0 driver, airtime + enforced duty cycle,
│    and a simulated radio. Driver datasheet-verified and mock-tested,
│    **never run against a radio**.)
│   └── src/bin/              18 production binaries (see below)
├── kernel/                   TS reference spec (ARCHIVED — 27k LOC, unmaintained)
├── webots/                   Current shadow audit docs (8 files)
│   └── _archive/             40+ older audit snapshots
├── oasis-rt/_archive_bins/   17 legacy test/validation binaries
└── _archive/                 Old planning docs
```

---

## Production binaries (23 `[[bin]]` in `Cargo.toml`)

⚠️ The table below lists the 18 of the hygiene round plus the ones added since; `tools/check_claims.sh` compares its count against `Cargo.toml` so the header cannot drift again.

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
| `SPORE\x0B` | v0B mesh + Ed25519 over `"OASIS-MESH-v0B"‖network_id‖origin_fp‖counter‖payload_len‖SHA-256(payload)` + persisted 128-bit counter window + relay-enforced revocation | **payload forgery, message suppression, post-reboot replay, cross-network replay, revoked origins** — all rejected at the first hop. v0A signed only 22 header bytes, so payload swaps kept a valid signature (12/50 accepted on silicon) and poisoned dedup to suppress the real message. ⚠️ `ttl`/`hops` stay unsigned (they mutate per hop) — range-checked only. v0B routers are **strict by default** (v8/v9/v0A refused: closes a downgrade hole). Sender counter lease (`tx_lease`) implemented and silicon-proven 2026-10-06, so a rebooted origin is no longer locked out |
| `SPORE\x0C` | v0C mesh = v0B + `forwarder_fp` + 16-byte per-link HMAC-SHA256 tag, checked before Ed25519, plus a per-link verification budget | **forced-verification denial of service** (C1): a forged frame costs the relay 0.70 ms instead of 179.4 ms on RP2040 (256×, silicon 2026-10-07), and under an 8/s outsider flood the relay delivers **60/60** legitimate messages at 19 % load against **39/60** at 110 % in v0B. Link keys are derived from the identities, never distributed. ⚠️ Against an **insider** the budget bounds CPU (143 of 144 tokens, 103 % → 40 %) but **starves legitimate traffic** (4/60): it is a CPU cap, not fairness |

Order classes (Phase 2 part G): `OAC1` byte 50 is a **signed** class — `0` = `Act` (9 conditions), `1` = `Stop` (3 conditions, and no input can block a valid one: `proof_stop_never_blocked`). An unknown class is refused, not defaulted.

Threat model: **19 of 23 threats covered in-protocol** (remaining: disk
wipe, operator key compromise, rogue pairing, post-quantum, traffic
analysis — post-quantum only partly closed since Phase 1.1: **authority messages**
(revocation, policy, and later enrollment/firmware/ownership) can be hybrid
Ed25519 + ML-DSA-44, but per-hop v0B, orders and the spore layer stay classical;
rogue pairing only partly closed since Phase 1.2: a node is accepted only with an
owner-signed attestation after proof of possession, but the RP2040 cannot protect
its key from physical/USB access; v0A closed 2 from the prior "17 of 23" count — insider
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
| **663 `oasis-rt` lib tests / 706 workspace tests pass in parallel** (49 test modules; 495 / 534 before Phase 1.1. Counted with `-- --list`, not by adding per-phase deltas — see the note in the header. ⚠️ 706 is what **passes**; `-- --list` declares 708, the two extra being doc-test blocks marked `ignore`. Publishing the declared count as the passing one is how README.md briefly said 689) | `cargo test -p oasis-rt --release --lib`, `cargo test --workspace --release` | ✅ |
| **195 Kani proof harnesses** (53 mesh incl. v0A/v0B, 3 pre-filter v0C, 15 actuation gate + parts G/H/J, 6 part I journal, 4 part K TimeView, 5 C9 quorum, 4 B1 MAVLink carrier, 2 Modbus TCP, 5 Modbus TCP reads, 5 OSE1 confidentiality, 7 authority + fragment, 7 enrollment + ownership, 3 firmware, 3 Modbus gateway, 6 revocation + sender lease, and 67 across the 19 older mechanism and rclcpp-primitive modules) | `cargo kani --lib -Z stubbing` (WSL) — full CBMC pass NOT re-run (slow under WSL; 3.3 GB); verified individually: 16/16 E/F, 7/7 Phase 1.1 (`…/pq/`: run 1 refuted `proof_auth_precheck_no_downgrade`, a real defect, fixed in `3e67254`), 7/7 Phase 1.2 (`…/enroll/`: run 1 hit a CBMC out-of-memory, harness reshaped, with a negative control), 3/3 Phase 1.3 (`…/fwupdate/`, with a negative control), 3/3 Phase 1.4 (`evidence/kani/2026-10-07/modbus/`; its first negative control was invalid, one mutation never applied, redone), 3/3 Phase 2.1 (`…/prefilter/`, negative control fails 3/3 as intended), **14/14 Phase 2 G/H/I** (`…/hardening/`, stamp `8926609`; run 1 **refuted** `proof_journal_parse_total` on a real 4-byte hole in the entry format, fixed `f123447`; 2 harnesses run under a **declared stub** because a GOTO program containing sha2 is OOM-killed here; negative control 3/3 FAILED as intended) | ⚠️ ****175/182 verified** in the sequential sweep of 2026-10-08 (`…/sweep/`), 0 refuted, 0 undetermined; the first parallel run (`…/full/`) had given 130/152 before the OOM killer, which was the parallelism rather than the harnesses. The **7 heavy** harnesses are reserved for CI, so "189/189" would still be false |
| **Hybrid authority messages (Phase 1.1, `OAU1` + `OFR1`)** | `authority` + `fragment`: signed suite byte (Ed25519, or hybrid Ed25519 AND ML-DSA-44, FIPS 204), per-kind minimum suite never lowered and persisted in two flash slots, downgrade refused before any signature work, bounded reassembly, store-and-forward. ML-DSA-44 on device = `libcrux-ml-dsa` 0.0.10 (arithmetic/NTT/serialization formally verified; pre-1.0); RustCrypto `ml-dsa` 0.1.1 as oracle; NIST ACVP sigVer 15/15 on both; byte-for-byte keygen/sign match over 8 seeds; 20 072 single-bit flips 0 accepted. **RP2040 bake-off** (3×3×K5): libcrux 198 ms / 44.8 KB stack vs `ml-dsa` 178 ms / 84 KB; full hybrid gate **377 ms, 48.7 KB stack**; flash +125 KB. **Silicon A→B→C (stamp `1a9b461`)**: 14-fragment hybrid revocation applied and re-originated per hop; altered fragment → hash mismatch, altered message → `BadSignature`, nothing forwarded; policy raise → legacy ORV1 refused, Ed25519-only `OAU1` refused as downgrade in 4 ms; policy and hybrid list restored after a **real power cut** of B. See `docs/specs/PQ_AUTHORITY_SPEC.md`, `evidence/silicon/2026-10-06/pq/REPORT.md`. ⚠️ Wired UART only; ~5.5 s of computation per hop per 14-fragment hybrid message (first stated as 7.6 s with a wrong 341 ms sign time; uart_mesh signs v0B in 178 ms — erratum in the report; fragments now paced 100 ms apart after a FIFO overrun in Phase 1.2); no rate limit yet (Phase 2); kind 3 (firmware manifest) is used since Phase 1.3 | ✅ |
| **Enrollment and ownership transfer (Phase 1.2)** | `identity` (on-board keygen: 4 096 ROSC bits, SP 800-90B RCT/APT health tests, SHA-256 conditioning, one-time tool-nonce mix-in, proof of possession; all-zero seed refused), `enrollment` (OAU1 kind 2 attestation: pk, role, permissions, `enroll_seq`; fp derived; revoked never enrolled; seq strictly rising), `ownership` (offer signed by the current owner + acceptance signed by the offered keys, bound to the offer digest; `NeedsResign` before a second transfer), PC tool `oasis_enroll`. No compiled seeds or registry on the boards any more: unenrolled = `unknown sender` at the first hop; actuation "authorized" = enrolled with `ACTUATE`. **Silicon (stamps `5c4bd96`/`4713703`)**: 3 distinct keys at first boot; ROSC 43–47 % ones, MCV 0.81–0.91 bit/sample; unenrolled A refused by B and unenrolled B by C (14/14 drops); tool enrollment with proof of possession; attestation propagated A→B→C; B's order `NotAuthorized`, A's `Act`; transfer o1→o2 on 3 boards (o3 acceptance refused, o1 messages refused after, `NoPendingOffer`, `NeedsResign`); **real power cut** of B: same key, owner o2, peers, revocation re-verified with o2. See `docs/specs/ENROLLMENT_OWNERSHIP_SPEC.md`, `evidence/silicon/2026-10-06/enroll/REPORT.md`. ⚠️ Key readable from flash (BOOTSEL/SWD, no secure element); ROSC not a validated source (datasheet §2.17.5); ownership per network, not per device; only `ACTUATE` enforced | ✅ |
| **Signed A/B firmware update (Phase 1.3)** | `firmware` (OAU1 kind-3 manifest `version‖image_len‖sha256‖hw_id`, hybrid required by default; image header `OFWI`+version at 0xC0; pure install rule: authorized, right hardware, 0 < len ≤ 512 KiB, hash, image version = manifest, `version ≥ floor` and `> running`; floor in `tx_lease::DualSlotStore`, raised only after the new image's self-test) + `oasis-bootloader` (`embassy-boot` 0.7.0 / `embassy-boot-rp` 0.10.0: power-fail-safe swap, trial boot with revert, 8 s watchdog, USB held in reset during the swap, 3 failed boots → BOOTSEL). The new image confirms itself only at its main-loop entry. 3 `fw_*` tests, 3 Kani (verified + negative control, `evidence/kani/2026-10-06/fwupdate/`). **Silicon (stamp `200d96b`, bootloader `067855c`, 3× RP2040)**: valid v2 installed and confirmed; 1-byte-tampered image `HashMismatch`; older v1 `Rollback`; manifests signed by o3 and by o1 `NotAuthorized`; image hanging in its self-test reset by the watchdog and **reverted**, floor unchanged; **real power cut during the upload** (v2 boots, partial image refused) and **during the swap** (breadcrumbs: cut inside `prepare_boot`, swap resumed at power-on, v3 confirmed, floor 3); A and B updated v1→v3, A→B→C v0B verified afterwards. See `docs/specs/FIRMWARE_UPDATE_SPEC.md`, `evidence/silicon/2026-10-06/fwupdate/REPORT.md` (incl. 7 failed first boots: my UF2 merger hit RP2040-E14). ⚠️ No secure boot on the RP2040: BOOTSEL, SWD or the test firmware's `b` bypass everything, floor included; USB only, not over the mesh; the bootloader is neither updatable nor signed; a functional bug after the main-loop confirmation is not reverted; one power cut per case | ✅ |
| **Modbus RTU gateway for brownfield devices (Phase 1.4)** | `modbus_gateway` (pure, `no_std`): an `OMB1` order (FC06, or FC16 ≤ 8 registers) goes through the **unchanged Part F gate**, with "within limits" = configured unit + register map + per-register range. The RTU frame (CRC-16/MODBUS via the `crc` crate) is built only in the `Act` branch. 10 `mb_*` tests: `rmodbus` 0.12.2 (independent) parses and applies every frame, plus an end-to-end v0B run. 3 Kani: no frame without `Act`; a frame = the order within the rules; parsers total (`evidence/kani/2026-10-07/modbus/`, with a negative control). **Silicon (stamps `a39d5fe`/`bbee4cf`, 3× RP2040)**: A = brownfield device on `rmodbus` (no OASIS code) counting every byte on its UART1 bus; C = gateway; B = order origin. None of these put a byte on A's bus: forged key, modified payload, byte-exact replay, executed `cmd_seq` again, expired order, previous-boot order after a C power-on, unlisted register, out-of-range value, FC05, `ACTUATE` withdrawn, R14 unsafe, raw Modbus write injected unframed / mesh-framed / inside a validly signed envelope. **4 `Act` decisions = 4 frames sent = A's 4 bus writes, byte for byte**. Control: A alone executes the same raw write. See `docs/specs/MODBUS_GATEWAY_SPEC.md`, `evidence/silicon/2026-10-07/modbus/REPORT.md`. ⚠️ Protects only if the gateway is the device's only path; TTL not RS-485, one device, FC06 only on silicon (FC16 on PC); register map compiled, not signed; a C power-on put one stray byte on A's bus (no write); catch-up with DOME Sentry, the per-write authorization is the only possible edge (not documented by Veridify) | ✅ |
| **Relay pre-filter against forced verification (Phase 2.1, `SPORE\x0C`)** | Closes `POSITIONING_GAPS.md` C1. `mesh::prefilter` (pure, `no_std`): per-link MAC key `K(X↔Y) = HKDF-SHA256(X25519(our_sk, peer_pk), DOMAIN‖fp_min‖fp_max)` derived from the Ed25519 identities — **nothing distributed, no epoch, no flash slot**, and one stolen node yields only its own links (per-link keys chosen over a shared network key, decision 2026-10-07); 16-byte truncated HMAC-SHA256 tag over the origin-signed core, hop-invariant so a relay recomputes it for the downstream link without re-signing; keys cached per peer (`LinkKeys`, no-alloc); integer token bucket per ingress link (2/s, burst 24 to absorb one fragmented authority message). v0C = v0B body + `forwarder_fp` + tag (123 B header); `process_v0c` runs network → known forwarder → **tag** → **budget** before Ed25519, and drops non-v0C magic up front (anti-downgrade). 16 `prefilter` tests incl. RFC 4231, per-field tamper, A→B→C relay and an **Ed25519-call counter** proving zero verifications on a bad tag / past budget; 3 Kani harnesses **verified 3/3** (2.67 s / 4.99 s / 7.71 s, tree `b361555`, `evidence/kani/2026-10-07/prefilter/`; negative control 3/3 FAILED as intended, one mutation per harness, the script aborting unless all three applied). *(This line said "written but NOT RUN" until 2026-10-08: stale, and it understated the repo's own evidence — the run exists and is checksummed.)* **Silicon (B→C, stamp `0baec61`)**: a forged frame costs the relay **179.4 ms before, 0.70 ms after — 256×** (forged-only runs, 30 s, `crc_fails=0`; two rates agree to 1 µs after removing the one-off 190 ms per-peer X25519). Saturation by forged traffic alone: 5.6 frames/s → past what the link carries. See `docs/specs/RELAY_PREFILTER_SPEC.md`, `evidence/silicon/2026-10-07/prefilter/REPORT.md`. **Availability measured** after adding interrupt-driven UART RX (a 4 KiB ring filled by `UART0_IRQ`, since polling cannot capture bytes during a 185 ms verify): at 8 forged/s + 1 legitimate/s for 60 s, v0B delivered **39/60** legitimate messages at **110 %** load (saturated, 371/539 frames received) while v0C delivered **60/60** at **19 %** with **539/539 frames and 0 CRC failures**. **The token bucket fires on silicon** against an **insider** — any enrolled neighbour holds the link key, and the tag covers origin/counter/length/payload but *not* the signature, so it can make a valid-tag/broken-signature frame: the budget then spent exactly **143 of its 144 tokens** (2/s × 60 s + burst 24), cutting load 103 % → 40 % and CRC failures 67 → 0. ⚠️ **But legitimate delivery fell 38/60 → 4/60**: the budget is a **CPU cap, not a fairness mechanism** (it runs before the signature check, so it cannot tell a legitimate frame from an insider's) and must **not** be presented as protecting availability against an insider; two nodes, one wired link, no radio; single runs, not banded. The report's §5 records **six** measurement defects found and fixed (per-frame X25519 at 370 ms, a self-pacing injector, a harness losing one of two commands per USB write, polled receive, a terminal node logging per frame, and a budget that could not fire until the insider model existed) | ✅ |
| **Stop asymmetry, supervision liveness, decision journal (Phase 2 parts G/H/I)** | `docs/AUTHORITY_HARDENING_SPEC.md`, the three blocking gaps of the chosen lead segment (autonomous mobile machinery, `partners/SEGMENT_COMPARISON.md`). **Part G**: an order now carries a signed **class** byte (`OAC1` byte 50, taken from the reserved bytes, so pre-G orders decode as `Act` and `parse_oac1` still accepts `Act` only). `stop_decision` keeps **3 of the 9 conditions** — authenticity, the new `STOP` permission, non-revocation — and drops freshness, the sensor-state lock, limits and the sequence counter; for the sensor lock the sense is *inverted* (a lost sensor is a reason to stop). A stop **latches** (ISO 13850:2015 4.1.1.2) and only a local action clears it. **Part H**: `OSB1` beacon (kind 4 of the `OAU1` envelope, inheriting phase 1.1's signed suite byte), a 9th condition on `Act` only, bounded by `MAX_SUPERVISION_MS` = 5 min — at SF12 the EU868 duty cycle allows ~12 frames/hour, so second-by-second supervision is impossible on that link. **Part I**: hash-chained journal of **every** decision, accepted *and refused* (Annex III 1.1.9 says « légitime **ou illégitime** »), `verify` distinguishing Intact / Broken / SeqGap / Unconfirmed, plus `oasis_journal_verify` cross-checked against an **independent Python implementation** of the chain (5 verdicts, 5 exit codes agree). 24 tests, 14 Kani harnesses. 26 tests, 14 Kani harnesses, **14/14 verified** (`evidence/kani/2026-10-07/hardening/`, negative control 3/3 FAILED as intended; run 1 **refuted** `proof_journal_parse_total` on a real 4-byte hole in the entry format, fixed `f123447`; 2 harnesses run under a **declared stub**). **Silicon 2026-10-08, 3× RP2040, stamp `a09781e`** (`evidence/silicon/2026-10-08/hardening/`): three stops that would each be *refused* as an act — stale `cmd_seq`, earlier `boot_id`, lost sensor — were all **accepted**; a forged stop dropped at the v0B layer before the gate ran; an origin with `ACTUATE` but not `STOP` → `NotAuthorized` with no latch; with the stop latched **both** the LED and the Modbus gateway refuse and **board A (rmodbus, no OASIS code) stayed at `writes=4`**, then 5 after the local clear — 2 `Act` = 2 frames = 2 writes; supervision dead → act refused, **stop still accepted**; beacon replay and wrong-boot beacon both `applied=false`; the journal read off flash verified **intact, exit 0** over 10 entries of which **4 are refusals**, and one flipped bit makes it **exit 1**. **S8, a real power cut of board C** (operator pulled the cable): `boot_id` 47071 → 48350, the 4 entries survived in flash and **verify against the head captured before the cut** (exit 0, decisions byte-identical, refusals included); a decision taken in the new boot wrote after them and **both** chains then verify, the `boot_id` per entry being what separates them. ⚠️ The cut fell outside the ~200 ms window between writing an entry and committing the head, so the "**at most one unconfirmed entry**" verdict stays designed and unit-tested, **not demonstrated** (it needs a switched supply triggered on the write). S8 made one property explicit: the board does **not** re-verify the previous boot’s journal — the operator does, with the head they hold, which is the external anchor in miniature; the revoked-origin stop was not run (revocation is permanent and would end the campaign); the ring never wrapped; the journal is tamper-evident against a **remote** attacker only — BOOTSEL and SWD rewrite entries and head together. The campaign found **four defects of mine**: one latch for two actuators, the latch flag set but never read by the pure rule, `content_kind` not knowing `OSB1` so the beacon was relayed instead of consumed, and a log line **before the main loop** that spun on USB until the watchdog parked a board in BOOTSEL (three wrong diagnoses before the right one, all recorded). **Non-regression 2026-10-08 after C9 and B1** (`evidence/silicon/2026-10-08/regress/`, 5/5): valid order → `Act`; previous-boot order → `Reject(Expired)`, journalled; `OAS1` addressed to actuator 1 → LED latched, **gateway not** latched; a valid order during the latch → `Reject(Stopped)` with board A unchanged at `writes=7`, `rx_bytes=57`; journal read off flash → **`VERDICT intact entries=7`, exit 0**, carrying those four decisions in order, and one flipped hex digit → **`broken at=6`, exit 1**. ⚠️ Ran on the **flashed** firmware `6d3429f`/`ad8f24a`, **not HEAD**: B and C sit under the A/B bootloader with floor = running version (25/25, 26/26), so relevelling would raise the floor irreversibly for what is only C9's tenth reject counter in a log line. R5 returned "intact" **twice** before it was a real test (a filtered boot's entry, then `JRN_END`, which also starts with `JRN_E`) | ✅ |
| **Compact stop and commander clock view (Phase 2 parts J/K)** | `docs/AUTHORITY_HARDENING_SPEC.md` §J and §K. **J closes the reduction half of C3**: sizes read out of the silicon logs (v0B header 99 B, order 153 B, Modbus 132 B, beacon 131 B, attestation 221 B, revocation 234 B +8/node), then `docs/lora_budget.py` computes time on air — **the LoRaWAN payload cap is 51 B at SF10–SF12 while the v0B header alone is 99, so no OASIS message fits at long range on LoRaWAN class A**; on raw LoRa (255 B PHY) the SF12 envelope is **6 orders, 3 revocations, 7 beacons per hour per band**. `OAS1` carries only what the stop rule reads: **54 B → 11 B, 153 → 110 on the wire, 6 → 8 stops/h at SF12**, and `OAC1` class `Stop` stays accepted so nothing proven is withdrawn. **K closes C4**: until 2026-10-08 the PC read the actuator’s `boot_id` and `now_ms` over USB before every order — a lab shortcut. `TimeView` extrapolates a signed `OTM1` beacon (20 B payload, 119 on the wire) with the commander’s own monotonic clock; **every estimation error ends in a refusal, never an unintended execution** (tested against the real gate), and `MAX_VIEW_AGE_MS` is 1 h because drift is 40 ms/h (core clock measured 124.9986 vs 125 MHz) while an unnoticed reboot is the real risk. 12 tests, **7/7 Kani verified with no stub** (`evidence/kani/2026-10-08/jk/`), negative control 3/3 FAILED as intended. **Silicon 2026-10-08, 3× RP2040, stamps `6d3429f`/`26` (`evidence/silicon/2026-10-08/jk/`): part J 6/6, part K 6/6.** J: an `OAS1` from an origin with `ACTUATE` but not `STOP` → `NotAuthorized` **latching nothing**; `actuator_id=0` latches **both** actuators while `actuator_id=1` latches the LED and **leaves the gateway working** — board A (rmodbus, no OASIS code) stayed at `writes=6` under both latches and went to 7 under the LED-only one, which is what proves the addressing is not decoration; `OAC1` class `Stop` still accepted; journal **intact, exit 0** over 4 stop decisions whose `cmd_seq` — the field the rule deliberately ignores — is what tells them apart. K: **in K3 the PC supplied one number, the validity in ms**, never read `boot_id` or `now_ms`, and B built the order from a signed beacon **46,9 s old** → `Act`; a view a whole boot stale → `Reject(Expired)`; a fresh beacon → `Act` again; a beacon whose clock does not advance → `applied=false`, view unchanged. K5 also confirmed the awkward half of `proof_timeview_apply_is_monotone_within_a_boot`: the new boot’s `now_ms` was **smaller** than the stale estimate and was accepted anyway, because the boot differs. ⚠️ Part K needed **one wire moved** (`B.GP1` from `A.GP0` to `C.GP0`) — the mesh wiring is one-way, so C could not reach B, and that is reported as it happened rather than smoothed over; the K4 reboot came from a **reflash, not a power cut**; `A.GP0 → B.GP1` is now disconnected so the three-hop chain is unavailable until the wire goes back; K6 is **not** a byte-exact replay (nothing can drive B’s RX from the PC, so `@Zb<ms>` forces the clock instead); there is **no beacon scheduler**; and the duty cycle is **enforced since 2026-10-08** — `LoRaTransport` owns an EU868 1 % budget, `send_envelope` charges the frame's airtime and refuses with `DutyCycleExceeded { needed_us, available_us, retry_in_ms }`, and a counting radio proves 50 refused frames never reach it; opting out needs the explicitly named `new_without_duty_limit`. ⚠️ The time-on-air formula is still **not verified at the source** and **no radio has ever transmitted** | ✅ |
| **Two-signature orders (C9, `OAQ1`)** | `quorum`: a critical order carries k of n operator signatures over `"OASIS-QUORUM-v1"‖network_id‖body`, checked by `oasis_operator_key::OperatorAuthority::verify_authorization`, which requires k **distinct** keys; carried public keys are selectors, not authority. `quorum_decision` adds the quorum **after** authenticity and authorisation and **before** the nine base conditions, so a quorum is an extra gate and never a bypass, and an untrusted sender is never told the policy. 9 `q_*` tests (one signature is not a quorum; the same operator twice is not two; an outsider twice is nobody; per-field tamper over all ten fields; cross-network refusal; a quorum is not a replay licence), 5 Kani **verified 12/12 with the J/K re-run** (`evidence/kani/2026-10-08/c9/`, negative control 2/2 FAILED as intended). Size, asserted by a test: `v0b + oaq1_len(2)` = **343 B** against a 255-byte PHY limit — and **257 B** even with one-byte signer indices and no setpoints, so the two-person rule **costs two frames** and travels by `OFR1` fragmentation. ⚠️ **Nothing on silicon**: no node holds an operator key *set*, so the k-of-n path is still PC-only, as it was for revocation. Implemented and proved, not demonstrated on hardware | ⚠️ PC only |
| **Signed order carried in MAVLink (B1, `V2_EXTENSION`)** | `mavlink_order`: a v0B envelope travels inside `V2_EXTENSION` (msgid 248, `CRC_EXTRA` **8**, 249 opaque bytes), chosen because a v0B envelope with an `OAC1` order is **153 B** and `TUNNEL` offers only 128. The opaque block is `len u16 ‖ envelope ‖ padding`, as the field's own description in `common.xml` requires, because MAVLink 2 senders trim trailing zeros — `truncate_trailing_zeros` models one and a test reassembles after it. `arm_decision_ctx` builds the `COMMAND_LONG` (`MAV_CMD_COMPONENT_ARM_DISARM`) **only** in the `Act` branch, after the **nine** Part F conditions. 7 `mo_*` tests over a real `MeshRouter` cover the four B1 cases and name **which barrier fired**: forged → `Drop("unknown sender")`, swapped payload → `Drop("bad mesh signature")`, byte-exact replay → `Drop("stale counter")` from the *persisted* window, `cmd_seq` replay and revoked origin → the gate. 4 Kani **verified 4/4** (`evidence/kani/2026-10-08/b1/`, negative control 3/3 FAILED as intended; run 1's two FAILED were *unwinding assertions*, not refutations — `run1/`). The `CRC_EXTRA` values were recomputed with the ten already in `mavlink_min.rs` as the control, which caught two bugs in my computation and one error in my own control value. **PX4 SITL 2026-10-08** (`evidence/silicon/2026-10-08/b1-sitl/`, PX4 `v1.18.0-beta1-985-gdf387bdec2`, SIH quadx): **5/5**. Valid → `GATE Act` → one `COMMAND_LONG(400)` → **`INFO [commander] Armed by external command`**; forged → `unknown sender`; tampered-after-signing → `bad mesh signature`; replay → `stale counter` on the second copy, so **one frame and not two**; revoked → `Reject(Revoked)` — **0 armings** in those three, read from PX4's own log. The `V2_EXTENSION` crosses a **real UDP socket between two processes**. Built with **no sudo** (venv + pip wheels) and **no Java** (SIH runs the flight model inside PX4). ⚠️ Simulation only: no Pixhawk, no airframe, no radio. ⚠️ The vehicle **never receives PX4's stream**, cause **unknown** (I first blamed mavlink's `-f`, which actually sets `_forwarding_on`, so that was wrong), so the arming is established from PX4's log and not from the test's own observation. The first run proved nothing: the readiness pattern matched any `INFO [commander]` line, so the order landed in the startup transient and PX4 denied arming on its own EKF grounds — fixed by waiting for `Ready for takeoff`, measured at 5 s. ⚠️ `OrderClass::Stop` is refused by this carrier: for an airborne vehicle the safe direction is not a disarm, and OASIS does not define what it is. See `docs/specs/MAVLINK_ORDER_SPEC.md` | ✅ |
| **Modbus TCP transport for the gateway** | `modbus_tcp` (merged from `claude/eloquent-ptolemy-oojjn0` on 2026-10-08, where it had been sitting outside `main`): most recent machines speak Ethernet, and the phase 1.4 gateway spoke RTU only, so no pilot in front of a real machine was possible. This adds the **transport** and **leaves the gate untouched** — `modbus_gateway::gateway_decision`, itself the Part F gate, is unchanged, which is the point: a new transport must not become a new way in. The HMI is not modified and writes plain Modbus TCP to an operator-side agent, which signs an `OMB1` order the machine-side gateway verifies before it writes. 7 `mbtcp_*` tests, 2 Kani harnesses **verified 2/2 with failing negative controls** (`evidence/kani/2026-10-07/modbus_tcp/`). See `docs/specs/MODBUS_TCP_SPEC.md`. ⚠️ **Nothing on silicon and no PLC**: the RTU path is the one proven on three RP2040 against an independent `rmodbus` device; TCP is software only. ⚠️ The 2 harnesses were **not** in the 2026-10-08 sweep, which ran before this merge | ⚠️ software only |
| **End-to-end confidentiality of a mesh payload (C13, `OSE1`)** | `sealed`: the payload is sealed **origin to addressed actuator**, key `HKDF-SHA256(X25519(identities), DOMAIN‖fp_lo‖fp_hi)` — **nothing distributed, and a relay holds no key**, unlike the pre-filter's per-link key. The AAD binds domain, network, both fingerprints and the counter, so a ciphertext lifted from one envelope will not open in another; the nonce comes from the v0B counter, strictly increasing and persisted. **v0B and the gate are untouched** — a sealed blob is just a payload, and `open` returns the `OAC1` bytes the Part F rule reads unmodified. 8 tests (exhaustive single-byte tamper sweep, five wrong AADs, a relay holding both public keys that still cannot open it), **5 Kani verified 5/5** with a 2/2 negative control (`evidence/kani/2026-10-08/sealed/`). Overhead **50 B**: a sealed order is 203 B on the wire, still one frame. ⚠️ **It does not hide addressing** — origin, counter, length and destination stay readable, so **traffic analysis is not closed**. ⚠️ Run 1 refuted two of my own harnesses and exposed a third passing **vacuously** (32-byte buffers below the 50-byte minimum); fixed, and each harness now asserts its branch is reachable. ⚠️ **Nothing on silicon** | ✅ |
| **Modbus TCP as running programs (pilot phase A)** | An audit found there was **no socket anywhere in `oasis-rt`**: the Modbus TCP layer was pure, so it decided and built frames and could never reach a PLC. `mbtcp_net` adds the transport — length-prefixed framing bounded by `MAX_LINK_FRAME`, the declared length refused **before** any allocation (a test declares 4 GiB and sends four bytes), read and write timeouts everywhere, and Modbus exchanges that read exactly what the MBAP header declares. `oasis_mbtcp_gateway` verifies, runs the **unchanged** Part F gate, writes to the PLC **only** in the `Act` branch, and appends **every** decision to the journal, refusals included. `oasis_mbtcp_agent` leaves the HMI unmodified, persists its sequence **before** sending so a crash can lose a number and never reuse one, takes its deadline from the **gateway's** clock (`OTQ1` → `OTM1`), and **never leaves the HMI silent** — a timeout becomes exception 0x0B. **Reads go through the gateway** (`modbus_read`, `OMQ1`/`OMV1`): authenticated by the same v0B envelope, bounded by the same register map — a span with one register outside it is refused whole and **the device is never asked**, which is what stops FC03 being a scanner — FC04 refused by name, at most 8 registers. A read is deliberately **not** put through the actuation gate: it changes nothing, so a latched stop does not block it (a stop is when an operator most needs to see the registers) and reads are **not journalled** (an HMI polls; a day at 10 Hz would evict the command history from the ring). **19 tests + 4 on real sockets**: 6 `mbtcp_net`, 4 `mbtcp_pilot`, 9 `modbus_read`, and an integration test whose PLC is really **`rmodbus`** in `ModbusProto::TcpUdp` — 9 attack cases, 5 read cases, and the ordered value read back out of its storage. **5 Kani harnesses verified 5/5** (`evidence/kani/2026-10-08/reads/`, stamp `92b2f84`, no stub; 2 negative controls FAILED as intended on a **named property** with zero unwinding assertions, and mutation B showed the length check is not cosmetic — removing it makes `parse_omv1` read out of bounds). **Latency, K=10 × 50 writes** (`evidence/bench/2026-10-08/mbtcp/`): an authorised write costs **459–468 µs against 45–50 µs direct**, of which **286–317 µs is OASIS** over an identical three-hop control and **257–278 µs of that is sign + verify + gate**. **Four defects found, three of them mine.** (1) The two binaries each held their own copy of the exchange and the agent's copy still had a bug the library had fixed — an unlisted register reached the HMI as 0x0A instead of 0x02 — while the test stayed green because it tested the library and the operator would have run the binary; everything down to the HMI loop is now in the library and the binaries are a listener and a thread. (2) A TCP connect per write, three per order, cost 2 867 µs of 3 131 µs of overhead and banded at **±724 %**; connections are kept and the clock is refreshed once per second through part K's `TimeView`. (3) `fs::write` of the sequence file per order cost **258 µs**, as much as the whole crypto, so `SeqStore` leases 64 numbers per write — `tx_lease`'s guarantee, a crash losing up to 64 and never reusing one. (4) `serve_gateway_conn` took the state lock **before** a blocking read, so one idle peer froze the gateway for a whole timeout with no attacker involved; `an_idle_connection_does_not_block_another` pins it and the control fails at 1.468 s against a 1.5 s timeout. **This PC as the test device, 2026-10-09** (`evidence/pilot/2026-10-09/`, `tools/pilot_campaign.sh`): the **shipped binaries** run as three processes against `oasis_test_plc`, a separate crate that depends on `rmodbus` and **not** on `oasis-rt` — so "no OASIS code in the device" is checkable from its manifest — driven by an HMI written in Python with nothing imported. **42 cases of 42, three times**; and since 2026-10-09 a signed **`ORV1` over the link** (`oasis_mbtcp_revoke`), which is what finally **demonstrates the 1.1.9 ¶5 change journal** — implemented, Kani-verified 6/6 and demonstrated nowhere until then, because its firmware wiring compiles and no board has been reflashed. C15–C17: a list verified, **persisted before applied**, applied, journalled `Change(Revocation) flags=0x10`; a non-greater epoch and a list signed by an untrusted operator both refused and journalled `flags=0x00` — « légitime **ou illégitime** » on the same chain, attributed to the operator, `VERDICT intact`. ⚠️ **One operator**: `oasis-operator-key` is a dev-dependency of `oasis-rt`, so k-of-n is refused by name here rather than accepted on one signature of several; the device's own write counter is the ground truth, and it ends each run at 208 applied writes. It covers what no in-process test can: config and seed files read from disk, a **restart of the agent** and a **restart of the gateway**, the persisted sequence, the persisted send counter and the journal on disk — read back by `oasis_journal_verify` as **intact, exit 0** with acceptances *and* refusals named, and **exit 1** when a refusal is rewritten as an acceptance. End to end: **1 017–1 248 µs** per authorised write, journal included. **Four defects it found, none visible from a library test**: (0) the gate ran on **three constant inputs** — `authorized: true, revoked: false, r14_safe: true` — where the firmware feeds `registry.allows(origin, ACTUATE)`, `router.is_revoked(origin)` and `efs.r14_safe_now()`. So **`gateway_id` was never compared** and gateway 1 executed an order addressed to gateway 7 (C12 now shows 0x0A and `Reject(NotAuthorized)`), and **revocation did nothing** on this path — though C13 shows the real guard is one layer **above** the gate, since v0B drops a revoked origin at the first hop before verifying its signature, so `ctx.revoked` is defence in depth and is documented as such. The journal also recorded `origin=0000` on every entry, so it said a decision happened and not **who** caused it; it now attributes a verified decision (C14, `origin=aa00`) and **keeps** `0000` on a rejected envelope on purpose, because an unverified origin claim written into evidence as fact is worse than none. ⚠️ Still weaker than the firmware: the TCP config carries **keys, not permissions**, so a key in it can command anything in the register map, and `ACTUATE` enrolment is not on this path. Then: (1) the gateway called `Journal::append` and **threw the bytes away**, so the chain lived in RAM and died with the process while this row claimed every decision was appended — 1.1.9 ¶5 asks to *collect* evidence, and a record that vanishes collects nothing; (2) the agent **locked itself out of its own gateway** after a restart, because the order sequence was persisted but the v0B **send counter** was not, so it restarted at 1 and the counter window refused it correctly — `tx_lease` fixes exactly this on the MCU and the host agent never had it; (3) reopening the journal files per decision cost **13.2 ms** of a 13.2 ms write, cut to **1.2 ms** by holding the handles, same durability order. The campaign also lied three times before it was honest — five passes against three dead processes, a negative control that wrote 00 over 00, and a 151 ms "latency" that was Python startup — all recorded in its README. ⚠️ **It is still not a real PLC**: a device simulator with no scan cycle, no ladder logic, no I/O and no real-time guarantee, answering instantly where an automate takes milliseconds, on one host over loopback. The protocol and the authorisation path are real; the machine is not, and the latency is a floor. ⚠️ The raw-Modbus injection leaves **no journal entry** — refused by framing before it becomes a decision — a known limit of the audit trail, not an oversight. ⚠️ Orders are served **one at a time** — the lock spans the PLC round trip, which is right for one device and a throughput ceiling for a fleet. ⚠️ A read reveals register values to anyone holding the agent's key: the envelope gives authenticity, not confidentiality. ⚠️ Keys are in config files — no compiled seed, but a file as readable as any on the host (C14 again) | ✅ end to end against `rmodbus`, measured |
| MAVLink v2 CRC + signing + replay | 29 in-suite tests + 300 real PX4 frames | ✅ |
| Ed25519 federation + signing | `ed25519_signing_roundtrip` + tamper rejection | ✅ |
| **Ed25519 per-node mesh signing (v0A)** | 10 tests inc. `v10_spoofed_origin_fp_rejected` | ✅ |
| **Mesh v0B — payload-bound, fresh, domain-separated (`SPORE\x0B`)** | 11 `v0b_*` tests (one per attack: exhaustive bit-flip of every signed byte, content-swap suppression, forged origin, immediate replay, post-reboot replay, post-Bloom-reset replay, foreign network, revoked origin, ttl inflation, invalid-then-valid, reorder-accept-once) + 7 Kani proofs (parser totality; preimage binds payload-digest/counter/network) + `bench_mesh_v0b`. Builds `no_std` `thumbv6m`. **Silicon (3× RP2040, stamp `d285ef4`)**: 0/150 pre-CRC bit-flips accepted (v0A: 12/50), suppression defeated, **T8 reboot-replay rejected across a real USB power-cut** from the flash-restored counter window, on-chip cost +1 %. See `docs/MESH_V0B_SPEC.md`, `docs/SECURITY_COMPARISON.md`, `evidence/silicon/2026-10-06/REPORT_MESH_V0B.md`. Follow-up (stamp `3a67e6e`, `evidence/silicon/2026-10-06/followup/`): strict mode 150/150 traced 0 accepted, downgrade refused, byte-exact replay across a relay power-cut refused, sender lease across an origin power-cut accepted. | ✅ |
| **Signed mesh revocation (ORV1, Part E)** | `mesh_revocation`: operator-signed (single or k-of-n via `oasis-operator-key`, `no_std`), epoch strictly increasing, permanent (superset), persisted before apply, forwarded once per epoch, catch-up beacons. 13 `rev_*` + 3 quorum tests, 3 Kani (verified 2026-10-06, `evidence/kani/2026-10-06/`). **Silicon (stamp `6daa0bc`)**: propagated A→B→C, revoked origin dropped at the first hop before its signature, old list refused (`Rollback`) at the first hop, list restored from flash after a real power-cut. See `evidence/silicon/2026-10-06/ef/REPORT.md`. ⚠️ A revoked node can still relay others' traffic; catch-up and k-of-n are PC-only | ✅ |
| **Actuation gate (Part F)** | `actuation::actuation_decision`, pure, 7 ordered conditions (v0B ok, authorized, not revoked, unexpired in the actuator's clock via `boot_id`, R14, within limits incl. a NaN guard, `cmd_seq` strictly newer). 15 `act_*` tests, 3 Kani (verified 2026-10-06, `evidence/kani/2026-10-06/`). **Silicon**: LED on GP25 driven only on `Act`; unauthorized, expired, replayed, R14-unsafe (entropy 0.908), over-limit and NaN orders all refused; previous-boot command refused after a reboot. ⚠️ LED not visually confirmed (pin level read back) | ✅ |
| Auto-arm PX4 via OASIS | PX4 log: `Armed by external command` | ✅ |
| Auto-takeoff via OASIS | PX4 log: `Takeoff detected` | ✅ |
| 4-waypoint mission complete | 3 WAYPOINT_REACHED events in adapter log | ✅ (1 successful run) |
| Altitude hold closed-loop | 1.94 m vs 2.0 m target (±6 cm) | ✅ |
| Spore v7 loss + FEC real UDP | bench + loss proxy: 98% @ 30% uniform, 82-90% @ 30% burst | ✅ |
| RFC 8439 ChaCha20-Poly1305 vector | test vector matches byte-for-byte | ✅ |
| All 11 mechanisms compile + test | 643 `oasis-rt` lib tests across 47 modules | ✅ |
| Android daemon 3h+ run | session_v0_5 on S23 FE, 121 290 ticks | ⚠️ claimed; logs not in repo |
| **MCU cross-compile** (`thumbv7em-none-eabi`) | `cargo build --target thumbv7em-none-eabi --lib --no-default-features --features mesh_bloom_mcu --release` | ✅ 0 errors |
| **A/B vs ROS 2 Jazzy** (Linux intra-process, K=10 medians) | OASIS 241 ns vs rclcpp intra 5 624 ns vs rclcpp DDS 52 411 ns at 16 B — 23–217× faster | ✅ measured |
| 7/7 rclcpp core primitives matched | pub/sub + services + actions + tf2 + timer + parameters + params-events (8/8 incl. tf) | ✅ |
| Real hardware test (Pixhawk + quad) | **none** | ❌ |
| External crypto audit | **none** | ❌ |
| Real MCU hardware boot | full T0–T6 suite (ChaCha20-Poly1305 RFC 8439, X25519 RFC 7748, Ed25519 sign/verify/tamper, R14 gate, mesh v8/v9/v0A incl. forge-reject, on-silicon timing) **PASS 3× on THREE RP2040 boards** (9/9 runs green). ⚠️ An independent review found 3 **test-quality** defects in the 2026-10-04 run — T0 echoed a hard-coded clock (never measured), T5-v9's tamper drop was a dedup false-positive (MAC verifier never reached), T4 had no negative control. All three **corrected and re-verified on silicon 2026-10-06** (3 boards × 3 runs, 9/9 green): T0 now measures the core clock (±1 %: 124.9986 MHz vs 125 MHz, per-board variance), T5-v9 asserts `Drop("bad mesh mac")` on a fresh router, T4 proves `blocked=50/50 && nominal_false_blocks=0/50`. The OASIS crypto/R14/mesh logic was **not** changed — only the tests. Plus a **wired-UART A→B→C v0A mesh relay** with per-hop Ed25519 verification (`uart_mesh.rs`, §10). ⚠️ Not over-the-air — **no LoRa radio**; no energy/secure-element/flight; T8 flash-persistence done 2026-10-06 (v0B window + sender lease + revocation list). See `evidence/silicon/2026-10-06/REPORT.md` (corrected) + `evidence/silicon/2026-10-04/REPORT.md` §13 (erratum). | ✅ |

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
2. **Does it break?** — 706 workspace tests + 195 Kani proof harnesses must pass before and after
3. **Is it bounded?** — fear ≤ 5×, entropy [0,1], latency < 1 ms, lux < 100 000
4. **Is it honest?** — every mechanism explicitly PROVEN vs EXPERIMENTAL
5. **Is it banded?** — **no single-shot bench number in the repo**. K=10 median ± half-spread or equivalent (Spore loss bench uses N=200 internal trials). Single-number claims are suspect.
6. **Is it resilient?** — 7/7 industrial tests, vitality-based graceful degradation, zero-config on 5 platforms
7. **Is it alive?** — the daemon does not die; it suffers, adapts, continues
8. **Cross-platform?** — Windows, Linux WSL, and MCU (`thumbv7em-none-eabi` cargo build passes; actual boot untested).

---

## Cargo features (5)

| Feature | Default | Purpose |
|---|---|---|
| `mesh_v10` | **on** | Ed25519 per-node mesh signing — `SPORE\x0A`, and with it v0B and v0C, the actuation gate's `v0b_ok`, revocation and the whole authority layer. **Off ⇒ no signed mesh at all.** Deliberately uses only `ed25519-compact`'s baseline API so it stays `thumbv6m` clean. ⚠️ This row was **missing** from this table until 2026-10-08: a default-on feature that gates the main guarantee went undocumented, and `tools/check_claims.sh` now counts the features so it cannot happen again. |
| `std` | on | Enables std-dependent modules (spore, federation, nerve, spinal, transport, mavlink_min, hal::KillSwitch). Off ⇒ no_std build for MCU. |
| `std_env` | on | env var config helpers (disable on no_std / sandbox) |
| `os_random` | on | OS RNG (getrandom). Off ⇒ MCU; caller supplies nonce via `*_with_material` / `*_with_nonce` AEAD APIs |
| `mesh_bloom_mcu` | off | Shrinks mesh Bloom 64 KiB → 2 KiB per router. 1% FPR threshold drops from ~52k to ~1.6k inserts. |

MCU build:
```
cargo build --target thumbv7em-none-eabi --lib --no-default-features --features mesh_bloom_mcu --release
```
Zero errors as of 2026-04-22.

**Flash and RAM, measured** (`llvm-size -A -d` on the `thumbv6m-none-eabi` release ELFs,
re-measured **2026-10-09**; flash = `.text` + `.rodata`, RAM = `.data` + `.bss`). RP2040 has
2 MiB of flash and 264 KiB of RAM, so none of these is close to a limit.

**Each row carries the command that produces it**, because two of these binaries change size
with features and the previous version of this table named no command — which left two of
its four numbers impossible to re-derive (see the erratum below):

| Firmware | Build | Flash | RAM | What it is |
|---|---|---:|---:|---|
| `uart_mesh` | `--features bootloaded` | **309 KiB** | **100 KiB** | everything: v0B/v0C mesh, authority, enrolment, firmware update, journal, Modbus gateway, and the 1.1.9 ¶5 change journal |
| `uart_mesh` | default | 299 KiB | 100 KiB | the same without the signed-update commands |
| `oasis-silicon-test` | default | 131 KiB | 160 KiB | the T0–T8 crypto/R14/mesh suite |
| `pq_bench` | `--features bench_lx` | 182 KiB | 16 KiB | the bake-off **with** libcrux ML-DSA-44 linked |
| `pq_bench` | `--features bench_rc` | 80 KiB | 16 KiB | the bake-off with RustCrypto `ml-dsa` |
| `pq_bench` | default | 66 KiB | 16 KiB | neither verifier linked — the harness alone |
| `modbus_device` | default | 17 KiB | < 1 KiB | the brownfield device, `rmodbus` only, **no OASIS code** |

⚠️ **Erratum, 2026-10-09.** The previous table gave `uart_mesh` **336 KiB**, which is
reproducible at **neither** feature set on this tree (299 default, 308 bootloaded before the
change journal, 309 after), and listed **66 KiB** against "the ML-DSA bake-off" when 66 KiB
is the build that links **no** ML-DSA at all — the bake-off costs 80 or 182 KiB depending on
the implementation. Two of the four rows matched exactly, so the tool was not the cause: the
table simply did not say which build each number came from. The change journal of Annex III
1.1.9 ¶5 costs **+1 KiB flash and 0 RAM**, measured against the commit before it.

⚠️ This line said "NOT measured (requires linking against `cortex-m-rt`)" until
2026-10-08, long after four firmwares had been linked and flashed. `tools/check_claims.sh`
prints the footprints; it does **not** yet compare them to this table, so these numbers are
re-derived by hand from the commands above.

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
The Cargo workspace (`oasis-rt` + 4 std crates) and the 3 excluded MCU crates
now reflect only active Rust.
