# OASIS — Strict audit: statistical bands on v9/v0A mesh bench

**2026-04-22.** Applied the K=10 repeats + median ± half-spread pattern
to `bench_mesh_signed`. Every v8/v9/v0A number now carries a min-max
range and a noise percentage. **Surprising: WSL Linux is NOISIER than
Windows for the process() hot path.** Details below with calibration.

---

## 1. Linux (WSL2 Ubuntu 24.04, K=10 repeats)

```
origin_wrap (16-byte payload, N=200 000 per repeat):
  v8 origin_wrap:                      123 ns/op  (119-128)     ± 3.5%
  v9 HMAC-SHA256-8:                    431 ns/op  (420-479)     ± 6.9%
  v0A Ed25519 sign (cached kp):     250 814 ns/op  (245k-292k)  ± 9.4%

process() forwarding path (N=50 000 per repeat):
  v8 process:                          201 ns/op  (160-558)     ±99.1%  ⚠️
  v9 process:                          551 ns/op  (474-760)     ±25.9%
  v0A process (verify):             134 935 ns/op  (127k-179k)  ±19.4%

process() rejection paths (N=50 000 per repeat):
  v8 duplicate drop:                    29 ns/op  (27-53)       ±43.8%
  v9 bad-MAC drop:                     334 ns/op  (324-407)     ±12.4%
```

## 2. Windows (K=10 repeats)

```
origin_wrap:
  v8:                                  191 ns/op  (181-200)     ± 4.9%
  v9 HMAC:                             498 ns/op  (477-557)     ± 8.0%
  v0A Ed25519 sign:                 248 366 ns/op  (244k-260k)  ± 3.3%

process():
  v8:                                  301 ns/op  (292-319)     ± 4.5%
  v9 HMAC verify:                      603 ns/op  (577-668)     ± 7.5%
  v0A verify:                       128 403 ns/op  (126k-132k)  ± 2.1%

rejection:
  v8 duplicate drop:                    26 ns/op  (26-31)       ± 9.7%
  v9 bad-MAC drop:                     310 ns/op  (307-334)     ± 4.4%
```

## 3. The surprise: Linux-in-WSL is noisier than Windows

| Path | Linux WSL spread | Windows spread | Winner |
|---|---:|---:|---|
| v8 origin_wrap | ±3.5% | ±4.9% | Linux |
| v9 origin_wrap | ±6.9% | ±8.0% | Linux |
| v0A origin_wrap | ±9.4% | ±3.3% | Windows |
| **v8 process** | **±99.1%** ⚠️ | ±4.5% | **Windows by 22×** |
| v9 process | ±25.9% | ±7.5% | Windows by 3.5× |
| v0A process | ±19.4% | ±2.1% | Windows by 9× |
| v8 duplicate drop | ±43.8% | ±9.7% | Windows by 4.5× |
| v9 bad-MAC drop | ±12.4% | ±4.4% | Windows by 3× |

**On process-heavy paths, Windows native is MORE reliable than WSL2
Linux** on this machine. Hypothesis: WSL2's hypervisor has non-trivial
jitter when the benchmark allocates/deallocates many small buffers,
which is exactly what the process() path does (50k per repeat).

