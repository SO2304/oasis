# SHADOW AUDIT — OASIS product-fit

Calibrated 2026-04-23. Hard-eye review of who buys this, against what,
at what price, and where it collapses. No deck speak. Same honesty
discipline as the technical shadow audits.

Core question — **is OASIS a product, a component, or a research artifact?**

---

## Part A — What we have to sell, plainly stated

After 2 days of TRL-6 execution (13 rounds, 4 gaps touched of 6 in the
defense-vertical plan), OASIS as of commit HEAD is:

> A 278 KB Rust kernel running on Cortex-M0+/M4F/M7 that combines
> (a) an insider-resistant Ed25519-per-envelope mesh signing layer,
> (b) 6 bio-inspired autonomy mechanisms proven byte-exact
> host↔MCU, (c) an anti-narrowband-jam FHSS simulation layer, and
> (d) a stub for real SX1262 radio integration.

What that list **actually** means in customer-language:

| Technical asset | What a buyer hears |
|---|---|
| Ed25519 per-envelope mesh sig | "If one node is captured, the attacker still can't spoof the others" |
| R14 entropy gate, 0 µs latency | "The drone refuses to act when sensor input is suspicious" |
| 6 mechanisms on M0+ with byte-exact host match | "Runs on $3 MCU, behaves like our desktop sim" |
| FHSS 90 % delivery under narrowband jam | "Survives a cheap jammer" |
| mesh_v10 on 3 STM32F4 via Renode | "Works across 3 separate MCU boards" |
| MAVLink v2 support | "Drops into PX4/ArduPilot pipelines" |
| No cloud dep, no_std ready | "No IT department required to deploy" |

What we **cannot** say yet, honestly:
- Runs on real radio (Gap 1 hardware ⏳)
- Externally audited (Gap 3 ⏳)
- Survives physical tampering (Gap 4 ⏳)
- Interoperates with STANAG C2 (Gap 5 ⏳)
- Tested in field (Gap 6 ⏳)

## Part B — Customer archetypes (specific orgs, not buzzwords)

### Archetype 1 — Small/tactical UAS swarm developer

**Who**: teams of 2-15 engineers at outfits like Quantum Systems,
Helsing, Tekever, Delair, Auterion Defense, or inside SOF units
(JSOC, French COS, German KSK, UK UKSF) that have R&D budgets and
build custom swarm firmware.

**Pain**: they currently roll their own mesh crypto (ChaCha20-Poly1305
+ maybe HMAC) or use nothing. MAVLink signing is point-to-point only.
No one in the open-source ecosystem offers **per-envelope mesh-layer
Ed25519** that runs on Cortex-M0+. They cobble together `ed25519-dalek`
+ `smoltcp` + their own protocol, badly.

**Budget profile**: license a kernel for €20k-€100k per deployment, OR
pay for integration support at €800-€1500/day. NOT €1M+ scope.

**What they'd actually pay for**:
- Drop-in Rust crate that compiles clean for their target MCU
- Audited crypto (Gap 3) — because their customer's customer asks
- Integration support — 5-10 days on their codebase
- A reference radio board (Gap 1 hardware)

**Deal size estimate**: €30k-€150k license + €20k-€80k integration = **€50k-€230k first deal**.

**Win rate estimate**: 1-in-5 of contacted leads → 20-40 qualified leads
per year = 4-8 deals = €200k-€1.8M ARR possible at maturity.

**Why we might win**: **nothing else in the open OSS space offers what
OASIS offers at this layer.** The proprietary alternatives are
closed-source and cost 10-50× more.

**Why we might lose**: integration pain, no STANAG, no field refs,
"who are you" question, procurement cycles.

### Archetype 2 — Defense prime tier-2 integrator

**Who**: companies like Hensoldt, Rheinmetall Electronics, Elbit
UAS-division, Leonardo DRS, Israel Aerospace Industries, Thales DAS.
They sell complete UAS systems to governments and need component
technology to differentiate.

**Pain**: they DON'T want to build a mesh crypto stack — they want to
*integrate* one that has a certification path and won't embarrass
them. Their in-house teams are busy on radar, optics, propulsion.

**Budget profile**: buy component IP at €500k-€5M, multi-year support
contracts €200k-€1M/year.

