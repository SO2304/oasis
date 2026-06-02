# SHADOW AUDIT — Gap 2 step 1: FHSS anti-narrowband-jam

**Context**: [SHADOW_AUDIT_DEFENSE_VERTICAL.md](SHADOW_AUDIT_DEFENSE_VERTICAL.md)
Gap 2 = jamming resilience. Gap 1 round 1 delivered the LoRa software
stack scaffolding ([SHADOW_AUDIT_GAP1_LORA_SCAFFOLDING.md](SHADOW_AUDIT_GAP1_LORA_SCAFFOLDING.md))
with a `SimulatedLoRaRadio`. This round extends that sim to a
**multi-channel bus** with a static jammer model, implements FHSS on
top, and quantifies anti-jam gain over 8-32 channels.

Scope honest: this is **byte-layer simulation**, not RF PHY. A real
SX1262 doing the same hop schedule against a real CW jammer should
match these numbers to within PHY margins (~5-10 % loss due to
residency, re-tune time, guard bands).

---

## What got built

- `fhss::channelized::ChannelBus` — shared Arc<AtomicU64> jam bitmap
  + channels for A↔B traffic. Packets carry a `(channel_idx, bytes)`
  tuple. Jam bit set on channel k → TX on channel k is silently dropped.
- `fhss::channelized::ChannelizedRadio` — endpoint; `set_channel()`
  re-tunes. Implements `LoRaRadio`.
- `fhss::hop_channel(seed, counter, n)` — SHA-256(seed || "oasis-fhss-v1"
  || counter) mod n. Deterministic, identical on TX and RX. Seed is the
  **same Ed25519 mesh seed** already used for v0A signing — zero new
  key distribution.
- `fhss::FhssRadio<R>` — wraps any `ChannelizedRadio` with per-packet
  hopping. `tx_hopped()` / `rx_hopped()` advance the shared counter on
  BOTH sides regardless of RX success, so slot alignment survives jammer
  drops (same invariant real FHSS systems maintain).

## Pre-audit predictions

| # | Prediction | Rationale |
|---|---|---|
| P1 | Baseline (no jam, 1 ch): 100 % delivery | No loss mechanism active |
| P2 | 8 ch FHSS, no jam: 100 % — FHSS alone doesn't cost delivery | Hop sequence is deterministic, no packet is "lost in space" |
| P3 | 1 ch jammed, no FHSS possible: 0 % delivery | Jammer kills the sole channel |
| P4 | 1 jam / 8 FHSS: ~87.5 % = 7/8 | Uniform hash over 8 channels, 1 blocked |
| P5 | 2 jam / 8 FHSS: ~75 % | 2 of 8 channels blocked |
| P6 | 4 jam / 8 FHSS: ~50 % | Half the channels blocked |
| P7 | 1 jam / 16 FHSS: ~93.75 % | Wider hop = better margin |
| P8 | 1 jam / 32 FHSS: ~96.875 % | Asymptote toward 100 as N grows |
| P9 | Half-spread tight (< ±10 %) | Deterministic simulation + reproducible seed = low noise |
| P10 | Hop sequence uniform over channels | SHA-256 is a good PRF |

## Measured results (K=10 banded, 80 packets per trial)

```
OASIS Gap 2 — FHSS delivery ratios (packets per trial: 80)

scenario                                  K=10  min   med   max   mean  halfspread
──────────────────────────────────────────────────────────────────────────────────
no jammer, 1 chan (baseline)                    1.000 1.000 1.000 1.000   ±0.000
no jammer, 8 chan FHSS                          1.000 1.000 1.000 1.000   ±0.000
JAM 1 of 1 chan (no FHSS possible)              0.000 0.000 0.000 0.000   ±0.000
JAM 1 of 8 chan, FHSS                           0.850 0.900 0.912 0.889   ±0.031
JAM 2 of 8 chan, FHSS                           0.637 0.762 0.825 0.748   ±0.094
JAM 4 of 8 chan, FHSS                           0.400 0.450 0.537 0.463   ±0.069
JAM 1 of 16 chan, FHSS                          0.887 0.925 0.975 0.926   ±0.044
JAM 1 of 32 chan, FHSS                          0.900 0.963 1.000 0.961   ±0.050
```

