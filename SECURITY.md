# Security Policy

OASIS controls real hardware (drones, robots, microcontrollers). Security
issues are taken seriously.

## Reporting

Do not open public issues for security vulnerabilities. Contact the
maintainers privately via the email listed in `Cargo.toml` (once public), or
open a GitHub Security Advisory.

## Threat model

OASIS agents can:
- Drive physical actuators (motors, wheels, propellers)
- Read persistent memory (pain memory, federated digests)
- Communicate cross-device via tension field or file-based federation

Relevant safety mechanisms (enforced in `oasis-rt`):

- **R9** — No physical agent without active safety sandbox
- **R14** — No physical action if entropy > critical threshold
- **R15** — Sensor loss = entropy spike + actuation freeze
- **R16** — Intention/reality deviation > threshold = massive entropy
- **R18** — Jitter < 5%, out-of-budget operations truncated
- **R20** — Unsigned node = atomization in < 1ms (KillSwitch)

## Known operational limits

- `drone_bridge.exe` reads `OASIS_SHARED` env var for federation directory.
  Treat this path as trusted. A malicious actor writing fake `*_state.json`
  or `*_fed.bin` can influence peer drones' behavior via federated digests.
  Run on isolated networks or sign federation payloads.
- No authentication on the stdin/stdout JSON protocol. Deploy the kernel
  alongside a trusted I/O layer (cflib for Crazyflie, Webots controller).
- `pool_push_test` is exposed in `FederatedMesh` as a test helper but is
  used by `drone_bridge` for digest emission — this is a documented design
  choice but would need a production-grade signed-emit path for deployment.

## Supported versions

`oasis-rt` is pre-1.0. Security fixes will be applied to the latest
published release only. Pin your version in `Cargo.toml`.
