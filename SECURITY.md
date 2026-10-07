# Security policy

OASIS authenticates commands and authority messages between microcontroller nodes
and gates physical actuation (drones, robots, industrial actuators). A vulnerability
can move real hardware, so reports are handled as a priority.

> **Status:** pre-1.0, not externally audited, not certified. The guarantees below
> are what the code, its tests and its Kani proofs show, with the limits stated
> next to them. See `CLAUDE.md` (validation matrix) for the evidence of each one.

## Reporting a vulnerability

- **Do not open a public issue.** Use GitHub private vulnerability reporting: the
  repository's **Security** tab, then **Report a vulnerability**. The maintainer must
  enable it in the repository settings before the repository is public.
- Please include:

  - the affected crate or firmware and the commit;
  - the impact (what an attacker gains: forged command, replay, denial of service,
    key exposure, etc.);
  - a reproduction or proof of concept;
  - whether you believe it is being exploited.
- We practise coordinated disclosure. A fix is published first, then an advisory
  that names the issue and credits the reporter, unless they ask otherwise.

**Response targets** (the maintainer must confirm these before the repository is
made public):

| Step | Target |
| --- | --- |
| Acknowledge the report | 5 working days |
| First assessment (valid / severity) | 15 working days |
| Fix or mitigation for a high or critical issue | 90 days, sooner if exploited |

## Scope

In scope:

- `oasis-rt`: mesh v0B, authority messages (`OAU1`, `OFR1`), enrollment, ownership,
  revocation, actuation gate, firmware update rule, Modbus gateway, spore crypto,
  MAVLink signing;
- `oasis-operator-key` and `oasis-secure-element`;
- the firmware crates `oasis-bootloader`, `oasis-silicon-test`, `oasis-mcu-demo`,
  `oasis-renode-m11` and `oasis-lora-transport`.

Out of scope, documented limits rather than vulnerabilities:

- **Physical access to an RP2040.** It has no secure boot and no protected key
  storage: BOOTSEL or SWD can read keys and write any image. The RP2350 would be
  the step up.
- **ROSC entropy.** The RP2040 ring oscillator is not a validated random source
  (datasheet §2.17.5). It is health-tested and conditioned only.
- **Brownfield devices.** A device behind the Modbus gateway is protected only if
  the gateway is its only path.
- **Test firmware commands** (`b`, `!`, `@J`, `@R`, `@K`, `@H` and others in
  `oasis-silicon-test`). They exist for the evidence runs and must not ship.
- **Known gaps** listed in `partners/POSITIONING_GAPS.md`, notably C1: a relay can
  be flooded with messages that force a signature verification.

Anything that breaks a stated guarantee is in scope, even within these areas. For
example: a forged or replayed order accepted, a downgrade accepted, or a Kani
property that does not hold.

## Supported versions

Pre-1.0: only the latest commit of the default branch receives security fixes. No
release has been published yet. Pin a commit.

## Supply chain

| What | How | Where |
| --- | --- | --- |
| SBOM | CycloneDX 1.5 JSON, one per crate, firmware SBOMs for their real targets (`cargo cyclonedx --format json --spec-version 1.5`) | `evidence/supply-chain/<date>/sbom/` |
| Known vulnerabilities | `cargo audit` on the workspace and on each firmware lockfile | `evidence/supply-chain/<date>/` |
| Licences, sources, bans, advisories | `cargo deny check` with `deny.toml` (permissive licences only, crates.io only, no wildcard versions; every advisory exception documented with its reason) | `deny.toml` |
| Parser robustness | fuzzing with `cargo-fuzz` (planned, Phase 2) | — |
| Formal proofs | Kani harnesses in `oasis-rt` (CI job `kani-proofs`) | `evidence/kani/` |

Dependencies with security weight are pinned exactly and justified in their
`Cargo.toml` (for example `libcrux-ml-dsa`, `embassy-boot`, `rmodbus`, `crc`).

## Security updates

Devices receive firmware only as images whose manifest is signed by the network's
current owner (hybrid Ed25519 + ML-DSA-44). The device checks hash, hardware id and
version against a persisted anti-rollback floor, and reverts automatically if the
new image does not reach its main loop. See
`docs/specs/FIRMWARE_UPDATE_SPEC.md` and `evidence/silicon/2026-10-06/fwupdate/`.
