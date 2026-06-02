# Grid use cases — Elia / HEDGE-IoT fit assessment

**Audience**: grid-ops architect, HEDGE-IoT principal investigator, or
R&D manager at a TSO/DSO evaluating OASIS for a pilot project.

Three concrete scenarios, each with: what the scenario is, what OASIS
can credibly do today, what it cannot, and rough cost/time to a pilot.

---

## Scenario 1 — Offshore wind-farm sensor mesh

### The problem

A 1 GW offshore wind farm has 100-200 turbines, each with local
instrumentation (vibration, temperature, nacelle orientation, blade
strain). Data must reach an onshore SCADA through the wind-farm's
collector network + export cable. Two issues:

1. **Connectivity is lossy** — RF link between turbine and onshore
   fiber head-end is shared, occasionally jammed by marine radar,
   degraded by weather. Current solution: cellular backup (expensive,
   congested).
2. **Insider threat is under-addressed** — if one turbine's edge
   device is compromised (physical access during maintenance is
   routine), the attacker can inject false readings to the SCADA,
   triggering bad DMS decisions.

### What OASIS can do today

| Requirement | OASIS fit |
|---|---|
| Per-turbine signed telemetry, insider-resistant | ✅ v0A Ed25519 per-envelope sig |
| Loss-tolerant transport (30 % loss → 98 % delivery) | ✅ FEC + repeat, measured |
| Narrowband jamming resilience | ✅ FHSS sim-verified (Gap 2 done) |
| Replay attack rejection | ✅ Bloom dedup + 128-bit replay window |
| Runs on edge MCU (turbine gateway) | ✅ Cortex-M0+/M4F, 278 KB footprint |
| No cloud dep | ✅ by construction |

### What OASIS cannot do today

| Requirement | Gap |
|---|---|
| LoRaWAN-compliant (for shared ISM network) | OASIS is P2P mesh, not LoRaWAN server-mediated |
| IEC 61400-25 (wind-specific SCADA) compliance | Outside scope; OASIS carries IEC 61400-25 messages as payload |
| Certified to IEC 62443 SL-3 (industrial cybersec) | No certification; deployable in pre-production / pilot only |
| External cryptographic audit | Not done yet (see SHADOW_AUDIT.md) |

### Realistic pilot setup

- 4-8 turbines × OASIS on their existing edge gateway Linux box
- 1 onshore concentrator
- 4-week soak test + 2 injected-adversary test sessions
- Output: 4-8 weeks of mesh telemetry + tamper-resistance data
- Budget: ~€40-80k incl. 10-15 engineer-days integration

### Honest fit score

**7/10** — direct match with the core mesh/insider-resistance story.
The main gap is certification; if the wind-farm owner is willing to
treat this as "R&D pilot on a non-critical sub-array," this is the
cleanest use case we've identified.

---

## Scenario 2 — Substation cyber-resilience layer

### The problem

A modern HV/MV substation has:
- PLCs / IEDs running IEC 61850 (GOOSE multicast over the station bus)
- Remote-terminal units connecting to the central SCADA
- Maintenance-access devices (laptops, HMI panels) periodically connecting

Post-2015 (Ukraine substation attacks) and post-2022 (nation-state
threats), **station-bus integrity** is a known weak point:
- GOOSE multicast has no authentication by default
- IEC 62351 (security addendum) extends 61850 but adoption is slow and
  vendor-locked

HEDGE-IoT and similar Horizon programs have been exploring
lightweight, vendor-neutral authentication that can be layered on.

### What OASIS can do today

| Requirement | OASIS fit |
|---|---|
| Vendor-neutral per-message authentication | ✅ v0A Ed25519 covers generic byte payload |
| Low-latency verification | ✅ ~141 µs host, ~206 ms MCU (fine for non-tripping messages) |
| Multi-hop mesh for RTU aggregation | ✅ TTL + Bloom dedup tested |
| Insider-resistant (against captured IED) | ✅ core design property |
| Runs on RTU-class MCU | ✅ |
| Backwards-compatible with IEC 61850 payload | ✅ (carries the MMS/GOOSE bytes opaquely) |

### What OASIS cannot do today

| Requirement | Gap |
|---|---|
| IEC 61850 / IEC 62351 certification | Not carried by OASIS; integrator's responsibility |
| GOOSE tripping-message latency (< 4 ms) | **Ed25519 verify is too slow for hard-real-time trip.** Use v9 HMAC (~0.5 µs) for those; reserve v0A for non-trip metadata |
| Integration with existing vendor PLCs (GE MU320, Siemens SIPROTEC) | Requires vendor SDK access or sniffer approach; case-by-case |
| Post-quantum migration | Not yet |

### Realistic pilot setup

- 1 substation (digital-twin or isolated lab bay)
- 3-5 simulated IEDs + 1 RTU + 1 SCADA concentrator
- OASIS layer sits between the station bus and the RTU
- Inject: tamper attack, spoofed GOOSE, replay of historical fault
  recording
