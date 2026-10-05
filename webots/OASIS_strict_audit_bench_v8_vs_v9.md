# OASIS — Strict audit: v8 vs v9 mesh bench + prior FPR error correction

**2026-04-22.** This round is purely measurement. It closes the
"v9 process() delta not measured" item from the last two audits and
— in the process — uncovers a real error in my prior Bloom FPR
estimates. Both are reported here.

---

## 1. Measured numbers (Windows 11 laptop, release mode)

### `origin_wrap` (building the envelope)

| Variant | ns/op | ops/s | Delta vs v8 |
|---|---:|---:|---:|
| v8 origin_wrap (no signing) | 204 | 4.9 M | baseline |
| v9 origin_wrap (HMAC-SHA256-8 tag) | **517** | 1.93 M | **+313 ns (+153 %)** |

The +313 ns is two SHA-256 compressions for HMAC (ipad + opad over a
22-byte message). Matches expectation.

### `process()` — forwarding path (unique msg_ids)

| Variant | ns/op | ops/s | Delta |
|---|---:|---:|---:|
| v8 process (parse + dedup + bloom insert) | 266 | 3.76 M | baseline |
| v9 process (parse + HMAC verify + dedup + bloom insert) | **558** | 1.79 M | **+292 ns (+110 %)** |

### `process()` — rejection paths

| Variant | ns/op | ops/s |
|---|---:|---:|
| v8 duplicate drop (exact HashSet hit) | **23** | 43.4 M |
| v9 bad-MAC drop (full HMAC compute + compare) | **330** | 3.03 M |

v9 bad-MAC drop is **14× slower** than v8 duplicate drop. This matters
because an attacker who knows the v9 swarm's magic byte can spam
bad-MAC envelopes at the victim. At 330 ns per rejection, a 3 M/s
attack saturates one core. **Real DoS window, but bounded.**

## 2. Prior FPR estimate — was wrong

My prior mesh-Bloom audit claimed:

> "False-positive rate at ~20 k inserts: ≈ 0.3 %."

Measurement disagrees. With 20 k msg_ids inserted into the default
2048-word / 5-hash Bloom, then 100 k fresh probes run through
`process()` (where accepted probes are also inserted, compounding
saturation):

```
probe size = 100 000, dropped-as-dup = 61 293, accepted = 38 707, FPR ≈ 61.3 %
```

### Why my prior estimate was off
I eyeballed the FPR against a hand-picked design point without running
the standard Bloom formula. Recomputing properly:

```
m = 131 072 bits,  k = 5 hashes
FPR(n) = (1 - exp(-k·n/m))^k
```

| n (inserts) | theoretical FPR |
|---:|---:|
|  5 000 |  0.03 %   |
| 10 000 |  0.44 %   |
| 13 000 |  1.0 %    |
| 20 000 |  **4.3 %** ← was claimed as 0.3 % |
| 26 000 | 10.0 %    |
| 40 000 | 26.6 %    |
| 54 000 | 50.0 %    |
| 80 000 | 78.6 %    |

I was **off by roughly 14×** at n = 20 000. The measured 61.3 % matches
the theoretical curve when you account for the bench's insert-as-you-probe
feedback: starting with n = 20 k and accepting ~39 k additional probes,
the Bloom saturates to n ≈ 60 k where theoretical FPR ≈ 63 %. The
measured number is consistent with theory; only my prior estimate was
not.

### What this means for the design and for the prior "fix"
- **The mesh-Bloom "fix" is weaker than previously claimed.** It keeps
  a legitimate dedup window for on the order of **10–15 k inserts**,
  not 50 k.
- **My prior recommendation** to reset "after ~50 k inserts" leaves the
  filter in a regime where ~45 % of fresh messages get dropped as
  false-positive duplicates. That's **unacceptable for most use
  cases**.
- **Corrected recommendation:** call `bloom_reset()` after **10–15 k
  inserts** for ≤ 1 % FPR. For a drone receiving 100 pkt/s from 40
  peers, that's ~30 seconds between resets. Frequent, but honest.