## Comparison: measured vs theoretical

| Scenario | Predicted (N-k)/N | Measured median | Gap to theory | Match? |
|---|---:|---:|---:|---|
| No jam 1 ch | 1.000 | 1.000 | 0.000 | ✅ P1 |
| No jam 8 ch FHSS | 1.000 | 1.000 | 0.000 | ✅ P2 |
| 1 jam 1 ch | 0.000 | 0.000 | 0.000 | ✅ P3 |
| 1 jam 8 ch FHSS | 0.875 | 0.900 | +0.025 | ✅ P4 |
| 2 jam 8 ch FHSS | 0.750 | 0.762 | +0.012 | ✅ P5 |
| 4 jam 8 ch FHSS | 0.500 | 0.450 | −0.050 | ✅ P6 (within half-spread) |
| 1 jam 16 ch FHSS | 0.938 | 0.925 | −0.013 | ✅ P7 |
| 1 jam 32 ch FHSS | 0.969 | 0.963 | −0.006 | ✅ P8 |

**10/10 predictions matched**, all deltas within half-spread bands.
Half-spreads ±0-9 % confirm P9 (low-noise deterministic sim). The
PRF uniformity (P10) is covered by a separate test
`hop_channel_is_deterministic_and_uniform` which asserts each of 8
channels gets hit 96-160 times out of 1024 samples.

## Quantitative findings

### Finding 1 — OASIS FHSS is a real anti-jam gain

Against a narrowband jammer:

| Without FHSS | With 8-ch FHSS | Gain |
|---:|---:|---:|
| 0 % delivery | 90 % delivery | +90 % |

This is the **only** anti-jam mechanism in OASIS today and it's in the
byte layer, portable across whatever radio supports multi-channel.

### Finding 2 — Channel count scales as expected

Single jammer, N-channel FHSS:

| N channels | Theoretical (N-1)/N | Measured median |
|---:|---:|---:|
| 8 | 87.5 % | 90.0 % |
| 16 | 93.8 % | 92.5 % |
| 32 | 96.9 % | 96.3 % |

Going from 8 to 32 channels costs essentially nothing (SHA-256 call per
packet, ~30 µs on Cortex-M0+ per our earlier measurement of mesh HMAC).
**Recommendation**: default to ≥ 16 channels once this moves to real
hardware, where SX1262 supports 868.1 / 868.3 / ... / 869.525 in the
EU868 sub-band (~8 usable channels at 125 kHz BW) or 902-928 in US915
(~64 channels).

### Finding 3 — Multi-jammer scaling is linear

| Jammers / 8 ch | Theoretical | Measured |
|---:|---:|---:|
| 1 | 87.5 % | 90.0 % |
| 2 | 75.0 % | 76.2 % |
| 4 | 50.0 % | 45.0 % |

Each additional jammed channel linearly reduces delivery. Beyond 50 %
blocked bandwidth, FHSS alone isn't enough — would need **DSSS** or
physical displacement. Gap 3 doesn't cover that.

### Finding 4 — Determinism bands

Half-spread across 10 seeds:

| Scenario | Half-spread |
|---|---:|
| 1 jam 8 ch | ±3.1 % |
| 1 jam 16 ch | ±4.4 % |
| 1 jam 32 ch | ±5.0 % |
| 2 jam 8 ch | ±9.4 % |
| 4 jam 8 ch | ±6.9 % |

Narrow bands (< 10 %) mean the measurements are repeatable; you'd see
similar bounds running against a real CW jammer on bench, with some
additional ±5 % from PHY margin effects not modelled here.

## Threat model coverage claims (honest)

What this Gap 2 round **actually** defends against:

| Threat | Covered? |
|---|---|
| Static narrowband CW jammer (fixed freq, continuous) | ✅ quantified above |
| Pulsed narrowband jammer (same freq, duty cycled) | Equivalent to less-than-100%-duty CW — FHSS still wins, less dramatically |
| Sweeping jammer (scans all N channels) | **Not modelled.** Performance depends on sweep rate vs hop rate. Next round. |
| Follower jammer (detects + jams current channel real-time) | **Not modelled.** Requires hop interval < follower latency — future hardware work. |
| Wideband barrage jammer (all N at once) | ❌ FHSS does **nothing**. Would need DSSS or physical displacement. |
| Reactive jammer (RF-detector-triggered) | **Not modelled.** Same as follower above. |

So: **FHSS Gap 2 round 1 closes the static-narrowband case**. Two more
rounds (sweep modelling + follower modelling) would close the bench
simulation scope. Real-world jammer validation requires a range permit.

## What we have NOT proven

1. Real RF hop performance (re-tune time, guard bands, PHY residency).
   The sim assumes instantaneous channel change; SX1262 spec is ~1 ms
   re-tune. At 868 MHz / 125 kHz BW / SF7 / 255 B payload, packet time
   is ~200 ms. So 1 ms re-tune is < 1 % overhead — small but nonzero.
2. Clock synchronization required for a real deployment. The sim
   advances the shared counter in lockstep; in real hardware, GPS-denied
   environments need a TX→RX clock sync scheme (common patterns:
   periodic beacon, NTP-over-mesh, or GPS-PPS if available). Not built.
3. Interaction with the v0A signature. The FHSS layer is **below** the
   OASIS frame, so Ed25519 verify happens after dehopping. No new
   crypto properties — but also no extra anti-spoof on the hop schedule
   itself. A sophisticated attacker with the seed *can* follow the
   hops; Gap 4 (anti-tampering) becomes more load-bearing.

## Gap 2 progression map

| Milestone | Status |
|---|---|
| Multi-channel sim radio | ✅ this round |
| Static narrowband jammer model | ✅ this round |
| Seed-derived deterministic hop sequence | ✅ this round |
| `FhssRadio` wrapper preserving counter across jam drops | ✅ this round |
| K=10 banded measurement 8/16/32 channels | ✅ this round |
| Sweeping jammer model | ⏳ next round |
| Follower jammer model + hop-interval bounds | ⏳ future |
| Real SX1262 FHSS on bench | ⏳ needs hardware (~$40) |
| Controlled range test against a cooperative jammer | ⏳ needs permit |

**Gap 2 at bench-simulation level for static narrowband = CLOSED.**

## Net change to OASIS defense-vertical readiness

Before this round:
> "Jamming resilience": XOR FEC on loss = yes; **no frequency hopping**,
> no DSSS, no anti-jam waveform

After this round:
> FHSS with 8-32 channel hop schedule driven by the shared Ed25519
> seed. Host-measured K=10: 1 jammer / 8 chan → 90 % delivery (vs 0 %
> without FHSS). Byte-layer; hardware integration = Gap 2 round 2.
> Sweeping / follower jammers not yet modelled.

Gaps closed so far: **1 (bench) + 2 (static narrowband, bench)**. Four
left: crypto audit, anti-tampering, STANAG, field references.

## Predictions for Gap 2 round 2 (sweep + follower models)

