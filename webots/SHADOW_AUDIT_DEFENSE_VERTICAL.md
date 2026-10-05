# SHADOW AUDIT — OASIS = defense off-grid hostile-terrain

Calibrated 2026-04-23. Repositions OASIS from "generic adaptive kernel
for drones" to its actual vertical: **contested-environment unmanned
systems communications and autonomy where the network is jammed, the
nodes are physically reachable by adversaries, and there is no cloud**.

Not for: warehouse cobots, industrial IoT, multi-vendor interop
playgrounds. The code already votes for this vertical; we're just
naming it.

---

# Part A — What the code actually says

## Evidence the vertical is defense off-grid hostile

Patterns that are **load-bearing in OASIS** and **economically useless
for IoT / warehouse / cobot**:

| Pattern | In OASIS | Civilian equivalent that would be cheaper/simpler |
|---|---|---|
| ChaCha20-Poly1305 AEAD per-packet on mesh | spore v3+ | TLS channel — but needs PKI and TCP |
| X25519 ECDH forward secrecy per session | spore v4 | DTLS (again, infra-heavy) |
| Noise-KK sender auth | spore v5 | JWT + HTTPS |
| Ed25519 revocation envelopes | spore v6 | ACL update via API |
| Monotonic counter + 128-bit sliding window replay | spore v7 | HTTP nonces |
| Multi-hop Bloom-dedup flooding | spore v8+ | MQTT broker star topology |
| HMAC-SHA256-8 per mesh hop | spore v9 | none needed — trusted network |
| Ed25519 per-node mesh sig (insider-resistant) | spore v0A | none needed — trusted fleet |
| R14 entropy gate (refuse-on-uncertainty) | kernel | exception handling |
| Unsigned node = atomize < 1 ms | R20 | quarantine policy |
| Offline no_std MCU deploy | oasis-rt MCU port | cloud-connected MQTT client |
| 64 KiB Bloom, 4096-entry dedup VecDeque | mesh | 8 entries would suffice in a trusted LAN |
| Kani formal proofs on mesh header parsing | 77 proofs | tests would suffice in trusted code |

The crypto stack covers **19 of 23 threats** (per CLAUDE.md). That's
the threat model of an adversarial network — **not** warehouses or
factories where the threat model is "the USB stick" and "the bored
employee."

Additional tells in the repo:
- `sim_200_drones` — swarm sim at 200 agents, typical of tactical mesh
- `udp_loss_proxy` with Gilbert-Elliott burst loss — **radio behavior
  model**, not wired-LAN
- `bench_spore_loss`: validated at **30 % loss with 98 % success** via
  FEC + repeat=2. 30 % uniform loss is what you see under mild jamming
  or non-line-of-sight tactical radio; wired LAN loss is < 0.01 %.
- M9 reflex "fire on surprise" + kill-switch HAL + M11 federated
  attestation — this is the mental model of a **contested autonomous
  UAS**, not a factory arm.

## Evidence against the generic-robotics framing

- The A/B vs ROS 2 Jazzy (session 2 of 2026-04-22) explicitly demoted
  to "incidental measurement, not the pitch" in `OASIS_IDENTITY.md`
  after the user correction: *"oasis n'est pas un concurent de ROS2
  c'est autre chose"*.
- Zero references to URDF, MoveIt, ros2_control, rosbag, rviz, Gazebo
  in the active crate. Zero robotic-manipulation primitives.
- Zero BACnet, Modbus, OPC-UA, industrial bus protocols. Zero factory
  workflow framing.
- MAVLink v2 is the **only** industry protocol supported — and MAVLink
  is specifically for military and civilian UAS, not cobots or IoT.

## What is honestly true today, for this vertical

| Capability | Status |
|---|---|
| Adaptive bio-inspired autonomy kernel | ✅ 6/6 PROVEN mechanisms on MCU + host (as of round 9) |
| Insider-resistant mesh federation | ✅ v0A, byte-verified across 2 STM32F4 under Renode |
| Ed25519 per-envelope sig at 206 ms on M0+ | ✅ measured K=10, constant-time confirmed |
| R14 entropy gate as DoS mitigation | ✅ demonstrated firing on threshold breach |
| Loss-tolerant transport (FEC + repeat) | ✅ 98 % @ 30 % loss (N=200 trials) |
| PX4 SITL 4-waypoint mission | ✅ 1 successful run |
| Android daemon 3h23 soak on S23 FE | ✅ 121 290 ticks zero crash |
| Multi-hop TTL + Bloom dedup on real MCU | ✅ 3-STM32F4 Renode run |
| Cycle-accurate MCU instruction stream exec | ✅ Wokwi + Renode, 11 TRL-6 rounds |

