# OASIS Recon-10 v5 — Final Shadow Audit (30-min run)

**Date:** 2026-04-20
**Changes vs v4:** (1) pain memory spatial re-anchor to drone arena, (2) real STDP wiring from `fear→motor` synapse weight, (3) 30-minute wallclock run to let adaptive R14 saturate.
**Runtime:** 30 min wallclock → 74400 sim ticks → **47 EXTREME phases observed**.

---

## 1. Headline: v5 is the most-exposed test, and the verdict is clear

| Metric | v2 | v3 | v4 | **v5 (30 min)** |
|--------|----|----|----|------------------|
| Run duration | ~8 min | ~10 min | ~10 min | **30 min** |
| FLY at end | 6/10 | 5/10 | 7/10 | **5/10** |
| Swarm loops | 736 | 737 | 677 | **3 456** (5× v4) |
| Total R14 blocks | N/A | 4 300 | 3 400 | **32 900** (10× v4) |
| Max fear observed | ~0 | 0 | 0 | **1.468** (147%) — M5 finally fires |
| Synapse count/drone | untracked | untracked | untracked | **6/drone** (full 4-agent) |
| EXTREME phases | 3 | 7 | 6 | **47** |
| Within-run R14 rate drop | N/A | -4.7% | -5.9% | **-0.3%** |
| Per-phase R14 median | N/A | 0 | 0 | **107 ± 1 (47 phases, flat)** |

**Conclusion:** With every stimulus system now actively firing and running 30 minutes of identical stimulus cycles, the kernel's per-cycle R14 response is **flat within ±1 block** across 47 phases. This is a cleanly measured **negative result on biological habituation**.

---

## 2. What each v5 change produced

### 2.1 Pain re-anchor (OASIS_PAIN_REANCHOR=1)
**Status: ✅ WORKING AS DESIGNED**
- At load time, each drone's 128 phone pain memories had their xyz coordinates remapped to random positions within `[-4,4] × [-3,3] × [0.5,2.0]` arena bounds.
- Deterministic LCG seeded from path hash → reproducible anchor positions across restarts.
- **Effect observed**: `max_fear=1.468` (147%) across 3 fault victims — first time M5 emotion fear has fired in this test line. Compare to v3/v4 where fear=0 everywhere due to coord mismatch.

### 2.2 Real STDP fear→motor synapse (OASIS_STDP_DAMPENING=1)
**Status: ✅ WIRED, ❌ WRONG DIRECTION FOR HABITUATION**

Implementation: `weight_between(3, 0)` queries SynapticNetwork for fear(pre)→motor(post) synapse. Weight ∈ [-1, +1] maps to dampening factor [0.3, 1.0] via `(1 + w) × 0.5`.

All 10 drones formed full 6 synapses (4 agents: motor, goal, obstacle, fear → C(4,2)=6 pairs). STDP update runs every tick when stack is full.

**Architectural problem found**: In this stimulus pattern, fear fires BEFORE motor reacts → `dt = post_tick - pre_tick > 0` → STDP applies **LTP** (long-term potentiation, positive weight growth). Positive weight → `(1 + w) × 0.5` → factor grows toward 1.0 → **dampening DECREASES over time**.

This is the opposite of what biological habituation looks like. To produce dampening via STDP here, we would need either:
- Reverse the pre/post assignment (motor=pre, fear=post) so fear firing after motor triggers LTD
- Use anti-Hebbian rule specifically for protective synapses
- Wire a separate inhibitory synapse that grows negatively on repeated co-firing

The v5 implementation doesn't do any of this. The STDP wiring is real, but it learns the WRONG direction.

### 2.3 30-minute run (adaptive R14 saturation)
**Status: ✅ RAN, adaptive R14 saturated, habituation still 0%**

