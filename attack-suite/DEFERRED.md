# Part D — comparative attack suite: DEFERRED

**Status: différée en attente d'environnement de simulation** (deferred pending a
simulation environment). Decision by the project owner on 2026-10-06: a strict
**15-minute timebox** for the WSL install. If it failed, Part D was to be deferred and
all effort spent on RP2040 physical validation.

## What the timeboxed probe established

Raw log: `env_probe/zephyr_wsl_probe.log`. Script: `env_probe/zephyr_wsl_probe.sh`.
Timebox: 10:44:28 → 10:59:28 UTC, enforced with `timeout` inside WSL.

| Step | Result |
|---|---|
| WSL Ubuntu 24.04, 3.3 GB RAM, 4 cores, 929 GB free | ✅ |
| cmake 3.28, ninja 1.11, gcc 13.3, git, python 3.12 | ✅ present |
| `pip install --user west` | ❌ attempt 1 blocked by PEP 668 (externally managed Python), at 18 s |
| passwordless sudo → `apt install device-tree-compiler`; `west` in a venv | ✅ attempt 2 |
| `west init` Zephyr v3.7.0 (pinned) | ✅ ~3 min |
| `west update` restricted to Zephyr + BabbleSim | ❌ the project filter fetched only `tinycrypt`; BabbleSim and the nRF HW models never arrived |
| BabbleSim build / BT Mesh bsim test build | ❌ no `tools/bsim`; CMake configure errors |

Probe ended at 10:51:07 UTC with **no buildable Bluetooth Mesh simulation**. No third
attempt was made: the remaining failures are exactly the dependency/manifest tuning
the timebox was meant to stop.

## What is NOT claimed because of this

- No executed comparison against Bluetooth Mesh, Reticulum or Meshtastic exists.
  Reticulum needs no WSL, but it was deferred too, under the "RP2040 only" decision.
- The "OASIS alone in the lead, with proof" claim of
  `prompts/OASIS_REVOCATION_GATE_ATTACKSUITE.md` **cannot be made**: Parts E and F are
  proven for OASIS only.
- Competitor rows in `docs/SECURITY_COMPARISON.md` and
  `partners/COMPETITIVE_ANALYSIS.md` keep their "non vérifié" markers.

## To resume later

- A working Zephyr workspace probably needs the full `west update` (several GB),
  without the hand-written project filter, then
  `cd tools/bsim && make everything` (or Zephyr's documented `bsim` setup), then
  `tests/bsim/bluetooth/mesh/compile.sh`.
- Budget at least an hour on a Linux host with ≥ 8 GB RAM (this WSL has 3.3 GB).
- Reticulum (`pip install rns==<pinned>`, local `TCPInterface` instances) is the
  cheapest system to start with.
- The partial workspace is left in WSL at `~/zephyr-probe`, with `west` in
  `~/zvenv`, so it can be reused or deleted.