## What is claimed or implied but NOT honestly true for this vertical

| Claim | Reality |
|---|---|
| "Radio transport working" | `transport.rs` has a **LoRa frame stub**, no SX127x/SX1262/nRF905 driver, no SDR integration |
| "Jamming resilience" | XOR FEC on loss = yes; **no frequency hopping, no DSSS, no anti-jam waveform** |
| "Audited crypto" | **zero external audit**. Self-written spore v3-v0A, self-tested. |
| "Anti-tamper" | kill-switch on software signal only. **No secure boot, no TEE, no fuse, no attestation chain to silicon root.** |
| "STANAG compliant" | **None.** MAVLink v2 is industry-standard but not STANAG 4586 / 4609 / 4660. |
| "Field-tested" | **One** PX4 SITL run + one Android phone soak. **Zero** controlled outdoor flights, zero third-party range testing, zero customer references. |

These 6 gaps = the 6 holes to close, in the exact order the user
called them out. Each of them is load-bearing for the defense vertical
and each one is currently waved past in pitch documents.

---

# Part B — The 6 gaps, in order

## Gap 1 — Real radio transport

### Current state

- `oasis-rt/src/transport.rs` (298 LOC) has a Transport trait and a
  **file-transport** implementation (for tests) plus a **LoRa frame
  stub**.
- Actual production transports used: UART (Renode / Wokwi demos), UDP
  (PX4 SITL, spore_recv_v2 listener), MAVLink v2 over TCP/UDP.
- Zero actual sub-GHz / VHF / UHF / SATCOM driver code.

### Gap delta

What's needed:
- **SX126x driver** (LoRa, 868/915 MHz, commodity, range ~5 km LOS at
  spreading factor 7-12, BW 125-500 kHz) — open-source Rust drivers
  exist (e.g. `sx126x-rs`). Adaptation = 1-2 weeks for a `Transport`
  impl.
- **Alternative or additional**: SiK radio driver (3DR Telemetry, 433/915
  MHz, already de-facto on tactical UAS). Open-source, firmware-level
  integration via UART is straightforward.
- **Alternative or additional**: nRF24L01+ / nRF52 BLE for very short
  range / covert swarm.
- **Aspirational**: SDR integration (LimeSDR, HackRF) — for anti-jam
  waveform R&D. Weeks to months.

### Acceptance criteria

1. One SX1262 or SiK radio connected to the RP2040 / STM32H7 via SPI
   or UART, sending a v0A SPORE envelope over the air, received by a
   second board, Ed25519 signature verifies.
2. Bench: cover a measured range (outdoor, line-of-sight). Record
   packet loss rate per distance bin.
3. Shadow audit on wire format — does our 125 B envelope fit in a LoRa
   payload window at all spreading factors? (Yes for SF7-SF10, risk
   at SF11-SF12 where max payload is 50 B — may need fragmentation.)

### Effort estimate

- **Baseline (SiK or SX126x, 1 link, fixed freq)**: 1-2 weeks.
- **Full multi-radio abstraction + runtime radio selection**: 1-2 months.
- **SDR-based anti-jam waveform R&D**: outside scope of a single round.

### TRL after closing this gap

TRL 6 on real radio wire, TRL 7 if outdoor range-tested.

---

## Gap 2 — Jamming resilience

### Current state

- **Loss tolerance yes**: bench_spore_loss shows 98 % @ 30 % uniform
  loss, 82-90 % @ 30 % Gilbert-Elliott burst loss with FEC + repeat=2.
- **That is not jamming resilience.** 30 % uniform loss = signal
  degradation. A jammer produces 100 % loss in a frequency band and
  often targeted time patterns.

### Gap delta

What jamming-resilient radio comms actually need (in roughly increasing
sophistication):

1. **Frequency hopping spread spectrum (FHSS)** — requires radio
   hardware that supports fast retuning (SX1262 can, ~1 ms per hop).
   Software: hopping schedule sync between nodes, pseudorandom sequence
   seeded from shared key.
2. **Direct-sequence spread spectrum (DSSS)** — requires wider-band
   radio (SDR typically). Harder to procure but robust to narrowband
   jamming.
