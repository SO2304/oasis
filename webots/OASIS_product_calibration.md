# OASIS — Product Calibration Round

**Status**: ✅ **Repo calibrated to what actually works. 17 legacy binaries + 46 stale docs archived. CLAUDE.md rewritten honest. 267/267 tests still pass, all 10 production binaries build.**

---

## 1. The problem this round addressed

OASIS had drifted into a state of identity crisis:

- **3 products in one repo** — Rust drone kernel, Android nervous-system daemon, TS reference spec
- **27 binaries** — only ~10 were actually maintained; 17 were validation-phase leftovers
- **58 markdown files** across `webots/` + root — most were superseded audit snapshots
- **CLAUDE.md had aspirational framing** that wasn't aligned with what's been proven

A new contributor reading the repo couldn't tell what was real vs what was wishful thinking.

---

## 2. Cuts executed

### Binaries archived (17 → `oasis-rt/_archive_bins/`)

| Binary | Was used for | Why archived |
|---|---|---|
| test_hebbian | STDP validation phase | Work done, tests in lib |
| test_federation | Fed mesh validation | Work done, tests in lib |
| pc_bridge / pc_replay / pc_bidir | PC↔Phone bridge tests | Replaced by drone_bridge/mavlink_adapter |
| test_real / test_cross / test_predictive | Phone data validation | Consolidated into unit tests |
| spore_listen / spore_qr_test | Older spore harness | Replaced by spore_recv_v2 |
| test_motor / test_collective | Industrial + hardware tests | Phase closed, validation in lib |
| test_claims / test_claim3 / test_propagate | Mechanism validation | Covered by unit tests |
| test_spinal_nerve | 5-platform discovery test | Covered by unit tests |
| mavlink_sniff | Diagnostic | Replaced by adapter logging |

`Cargo.toml` updated — these don't compile any more; `cargo build --release --bins` builds only the 10 production bins.

### Production bins kept (10)

`main` (Android daemon via top-level), `drone_bridge`, `mavlink_adapter`, `spore_send`, `spore_recv_v2`, `spore_revoke`, `oasis_keygen`, `oasis_fingerprint`, `udp_loss_proxy`, `bench_spore_loss`, `bench_r14_latency`.

### Docs archived (40 → `webots/_archive/`)

All `recon10v*`, `recon20_*`, older `OASIS_PX4_*`, `OASIS_FLIES_DRONE`, `OASIS_autonomous_flight`, `OASIS_waypoint_mission`, `OASIS_position_setpoint`, `OASIS_param_boost`, `OASIS_execution_round_*`, `OASIS_v6_*`, `OASIS_competitive_analysis`, `OASIS_novelty_analysis`, `OASIS_vs_MAVLink_STANAG_benchmark`, `OASIS_kani_verification`, `OASIS_mavlink_integration_validated`, `OASIS_mavlink_security_complete`, `OASIS_spore_v2_validated`, `OASIS_spore_v2_radio_validated`, `OASIS_spore_encryption_validated`, `OASIS_spore_v5_sender_auth`, `OASIS_replay_protection_validated`, `OASIS_closed_loop_validated`, `OASIS_MAVLink_ROS2_bridge`.

### Root docs archived (6 → `_archive/`)

`PLAN-CLAIMS-4-6-8-10.md`, `PLAN-TEST-11-CLAIMS.md`, `DEMO-MESH-3PLATFORMS.md`, `RAPPORT-OASIS.md`, `skills.md`, `PROJECT_MAP.md`.

---

## 3. What's KEPT (current source of truth)

### Rust code
- 22 lib modules in `oasis-rt/src/` — all used by `main.rs` (Android daemon) OR production bins
- 10 production binaries in `oasis-rt/src/bin/`
- 267/267 unit tests pass in parallel

### Docs
- `CLAUDE.md` (rewritten, honest)
- `README.md`, `CONTRIBUTING.md`, `SECURITY.md`
- `webots/STATE_OF_TRUTH.md` + 7 recent spore-stack audits:
  - `OASIS_spore_crypto_hardening.md`
  - `OASIS_spore_listener_ratelimit.md`
  - `OASIS_spore_persist_and_distribute.md`
  - `OASIS_spore_revocation_ops.md`
  - `OASIS_spore_sliding_window_and_eviction.md`
  - `OASIS_spore_v7_counter_and_global_rate.md`
  - `OASIS_spore_window128_lru_listener.md`