**What they'd actually pay for**:
- Long-term support commitment (5-10 years, standard defense term)
- Audit report they can show their government customer (Gap 3)
- STANAG 4586 adapter OR a clear path to it (Gap 5)
- IP indemnification + export-control compliance

**Deal size estimate**: **€500k-€3M per design win**, very rare (1-3 per
year if you're well-connected).

**Win rate estimate**: extremely low without existing relationships.
Defense primes don't buy from unknowns; they buy from known integrators
or invest directly.

**Why we might win**: only if we partner with an introducer they trust.
Cold outreach = 0 %.

**Why we might lose**: everything above + no existing customer case
study + no vendor relationships + no SEC-cleared personnel.

### Archetype 3 — Research / academic lab

**Who**: DARPA-funded university labs (MIT Lincoln, CMU RI, Stanford,
TU Delft, LAAS-CNRS, Fraunhofer FKIE), European Horizon projects,
national defense R&D (DGA, BAAINBw, NCIA).

**Pain**: they want a kernel they can publish papers against, that
won't break between Rust releases, that they can modify.

**Budget profile**: typically **€0 for licenses** — open source
expected. Hardware + contractor support at €50k-€300k per project.

**What they'd actually pay for**:
- Custom feature development (mechanism M12-M15 for their paper)
- Integration with their specific radio/sensor platform
- Joint publications — often counts more than cash

**Deal size estimate**: **€30k-€150k per project**, 3-8 projects per year.

**Win rate estimate**: high IF the code is published openly and cited
a few times. Low otherwise — academia doesn't adopt closed-source.

**Why we might win**: OASIS-as-OSS could become the "ROS 2 of tactical
mesh" if it's the cleanest code in the space. Research labs amplify.

**Why we might lose**: if we go closed-source / proprietary, we lose
this channel entirely.

### Archetype 4 — Border / coast guard / civil protection

**Who**: Frontex, French Gendarmerie Aérienne, Italian Guardia Costiera,
US Coast Guard R&D. Civilian-ish, less procurement-rigid.

**Pain**: deploy UAS for search/rescue, illegal-crossing detection,
pollution monitoring, persistent surveillance. Budget from civilian
side but tactical requirements.

**Budget profile**: €100k-€500k per deployment, 2-5 deployments/year
per agency.

**What they'd actually pay for**:
- Legal-interoperable mesh (RNRS in France, etc.) — NOT STANAG
- Jamming resilience (agriculture RF noise, GNSS jamming near ports)
- Secure command chain from operator to drone
- "Works in areas with no cellular coverage" — off-grid by default

**Deal size estimate**: **€150k-€400k per deployment**, via integrator.

**Win rate estimate**: moderate. Less procurement-rigid than defense
primes, but still want references.

**Why we might win**: OASIS's off-grid + mesh + no-cloud model fits
exactly. Civilian tolerance for "pre-production" is higher than NATO.

**Why we might lose**: still competing with Persistent Systems MPU5
mesh radios + whatever autonomy they glue on top.

---

## Part C — Competitor map

Who else claims to do this, and where they leave gaps:

| Competitor | What they are | Where they win | Where they leave room |
|---|---|---|---|
| **Silvus StreamCaster** / Persistent Wave Relay | Military mesh RADIOS (RF hardware) | Proven, certified, expensive ($5k-$20k per node) | No autonomy layer, no byte-sig, closed firmware |
| **Anduril Lattice / Ghost** | Full vertical stack (drones + AI + mesh) | Huge, vertically integrated, $billions in funding | $100k+ per node, closed, won't sell to Europeans |
| **Shield AI Hivemind** | Swarm autonomy SW licensed to integrators | Production, USN-used | Closed, priced for primes not startups |
| **Swarmer** (Ukraine, ex-Ring) | Battlefield swarm SW | Combat-proven 2024-2025 | Ukraine-focused, small company, closed |
| **PX4 / ArduPilot** | Open-source autopilots | Free, huge community, de-facto | Autopilot not autonomy, no insider-resistant mesh |
| **ROS 2 + DDS** | Generic robotics middleware | Dominant in academia | Wrong threat model (trusted network), heavy |
| **Meshtastic** | Hobbyist LoRa mesh | Free, runs on ESP32, huge community | No crypto worth mentioning, no autonomy |
| **Betaflight / INAV** | FPV flight controllers | Hobbyist standard | No mesh, no crypto |
| **Armada / Saker AI (EU startups)** | Defense swarm autonomy | EU-sovereign alternative | Early-stage same as us |

**Where OASIS slots in honestly**:

- Cheaper than Silvus/Persistent (software layer vs their radio hardware).
- Less ambitious than Anduril/Shield AI (component vs full stack).
- More serious than Meshtastic (byte-sig crypto vs hobbyist PSK).
- Different layer than PX4 (autonomy+mesh on top of PX4, not
  replacing it).
- Comparable peer to Armada / Saker AI (early-stage EU defense SW),
  with a more-developed crypto story but smaller autonomy demos.

**The honest positioning one-liner**:

> "OASIS is the insider-resistant mesh-signing + bio-adaptive autonomy
> kernel that runs on $3 MCU and plugs into PX4. Meshtastic-grade
> hardware cost with defense-grade crypto and sub-µs safety gating."

Not huge. But real.

---

## Part D — The killer use case

Of all the archetypes above, ONE has the cleanest narrative fit with
what we've built TODAY:

### **Archetype 1 (small tactical UAS developer) buying the kernel for their existing radio stack**

Why this one wins:
1. They **already have their own radio**. We don't need to ship
   hardware. Gap 1 hardware becomes lower-priority.
2. They have **their own autopilot** (PX4). We slot in above MAVLink
   signing, below their mission logic.
3. They have **small budgets but fast purchase cycles** — no
    18-month STANAG qualification.
4. Their customer is often an R&D unit or SOF — **tolerates
   pre-certification prototypes** with real-world evidence.
5. Their pain is **real and unmet by alternatives** — insider-resistant
   per-envelope mesh sig at the MCU layer doesn't exist in the open
   OSS world.

**What the deal looks like**:

- They license the `oasis-rt` crate + `oasis-lora-transport` + future
  `oasis-sx1262-driver` for a specific target MCU.
- License = **perpetual per-fleet** at €40k-€120k depending on fleet
  size (tier by node count).
- Plus **7-14 days of integration support** at €1500/day = €10k-€20k.
- Plus **annual maintenance** at 20 % of license = €8k-€24k/year.

**First-deal value**: €60k-€160k. Yearly recurring: €8k-€24k.

**Who to contact first** (specific, not hypothetical):
- Tekever (Portugal) — autonomous maritime UAS, small team, EU-positioned.
- Quantum Systems (Germany) — Vector / Trinity UAS, has R&D appetite.
- Auterion Defense (Switzerland/US) — already PX4-aligned.
- Helsing — bigger but Europe-native, buys components.
- Armada Aerospace / Saker AI — fellow EU startups, partnership
  rather than competition.

**Outreach message** (sketch):

> "You're building tactical UAS on PX4. You ship mesh firmware today
> that does point-to-point MAVLink signing. One of your nodes gets
> captured — how do you prevent the attacker from impersonating it
> to the rest of your fleet? Right now you either accept the risk, or
> you roll your own per-hop crypto (badly). OASIS is a Rust crate
> that gives you that: 125 B Ed25519-signed envelopes at the mesh
> layer, no trusted infrastructure, runs on your Cortex-M0+. We've
> proven it byte-exact across 3 STM32F4 boards under Renode. Want a
> 30-min walkthrough?"

This is the pitch we can honestly send **today**. Not to Lockheed.
Not to DoD. To the 15-person defense-UAS startups who are building
the thing and hit exactly this wall.

---

## Part E — Go-to-market (realistic paths)

### Path 1 — "OSS + consulting", 0-24 months

- Publish `oasis-rt` + `oasis-lora-transport` under permissive license
  (Apache-2.0 or MIT).
- Blog the shadow audits — they're already honest and technical.
- Wait for inbound from Archetype 1 and Archetype 3.
- First deal = €20k-€50k consulting to integrate.
- This is the **only path that works without outside capital**.

Expected outcome year 1: 2-4 paid integrations, €40k-€200k revenue.
Enough to continue. Not enough for a real company.

### Path 2 — "Component supplier to one tier-2 integrator", 6-18 months

- Identify ONE integrator (from the Tekever / Quantum / Helsing list).
- Approach via a warm intro (board-level, angel, former
  defense-ministry contact).
- Pitch: exclusive or semi-exclusive kernel integration for their
  swarm product line. **They brand it, we supply.**
- Term: €200k-€500k dev contract + perpetual license + royalty.
- Expected close: 9-12 months from first meeting.

Expected outcome year 1-2: one design win = €300k-€700k.
Multi-year upside depends on their product success.

### Path 3 — "Raise + build the full product", 18-36 months

- Close seed round €1.5M-€4M.
- Hire 3-5 engineers (radio, RF, embedded, integration, biz).
- Buy hardware, close Gap 1 (real radio), Gap 2 round 3 (bench jammer).
- Get Gap 3 (audit) — €80k-€150k from audit firm.
- Attempt first STANAG 4586 adapter (Gap 5 partial).
- Deliver first field reference (Gap 6 step 1).
- Pitch tier-1 primes and SOF units with portfolio.

Expected outcome year 3: series A round €10M-€20M on design wins.

**Risk**: dilution, defense-VC scarcity in Europe, typical 40-60 %
failure rate at seed→A, lots of closed-room meetings before anything
lands.

### Path 4 — "Don't commercialize — stay a research artifact", ongoing

- Publish papers, collaborate with labs, let industry fork.
- Build reputation but no revenue.
- Becomes the "LuaJIT" or "OpenSSL" of tactical mesh — widely used,
  never paid.
- Pays the bills via adjacent consulting / day-rate work.

Expected outcome: ~€60k-€120k/year consulting income, indefinitely.
No exit, but no boss.

### Which path to pick

Honest read of today's state:
- Code quality = good enough for Path 1 or Path 4 now.
- Reputation / network = need to build for Path 2.
- Capital / team = required for Path 3.

**Path 1 + gradual evolution to Path 2** is the credible 12-month
plan. Path 3 is a "if someone offers us money" plan, not a drive
plan. Path 4 is the "it's already a nice hobby" fallback.

---

## Part F — Anti-patterns (things we must NOT do)

1. **Don't call it a "platform"**. It's a kernel. Platforms are
   multi-service + API + support org. Calling a Rust crate a platform
   is the tell of someone who's never shipped.

2. **Don't pretend defense-readiness**. Our gap-audit is explicit:
   no STANAG, no external audit, no field refs. Claiming otherwise in
   a defense pitch gets you disqualified **and** blacklisted.

3. **Don't chase "universal adaptive AI" framing**. The previous
   calibration already demoted this. Same discipline here — OASIS is
   tactical mesh + autonomy for contested UAS, not Jarvis.

4. **Don't try to sell to primes with cold email**. They won't
   answer. Path through warm intros only, or lose the year.

5. **Don't close-source before Path 1 generates revenue**. Closing
   too early kills the research channel (Archetype 3) and the OSS
   credibility that gets us Path 1 deals at all.

6. **Don't bundle radio hardware until hardware validated**. Gap 1
   hardware round is 2-3 hours of code + 1 afternoon outdoor. Easy.
   But don't pre-sell it before it exists.

7. **Don't promise things that are multi-year** (STANAG, audit,
   field refs) on a 30-minute sales call. Ask for the first deal
   NOW on current evidence.