### Concrete fix paths (not done this round)
1. **Bigger Bloom** — bump `BLOOM_WORDS` from 2048 to 8192 (64 KiB per
   router). Raises 1 % FPR threshold from 13 k to ~52 k inserts.
2. **Counting Bloom** — allow entries to age out individually. ~4× RAM.
3. **Timestamped dedup** — add a monotonic tick to each envelope; drop
   anything older than MAX_AGE. Requires wire-format change. Best fix.

## 3. Scoreboard

| Metric | Pre | Post |
|---|---|---|
| Lib tests | 396 | 396 |
| Kani proofs VERIFIED | 67 | 67 |
| Kani failures | 0 | 0 |
| Binaries | 15 | **16** (+bench_mesh_signed) |
| Measured v8/v9 deltas | unmeasured | **measured** |
| Prior FPR claim status | unverified | **confirmed wrong, 14× off** |

## 4. What this audit closes

- ✅ v8/v9 process() cost delta now has concrete numbers (+292 ns).
- ✅ v9 origin_wrap cost delta measured (+313 ns).
- ✅ v9 bad-MAC DoS cost quantified (330 ns = 3 M rejections/sec).
- ✅ **My prior Bloom FPR estimate is publicly corrected.**

## 5. What this audit does NOT close

### 🔴 FPR is now disclosed as higher than previously claimed, but not fixed
I chose to document the correction rather than ship a bigger Bloom
this round. Fixing it in code means either +48 KiB RAM per router or
a wire-format change. Both are non-trivial. Flagging as a known gap,
not covering it up.

### 🔴 Still-open items from prior audits
- Per-node Ed25519 (insider attacker).
- `tx_counter` persistence.
- Key rotation for `MeshMacKey`.
- HKDF helper to derive `MeshMacKey` from swarm PSK.
- 200-drone sim re-run with v9.
- MCU/embedded target characterization.
- No A/B vs ROS 2 on matched hardware.
- 64-bit MAC tag, not 128-bit.

### 🔴 Bench limitations disclosed
- Single laptop, Windows, release mode, unloaded.
- No cross-platform validation (Linux, macOS, ARM64 untested this round).
- Forward-path bench runs through a SINGLE hop router, not a realistic
  chain of 3–8 hops under flood dedup pressure.
- FPR check uses in-memory envelope replay, not UDP-on-the-wire with
  loss + reorder.
- Warm-up is only 10 % of main iteration count; no attempt to isolate
  Windows scheduler jitter.

## 6. Concrete numbers to cite from now on

When anyone asks about v9 cost:
> "v9 signing adds ~300 ns per origin_wrap and ~290 ns per process().
> That's a 2× CPU cost on the mesh hot path vs unsigned. At 100 pkt/s
> per drone that's 60 µs/sec = 0.006 % of one core."

When anyone asks about Bloom:
> "16 KiB Bloom with 5 hashes. ≤ 1 % false-positive rate for the first
> 13 000 inserts. Past that, FPR climbs fast — reset is recommended every
> 10–15 k inserts. I previously quoted a lower FPR; that was wrong by
> an order of magnitude."

## 7. Self-critique of the audit process

Three audits in a row claimed things that were either not measured or
measured wrong:
- Session start: Kani proof counts self-consistent but rigor levels
  were not broken down until I did it strictly.
- Mesh bloom audit: quoted FPR without computing it. Off by 14×.
- V9 audit: cited expected +3 µs MAC cost, actual is +0.3 µs (10×
  better than guess — wrong in the other direction).

Pattern: I was doing order-of-magnitude estimates when I should have
been doing benchmarks. This round is a course correction. The audit
pattern needs a measurement step as a regular gate, not an optional
flourish.

## 8. Next session candidate

Either:
- **Bigger Bloom (8 192 words = 64 KiB)** + run the bench again to
  verify FPR curve. Cheap, improves the correction.
- **Timestamp dedup** (SPORE\x0A wire-format change) — proper fix.
- **Per-node Ed25519** — close insider case.
- **`tx_counter` persistence** — still unstarted.

I recommend the bigger Bloom. It's the direct, measurable response to
the error documented here.