3. **Low probability of intercept (LPI) / low probability of detection
   (LPD)** — requires waveform design (e.g., spreading codes, null-
   steering, power minimization). Expert work.
4. **Anti-jam modulation** — generally classified / ITAR-controlled in
   practice. Out of scope for open-source R&D.

OASIS currently has **none** of these. The FEC it has helps against
**noise**, not against **jamming**.

### Acceptance criteria

Stepped, from easiest:
- **1a.** FHSS over SX1262 with 8-channel rotation synchronized via the
  existing mesh-sig timestamp. Measured BER under a simulated
  narrowband jammer (sweep or CW). Target: < 5 % packet loss under a
  jammer that would cause 100 % loss on a fixed-freq link.
- **1b.** Demonstrate that the spore v7 replay window survives FHSS
  (should — msg_id is independent of RF channel).
- **2.** Under Gilbert-Elliott + band-jammer model, measure
  end-to-end v0A envelope success. Publish K=10 banded.

### Effort estimate

- **FHSS over SX1262 with fixed 8-channel sequence**: 2-3 weeks.
- **Adaptive hopping + jammer detection + avoid**: 1-2 months.
- **DSSS**: requires SDR procurement + classified-adjacent waveform
  work, 6+ months and partnership.

### Honest caveat

Jamming resilience is the domain where **military radios exist and
civilian prototypes struggle**. Without SDR, FHSS is the ceiling. With
SDR, most serious work is regulated. OASIS at best becomes "FHSS-ready
mesh signing layer" — the actual anti-jam is in the radio, not in the
OASIS kernel.

### TRL after closing this gap

TRL 5 on jam-resilient waveform integration (bench only), TRL 6 if
a prototype radio + OASIS survives a controlled jammer range test.

---

## Gap 3 — External crypto audit

### Current state

CLAUDE.md line:
> External crypto audit — none

Self-audited only. 77 Kani proofs cover **mesh header parsing + some
invariants**, not cryptographic properties (curve math, constant-time,
side channels are **not** covered by Kani — Kani would bit-blast and
time out).

### Gap delta

What a real external audit would cover:

1. **Formal review of the SPORE protocol itself**:
   - v3 AEAD construction: are we using ChaCha20-Poly1305 correctly
     per RFC 8439? Nonce uniqueness? AAD binding?
   - v5 Noise-KK: are we matching the reference framework? Session
     rekey? Replay window?
   - v7 monotonic counter: is the 128-bit sliding window correct under
     reorder? (spec + code review).
   - v0A Ed25519: does `sign(msg_id || origin_fp)` bind enough? Does
     the attacker ever benefit from a bound-swap?