| # | Prediction |
|---|---|
| N1 | A sweeping jammer with sweep period ≥ 8 × packet time will deliver ~87.5 % (same as static, because by the time it visits a hop, we've moved on) |
| N2 | Sweeping at half the packet rate = jams every second packet = 50 % delivery |
| N3 | A follower jammer with latency > hop interval is equivalent to no jammer |
| N4 | Real hardware (SX1262 at 1 ms re-tune) → cross-over hop interval ≈ 10-20 ms to outpace a fast follower |

---

# Round 2 — sweep + follower jammer models

**Date**: 2026-04-23.
**What's new**: generic `Jammer` trait + virtual-time clock on the
ChannelBus. `StaticJammer`, `SweepJammer`, `FollowerJammer` implement
it. Each TX advances the clock by `packet_duration_ms` and queries all
registered jammers for the (channel, start_ms, duration) tuple.

## Sweep jammer — measured

Sweep jammer covers all N channels in `period_ms`, dwelling
`period_ms / N` per channel. OASIS FHSS hops per packet.

```
FHSS 8ch, sweep P=800 ms (one dwell per packet)    K=10  med=0.900  ±0.075
FHSS 8ch, sweep P=80 ms  (fast, sub-packet dwell)  K=10  med=0.887  ±0.056
FHSS 8ch, sweep P=8000 ms (slow, ~10 pkts/dwell)   K=10  med=0.863  ±0.044
```

| Prediction | Actual | Match? |
|---|---|---|
| N1: sweep ≥ 8× packet → ~87.5 % | 0.900 (P=800 ms) | ✅ |
| N2: sweep ≈ half pkt rate → 50 % | **not reproduced — 88.7 % at P=80 ms** | ❌ |

N2 was **wrong**. I predicted a fast sweep would jam more; in reality
a fast sweep visits each channel briefly, so by the time it revisits,
we've already transmitted on a different channel. Fast sweep doesn't
meaningfully differ from the narrowband equivalent.

The **worst case for sweep** (86.3 % vs 90.0 %) is actually the **slow
sweep**: jammed channel stays jammed for 10 consecutive packets, and
during that window our FHSS hops onto it ~10/8 = 1-2 times. Still in
the 85-90 % band though — sweep never meaningfully worse than static
narrowband against 8-channel FHSS with random TX timing.

**Finding**: sweep jammers are **not a meaningful escalation** over
static narrowband as long as FHSS channels ≥ 8 and the TX timing is
uncorrelated with sweep timing. Delivery stays in the 86-90 % band.

## Follower jammer — measured

Follower detects TX then switches to that channel after `latency_ms`.
If the resulting overlap ≥ 20 % of packet duration, LoRa CRC fails.

```
FHSS 8ch, follower lat=10 ms (very fast)           K=10  med=0.000  ±0.000
FHSS 8ch, follower lat=50 ms (half packet)         K=10  med=0.000  ±0.000
FHSS 8ch, follower lat=85 ms (near packet, < 20%)  K=10  med=1.000  ±0.000
FHSS 8ch, follower lat=120 ms (slower than pkt)    K=10  med=1.000  ±0.000
```

| Prediction | Actual | Match? |
|---|---|---|
| N3: latency > hop interval → no effect | ✅ 100 % at 120 ms (> 100 ms pkt) | ✅ |
| N4: cross-over at short hop interval | overlap threshold cross is at **85 ms** (= 85 % of 100 ms) | ✅ partial |

**Finding**: follower jammer performance is **binary** against FHSS.
Either latency < 0.8 × packet_duration (follower wins, 0 % delivery)
OR latency ≥ 0.8 × packet_duration (FHSS wins, 100 % delivery). No
middle ground because FHSS keeps moving so prior-channel jam is
irrelevant for the NEXT packet.

This is actually worse than I expected (N3 was over-optimistic — not
"> hop interval", but "> 80 % of packet duration"). If the adversary
can detect + switch faster than the packet duration, FHSS is
catastrophically defeated.

## The follower defense — measured

**Short packets outrun the follower**:

```
FHSS 8ch, packet=10 ms + follower lat=20 ms    K=10  med=1.000  ±0.000
```

With packet duration 10 ms and follower latency 20 ms, the follower is
ALWAYS late. 100 % delivery. This is the canonical FHSS design rule:

> Packet duration must be shorter than the fastest plausible follower
> latency for the threat model.

On real SX1262 hardware:
- SF7 / BW500 / 20 B payload = 8 ms time on air. Follower < 8 ms
  requires sophisticated SDR (commercial adversaries: rare).
- SF12 / BW125 / 200 B payload = 1.3 s time on air. Follower < 1.3 s
  is trivial with any software-defined-radio jammer.

**Design implication**: for contested environments, tune the link for
**shortest viable packet time** — accepts range reduction (higher BW,
lower SF) to buy follower immunity. OASIS's 125 B v0A envelope needs:

| SX1262 config | Time on air | Follower immune up to |
|---|---:|---:|
| SF7 / BW500 | ~30 ms | latency < 24 ms |
| SF7 / BW125 | ~130 ms | latency < 104 ms |
| SF9 / BW125 | ~400 ms | latency < 320 ms |
| SF12 / BW125 | ~2.8 s | latency < 2.2 s |

So a tactical OASIS deployment wanting follower-jammer immunity against
a ~25 ms-latency adversary needs **SF7 / BW500**, at the cost of ~3 km
range instead of ~10 km. Real operational tradeoff; we can name it.

## Combined threat coverage after round 2

| Threat | Round 1 | Round 2 |
|---|---|---|
| Static narrowband CW | ✅ quantified | — |
| Sweep narrowband | — | ✅ 86-90 % (same band as static) |
| Follower, fast (lat < 0.8 × pkt) | — | ✅ **0 %** — FHSS fails |
| Follower, slow (lat ≥ 0.8 × pkt) | — | ✅ 100 % |
| Wideband barrage (all N at once) | ❌ | ❌ (still needs DSSS) |
| Reactive (RF-triggered) | — | equivalent to follower |

**5 threats of 6 quantified.** The 6th (wideband barrage) requires
DSSS which is a hardware-layer fix (SDR, wider spreading, or physical
spatial separation) and cannot be fixed in the OASIS layer.

## Updated predictions for round 3 (field / real hardware)

Honest, written before doing:

| # | Prediction |
|---|---|
| M1 | An SX1262 at SF7/BW500 on a Pi Pico W hopping 8 channels vs a cheap RTL-SDR-based CW jammer will deliver 85-90 % packets (matches sim band) |
| M2 | A sophisticated SDR-based follower jammer (HackRF + custom firmware, < 25 ms latency) against SF7/BW125 ≈ 130 ms packets will drop delivery to near 0 as predicted |
| M3 | The effective range of OASIS SF7/BW500 at 14 dBm EU868 outdoors LOS will be 300-600 m — enough for a swarm formation, not enough for extended mission |
| M4 | Switching to SF7/BW125 for range while under a cooperative adversary follower jammer will demonstrate the follower-immunity tradeoff empirically |

Round 3 requires ~$40 hardware and a permit-accessible range. The
simulation results here are the design-spec for that test.

## Gap 2 status after round 2

| Sub-gap | Bench-sim | Real hardware |
|---|---|---|
| FHSS static narrowband | ✅ | ⏳ |
| FHSS sweep | ✅ | ⏳ |
| FHSS follower (slow) | ✅ | ⏳ |
| FHSS follower (fast) | ✅ (fails — expected) | ⏳ |
| DSSS wideband | ❌ (out of scope) | ❌ |

**Gap 2 at bench-simulation level = CLOSED for all tractable adversary
classes.** Wideband-barrage requires hardware-layer mitigations (DSSS,
physical displacement) that are outside OASIS's software responsibility.

Honest net-change text for the defense-vertical document:

> Before: "Jamming resilience" = XOR FEC, no anti-jam waveform
> After round 1: FHSS over N channels, static narrowband quantified
> After round 2: sweep and follower jammers quantified. Delivery
> matches theoretical (N-k)/N for narrowband/sweep; binary pass/fail
> for follower depending on adversary latency vs packet duration.
> **FHSS is genuinely a defense against narrowband threats; it is
> genuinely defeated by fast-follower threats.** Mitigation of the
> latter requires shorter packets (higher BW, lower SF) at the cost
> of range. The design tradeoff is now documented, not hand-waved.
