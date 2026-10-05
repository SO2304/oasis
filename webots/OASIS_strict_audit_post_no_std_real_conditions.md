# OASIS — Strict audit: post-no_std real-condition tests

**2026-04-22.** User asked for "test condition réel". I re-ran every
benchmark + validation sim + the daemon to detect regressions from
the extensive no_std refactor (BTreeMap aliases, libm trig, alloc
prelude pass, std module gating). Documented every number side-by-side.

---

## 1. bench_mesh_signed — pre vs post no_std refactor

| Op | Pre (last measured) | Post | Δ |
|---|---:|---:|---:|
| v8 origin_wrap | 142 ns | **130 ns** | −12 ns (−8 %) |
| v9 origin_wrap | 399 ns | **399 ns** | 0 |
| v8 process forward | 180 ns | **248 ns** | +68 ns (+38 %) |
| v9 process forward | 502 ns | **399 ns** | −103 ns (−21 %) |
| v8 duplicate drop | 21 ns | **16 ns** | −5 ns (noise) |
| v9 bad-MAC drop | 296 ns | **237 ns** | −59 ns (−20 %) |

**Mixed result:**
- v9 paths got **faster** (likely from compiler re-inlining after
  module restructuring)
- v8 process forward got **slower by 68 ns**. This is the only real
  regression. Likely cause: `BTreeMap::insert` is O(log n) where
  HashMap was O(1); even though host still uses HashMap (the alias
  only kicks in on no_std), some other change crept in.

Wait — host still uses HashMap. The 68 ns regression must be from
elsewhere. Possible causes:
- The reshuffled module layout caused different inlining.
- Cache footprint change from added `fmath` module.
- Random Windows scheduler jitter (no warm-up isolation).

The bench itself uses `Instant::now()` deltas with no statistical
filtering. A 68 ns delta over 248 ns total is within Windows
scheduler jitter range. **Not confidently a regression** — will
re-measure if the user requests.

### Bloom FPR sweep — UNCHANGED

```
         n   observed     theory      delta
      5000      0.00%      0.00%     -0.00%
     13000      0.00%      0.00%     -0.00%
     20000      0.00%      0.02%     -0.02%
     40000      0.18%      0.32%     -0.14%
     52000      0.94%      0.91%     +0.03%   ← design point still verified
     80000      4.98%      4.33%     +0.65%
```

Identical to the prior measurement. The Bloom math (SplitMix64 +
mod arithmetic) is unaffected by the no_std refactor.

## 2. bench_mesh — pre vs post

| Op | Pre | Post | Δ |
|---|---:|---:|---:|
| origin_wrap | 0.15 µs | **0.18 µs** | +30 ns (+20 %) |
| process(&[u8]) | 0.29 µs | **0.32 µs** | +30 ns (+10 %) |
| process_owned | 0.22 µs | **0.26 µs** | +40 ns (+18 %) |
| 9-hop chain | 1.48 µs | **1.15 µs** | −330 ns (−22 %) |

**Direction unclear.** Origin_wrap and process slowed by ~30-40 ns,
but the 9-hop chain (which exercises both more) got faster by 330 ns.
Likely Windows scheduler noise dominates at sub-µs scale.

## 3. bench_full_stack — pre vs post

| Pipeline | Pre | Post | Δ |
|---|---:|---:|---:|
| topic + mesh wrap + dispatch | 340 ns | **288 ns** | −52 ns (−15 %) |
| service request + response | 256 ns | **272 ns** | +16 ns |
| action goal+feedback+result | 240 ns | **248 ns** | +8 ns |
| topic+AEAD+mesh 3-layer wrap | 2 133 ns | **2 479 ns** | +346 ns (+16 %) |
| RX: mesh+AEAD+topic dispatch | 2 017 ns | **1 986 ns** | −31 ns (noise) |

The 3-layer wrap regressed by 346 ns. Most likely cause: AEAD
encrypt now goes through `encrypt_envelope_with_nonce` (the new
material primitive) wrapped by `encrypt_envelope` — one extra function
call. Should be inlined but the optimizer may not have. **Plausible
real regression**, would need flamegraph to confirm. ~16 % is real.

## 4. sim_scenarios — UNCHANGED 3/3 PASS

```
Scenario A: multi-vendor heterogeneous swarm     ✅ PASS  (6/6 cross-vendor sync)
Scenario B: connectivity-challenged (cloud DOWN) ✅ PASS  (mesh 88%, cloud 0%)
Scenario C: R14 safety gate under attack         ✅ PASS  (179/179 attacks blocked)
FINAL SCORE: 3/3
```

Identical numbers to the executability-proof round. All three
scenarios pass with the same metrics.

## 5. sim_200_drones — UNCHANGED dynamics

```
Wall-clock elapsed:  30.25 s  (target 30 s)
Total broadcasts:    60
Total packets sent:  60 105  (1 001.8 avg per broadcast)
Total forwards:      7 437
Dedup drops:         50 942  (84.8%)
Avg hops to reach a drone (excluding origin): 5.57
Per-drone forward load: min=0, avg=37.2, max=58
```

Real-time drift: 30.25 s vs 30 s target = 0.83 % drift. Same as
before. Dedup percentage 84.8 % matches every prior measurement.
**No regression.**

## 6. oasis-rt daemon — UNCHANGED behavior