---

## Part G — Decision points (forks the product hits soon)

### Fork 1 — License choice

- Apache-2.0 or MIT = research + OSS friendly, but integrators can
  fork and not pay.
- BUSL-1.1 (like Sentry, CockroachDB) = commercial source-available,
  converts to Apache after 3 years. Keeps OSS friendliness and first-
  mover revenue.
- Proprietary = caps the upside to Archetype 1/2/4 only, loses 3.

Recommendation: **BUSL-1.1 on the mesh/crypto/radio crates, MIT on
the bio-mechanism crates**. Defense-critical = source-available but
not free for commercial; research-ish = unrestricted.

### Fork 2 — Hardware-bundle or pure-SW

- Pure SW: zero inventory, faster iteration, smaller surface.
- SW + reference hardware kit (€200 kit with 2 Pi Picos + SX1262 HATs):
  makes it buyable, demo-able, but inventory problem.

Recommendation: **pure SW year 1**. Hardware kit year 2 if inbound
justifies.

### Fork 3 — Name / framing

Current positioning docs oscillate between:
- "bio-inspired adaptive AI kernel" (old framing)
- "tactical mesh + autonomy for contested UAS" (current calibration)
- "insider-resistant mesh signing layer" (what buyers understand)

Recommendation: **lead with the last one**. Bio-mechanisms are the
differentiator at engineering level but aren't why buyers write a
purchase order. Lead with the crypto/mesh, mention autonomy as
included.

