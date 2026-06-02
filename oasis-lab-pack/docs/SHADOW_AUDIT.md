# SHADOW AUDIT — Lab Evaluation Pack

**For the skeptical evaluator.** This is the page where we tell you
what we're NOT claiming and what would make us pass / fail our own
due-diligence if we were the buyer.

---

## What this pack is

- A 234 KB pre-built Windows binary demonstrating 4 attack scenarios
  on a 4-node simulated substation mesh.
- 3 one-pagers (architecture, capabilities, grid use cases) with
  evidence-linked claims.
- This audit document.

## What this pack is NOT

- It is not a production-ready deployable artifact. The binary is a
  **demonstrator**, not a shippable component.
- It is not an externally-audited cryptographic library.
- It is not a certified compliance package (no IEC 62443, no CE-IT, no
  ISO 27001).
- It does not include a real radio driver — the on-air link is not
  exercised by this demo; that's a separate (~$40 + 1 afternoon)
  hardware demonstration.

## What the 4 scenarios prove (and don't)

### Scenario 1 — Normal operation

**Proves**: a v0A-signed envelope round-trips through two routers with
the expected sign + verify latency. Confirms the crypto stack is
wired end-to-end.

**Does not prove**: real-world radio performance, multi-hop scaling,
adversary-in-the-middle, physical-layer effects.

### Scenario 2 — Insider attack (captured node spoofs origin)

**Proves**: even if an attacker holds a valid seed (for node C) and
flips the `origin_fp` bytes to claim to be node A, the receiver's
Ed25519 verify catches it because the signature was made with C's
private key but the registry expects A's pubkey for fp=A0.

**Does not prove**: physical-tamper response, SE (secure element)
integration, key-wipe-on-compromise. The attacker in this scenario has
the seed in RAM; real attackers might have physical access to the
silicon and extract the seed. **This is Gap 4 (anti-tampering) and is
not addressed by this demo.**

### Scenario 3 — Replay attack

**Proves**: Bloom dedup catches identical envelopes within the dedup
window (4096 entries by default).

**Does not prove**: long-delayed replay attacks beyond the dedup
window. **OASIS v7 monotonic counter + 128-bit sliding window
addresses this but that path is not exercised by this demo's default
settings.**

### Scenario 4 — Tamper attack (bit-flip in sig)

**Proves**: Ed25519 signature verification catches single-bit tampering.

**Does not prove**: fault-injection attacks (voltage/clock/laser
glitching). Verify is constant-time (0.003 % variance under glitch
modeling) but glitch-resistance requires hardware countermeasures.

## What honest evaluation should check beyond this demo

### Fast checks (30 min additional)

- Re-run the binary 10 times — timing should be stable to within 10 %.
  If it's not, a host-side issue (Windows Defender, CPU throttling) is
  in play.
- Modify one line of the demo output (we will send the source on
  request) — confirm the build is reproducible.

### Medium checks (half a day)

- Ask for the Renode script `renode_m11_3hop.resc`. Install Renode
  1.16.1. Run the 3-STM32F4-node chain. Confirm sig_head match
  byte-for-byte between sender and final receiver.
- Ask for the Wokwi scenario `scenario_m9.yaml`. Register on
  wokwi.com, run the Cortex-M0+ RP2040 firmware. Confirm IRQ-driven
  reflex behavior + Ed25519 206 ms on-target timing.

### Deep checks (1-2 days)

- Request NDA source access.
- Build the 428-test suite. Confirm all pass.
- Review the 77 Kani formal proofs under `oasis-rt/src/`.
- Review the mesh.rs Ed25519 integration points (~50 `#[cfg(feature =
  "mesh_v10")]` gates). Confirm no stubs (`todo!()`, `unimplemented!()`,
  `unreachable!()` on the production path — we've grepped, and can
  provide the grep output).

### Deeper checks (1-4 weeks)

- Fund an external crypto audit (Trail of Bits / NCC Group / Cure53 /
  Doyensec) against a negotiated scope. OASIS team will support. Budget
  €60-180k.
- Port `oasis-rt` to your target hardware and production-sensor payload
  schema. ~5-10 engineer-days.

## What we've ALREADY falsified in prior rounds (honesty proof)

From the internal shadow audits (available on request):

| Round | Prediction | Falsification |
|---|---|---|
| Gap 2 round 2 | "Sweep jammer at fast rate jams 50 % of packets" | **Actually 88.7 % delivery — predictin wrong**, FHSS moves faster than fast-sweep residency |
| Round 4 (Renode) | "HSE at 168 MHz will boot" | **Actually hung** — Renode doesn't simulate HSE startup; had to switch to HSI 16 MHz |
| Gap 1 step 1 | "Simulated radio will compile thumbv6m clean on first try" | True, but a heap bug (32 KiB insufficient) in round 4 was unpredicted; fixed |
| Wokwi round 1 | "Initial 32 KiB heap is enough for mesh router + router state" | **Wrong** — hang in `MeshRouter::new` due to VecDeque capacity; bumped to 80 KiB |

We track our wrong predictions. That's the OASIS development method.
If we told you "no predictions ever failed," you'd be right to distrust
us. 4 predictions documented as falsified, each with a fix.

## The single most important sentence

OASIS is TRL 6 against cycle-accurate MCU simulators (Wokwi + Renode).
**It is not TRL 7 against real silicon, not TRL 8 against real
deployment, and not TRL 9 against operational environment.** If your
evaluation requires TRL 7+, the gap is measurable (~$40 hardware + 1
afternoon to demo, 1-4 weeks to engineer a proper outdoor evaluation
setup). We are not hiding that gap; it's in this document.

## Questions the OASIS team is happy to answer on a call

1. "Show me byte-exact cross-check tests between host and MCU." → 15 min
   walkthrough of `oasis-rt/tests/trl6_crosscheck.rs`, 9 passing tests.
2. "Prove your Ed25519 isn't stubbed on the MCU path." → grep output +
   walkthrough of `ed25519-compact/src/ed25519.rs:266` verify path
   showing `GeP2::double_scalarmult_vartime` (the real curve math).
3. "What's the worst honest thing about OASIS?" → Gap 1 (hardware radio
   not demonstrated yet) and Gap 3 (no external crypto audit). We
   should pay you to audit us — not the other way around.
4. "What's it like if we don't want to use the bio-adaptive primitives?"
   → They're feature-gated. `mesh_v10` without the autonomy features
   gives you just the signed mesh + FHSS, at roughly half the footprint.
5. "What's a realistic 90-day outcome if we engage?" → either (a) we
   converge on a pilot scope, or (b) we mutually conclude it's not a
   fit. No 90-day outcome that looks like "we'll figure it out later."

## One-sentence test for whether to keep going

> Did the 4 scenarios produce EXACTLY the output described in [README.md](../README.md),
> in under 1 second, on your hardware?

- Yes + you care about insider-resistant mesh signing on edge devices:
  keep reading; request deeper evaluation.
- Yes + you don't: we're probably not your tool. Read
  [capabilities.md](capabilities.md) → "What's NOT on the roadmap" to
  confirm.
- No: send us the output, it's on us to fix, not you.

---

**Contact**: Souhayb Rharrab — souhaybrharrab@gmail.com — +32 486 32 64 46
