# OASIS — Strict audit: 64 KiB Bloom + measured FPR curve

**2026-04-22.** Direct, measurable fix for the FPR underestimate
documented in the previous audit. `BLOOM_WORDS` raised 2 048 → 8 192
(16 KiB → 64 KiB per router), and the bench was extended to sweep FPR
across multiple `n` values so future claims can be checked, not
estimated.

---

## 1. What shipped

### Code changes
- `BLOOM_WORDS: usize = 8192` (was 2048). Bloom bit count: 524 288 (was 131 072).
- Module docstring corrected with the new FPR table, explicit reference to the prior error, and updated operator guidance (reset every ~40 k inserts, was ~10–15 k).
- `long_memory` field doc updated accordingly.

### Bench changes
- `bench_mesh_signed` now sweeps FPR across `n ∈ {5k, 13k, 20k, 40k, 52k, 80k}` with a **fresh router per `n`** (no probe-feedback pollution) and compares observed vs theoretical `(1 - exp(-kn/m))^k`.

## 2. Measured FPR curve (observed vs theory)

```
         n   observed     theory      delta
      5 000      0.00 %      0.00 %      0.00 %
     13 000      0.00 %      0.00 %      0.00 %
     20 000      0.00 %      0.02 %     −0.02 %
     40 000      0.18 %      0.32 %     −0.14 %
     52 000      0.94 %      0.91 %     +0.03 %
     80 000      4.98 %      4.33 %     +0.65 %
```

- **Design point verified**: at `n = 52 000`, observed FPR = 0.94 % ≈ theoretical 0.91 %. This is the 1 % threshold I claimed.
- **Theory matches measurement** within 0.65 % across all test points (noise dominated; 5 000 probes per `n` is not a large sample).
- **No more hidden probe-feedback compounding**: the prior bench ran 100 k probes through one saturating bloom; this bench uses a fresh router per `n` and probes only 5 000, keeping observed FPR close to the pre-probe design value.

## 3. Throughput after the resize

Re-run of the hot-path bench:

| Op | pre-64 KiB (16 KiB Bloom) | post-64 KiB | Δ |
|---|---:|---:|---:|
| v8 origin_wrap | 204 | 219 | **+15 ns (+7 %)** |
| v9 origin_wrap | 517 | 514 | −3 ns (noise) |
| v8 process forward | 266 | 308 | **+42 ns (+16 %)** |
| v9 process forward | 558 | 599 | **+41 ns (+7 %)** |
| v8 duplicate drop | 23 | 23 | 0 |
| v9 bad-MAC drop | 330 | 319 | −11 (noise) |

**The 41–42 ns regression on the process() hot path is real.** Most
likely cache pressure: the 64 KiB Bloom doesn't fit L1 (typically
32 KiB on x86_64), so each of the 5 Bloom bit touches may incur an
L2 access. On a Cortex-M MCU with smaller caches (or no cache), the
relative cost will be higher.

This is the direct trade I signed up for:

> "Quadruple the Bloom and the 1 % FPR threshold moves from ~13 k to
> ~52 k inserts. Cost: +48 KiB RAM + ~40 ns per process() call."

## 4. Scoreboard

| Metric | Pre | Post |
|---|---|---|
| Lib tests | 396 | **396** |
| Kani proofs | 67 | **67** |
| `MeshRouter` heap per instance | ~56 KB | **~104 KB** (+48 KiB) |
| 1 % FPR threshold (Bloom) | ~13 k inserts | **~52 k inserts** (4×) |
| process() forward latency (v8) | 266 ns | 308 ns (+16 %) |
| 200-drone in-process sim RAM budget (rough) | ~11 MB | ~21 MB |

## 5. What this audit does

- ✅ Delivers the concrete fix I recommended.
- ✅ Verifies measurement matches theory (1 % at 52 k, 5 % at 80 k).
- ✅ Discloses the real cost honestly (+40 ns process, +48 KiB RAM).
- ✅ Improves the bench itself — future FPR claims can now be
  re-verified with a single command.

## 6. What this audit does NOT do

### Not fixed this round
- **No feature flag to opt out on MCU targets.** For Cortex-M with
  e.g. 32 KiB RAM total, 64 KiB Bloom is impossible. Conditional
  compilation (`#[cfg(feature = "mesh_bloom_full")]` etc.) not yet added.
- **No per-router Bloom sizing.** `new()` / `new_signed()` always
  allocate 64 KiB. Callers can't request a smaller filter even if they
  know their workload stays under 10 k inserts.
- **No auto-reset on threshold crossed.** `bloom_reset()` is still
  manual. An operator who forgets will eventually see FPR climb.

### Not measured this round
- **No benchmark on Linux / macOS / ARM.** All numbers are Windows.
  Cache behaviour may differ.
- **No real-radio UDP measurement.** The bench is in-process.
- **No 200-drone sim re-run with the bigger filter.** Would confirm
  end-to-end reach rate under realistic flood dedup pressure. Not done.
- **No comparison with a bigger filter (e.g. 128 KiB, 256 KiB).** The
  64 KiB choice was driven by the "push 1 % threshold to ~50 k" target;
  I did not sweep other sizes.

### Still carried from prior audits (unchanged)
- Per-node Ed25519 for insider-attack resistance.
- `tx_counter` persistence.
- `MeshMacKey` key rotation / HKDF derivation helper.
- A/B vs ROS 2 on matched hardware.

## 7. Updated operator guidance

For a router using default signing:

| Pkt/s in | Reset every |
|---:|---:|
|  10 | ~70 min |
|  50 | ~14 min |
| 100 | ~7 min |
| 500 | ~90 s |

"Reset every" = `40 000 / (pkt/s)`. Past that, FPR starts climbing;
above 80 k inserts (~5 % FPR) the filter should definitely be reset.

## 8. Self-critique

This round was a direct follow-through on the previous audit's
recommendation. Took ~1 hour of coding + benching. The measurement
pattern (theory column next to observed in the bench output) is
something I should adopt for every future bench — it makes drift
obvious.

The 41 ns process() regression is acceptable but NOT free. If a caller
is CPU-bound, this is real — they should know before adopting the new
default. The audit discloses it.

## 9. Next-step candidates

Same menu as before, minus the "bump Bloom" item:

1. **Feature flag for MCU** — `#[cfg(feature = "mesh_bloom_small")]`.
   Smallest code change that unblocks embedded users.
2. **`tx_counter` persistence** — still unstarted.
3. **Per-node Ed25519** — closes insider case left open by v9 MAC.
4. **Typed messages derive macro** — biggest DX gap vs ROS 2.
5. **200-drone sim re-run with v9 + 64 KiB Bloom** — validation.

I lean toward **#1** (feature flag) as the cheapest concrete win, then
**#2** (persistence) because those two together finalize mesh routing
as "production-ready" for both MCU and long-running deployments.