- 6-8 week pilot, demo to auditor / supervisor
- Budget: ~€60-120k incl. 15-25 engineer-days

### Honest fit score

**6/10** — the core tech matches, but real-time constraints on GOOSE
tripping push OASIS's public-key crypto out of the hard-real-time
path. Hybrid v9-for-trip + v0A-for-metadata is the honest architecture;
see if your use case tolerates that split.

---

## Scenario 3 — Distributed Energy Resources (DER) edge coordination

### The problem

HEDGE-IoT's architectural thesis (if I read the Horizon calls correctly):
**more decision-making pushed to edge nodes** as DERs proliferate. PV
inverters, battery storage, EV chargers, demand-response aggregators
each need to coordinate locally (when the cloud is down) AND prove to
the DSO that they're behaving per contract.

Two issues:
1. **Peer-to-peer trust** — an EV charger and a PV inverter from
   different vendors need to agree on a local grid-balancing decision
   without a trusted central broker.
2. **DSO-side attestation** — the DSO receiving aggregated data needs
   to verify which edge node signed which commitment, without
   certificate rotation nightmares.

### What OASIS can do today

| Requirement | OASIS fit |
|---|---|
| Lightweight per-node identity (Ed25519 seed + pubkey) | ✅ |
| Attested broadcast (every commitment signed) | ✅ v0A envelope design |
| Peer-to-peer trust without central broker | ✅ by design (each node validates others via local registry) |
| Revocation for compromised devices | ✅ spore v6 envelopes |
| Runs on commodity DER controller hardware | ✅ Cortex-M4F-class is typical, Raspberry Pi also fine |
| Disconnected operation | ✅ no cloud dep |

### What OASIS cannot do today

| Requirement | Gap |
|---|---|
| IEEE 2030.5 (SEP 2.0) compatibility | Outside scope; OASIS transports the payload |
| OpenADR 3.0 integration | Same |
| Formal transactive energy market primitives | OASIS is the signed mesh; the market logic sits above |
| Key-issuance at deployment scale (100k+ devices) | Primitives exist; tooling for mass provisioning not packaged |

### Realistic pilot setup

- 5-10 simulated DERs (PV, EV charger, battery)
- 1 DSO-side aggregator
- 1 adversary simulator (compromised DER, spoofed commitment)
- 4-6 week pilot validating: peer-to-peer attested signaling, DSO-side
  verification, compromise response
- Budget: ~€50-100k

### Honest fit score

**8/10** — this is arguably the BEST fit of the three scenarios. DER
edge coordination has all the right properties: no cloud, peer trust,
cryptographic attestation, embedded-class hardware. HEDGE-IoT's
decentralized edge-computing framing aligns closely with OASIS's core
model.

---

## Cross-scenario summary

| Scenario | Fit | Cost to pilot | Biggest gap |
|---|---:|---|---|
| Offshore wind sensor mesh | 7/10 | €40-80k | Certification (IEC 62443) |
| Substation cyber-resilience | 6/10 | €60-120k | Real-time trip latency |
| DER edge coordination | **8/10** | **€50-100k** | Mass-provisioning tooling |

## Where HEDGE-IoT's call text points

If the call emphasizes:
- **"IoT-edge security"** → scenarios 1 and 3
- **"Decentralized energy markets"** → scenario 3
- **"Smart substations"** → scenario 2
- **"Supply-chain and insider threat"** → all three, especially 1 & 2

## Where Elia specifically is likely to be interested

Elia's public R&D priorities (as of 2026 public materials):
- **Offshore grid** (Princess Elisabeth Island, Nautilus interconnect)
- **System integrity** with increasing renewables share
- **Digital substation** roll-out
- **Cross-border data exchange** (needs authenticated telemetry)

Scenarios 1 (offshore wind sensor mesh) and 2 (substation bus
integrity) map directly. Scenario 3 is more DSO-side than TSO-side
so less direct for Elia, but relevant if they're looking at
transactive energy at the interconnect level.

---

## Recommendation for the 1-hour evaluation

1. Read [capabilities.md](capabilities.md) (10 min) — confirm what's
   real vs what's aspirational.
2. Read [SHADOW_AUDIT.md](SHADOW_AUDIT.md) (10 min) — understand the
   6 gaps (radio, jamming, audit, anti-tampering, STANAG, field refs)
   before deciding.
3. Read [architecture.md](architecture.md) (15 min) — see where OASIS
   slots into your stack.
4. Pick ONE of the three scenarios above that most closely matches
   your 12-month R&D priority (25 min).
5. Ask for a 30-min screen-share with the OASIS team on that scenario
   specifically — expect an honest "here's what we can credibly do
   for this in 3-6 months" conversation.

The OASIS team is deliberately **not promising to solve all three
scenarios**. The shadow audit discipline means we name what we can and
can't do per scenario, rather than pitching a one-size-fits-all.

---

**Contact**: Souhayb Rharrab — souhaybrharrab@gmail.com — +32 486 32 64 46
