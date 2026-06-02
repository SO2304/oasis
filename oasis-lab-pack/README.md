# OASIS — Lab Evaluation Pack

> **Just want to run the demo?** → open [START_HERE.txt](START_HERE.txt)
> (3 seconds to read, then ~1 second to run the binary).

**Goal**: 30-minute sanity check + 1-hour deeper evaluation for an
external lab (grid operator / IoT edge / defense) considering whether
OASIS is worth a proper integration test.

**Who this is for**: engineers and technical decision-makers who've
never heard of OASIS and don't want to install a Rust toolchain just
to see whether the thing works.

---

## Level 1 — 30-minute engineer quick-test

### What's in the box

Three pre-built binaries, statically linked, self-contained.
**No install, no network, no Rust toolchain, no Docker.**

| File | Target | Size | Works on |
|---|---|---:|---|
| [binaries/oasis_grid_demo-linux-x86_64](binaries/) | x86_64 Linux (musl static) | 462 KB | Ubuntu, Debian, Alpine, RHEL, CentOS, Fedora, Arch — any distro |
| [binaries/oasis_grid_demo-linux-aarch64](binaries/) | ARM64 Linux (musl static) | 412 KB | Raspberry Pi 4/5, Jetson Nano/Xavier, AWS Graviton, Apple Silicon under Docker |
| [binaries/oasis_grid_demo.exe](binaries/) | x86_64 Windows PE | 234 KB | Windows 10/11, WSL, Wine |

Launchers auto-detect the host OS/arch and pick the right binary:

| Launcher | When to use |
|---|---|
| [run_demo.sh](run_demo.sh) | Linux, macOS, WSL, Git Bash, Cygwin |
| [run_demo.bat](run_demo.bat) | Windows cmd / double-click |

### Run it

**Linux / macOS / WSL**:
```bash
chmod +x run_demo.sh
./run_demo.sh
```

**Windows**: double-click `run_demo.bat`, or from cmd:
```
.\run_demo.bat
```

Completes in under 1 second. No environment variables, no config
files, no argument parsing — it just runs.

### What it does

The launcher runs **two demos in sequence**:

**Demo v2 — each layer in isolation** (8 scenarios, pedagogical first-contact)

| # | Layer | Scenario | Expected |
|---|---|---|---|
| 1-3 | L1 / M11 mesh | Normal / insider spoof / replay | Accept / reject / reject |
| 4 | L2 / R14 entropy gate | SCADA cmd under high sensor uncertainty | Refuse |
| 5 | L3 / M9 reflex arc | Transformer vibration 6× spike | FIRE |
| 6 | L4 / M10 pressure field | Edge agent near hazard zone | Reroute |
| 7-8 | L5 / Vitality + KillSwitch | Sensor loss / geofence breach | Downgrade / latch |

**Demo v3 — layers coordinated** (4 chained scenarios, operational reality)

| # | Layers | Scenario | What it shows |
|---|---|---|---|
| C1 | L1 ∩ L2 | Insider with valid keys; receiver's local entropy high | Sig OK + R14 says no = REFUSE — both layers participate in one decision |
| C2 | L3 → L4 | Reflex fires on anomaly; handler injects Repulsive zone; M10 reroutes | Subsequent navigation avoids the new hazard |
| C3 | L5 ⇒ L5 | Sensor losses accumulate over 100+ ticks → Vitality reaches Dead | KillSwitch latches via PanicReason::VitalityDead |
| C4 | L1 → L5 | Full DER setpoint pipeline | Mesh sig → entropy → pressure-field → geofence → vitality, then mid-execution fault triggers reflex → cascade → halt |

**Demo v4 — all 11 OASIS mechanisms, honestly tagged**

| # | Mechanism | Status | Demo shows |
|---|---|---|---|
| M1 | Tension field | **PROVEN** | 128D vector algebra exact, byte-match host |
| M2 | HyperState + R14 | **PROVEN** | entropy + R14 gate at 2 thresholds |
| M3 | Efference copy | EXPERIMENTAL | predict/reflect API responsive |
| M4 | Temporal branching | EXPERIMENTAL | 8-branch fitness selection |
| M5 | Emotional gain | **PROVEN** | fear=0.6212 near hazard, decays to 0 far |
| M6 | Morphogenesis | EXPERIMENTAL | 4-agent role differentiation |
| M7 | Synaptic Hebbian | **PROVEN** | synapse formation + weight=0.3067 byte-match |
| M8 | Dream consolidation | EXPERIMENTAL | record/dream replay (CLAUDE.md notes blocked in long live runs) |
| M9 | Reflex arc | **PROVEN** | sigma-outlier fires, baseline silent |
| M10 | World model / pressure | EXPERIMENTAL | gradient-descent escapes hazard |
| M11 | Federated resonance | **PROVEN** | Ed25519 mesh accept/reject |

EXPERIMENTAL = API responsive without panicking. **NOT** production-
ready. To promote a mechanism to PROVEN, the OASIS project requires
≥3 unit tests + ≥1 long-run real-hardware validation + byte-exact
host↔MCU equivalence test.

**v2 is the spec sheet. v3 is the operational manual. v4 is the full
honest mechanism roster.**

### What you should see

Header + 4 scenario blocks + summary. Each scenario shows:
- The raw Ed25519 sign/verify latency in µs
- Whether the envelope was accepted or rejected and why

Expected final lines:

```
══════════════════════════════════════════════════════════════════
  SUMMARY:  8 PASSED,  0 FAILED  (out of 8 scenarios)        ← v2
══════════════════════════════════════════════════════════════════
...
══════════════════════════════════════════════════════════════════
  v3 SUMMARY:  4 PASSED,  0 FAILED  (out of 4 chained scenarios)
══════════════════════════════════════════════════════════════════

[DONE] Both demos PASSED. See docs/ for level-2 evaluation.
```

Exit code: `0`. Any other result = something is genuinely broken,
contact us.

### Timings to expect

On a modern Intel/AMD laptop (~3 GHz):
- Ed25519 sign: ~100-300 µs per envelope
- Ed25519 verify: ~100-150 µs per envelope
- Tamper/replay rejection: ~15-150 µs

On a Cortex-M0+ MCU (measured under cycle-accurate sim, **not** visible
in this demo but available on request): verify is ~206 ms. This is the
honest cost of public-key crypto without hardware acceleration — see
[docs/capabilities.md](docs/capabilities.md).

---

## Level 2 — 1-hour technical decision-maker review

After the 30-minute run, the following one-pagers answer "should we
spend engineering time on a real integration test?"

| Document | Audience | Time |
|---|---|---|
| [docs/architecture.md](docs/architecture.md) | Architect / integrator | 15 min |
| [docs/capabilities.md](docs/capabilities.md) | Engineer / evaluator | 15 min |
| [docs/grid_use_cases.md](docs/grid_use_cases.md) | Grid ops / HEDGE-IoT PI | 15 min |
| [docs/SHADOW_AUDIT.md](docs/SHADOW_AUDIT.md) | Anyone skeptical | 15 min |

Architecture covers: where OASIS sits in a grid-edge stack, what it
integrates with (MQTT, Modbus, SCADA, PLC), what it does NOT replace.

Capabilities covers: honest list of what works today (with evidence
citations), what's scoped but not built, what needs external work
(audit, field refs, standards compliance).

Grid use cases covers: 3 concrete scenarios Elia or HEDGE-IoT might
run the integration against — offshore wind sensor mesh, substation
cyber-resilience, DER coordination edge — with honest fit assessment
for each.

Shadow audit is where the skeptical reader starts: **what we're NOT
claiming** and what's still at simulator-only TRL 6 stage.

---

## What to do after the 30-minute test

### If the demo PASSED and you want to go further

Options, from cheapest to most involved:

1. **Read [docs/SHADOW_AUDIT.md](docs/SHADOW_AUDIT.md)** first — honest
   gap-analysis, will tell you if OASIS is even the right fit. ~15 min.
2. **Ask for a walkthrough** — 30 min screen-share with the OASIS team
   on your hardest adversary scenario.
3. **Request source access** — the Rust crate is available for
   evaluation under NDA; open-source release pending. Ask for the
   `oasis-rt` + `oasis-lora-transport` repos.
4. **Pilot integration** — 5-10 engineer-days to integrate on your
   actual substation/sensor hardware. Contractor rate.

### If the demo did NOT PASS

Send us the full terminal output + your OS version. Any non-PASS is
either a build bug or a compatibility issue — both are on us, not you.

---

## Contact

**Souhayb Rharrab**
souhaybrharrab@gmail.com
+32 486 32 64 46

Technical questions, source access requests, integration support, or
30-min walkthrough scheduling — email or call directly.

### Binary integrity

SHA-256 of all 9 binaries (v2 + v3 + v4, 3 targets each):

```
6f29ab3d8350677f80fb3f922afdb197d8311263e1f7b4e1aa17b76fa95c3a01  oasis_grid_demo-linux-x86_64
9ec407629b2665b9c4f7bd6c66baa1912538a4b11bf7b0f314444d8a6a4612ae  oasis_grid_demo-linux-aarch64
7634c46891793ba2401708e9d7e193b2986c7267b0c079e9869406d34a10cfae  oasis_grid_demo.exe
31e6a827d8acdbdc43dcf11fecc8a589fe8ca394f50d56be084041d7c49681aa  oasis_grid_demo_v3-linux-x86_64
9f4c9e70d0291ad5e5bdc31881cf7283079c982d6520838c84e5b9d2dda9eee2  oasis_grid_demo_v3-linux-aarch64
05daf8131d28738bc1628b2f1a7c2fce04989f219ed28722dddc9d01f0ea18ad  oasis_grid_demo_v3.exe
be3ea127f61e74dfe6e0142b614a60f685b2858fa3cd929b03b454d86a57c737  oasis_grid_demo_v4-linux-x86_64
c80a93d6a88210923d0a703e7064d4931956322b438b5c9d3d6e5a6b315be7d1  oasis_grid_demo_v4-linux-aarch64
2fee3c5bad76d06eff1ca9980a6e7cec7940e2ad9498d4669630c6c78ae14d38  oasis_grid_demo_v4.exe
```

Verification commands:

- **POSIX / Linux / macOS / WSL**:
  ```bash
  sha256sum binaries/oasis_grid_demo-linux-x86_64
  sha256sum binaries/oasis_grid_demo-linux-aarch64
  sha256sum binaries/oasis_grid_demo.exe
  ```
- **Windows cmd**:
  ```
  certutil -hashfile binaries\oasis_grid_demo.exe SHA256
  ```

Output must match byte-for-byte. Any mismatch — do **not** run the
binary; contact the number/email above.