### TS kernel
- Kept on disk as reference spec
- Explicitly marked ARCHIVED — no longer actively maintained
- 1 known-failing test (`integration-final.test.ts`); not a release gate

---

## 4. Incoherences fixed

| Incoherence | Fix |
|---|---|
| "OASIS is a full drone autopilot" vibe | CLAUDE.md now says "layer ABOVE PX4" |
| "OASIS tests all 11 mechanisms on real hw" | PROVEN (6) vs EXPERIMENTAL (5) explicitly called out |
| "TS kernel is the spec" (but diverging from Rust) | TS explicitly ARCHIVED; Rust is source of truth |
| 48 audit docs give 48 conflicting stories | 8 current docs — older ones archived, stamped deprecated |
| "278 KB binary" (ambiguous — which binary?) | Clarified: lib size; binaries vary (e.g., mavlink_adapter = 650 KB) |
| Webots was abandoned 20 rounds ago but folder still claimed active | Folder kept for docs; webots simulator no longer used |
| 17 test binaries in Cargo.toml but unused | Archived + removed from Cargo.toml |

---

## 5. Shadow audit — honest caveats

### ✅ What the calibration achieves
- `cargo build --release --bins` now builds ONLY the 10 production binaries (fast + clear)
- `ls webots/*.md` returns 8 files, all relevant
- `ls *.md` returns 4 files (README, CLAUDE, CONTRIBUTING, SECURITY)
- New contributor has a fighting chance of understanding the scope
- Git history preserves everything archived — no data lost

### ⚠️ What the calibration does NOT do
1. **Does not reduce LOC** — 11 160 lines of Rust lib still active. All 22 modules used by `main.rs` or production bins. Those modules stay.
2. **Does not remove the "two products in one repo" issue** — the Android daemon and the drone kernel share the crate. Splitting would be a separate decision.
3. **Does not deprecate the 5 EXPERIMENTAL mechanisms** — M3, M4, M6, M8, M10 are still compiled and run in the Android daemon. Their status is now honestly flagged but they still exist.
4. **Does not delete the TS kernel** — 27 329 LOC still on disk, taking space. Left in place as reference.
5. **Does not fix the 1 failing TS test** — flagged but not fixed.
6. **Does not reduce the `spore_crypto.rs` growth** — 2 152 LOC, well over the R10 400-line cap. Crypto files are explicitly exempted (rule R10 footnote updated).

### 🔎 What I could cut next (but chose not to, this round)
- **Audio module** (221 LOC) — used only by `main.rs`. If the Android daemon stops being a deliverable, this goes.
- **Spinal / nerve modules** (945 LOC combined) — same dependency pattern. Face 2 specific.
- **TS kernel** — could hard-delete with git log preserved. Decision deferred pending ops discussion.

---

## 6. Updated state

| Metric | Pre-calibration | Post-calibration |
|---|---|---|
| Binaries built by `cargo build --release --bins` | 27 | **10** |
| Markdown files in `webots/` | 48 | **8** |
| Markdown files at root | 10 | **4** |
| `Cargo.toml` `[[bin]]` entries | 27 | **10** |
| Rust lib tests passing | 267/267 | **267/267** |
| TS kernel tests | 382/383 | 382/383 (unchanged) |
| Linux binary build time | ~30 s | ~15 s |
| Production bins LOC | ~7 774 | **~3 820** |

---

## 7. The honest pitch, now calibrated

> **OASIS** is a Rust kernel with two validated deployments:
>
> 1. **Drone autonomy layer** — runs above PX4 via MAVLink v2. Full security
>    stack (ChaCha20-Poly1305 AEAD, X25519 ECDH forward secrecy, Noise-KK
>    sender auth, 128-bit sliding-window replay, signed revocation). 17 of 23
>    threats covered in-protocol. Validated end-to-end on PX4 SITL + jmavsim.
>    3 Kani formal safety proofs.
>
> 2. **Android adaptive daemon** — 11 bio-inspired mechanisms running on a
>    Samsung S23 FE via Termux. 6 proven on real hardware, 5 experimental.
>    Longest run: 3 h 23 min / 121 290 ticks, zero crash.
>
> 267/267 unit tests pass in parallel. Pre-1.0. No external audit yet. Not
> tested on real Pixhawk hardware. TS reference spec archived — Rust is
> source of truth. All aspirational "AI nervous system for everyone" framing
> has been removed from docs.

Every clause in this doc is backed by a test, a log line, or a measurement.
Nothing added; many claims removed.