2. **Implementation review**:
   - Is `ed25519-compact` the right choice? (Answer likely yes — it
     descends from Frank Denis's constant-time work.)
   - Is our HMAC-SHA256-8 truncation safe? (Yes per NIST SP 800-224.)
   - Are the constant-time properties preserved in our wrappers?

3. **Side-channel analysis** (if scope allows):
   - Power analysis on the M0+ during Ed25519 verify.
   - EM emanation test.
   - Cache timing attacks on the host.

4. **Threat-model stress test**:
   - Fuzzing: AFL++ or libFuzzer on `parse_envelope`, `process`,
     `mesh_v10_verify`.
   - Differential testing: compare our SHA-512 / Ed25519 outputs to
     multiple reference implementations on a large corpus.

### Acceptance criteria

- Written report from a recognized firm (Trail of Bits, NCC Group,
  Cure53, Doyensec, Least Authority, or equivalent).
- Findings classified (informational / low / medium / high / critical).
- All medium+ closed or explicitly accepted with rationale.
- Public summary with repo-linked commit hashes.

### Effort estimate

- **Scope review + quote**: 2-4 weeks elapsed.
- **Engagement duration**: typical crypto audit = 3-8 person-weeks,
  ~$60k-$180k USD for a protocol + implementation scope.
- **Remediation round**: 2-6 weeks our side.

### TRL after closing this gap

Still TRL 6 on the kernel, but the crypto becomes "audited" which is
often the gate for defense contracts. Non-audited crypto is a silent
disqualifier in formal procurement even if technically correct.

---

## Gap 4 — Anti-tampering (physical + boot)

### Current state

- `hal::KillSwitch` is a **software abstraction**. On signal, it halts
  actuation. It does not prevent the attacker from extracting firmware,
  rewriting keys, or replacing the device.
- `spore v6` revocation rejects compromised **identities**, not
  **physical devices**. Once an attacker holds your device, they hold
  the seed.
- Zero secure-boot integration. Zero TEE usage. Zero fuse-based
  rollback protection. Zero tamper-respondent enclosure references.

### Gap delta

Layered defenses in order of difficulty:

1. **Secure boot chain**:
   - RP2040 has no cryptographic secure boot by default (mask ROM
     loads from flash unconditionally). **Design loss for this vertical**
     on RP2040.
   - STM32H7 / NXP i.MX RT / Ambiq Apollo4 / Nordic nRF9160 all have
     secure-boot ROMs with signed-image enforcement.
   - **Decision point**: either pick a secure-boot-capable MCU as
     primary target, or accept "attacker gets RCE" threat.

2. **Hardware Security Element (SE) or TEE**:
   - ATECC608B / OPTIGA Trust M / NXP SE050 — $1-3 parts, hold keys
     in tamper-resistant silicon. Mesh seed never leaves SE.
   - Enables **real v0A**: attacker with physical device still can't
     sign as that node.
   - 1-2 week integration for a basic sign-via-SE pattern.

3. **Rollback protection**:
   - OTP fuse monotonic counters on STM32H7 / i.MX RT / nRF9160.
   - Prevent downgrade to pre-revocation firmware.
   - Integrates with gap 3's revocation envelopes — the device itself
     refuses to load revoked firmware versions.

4. **Tamper detection**:
   - Mesh enclosure + tamper switch + RTC-backed tamper log.
   - On tamper: wipe SE keys (SE self-destructs on command).
   - Gives "atomize" semantic (R20) physical teeth.

5. **Anti-DPA / fault injection hardening**:
   - Constant-time verify already done (we measured 0.003 % variance).
   - Glitch protection is much harder — generally requires chip
     selection (secure MCU like NXP S32K3 or STM32H5) plus
     glitch-detection countermeasures in SW.

### Acceptance criteria

- Secure-boot-capable MCU (STM32H7 or better) loads a signed OASIS
  firmware, refuses unsigned / downgraded images.
- Ed25519 signing key lives in an ATECC608B or equivalent SE, never
  appears in MCU SRAM.
- Tamper switch triggers SE-wipe + federation revocation-broadcast
  before node physically atomizes.
- Bench: replace MCU, attempt to sign a v0A envelope with the extracted
  state — **must fail** because the SE is gone.

### Effort estimate

- **Minimal (secure-boot + SE-held seed)**: 1 month.
- **Full (rollback + tamper + SE-wipe + downgrade rejection)**: 2-3 months.
- **Glitch-protection**: 6+ months, expert work.

### TRL after closing this gap

TRL 6 on secure-boot+SE. TRL 7 only after a controlled tamper-attempt
test by an independent team.

---

## Gap 5 — STANAG interoperability

### Current state

- **MAVLink v2** is the only industry protocol supported. MAVLink is
  de-facto for UAS communications but **not** a STANAG standard.
- Zero STANAG **4586** (UAS Control System), 4609 (motion imagery),
  4660 (Interoperable Command and Control Data Link), 5516 (Link-16)
  references in the codebase.
- For allied defense procurement, STANAG interop is often mandatory —
  a UAS that doesn't speak 4586 is **disqualified** in NATO tenders
  regardless of technical merit.

### Gap delta

Ordered by procurement relevance:

1. **STANAG 4586 VSM** (Vehicle Specific Module):
   - The protocol UAS ground control stations speak to UAVs.
   - Written in ICDs, very heavy XML-based descriptors.
   - Open-source implementations are rare; most implementations are
     defense-prime-internal.
   - Effort: 2-4 months for a baseline VSM.

2. **STANAG 4609** (Motion Imagery):
   - KLV-encoded metadata in MPEG-TS. Geo-referenced video.
   - Relevant if OASIS is to handle ISR payloads.
   - Effort: 1-2 months for a basic KLV encoder/decoder.

3. **STANAG 4660** (Interoperable C2 Data Link):
   - Specifically for UAS C2 over tactical data links.
   - Complements 4586.
   - Effort: similar to 4586, often paired.

4. **STANAG 5516 / Link-16**:
   - Classified waveform (TSEC/KY-58 or KY-100), controlled distribution.
   - Out of scope for open-source. Requires classified partnership.

### Acceptance criteria (for the realistic subset)

- OASIS can emit and parse STANAG 4586 VSM messages over the existing
  mesh transport.
- Interop demo with an open-source GCS that speaks 4586 (e.g.
  OpenGCS if such a thing exists — otherwise requires a partnership
  with a vendor who has a 4586-compliant GCS).
- A formal compliance matrix per STANAG 4586 message type.

### Effort estimate

- **4586 VSM baseline**: 2-4 months per compliance level.
- **4609 KLV for ISR**: 1-2 months.
- **Full NATO UAS integration**: years, requires vendor partnerships.

### Realistic positioning

OASIS itself doesn't need to **be** a STANAG 4586 GCS. It can be the
**autonomy + mesh signing layer** that sits below the STANAG protocol
adapter. Framing:

> OASIS provides adaptive autonomy and insider-resistant mesh signing
> for UAS; STANAG 4586 adapters plug into the transport layer.

That framing lets us say "STANAG-ready" without having implemented
4586 ourselves — we provide the transport substrate, a certified
STANAG vendor provides the protocol. Partnership path, not build.

### TRL after closing this gap

Not TRL-gated; this is a **market-access gap**, not a technical one.
TRL stays where it is; procurement eligibility changes.

---

## Gap 6 — Field references

### Current state

- **1** PX4 SITL mission: 4 waypoints, altitude 1.94 m vs 2.0 m target.
  One successful run, logged, documented.
- **1** Android daemon soak: 3h23 on Samsung S23 FE, 121 290 ticks,
  zero crash. Single device, single session.
- **Zero** outdoor flights. Zero third-party witness. Zero customer
  deployment. Zero letter of intent. Zero pre-order.

For a defense vertical this is essentially "lab prototype" stage. The
DoD / equivalent European procurement SDK (NCIA, DGA, BWB, etc.) wants:
- Controlled range test with independent observers.
- Unclassified field report.
- Service pilot (SOF unit, coast guard, border force, civil protection
  agency willing to trial).
- Failure mode documentation from real deployment.

### Gap delta

Stepped, from cheapest:

1. **Controlled outdoor flight** — commodity quadcopter with OASIS on
   the MCU, simple mesh demo in a field, single operator, video
   record. "Look, it works outdoors." Effort: 2-4 weeks (hardware
   procurement, permits, weather).

2. **Multi-vehicle outdoor flight** — 3-5 UAVs, mesh routing observed
   during real flight, including a deliberate link degradation test
   (move one vehicle behind an obstacle). Effort: 2-3 months.

3. **Jamming test** — controlled EW range session with a cooperative
   jammer. Most countries have ranges available to researchers with
   permits. Effort: 3-6 months including permit + scheduling.

4. **Operator trial** — find a unit willing to spend a week with the
   system in representative-mission conditions. This is the gate to
   defense sales. Effort: 6-12 months + relationships.

5. **Customer reference** — even one agency willing to say publicly
   "we used this in mission X" transforms the conversation. Rare in
   the first 2 years of any defense startup.

### Acceptance criteria

- A written field report (unclassified), linking to this repo at a
  specific commit hash.
- Independent witness (military or civilian agency observer) present at
  the test.
- At least one unplanned failure mode documented honestly.

### Effort estimate

- First outdoor flight: 1 month (incl. hardware + permits).
- First multi-vehicle: 3-6 months.
- First jamming test: 6 months.
- First operator trial: 12+ months.

### TRL after closing this gap

The one gap that moves the whole project from **TRL 6** (sim-validated)
to **TRL 7-8** (real environment). None of the other 5 gaps can
substitute for this one — audits, STANAG compliance, and even
hardened radios are necessary but not sufficient.

---

# Part C — Shadow audit AFTER (if all 6 gaps closed)

## What "credible for defense off-grid hostile UAS" would look like

| Dimension | Today | After all 6 gaps closed |
|---|---|---|
| Radio transport | UART / UDP / MAVLink sim | SX1262 / SiK / FHSS-capable, range-tested |
| Jamming resistance | Loss tolerance only | FHSS, DSSS-ready, measured under jammer |
| Crypto | Self-audited, 77 Kani proofs | 3rd-party audited (TrB / NCC / Cure53), public report |
| Anti-tamper | Software KillSwitch | Secure-boot MCU + SE-held seed + tamper-wipe |
| Interop | MAVLink v2 only | MAVLink + STANAG 4586 via adapter |
| Field validation | PX4 SITL + phone soak | Multi-vehicle outdoor + jamming range + 1 operator trial |
| TRL | 6 (sim) | 7 (relevant environment) |
| Procurement eligibility | disqualified (no STANAG, no audit) | qualified for R&D contracts, possibly for trial procurement |

## Honest sequencing

The 6 gaps have very different scales:

| Gap | Effort | Who can close it |
|---|---|---|
| 1. Radio transport | 1-2 weeks | solo engineer |
| 2. FHSS jamming | 2-3 weeks | solo engineer |
| 3. Crypto audit | 3-8 person-weeks + $60-180k | audit firm |
| 4. Anti-tampering | 1-3 months | solo + hardware partner |
| 5. STANAG 4586 | 2-4 months OR a vendor partnership | team + partnership |
| 6. Field references | 3-12 months + relationships | operator + unit + permits |

**Gaps 1, 2, 4 are tractable by a small team**. Gaps 3 is a money
problem. Gap 5 is a partnership problem. Gap 6 is a relationships
problem.

The cheapest immediate credibility gain is **gap 1 + gap 2 combined**:
FHSS over a real SX1262 radio with an OASIS v0A envelope surviving a
controlled narrowband jammer. That's 4-6 weeks total, produces a video
that answers the "does this actually survive jamming?" question, and
costs only hardware (~$300 for 2 nodes + a jammer signal source).

## What we have to STOP claiming until these close

From CLAUDE.md and related docs:

- **"Radio-ready"** — not true. Remove or qualify ("LoRa-protocol
  compatible at wire format level, hardware driver pending").
- **"Anti-jam"** — not true. Replace with "loss-tolerant" (true).
- **"Production-grade"** — never written, but the architecture doc
  tone occasionally implies it. Downgrade to "pre-production prototype".
- **"Audited"** — never written, but internal confidence in crypto is
  high. State clearly "no external audit to date".
- **"Field-validated"** — replace with "PX4 SITL + 3h23 Android soak".
  Keep honest, add the obvious gap.
- **"Defense-ready"** — never written; don't start. Replace with
  "defense-oriented architecture, pre-certification".

## What we have EARNED the right to claim

After today's rounds 1-9:

- **Kernel executes on Cortex-M0+ instruction stream** (Wokwi +
  Renode, 11 rounds, 31/31 predictions matched).
- **6/6 PROVEN bio-mechanisms run on MCU** with byte-exact outputs.
- **Insider-resistant mesh signing** (v0A Ed25519) proven across 2 and
  3 separate MCU instances.
- **R14 entropy gate** demonstrated as DoS mitigation with 0 measurable
  latency on hot path.
- **Ed25519 constant-time on M0+** verified empirically (< 0.003 %
  variance accept-vs-reject).
- **428 host tests + 77 Kani proofs + 9 cross-check tests** passing
  post every structural change.

That's a real engineering story. It's just not a defense-procurement
story **yet**.

---

# Part D — Recommendation

## The honest pitch after today

"OASIS is an adaptive autonomy + insider-resistant mesh-signing kernel
for unmanned systems in contested environments. Compiled for
Cortex-M0+ and STM32F4. Six known gaps to defense-grade: radio
drivers, anti-jam, external audit, anti-tampering, STANAG adapter,
field references. Engineering-tractable path for the first two; the
rest need funding or partnerships."

Not "market-ready". Not "production". Honest about what's real and
what's ahead. That framing is **more credible** to defense buyers than
overclaiming, because the ones who matter have seen every pitch deck
and they pattern-match pitches that overclaim as amateur.

## Next engineering round (if continuing in this direction)

**Gap 1 opened** = biggest credibility jump per hour of work:
- Procure 2× SX1262 LoRa HAT for RP2040 or similar
- Write the `Transport` trait impl (`oasis-rt/src/transport.rs` already
  has the shape, just needs a driver)
- First flight: send a v0A envelope over real RF, receive on the
  other board, verify signature, print result
- Log packet-loss distance curve

That's 1-2 weeks of focused work, minimal hardware cost, and it
transforms the claim "OASIS can run on MCU" into "OASIS can run on
MCU and talk over real radio". The difference matters for anyone who
evaluates this for the intended vertical.

Everything else — audit, STANAG, tamper, field — is downstream of
"yes it really works on a radio."