### Fork 4 — Defense vs dual-use

Defense-only = smaller market, harder procurement, ITAR-adjacent
risks, EU-US tension, export controls.

Dual-use (civilian SAR, inspection, agriculture in addition) =
broader, less regulated, but price-sensitive, less loyal.

Recommendation: **defense-first positioning with dual-use as a
secondary channel**. Don't abandon civilian but don't lead with it.
The crypto story plays in both but resonates in defense.

### Fork 5 — EU vs transatlantic

- EU-focused: easier relationships, aligned on sovereignty, slower.
- Transatlantic: bigger wallets, ITAR headaches, SBIR cycles.

Recommendation: **EU year 1** (sovereignty is a selling point right
now), **transatlantic year 2+** via partner primes.

---

## Part H — Net verdict

### What OASIS is TODAY

A 278 KB Rust kernel that does tactical mesh signing + bio-autonomy +
FHSS, proven byte-exact on MCU silicon models, not yet on real radio,
not yet audited, not yet STANAG. **Credible prototype, 12-24 months
from qualified product.**

### What OASIS could be at 12 months

With Path 1 (OSS + consulting):
- 2-4 paid integrations with small tactical UAS teams
- 1-2 research lab collaborations
- Gap 1 hardware closed (real radio demo)
- Gap 2 round 3 closed (bench jammer with real radio)
- Gap 3 (audit) funded from consulting revenue
- €40k-€200k revenue
- Reputation: "the insider-resistant tactical mesh kernel"

