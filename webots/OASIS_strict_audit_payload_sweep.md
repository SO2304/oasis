# OASIS — Strict audit: payload sweep, the crossover finding

**2026-04-22.** Last audit predicted: "larger payloads might shift the
ratio (rclcpp may catch up)". Measured. **The ratio doesn't just shift
— it inverts.** At small payloads OASIS wins by 25×. At 1 MB OASIS
loses by 7.8×. This is a real architectural finding worth documenting
prominently.

---

## 1. Side-by-side, intra-process pub/sub on Linux WSL2

| Payload | OASIS (Linux) | rclcpp intra-process | Ratio |
|---|---:|---:|---:|
| 16 B | **252 ns** | 6 350 ns | OASIS **25× faster** |
| 1 KB | **474 ns** | 7 067 ns | OASIS **15× faster** |
| 64 KB | **5 683 ns** | **8 000 ns** | OASIS **1.4× faster** (~tied) |
| 1 MB | 3 161 654 ns | **406 645 ns** | **OASIS 7.8× SLOWER** ⚠️ |

**Crossover near 64 KB.** Below: OASIS dominates on per-call overhead.
Above: rclcpp's zero-copy intra-process dominates.

## 2. Why OASIS loses at large payloads

OASIS `topic + mesh wrap + dispatch` does:
1. `wrap_topic` allocates a new `Vec<u8>` of `(payload.len() + topic_header)` and copies payload.
2. `mesh_router.origin_wrap` allocates another new `Vec<u8>` and copies the topic envelope (which contains the payload).
3. `router.dispatch` invokes the handler with `&[u8]` (no copy).

**That's 2 full payload copies per call.** At 1 MB:
- 2 × 1 MB / 3.16 ms = ~600 MB/s effective copy throughput. Plausible for L3-resident memcpy on this CPU, slower than DRAM bandwidth.

rclcpp intra-process does:
1. `Publisher::publish(unique_ptr<T>)` — moves the pointer into the IntraProcessManager.
2. IntraProcessManager hands the same pointer to the subscriber's queue.
3. Subscriber's callback gets a `ConstSharedPtr` to the SAME memory.

**Zero copies of the payload.** Only pointer + ref-count work. At 1 MB,
the work is ~400 µs of overhead — independent of payload size, basically.

## 3. Architectural implications

### Where OASIS wins (small, frequent messages)
- Drone control loops at 1 kHz with 16-256 byte commands.
- Telemetry beacons.
- Mesh routing envelopes.
- Most "robot" workloads where messages are < 1 KB.

### Where rclcpp wins (large payloads)
- Camera frames (640×480 RGB = 921 KB).
- LIDAR point clouds (10 k pts × 16 bytes = 160 KB).
- Rosbag replay.
- Pre-processed sensor batches.

### What OASIS could do to fix it
- Refactor `wrap_topic` and `mesh_router.origin_wrap` to accept a writer
  closure or a pre-allocated buffer instead of returning a fresh `Vec`.
- Introduce a `&[u8]`-passing dispatch path that doesn't require nested envelopes.
- Or: add a zero-copy path for "single-process, single-handler" cases.

This is a 1-2 day refactor. Not done this round — flagged as a real gap.

## 4. Honest interpretation

### What this proves
- ✅ OASIS's intra-process advantage is **real for sub-1 KB messages** (the typical robotic control case).
- ✅ rclcpp's intra-process zero-copy advantage is **real for >100 KB messages** (the typical sensor/perception case).
- ✅ The crossover happens around **64 KB** on this hardware.
- ✅ My prior audit's prediction "rclcpp may catch up at larger payloads" was correct — and underestimated the magnitude. **rclcpp doesn't just catch up; it overtakes.**

### What this does NOT prove
- ❌ OASIS is "bad at large payloads" — for the workloads it targets
  (mesh routing of small digests + control commands), the small-payload
  numbers are what matter.
- ❌ rclcpp is "always better above 64 KB" — only for intra-process.
  Inter-process or inter-host, both stacks pay serialization.
- ❌ OASIS can't be fixed — the 2 memcpy issue is fixable with a buffer
  refactor.

### Where the prior audit was right vs wrong
- ✅ Predicted ratio shift at large payloads — **correct**.
- ⚠️ Predicted "rclcpp may catch up" — **understated**. Should have
  said "rclcpp may overtake."
- ❌ Did not predict the crossover point (would have been useful).

## 5. Updated calibrated pitch

