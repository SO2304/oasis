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

## Support period

The Cyber Resilience Act sets, for a product with digital elements placed on the market,
a **support period** during which vulnerabilities are handled — at least **5 years** of
assistance, with security updates remaining **available for 10 years** after the last
update is issued (Regulation (EU) 2024/2847; see
[`docs/compliance/CRA.md`](docs/compliance/CRA.md) for the verbatim articles and dates).

**Today this project owes no support period, and says so rather than implying one.**
Verified at the source during phase 0: a free and open-source component that is **not
monetised** is outside the CRA's scope (recitals 18 and 20), and Article 13(5) places the
due-diligence duty on the integrator who ships it in a product. OASIS is MIT-licensed,
pre-1.0, unreleased, and sold to nobody.

What changes the day it is monetised, stated now so that no buyer discovers it later:

| | Obligation | Can a single maintainer meet it? |
|---|---|---|
| Assistance | 5 years minimum | **Not alone.** This is the honest answer to `POSITIONING_GAPS.md` **D5** and **D9** |
| Update availability | 10 years after the last update | Needs an organisation, not a person |
| Vulnerability reporting | 24 h early warning, 72 h notification to ENISA and the CSIRT | Process exists above; the legal entity does not |

So the position is: **the evidence and the code are available under MIT; a support
commitment is not, and will not be until there is an entity able to carry it.** An
integrator who needs a supported component needs a supplier, and that is a commercial
question before it is a technical one.

## Supply chain

| What | How | Where |
| --- | --- | --- |
| SBOM | CycloneDX 1.5 JSON, one per crate, firmware SBOMs for their real targets (`cargo cyclonedx --format json --spec-version 1.5`) | `evidence/supply-chain/<date>/sbom/` |
| Known vulnerabilities | `cargo audit` on the workspace and on each firmware lockfile | `evidence/supply-chain/<date>/` |
| Licences, sources, bans, advisories | `cargo deny check` with `deny.toml` (permissive licences only, crates.io only, no wildcard versions; every advisory exception documented with its reason) | `deny.toml` |
| Parser robustness | `cargo-fuzz` over **26 parsing and checking entry points in 11 targets** (counted from the tree, not tallied: `grep -ohE "parse_[a-z_0-9]+\|check_[a-z_0-9]+\|read_frame\|modbus_read_request\|from_text" oasis-rt/fuzz/fuzz_targets/*.rs \| sort -u \| wc -l`, and `tools/check_claims.sh` now compares it so it cannot drift; this line said "13 parsers in 9 targets", itself an undercount). Nine (v0B, ORV1, OAC1, MAVLink, authority, fragment, enrollment, firmware, Modbus RTU): **~1.5 × 10⁹ executions** at 3 600 s per target, one real finding — a non-canonical `OAC1` encoding, where the reserved bytes were not required to be zero, fixed in `f26195b`. Four with the TCP read path (`OMQ1`, `OMV1`, the HMI read frame, the device's answer): **2.23 × 10⁷ executions, 0 crashes**. Five more on 2026-10-09 at 240 s per target, **2.32 × 10⁷ executions, 0 crashes** (`evidence/fuzz/2026-10-09/`): `parse_tcp_write` and `check_tcp_response` — **the TCP path's front door, which no target had ever reached**, because the existing `modbus` target covers `modbus_gateway::check_response`, the *RTU* checker over an RTU frame — plus the length-prefixed link framing (`read_frame`, `modbus_read_request`, made generic over `Read` so a target can drive them at all) and `Config::from_text`. The totals are kept apart on purpose: they did not contribute to each other, the newest ran **15× shorter**, and averaging them would hide that the TCP path is the least-fuzzed in the repo. ⚠️ `ORV1` over TCP is **not** new here: `parse_orv1` has been fuzzed since 2026-10-07; what was missing was the framing that carries it | `evidence/fuzz/2026-10-07/`, `…/2026-10-08/`, `…/2026-10-09/` |
| Formal proofs | Kani harnesses in `oasis-rt` (CI job `kani-proofs`) | `evidence/kani/` |

Dependencies with security weight are pinned exactly and justified in their
`Cargo.toml` (for example `libcrux-ml-dsa`, `embassy-boot`, `rmodbus`, `crc`).

## Security updates

Devices receive firmware only as images whose manifest is signed by the network's
current owner (hybrid Ed25519 + ML-DSA-44). The device checks hash, hardware id and
version against a persisted anti-rollback floor, and reverts automatically if the
new image does not reach its main loop. See
`docs/specs/FIRMWARE_UPDATE_SPEC.md` and `evidence/silicon/2026-10-06/fwupdate/`.