### What OASIS could be at 36 months (if Path 2 lands)

- 1 design win at a tier-2 integrator
- Their product ships with OASIS inside; they brand, we supply
- Royalty + support revenue €200k-€500k/year
- Case study unlocks Archetype 1 deals at higher prices
- STANAG-4586 adapter built by integrator or jointly
- Seed round plausible at this point if we want Path 3

### What OASIS CANNOT be without significant outside capital or luck

- A defense-prime-supplier with STANAG-compliant product catalog
- A full-stack alternative to Anduril / Shield AI
- A "platform" with hundreds of customers
- A €100M exit in under 5 years

### The single biggest risk to product-fit

**The founder-market-fit gap.** OASIS today reads as technically
credible but strategically under-networked. Most deals in this vertical
are relationship-driven. Unless the person running sales has existing
defense relationships (ex-military, ex-Thales/Hensoldt/Helsing, or
ex-DoD), Path 2 and 3 are fantasy.

Path 1 works without those relationships — it's inbound-driven,
technical credibility is the only gate. Path 4 also works.

### Recommendation (honest, narrow)

For the next 6 months:

1. **Close Gap 1 hardware** (~$40, 1 afternoon) — put a video on
   GitHub showing an SX1262-based v0A envelope crossing between two
   Pi Picos outdoors, ~500 m LOS. This is the single most credible
   signal possible for Archetype 1.
2. **Open-source the kernel under BUSL-1.1** (or Apache-2.0 if BUSL
   feels heavy for a single-person project). Let Archetype 3 find it.
3. **Write 2-3 blog posts** explicitly naming the gaps + the
   differentiators. The shadow audits we already have are 80 % there.
4. **Warm-intro 5 EU tactical UAS startups** from the Tekever /
   Quantum / Auterion / Armada / Saker list. Ask for 30 min. Expect
   1 of 5 to turn into a conversation that turns into a first deal.
5. **Do NOT** chase primes, DoD, STANAG, audit, or any of the
   multi-year commitments. Those are earned after the first deal,
   not before.

At 6 months: revenue or no revenue. If revenue, continue Path 1 and
evaluate Path 2. If no revenue, honestly evaluate whether OASIS is
a product or a Path-4 artifact.

### The question I can't answer

Whether the founder(s) have or can build the defense relationships
required for Path 2 to work. Technical quality is necessary but not
sufficient. The code is now good enough. The network may or may not
be. That's the shadow audit gap no one else can close for them.