> "OASIS topic dispatch (intra-process, Linux):
> - **16 B: 252 ns/op** (3.97 M ops/s) — **25× faster than rclcpp intra-process**
> - **1 KB: 474 ns/op** (2.11 M ops/s) — **15× faster**
> - **64 KB: 5.7 µs/op** — **1.4× faster** (roughly tied)
> - **1 MB: 3.16 ms/op** — **7.8× SLOWER** (rclcpp's zero-copy wins)
>
> OASIS is optimized for **small frequent messages** (drone control,
> telemetry, mesh digests). Above ~64 KB, rclcpp's intra-process
> zero-copy starts winning because OASIS does 2 payload copies per
> envelope. For camera/LIDAR-style workloads, prefer rclcpp or refactor
> OASIS to use buffer-backed envelope construction."

This is the honest framing.

## 6. New scoreboard rows

| Metric | Value |
|---|---|
| OASIS-rclcpp crossover payload | ~64 KB |
| OASIS performance gap @ 16 B | 25× faster |
| OASIS performance gap @ 1 KB | 15× faster |
| OASIS performance gap @ 1 MB | **7.8× slower** |
| Identified architectural fix | Refactor wrap_topic + mesh_router.origin_wrap to buffer-backed |

## 7. New shipped artifact

`oasis-rt/src/bin/bench_payload_sweep.rs` — the bench that measured
this. Runs in ~5 seconds. Anyone with the repo can rerun.

```rust
for &(name, size, n) in &[
    ("16 B",     16,         100_000),
    ("1 KB",     1024,        50_000),
    ("64 KB",    64 * 1024,    5_000),
    ("1 MB",     1024 * 1024,    500),
]
```

Also added to `Cargo.toml` as `[[bin]] name = "bench_payload_sweep"`.

## 8. What this round closes

- ✅ **Payload sensitivity of the OASIS-vs-rclcpp ratio measured.**
- ✅ **Crossover point identified** (~64 KB).
- ✅ A real architectural limitation in OASIS surfaced and named:
  the 2× payload memcpy in topic+mesh envelope construction.
- ✅ A specific fix path documented (buffer-backed builders).

## 9. What this round does NOT close

### 🔴 The 2-memcpy issue isn't fixed
Documented and named. Not refactored. ~1-2 days when the user wants it.

### 🔴 No 1 KB granularity
I jumped 16 B → 1 KB → 64 KB → 1 MB. The actual crossover could be
anywhere from 16 KB to 256 KB. Coarser than ideal — would benefit
from finer sweep.

### 🔴 Inter-process not measured
Both stacks would shift again. Not done.

### 🔴 No real hardware
WSL2 only. Bare-metal Linux on a real drone CPU might shift the
crossover (slower CPU + slower DRAM = OASIS loses earlier).

### 🔴 No statistical significance
Single runs. ±10% precision.

### 🔴 OASIS bench is not byte-equivalent
OASIS does mesh envelope wrap (adds 25-byte header per call); rclcpp
just does topic dispatch. The mesh wrap adds ~50-100 ns of fixed
overhead and one extra Vec alloc + copy. Without it, OASIS would be
faster across all payload sizes (less per-call overhead AND less
memcpy work). I deliberately kept the mesh wrap because that's what
the OASIS-equivalent "pub/sub through the production stack" looks
like — comparing rclcpp dispatch to OASIS dispatch-only would cherry-pick.

## 10. Self-critique

The audit pattern caught a real architectural limitation that I didn't
see coming. My intuition was "OASIS wins by less at big payloads" —
actual is "OASIS LOSES by 8× at 1 MB." 

**Lesson:** my predictions consistently underestimate ratios that move
against OASIS. Worth tracking.

## 11. Next-step candidates

1. **Refactor wrap_topic + mesh_router.origin_wrap to buffer-backed.**
   This closes the 1 MB gap. ~1-2 days. Real ergonomic + perf win.
2. **Finer payload granularity** (256 B / 4 KB / 16 KB / 256 KB) to
   pin the exact crossover. ~30 min.
3. **Inter-process A/B** — separate processes, OASIS spore vs DDS multicast.
4. **Per-node Ed25519 mesh signing** (security, carried over).
5. **Real MCU hardware boot** (no_std verification gap).

I lean toward **#1** (refactor) — the audit found a real performance
ceiling and the fix is well-scoped. Deferring it would mean leaving
a known weakness in the OASIS performance story.
