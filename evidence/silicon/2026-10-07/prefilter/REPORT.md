# Phase 2.1 on silicon — relay pre-filter against forced verification (C1)

Spec: `docs/specs/RELAY_PREFILTER_SPEC.md`. Gap: `partners/POSITIONING_GAPS.md` C1.

**Setup.** Two RP2040 boards, wired B.GP0 → C.GP1 (one way, 115 200 baud).
C = relay under test (terminal node), B = injector **and** legitimate origin. A was left
running the Phase 1.4 Modbus device and was not touched. One firmware, two modes
(`@O0` v0B / `@O1<next-hop fp>` v0C), same instrumentation, so before/after is measured
on one stamp. Logs are LF-only from USB-CDC; `harness/` holds the scripts, `artifacts/`
the signed manifests and image hashes.

Firmware versions installed through the **signed update path** (o2 hybrid manifests),
each one a fix to the measurement (§4): v6 → v7 (quiet mode) → v8 (link-key cache) →
v9 (pre-signed legitimate frames) → v10 (`@Y` third field). **The valid results below
are from v10, stamp `0baec61`.** Earlier sweeps are kept in this directory but are
void, and the file names say so.

## 1. What a forged frame costs the relay

The C1 attack: anyone listening to the link can fabricate envelopes that pass every
cheap check (network, known origin, ttl, fresh counter) and force a full Ed25519
verification. `@Y<rate>,<secs>,0` sends **forged frames only** — one signed envelope
whose counter field is then bumped per frame, which is what an attacker does (it does
not sign) and what makes the per-frame cost unambiguous.

30 s per run, no legitimate traffic, **nothing lost on the wire** (`crc_fails` 0, every
frame accounted for):

| Mode | rate | frames | total busy | **per forged frame** | max |
|---|---:|---:|---:|---:|---:|
| **v0B (before)** | 1/s | 30 | 5.379 s | **179.3 ms** | 181.7 ms |
| **v0B (before)** | 2/s | 60 | 10.762 s | **179.4 ms** | 181.3 ms |
| **v0C (after)** | 1/s | 30 | 0.210 s | **0.70 ms** | 190.1 ms ¹ |
| **v0C (after)** | 2/s | 58 | 0.230 s | **0.70 ms** | 190.1 ms ¹ |

¹ The 190 ms maximum is the **first frame only**: it pays the one-off X25519 link-key
derivation (`dh_derivations=1` in both runs). Removing it gives
(210 430 − 190 073)/29 = **702 µs** and (230 090 − 190 105)/57 = **701 µs** — two
independent rates agreeing to 1 µs.

**Result: 179.4 ms → 0.70 ms per forged frame, 256× cheaper**, plus 190 ms once per
peer. `busy_us` times only the `process*()` call, so it is free of logging and I/O.

Saturation of one relay by forged traffic alone: **5.6 frames/s before**
(1 / 0.1794), ~1 400/s after on CPU — far above what this UART can carry, so the link
becomes the limit instead of the CPU. That is the point of the change.

The 0.70 ms is the whole receive path for a refused frame (header parse, registry
lookup, cached-key fetch, HMAC-SHA256 over ~140 B, budget), not a bare HMAC.

## 2. Availability under flood — inconclusive on this testbed

With legitimate traffic mixed in (1/s) at 4 and 8 forged/s, 60 s:

| Mode | rate | sent (forged+legit) | frames seen by C | legit accepted | crc_fails | busy |
|---|---:|---:|---:|---:|---:|---:|
| v0B | 4/s | 240 + 60 | 37 | 31 | 32 | 6.63 s |
| v0B | 8/s | 479 + 60 | 33 | **0** | 33 | 5.82 s |
| v0C | 4/s | 240 + 60 | 33 | 16 | 16 | 3.08 s |
| v0C | 8/s | 479 + 60 | 60 | 16 | 17 | 3.11 s |