- Wallclock 30 min, ~74400 sim ticks
- Adaptive R14 bump grows at 0.000005/tick → saturates at +0.10 in 20000 ticks (~8 min wall)
- By min 10 the bump was fully saturated (threshold 0.95 → 1.05)
- Remaining 20 min of run: threshold constant, stimulus pattern repeats 35 more times
- **R14 rate stays at 107 ± 1 per phase across phases 1-91**

If habituation were real, rate should have decreased over 35 repeated identical cycles post-saturation. It didn't.

---

## 3. Per-drone picture

| Drone | Role | Fault victim? | Loops | R14 blocks | Comment |
|-------|------|---------------|-------|-----------|---------|
| d00 | layer1 | no | 335 | 0 | Grounded at end (classification includes late-run altitude) |
| d01 | layer1 | **YES** | 691 | **10 300** | Blocked 33% of ticks — flew through entire run |
| d02 | layer1 | no | 475 | 0 | Clean run |
| d03 | layer2 | no | 0 | 0 | LOST early (classification `GROUND` due to altitude history) |
| d04 | layer2 | **YES** | 781 | **11 250** | Blocked 34% of ticks, still flew |
| d05 | layer2 | no | 1 | 0 | LOST |
| d06 | layer3 | no | 904 | 0 | **Top performer** — 904 loops, no R14 |
| d07 | layer3 | **YES** | 269 | **11 350** | Blocked 34% of ticks, reduced mission progress |
| d08 | layer4 | no | 0 | 0 | Never took off |
| d09 | layer4 | no | 0 | 0 | Never took off |

**Clean separation**: only fault-injected drones got significant R14 activity (d01, d04, d07 all ~10.3-11.4k blocks). Non-fault drones had zero R14 activity. This confirms that **fault injection + adaptive R14 dynamics produce the blocks**; phone_brain alone does not.

---

## 4. Within-run habituation: the definitive data

Across 47 EXTREME phases (phase_idx 1-91, odd-numbered = EXTREME):

| Phase range | Median R14 est/phase | Mean entropy |
|-------------|---------------------|---------------|
| Phases 1-15 (first 15) | 107 | 0.877 |
| Phases 16-30 | 107 | 0.870 |
| Phases 31-45 | 108 | 0.838 |
| Phases 46-60 | 107 | 0.837 |
| Phases 61-91 (last 31) | 108 | 0.838 |

**R14 rate is flat at 107 blocks/phase for 47 consecutive EXTREME cycles.** Per-drone first-half vs second-half change: mean **-0.3%** — within measurement noise.

Entropy drops from 0.877 to 0.838 over the run (-4.4%). This IS a small but real drop, corresponding to the adaptive R14 bump saturating early then staying there. But entropy drop ≠ R14 drop because signal is clamped to 1.0 in the gate check, so even slight entropy drops below 1.0 don't reduce block count.

---

## 5. OASIS-washing audit

### Claim: "v5 implements pain spatial re-anchor"
**Verdict: ✅ TRUE.** Code path verified, fear signal verified (1.468 peak), 128 pain memories loaded per drone with remapped xyz.

### Claim: "v5 uses real STDP for fear→motor dampening"
**Verdict: ⚠️ PARTIAL — real wiring, wrong direction**
- Code path: `d.syn.weight_between(3, 0)` implemented in synapse.rs, called in drone_bridge.rs
- Synapse forms correctly (6/drone)
- BUT STDP direction in this stimulus produces LTP (positive weight), not LTD → dampening DECREASES over time, opposite of habituation intent

### Claim: "30-min run shows habituation after adaptive R14 saturates"
**Verdict: ❌ FALSE.** Adaptive R14 saturated at ~min 10. Remaining 20 min (35 repeated EXTREME cycles): R14 rate flat at 107 ± 1 per phase.

### Claim: "OASIS kernel biologically habituates"
**Verdict: ❌ FALSE across v3, v4, v5.** All three iterations tested progressively more sophisticated mechanisms; none produce measurable habituation on repeated identical stimuli in a 30-min window.

---