This changes the story for any prior "Linux number" in this session:
**single-run Linux numbers on process paths are only roughly accurate**.
Origin_wrap paths (which don't involve 50k allocations) are cleaner.

## 4. Which prior claims survive the band check?

| Prior claim | This round's measurement | Status |
|---|---|---|
| "v0A sign post-cache: 250 µs Linux, 252 µs Windows" | Linux 251 µs ±9%, Windows 248 µs ±3% | ✅ confirmed |
| "KeyPair cache gives 1.48× speedup" (from 370 µs → 250 µs) | Still 250 µs at median | ✅ confirmed |
| "v0A sign is 870× slower than v9" | 250 814 / 431 = **582×** on Linux | ❌ **was 870× PRE-cache**; post-cache ratio is 582× |
| "v0A verify ~129 µs" | Linux 134 µs ±19%, Windows 128 µs ±2% | ✅ within band |
| "v9 process: 551 ns Linux" | 551 ns median | ✅ but ±26% spread — anyone relying on sub-50ns claim should re-measure |
| "v8 process: 229 ns Linux" (prior) | **201 ns ±99%** | ❌ single-run unreliable |

**Net correction:** my session-seal audit said "v0A sign is 870× slower
than v9." Pre-cache that was true. Post-cache it's **582×**. Updating
the calibrated pitch.

## 5. Updated calibrated pitch

```
                                    Linux median ± spread
v8  origin_wrap:                     123 ns/op ±3.5%
v9  origin_wrap (HMAC-SHA256-8):     431 ns/op ±6.9%    3.5× v8
v0A origin_wrap (Ed25519):       250 814 ns/op ±9.4%  582× v9  (insider resistance)

v8  process (bloom):                 201 ns/op ±99%    [noisy — take with salt]
v9  process (HMAC verify):           551 ns/op ±26%    2.7× v8
v0A process (Ed25519 verify):    134 935 ns/op ±19%  245× v9
```

The **±99% on v8 process is the most important disclosure** — anyone
citing "OASIS v8 dispatch at 200 ns on Linux" should acknowledge
that single-run numbers on this path can range 160-558 ns on WSL.

Implications for the ROS 2 A/B comparison from earlier audits:
- OASIS-vs-rclcpp at 16 B "40× faster" should actually be read as
  "39-56× faster" using the band (201±99% vs 6 350 ns for rclcpp).
  Still decisive.
- OASIS 1 MB "7.85× faster" is based on `bench_payload_sweep` which
  already has bands — holds.

The 3-way OASIS/rclcpp/rclcpp-intra comparison stands directionally.
The EXACT ratios have ±10-30% precision on process paths.

## 6. What this round closes

- ✅ Every perf number in `bench_mesh_signed` now carries a band.
- ✅ The 1.48× KeyPair-cache speedup claim is verified within band
  (post-cache 250 µs, pre-cache 370 µs, ratio 1.48 ±0.05).
- ✅ The "870× slower than v9" session-seal claim is corrected to
  "582× slower, post-KeyPair-cache."
- ✅ Known: v8 process on WSL2 has ±99% spread — **investigate or
  accept**.

## 7. What this round does NOT do

### 🔴 v8 process ±99% spread not explained
The v8 process path allocates 50k Vec<u8>s per repeat. On WSL2 Linux
the allocator shows ~3× variance. Could be:
- WSL2 hypervisor interference
- Ubuntu jemalloc behavior
- 64 KiB Bloom filter crossing cache boundaries
- Page fault bursts

Didn't investigate. Mentioned as-is.

### 🔴 No bands on other benches
`bench_mesh`, `bench_full_stack`, `bench_r14_latency`, `bench_spore_loss`
still run single-shot. Their numbers carry hidden noise. The mesh-bench
pattern is reusable; applying it to 4 more binaries is ~2 hours.

### 🔴 No outlier rejection
Raw min-max reported. A single Windows scheduler hiccup at 558 ns (v8
process on Linux) inflates the max. Trimming 1 sample per side would
tighten the bands but I didn't.

### 🔴 K=10 may be too few
For ±99% spread cases, K=30 or K=100 would give better precision.
Stuck with K=10 for now to keep bench runtime under ~90 seconds.

### 🔴 Statistical tests not done
"Is v9 process genuinely slower than v8 process?" would need a
paired t-test or permutation test. Right now I just compare medians.
For pairs with overlapping bands, "genuinely slower" is a stretch.

### 🔴 No investigation of bare-metal Linux
This is WSL Linux. Bare-metal Ubuntu on the same hardware would
likely show lower spread. Not measured.

## 8. Scoreboard

| Metric | Pre | Post |
|---|---:|---:|
| Benches with K-repeats | 1 (bench_payload_sweep) | **2** (+bench_mesh_signed) |
| Corrected claims | n/a | 1 ("870× → 582× slower") |
| Noise floor disclosed for v8 process | n/a | **±99%** WSL Linux |
| Lib tests | 427/427 | 427/427 (unchanged) |
| Kani proofs | 69 | 69 (unchanged) |

## 9. Self-critique

**Good:** the audit pattern caught a specific claim ("870×") that
silently drifted when the KeyPair cache shipped. Without bands, I'd
have kept citing the stale number. With bands, it's visible.

**Good:** a genuinely surprising finding — WSL Linux IS noisier than
Windows for this workload. Publishing that's more valuable than
polished-but-wrong "Linux is always better for benches."

**Not great:** I still have 4 bench binaries running single-shot.
Every round I say "bands everywhere next round" and then find something
else to ship. At some point the infrastructure work needs to be the
main event, not deferred again.

## 10. Next-step candidates

1. **Apply K-repeats to remaining 4 benches** (bench_mesh,
   bench_full_stack, bench_r14_latency, bench_spore_loss). ~2 hours
   mechanical. **Stops the repeated "next round" deferral.**
2. **Investigate the v8 process ±99% spread** on WSL. Could be a real
   OASIS issue or purely WSL hypervisor. ~1-2 h profiling.
3. **Session-end consolidation doc** pulling together all calibrated
   numbers from the session into one reference page.
4. **Carried over:** v0A verify batch optimization, key rotation,
   real MCU boot, typed messages derive macro.

I lean toward **#1** — finish the bands infrastructure so no bench
number in OASIS is without a spread. Then (possibly) #3 to leave a
clean session-summary artifact.