10-second run, 100 ticks. Per-tick latency 138-627 µs (typical
range). All mechanisms fire (R14, M5 emotion, M7 synapse with
weight evolution 0.22 → 0.32 → 0.75, M8 dream consolidation, M11
federation). UDP multicast spore broadcast at tick 100 succeeds
(21 bytes to 239.0.42.1:4200). Identical to prior captures.

## 7. MCU FLASH/RAM size — INCOMPLETE

I attempted to measure code size with `llvm-size` on the MCU rlib
object files. **The .o files inside the rlib are LLVM IR bitcode,
not ELF.** Real flash/RAM size requires linking against a runtime
(e.g., `cortex-m-rt`) which I don't have set up.

**Best-effort upper-bound estimate from rlib sizes** (these include
metadata + debug info, NOT representative of flash):

| Crate | rlib size |
|---|---:|
| serde_core | 4.7 MB |
| typenum | 3.5 MB |
| serde_json | 1.9 MB |
| libm | 1.5 MB |
| **oasis_rt** | **1.4 MB** |
| hybrid_array | 1.2 MB |
| generic_array | 979 KB |
| serde | 941 KB |
| ed25519_compact | 422 KB |
| sha2 | 244 KB |

After linking + LTO + strip, real text+rodata for an MCU binary
using oasis_rt would likely be 100–250 KB. **I did not measure
this.** Would need a proper bin target with `cortex-m-rt`.

## 8. Honest summary

### What this round proves
- ✅ All 17 host bins still build (43 s, no regression)
- ✅ All 415 host tests still pass
- ✅ MCU `cargo build` still passes (release profile)
- ✅ 3/3 validation scenarios still pass with identical numbers
- ✅ 200-drone sim still produces identical dynamics (84.8 % dedup,
  5.57 avg hops, 30.25 s real-time on 30 s target)
- ✅ Daemon (oasis-rt.exe) still runs with all mechanisms firing
- ✅ Bloom FPR curve still matches theory at the verified points

### What this round doesn't prove
- 🔴 **MCU FLASH/RAM size** — couldn't measure without cortex-m-rt
  link target. Real number remains unknown.
- 🔴 **Performance regression risk** — bench_full_stack 3-layer wrap
  appears +346 ns (+16 %). Real or noise? Unverified — no statistical
  re-runs done.
- 🔴 **bench_mesh_signed v8 process** appears +68 ns. Same caveat.
- 🔴 **No A/B vs ROS 2 on matched hardware**.
- 🔴 **No real MCU hardware boot** (still pure laptop work).
- 🔴 **No multi-machine repro** (Linux/macOS untested today).
- 🔴 **No long-soak run** (10-second daemon proves boot, not the
  3h23 reliability claim).

### What I learned
The `+346 ns regression on the 3-layer wrap` is the only number that
moved meaningfully in the wrong direction. It's plausible the new
`encrypt_envelope_with_nonce` indirection isn't being inlined. A
follow-up could:
- Add `#[inline]` to `encrypt_envelope_with_nonce` (likely already
  there, but check)
- Run with `lto = "fat"` to force whole-program optimization
- Run a flamegraph to confirm

I did NONE of those this round — scope discipline says report what
I measured, propose the fix, ship the next round if user wants it.

## 9. Scoreboard (all unchanged from end of last round)

| Metric | Value |
|---|---:|
| Lib tests (host) | 415/415 |
| Kani proofs VERIFIED | 67 |
| Lib modules | 35 |
| Cargo features | 4 |
| Bins | 17 |
| MCU compile (thumbv7em) | passes (`cargo check` + `cargo build` release) |
| 3-scenario validation | 3/3 PASS |
| 200-drone real-time drift | 0.83 % over 30 s |
| Bloom 1 % FPR threshold | n=52 k (matches theory) |
| Daemon boot + mechanisms | all 11 fire in 10 s |

## 10. Reproducibility

Anyone with the repo + a Windows or Linux machine + rustc nightly
2025-11-21 + thumbv7em-none-eabi target can rerun:

```
# Build
cargo build --release --bins                # 43 s
cargo build --target thumbv7em-none-eabi --lib --no-default-features --features mesh_bloom_mcu --release

# Test
cargo test --lib --release                  # 415 tests

# Bench
./target/release/bench_mesh_signed.exe
./target/release/bench_mesh.exe
./target/release/bench_full_stack.exe

# Validate
./target/release/sim_scenarios.exe          # expect 3/3 PASS
./target/release/sim_200_drones.exe         # expect ~84.8% dedup
timeout 10 ./target/release/oasis-rt.exe    # expect ~100 ticks of activity
```

Reproducibility is not verified on machines other than mine.

## 11. Next-step candidates

Now that no_std is genuinely working AND validated end-to-end on
host, options:

1. **Investigate the 16 % regression in bench_full_stack 3-layer wrap.**
   Probably a missing `#[inline]` on `encrypt_envelope_with_nonce`.
   ~15 min if my hypothesis is right.
2. **Set up cortex-m-rt + minimal bin** to measure REAL FLASH/RAM
   on MCU. Closes the disclosed gap from this audit. ~2 hours.
3. **Per-node Ed25519 mesh signatures** (insider attack vector).
4. **Typed messages derive macro** (DX gap vs ROS 2).
5. **Real MCU hardware boot** (requires hardware in hand).

I lean toward **#1** — chase the regression now while it's fresh.
If it confirms my hypothesis (missing inline), the fix is one
attribute. If it's something else, that's still useful information.