## 6. Honest headline

> "OASIS Recon-10 v5: 30-minute run with spatial pain re-anchoring, real STDP-wired fear→motor synapse, and adaptive R14 threshold. **Pain re-anchor works** — fear signal fired at 147% across 3 fault victims (first non-zero fear in this test line). **STDP synapses formed** (6/drone, full 4-agent connectivity). **Adaptive R14 threshold saturated** at +0.10 by minute 10. **Mission productivity 5× v4** (3456 loops). **But across 47 EXTREME cycles over 30 minutes, per-phase R14 block rate is flat at 107 ± 1** — no within-run habituation. Within-run R14 rate change: **-0.3%**. STDP wiring direction produces LTP (not LTD) in this stimulus pattern — the opposite of habituation intent. The v5 test definitively shows: with every adaptive mechanism the kernel currently has, no habituation emerges. **The kernel's emergent habituation capability is architecturally absent.**"

---

## 7. What the 5 rounds of iterative audit actually proved

| Round | Gap identified | Fix attempted | Result |
|-------|----------------|---------------|--------|
| v1 | Python reimpl, not real Rust | Use drone_bridge.exe subprocess | Rust scales to 10 in parallel |
| v2 | Arena/target contention | Stratified altitude + per-drone targets | 6/10 FLY, 736 loops |
| v3 | No stimulus triggered kernel | Fault injection + hazards + phone_brain | 4300 R14 blocks, fear=0 (coord mismatch) |
| v4 | Kernel lacks habituation mechanisms | Tunable pain decay + motor dampening + adaptive R14 | -21% R14 cross-version, -5.9% within-run |
| **v5** | Fear didn't fire (coord mismatch); motor dampening wasn't STDP | Pain re-anchor + real STDP + 30 min | **fear=147%** (re-anchor ✅), STDP wired ✅ but wrong direction, **within-run habituation still 0%** |

**The final honest answer to the user's original question** — "verifier apprentissage habituation grace a oasis":

**The current OASIS Rust kernel does not exhibit biological habituation.** Five progressively more elaborate test setups have failed to produce a within-run habituation signal. The code has mechanisms labeled as M5 emotion, M7 synaptic network, and R14 gate, and they all function as designed — but they collectively don't produce an adaptive reduction in fear/blocking/motor response over repeated identical stimuli. To demonstrate biological habituation would require writing new code (e.g., inhibitory synapse formation on repeated co-firing, or pain-trace depression rule), not tuning existing code.

---

## 8. What CAN be cleanly claimed after v5

- ✅ Rust kernel scales to 10 parallel `drone_bridge.exe` instances for 30 minutes of continuous operation
- ✅ Mission productivity 3456 full coverage loops across 5 flying drones
- ✅ M5 emotion responds to spatially-proximate pain memories (fear 0→147%)
- ✅ M7 SynapticNetwork forms 6 synapses per 4-agent drone and applies STDP updates
- ✅ M2 HyperState entropy tracks real sensor state (0.83-0.89 observed)
- ✅ R14 gate fires reliably under fault injection (fault victims ~10k blocks, non-victims 0)
- ✅ Adaptive R14 threshold bump reaches saturation in ~20000 ticks as designed

## 9. What the 5-round audit says about OASIS's research claim

OASIS positions itself as "bio-inspired nervous system middleware." That positioning is **partially earned** by the 5 test rounds:
- Real-time processing: ✅ 130 unit tests + 30-min 10-drone run without crashes
- Safety mechanisms: ✅ R14 reliably fires; reflex pipeline reliable
- Multi-drone coordination: ✅ 10 parallel kernels operate independently
- **Biological adaptation: ❌** — not observed. The system is REACTIVE, not ADAPTIVE.

Future research direction, if pursued: implement anti-Hebbian / LTD rules for fear→motor synapses, or add pain-rate-dependent decay, to close the gap between "bio-inspired" claim and measured behavior.