**These numbers do not measure the pre-filter and must not be quoted as if they did.**
C saw 33–60 frames out of 300–539 sent, with 16–33 CRC failures in every run. The cause
is the firmware's UART receive path, not the crypto: there is no interrupt-driven RX ring
buffer, so when a 190 ms verification blocks the loop the 32-byte FIFO overruns, the HAL
discards the bytes, and the deframer loses sync and eats frames until it happens to
resynchronise. This limit was already recorded in Phase 1.2 ("no flow control; a receive
interrupt with a ring buffer would remove the dependency on timing").

The only thing visible through that noise: at 8 forged/s in v0B, **zero** legitimate
messages were accepted, against 16 in v0C. It points the right way but rests on runs
with 17–33 CRC failures, so it is an observation, not a measurement.

A clean availability figure needs an interrupt-driven RX ring buffer on the relay. Not
done; out of this phase's scope.

## 3. What the budget did

**Nothing, in every run: `drop_budget=0` throughout.** That is correct behaviour — the
forged frames die at the link tag and never reach the budget, and the legitimate rate
(1/s) is below it (2/s sustained, burst 24). The token bucket is therefore **unit-tested
only** (16 `prefilter` tests, including burst, sustained flood and a backwards clock);
its effect against an insider who holds a link key is **not measured on silicon**. Doing
so needs an injector that owns a valid link key, i.e. a third enrolled board in the mode.

## 4. Three defects this measurement found (all mine, all fixed)

Each would have produced a confident, wrong number.

1. **Per-frame X25519 (implementation, `a8d97a2`).** The first sweep measured **370 ms
   per frame** in v0C — *double* an Ed25519 verify, not a pre-filter. `process_v0c`
   derived the link key on every frame, and on a Cortex-M0+ an X25519 scalar
   multiplication costs about as much as the signature verification the filter exists to
   avoid. Fixed with `LinkKeys`, a no-alloc per-peer cache: one DH per peer, then HMAC
   only. The 190 ms first-frame cost in §1 is that DH, now paid once.
2. **The injector signed every forged frame (method, `8dc1822`).** It capped itself at
   3.7/s when 8/s was asked for, and worse, it *accidentally paced* the traffic so the
   relay could read everything — while the later v0C runs, with signing removed, sent
   catch-up bursts exactly when the relay was busy. The two halves of the comparison
   were measured under different traffic. Fixed: legitimate frames are signed **before**
   the timed loop, and the forged schedule re-anchors instead of accumulating.
3. **Two commands in one USB write (harness).** The firmware keeps a single line buffer
   per USB read, so `@O…` followed by `@S1` in one write silently lost the first. B
   stayed in v0B while C expected v0C, and every frame was dropped as a downgrade. It
   was timing-dependent on USB packet boundaries, so **earlier runs cannot be trusted at
   all**. `pf_run.sh` now sends one command per write and **verifies on both boards**
   that the mode took (`PF_MODE`, and `PF_FLOOD … v0c=`), aborting the run otherwise.

## 5. Limits

- **The 256× is a CPU figure per refused frame**, measured with forged traffic only. It
  is not an availability claim: see §2.
- **The budget is not silicon-measured** (§3), so the insider half of the design rests
  on unit tests.
- **Two nodes, one wired point-to-point link.** No radio; radio broadcast with N
  neighbours would need one tag per neighbour and is out of scope (TRL 6/7).
- **The relay's UART receive path saturates around the same rate as its verifier**
  (~5/s) on this firmware, so on this testbed the two limits cannot be separated above
  that rate.
- **Kani for the pre-filter is written but NOT RUN** (3 harnesses: 2 budget, 1 v0C
  parser). The local WSL is unusable since the rsync incident. The ordering property
  (no Ed25519 without a valid tag, within budget) is not Kani-modellable through real
  Ed25519/HKDF/HMAC and is covered by the Ed25519-call-counter tests instead.
- A stolen node still yields its own links' keys (flash is readable on an RP2040); the
  blast radius is one node, which is why per-link keys were chosen over a network key.
