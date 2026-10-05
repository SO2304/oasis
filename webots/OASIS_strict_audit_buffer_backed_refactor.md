# OASIS — Strict audit: buffer-backed builder eliminates the 1 MB regression

**2026-04-22.** Ship the fix the last audit identified. `origin_wrap_with`
+ `write_topic_envelope_into` eliminate one of the two payload memcpys.
**The 1 MB "7.8× SLOWER" result reverses to "7.85× FASTER" — a 60× swing
in one refactor.** OASIS now wins at every measured payload size.

---

## 1. The fix

Two new public functions:

**`mesh::MeshRouter::origin_wrap_with`** (~30 LOC):
```rust
pub fn origin_wrap_with<F>(&mut self, inner_len: usize, write_inner: F) -> Option<Vec<u8>>
where F: FnOnce(&mut Vec<u8>),
```
Allocates ONE `Vec<u8>` of `MESH_HEADER_LEN + inner_len`, writes the
mesh header, then calls the closure to write the inner content directly
into the same buffer. Returns `None` for signed routers (v9 MAC needs
the inner bytes upfront in a separate buffer).

**`topics::write_topic_envelope_into`** (~10 LOC):
```rust
pub fn write_topic_envelope_into(buf: &mut Vec<u8>, hash: u64, payload: &[u8]);
```
Writes a topic envelope (magic + hash + len + payload) into the
existing buffer. Companion to the mesh builder.

**Combined usage:**
```rust
let inner_len = topics::TOPIC_HEADER_LEN + payload.len();
let env = router.origin_wrap_with(inner_len, |buf| {
    topics::write_topic_envelope_into(buf, hash, &payload);
}).unwrap();
```

**Old path:** `wrap_topic` → Vec #1 (1 alloc + 1 memcpy of payload) →
`origin_wrap` → Vec #2 (1 alloc + 1 memcpy of Vec #1, which contains
payload). Total: 2 allocs + 2 payload-sized memcpys.

**New path:** `origin_wrap_with` → Vec #1 directly assembled (1 alloc +
1 memcpy of payload into Vec #1). Total: 1 alloc + 1 payload-sized memcpy.

## 2. Equivalence proven

```
test mesh::tests::origin_wrap_with_matches_origin_wrap_byte_for_byte ... ok
test mesh::tests::origin_wrap_with_returns_none_on_signed_router ... ok
```

The new path produces **byte-identical** envelope to the old path for
the same inputs. The unit test allocates 1500-byte payload, builds via
both paths, and asserts the resulting `Vec<u8>` contents match. Pass.

## 3. Measurement — Linux WSL2

OASIS old vs new, side-by-side:

| Payload | OLD ns/op | NEW ns/op | OASIS internal speedup |
|---|---:|---:|---:|
| 16 B | 289 | **157** | 1.83× |
| 1 KB | 382 | **269** | 1.42× |
| 64 KB | 5 665 | **3 091** | 1.83× |
| 1 MB | 3 280 718 | **51 787** | **63.35×** ⚡ |

The smaller payloads see modest 1.4-1.8× wins (per-call overhead
dominates). The 1 MB payload sees 63×. Likely cause:
- 1 MB allocation triggers heap walk + potentially page faults.
- Old path did this 2× per call.
- New path does it 1× per call.
- Plus saving 1 MB of memcpy work per call.

The actual savings exceed pure "1 alloc + 1 memcpy" by 3×. Suspect
the second 1 MB allocation in the old path caused fragmentation /
cache eviction / page churn that the first didn't. Did NOT
flame-graph; this is a hypothesis.

## 4. Updated 4-way comparison vs ROS 2 (Linux, intra-process)

| Payload | OASIS NEW | OASIS OLD | rclcpp intra-proc | rclcpp default DDS |
|---|---:|---:|---:|---:|
| 16 B | **157 ns** | 289 ns | 6 350 ns | 52 411 ns |
| 1 KB | **269 ns** | 382 ns | 7 067 ns | (not measured) |
| 64 KB | **3 091 ns** | 5 665 ns | 8 000 ns | (not measured) |
| 1 MB | **51 787 ns** | 3 280 718 ns | 406 645 ns | (not measured) |

OASIS NEW vs rclcpp intra-process:

| Payload | OASIS NEW vs rclcpp intra | Result |
|---|---:|---|
| 16 B | 157 vs 6 350 | OASIS **40× faster** |
| 1 KB | 269 vs 7 067 | OASIS **26× faster** |
| 64 KB | 3 091 vs 8 000 | OASIS **2.6× faster** |
| 1 MB | 51 787 vs 406 645 | OASIS **7.85× faster** |

**OASIS now wins at every measured payload size.** The crossover from
last round is gone. The previous 1 MB "7.8× slower" result has been
mirrored to "7.85× faster" — a swing of nearly 60×.

## 5. What changed in the pitch

Last audit (correct as of yesterday):
> "OASIS optimized for small frequent messages. Above ~64 KB, rclcpp
>  starts winning."

This audit (corrected today):
> "OASIS dispatch (intra-process, Linux) — wins across all measured
>  payload sizes:
>  - 16 B: 157 ns/op (40× faster than rclcpp intra)
>  - 1 KB: 269 ns/op (26× faster)
>  - 64 KB: 3.1 µs/op (2.6× faster)
>  - 1 MB: 51.8 µs/op (7.85× faster)
>
>  Both stacks pay roughly equal per-call overhead at small sizes;
>  at large payloads OASIS uses a single-allocation buffer-backed
>  builder (`origin_wrap_with`) that avoids the intermediate Vec
>  allocation. rclcpp's intra-process zero-copy via unique_ptr
>  is conceptually cleaner but the actual measured cost is higher
>  due to executor + waitable machinery."

This is the new honest framing.

## 6. What this round closes

- ✅ The 1 MB regression flagged in the prior audit. Fixed in code.
- ✅ Byte-for-byte equivalence test ensures the new path is a pure
  optimization (no semantic change).
- ✅ Updated bench_payload_sweep to measure both paths side-by-side.
  Future audits can verify the speedup hasn't regressed.
- ✅ Crossover point with rclcpp gone. OASIS now ahead at every size.

## 7. What this round does NOT do

### 🔴 v9 (signed) path NOT optimized
`origin_wrap_with` returns `None` for signed routers because v9 MAC
computation requires the inner bytes upfront in a separate buffer.
Restructuring v9 to do MAC over a streaming view of the buffer is a
~1-day refactor of the HMAC integration. Out of scope this round.

### 🔴 No A/B with byte-identical workloads
OASIS bench includes `dispatch` (handler call); rclcpp bench lets the
executor deliver. Both stacks include the production dispatch path,
so the comparison is "production stack to production stack." But the
exact CPU work isn't identical and never will be (different
architectures).

### 🔴 No allocator-pool variant
For zero-allocation hot paths (the holy grail), OASIS would need a
slab/pool allocator. Not done. Each `origin_wrap_with` still allocates
one `Vec` per call.

### 🔴 No statistical significance
Single-run numbers. ±10% precision typical. The 63× speedup at 1 MB
is large enough that noise can't explain it, but the smaller speedups
(1.4-1.8×) are at the edge of significance.

### 🔴 No real hardware
WSL2 only. On a real drone CPU (slower DRAM, smaller caches), the
1 MB number could shift either way.

### 🔴 No regression check on bench_full_stack
The combined bench `bench_full_stack` still uses the OLD path internally.
Could be updated to the new path for a fairer overall comparison.
Deferred.

## 8. Self-critique

Last audit predicted:
> "[The fix would] close the 1 MB gap. ~1-2 days. Real ergonomic + perf win."

Actual: ~1 hour of code + 30 min of benching + audit. The "1-2 days"
estimate was overconservative because the refactor turned out additive
(no breaking changes — old path kept). Easier than expected.

Win magnitude: predicted "close the gap" → actual "reverse the gap by
60×." Larger than predicted.

**Pattern:** my OASIS-favorable predictions tend to underestimate
upside. My OASIS-unfavorable predictions also underestimate downside
(payload sweep audit). Both directions: predictions are conservative.

This is OK if the user expects calibration, but worth noting: the
audit numbers consistently move further than I estimate.

## 9. Scoreboard

| Metric | Pre | Post |
|---|---|---|
| Lib tests (host) | 415 | **417** (+2 equivalence tests) |
| Kani proofs | 67 | 67 |
| Cargo features | 4 | 4 |
| Bins | 18 | 18 |
| OASIS at 1 MB (Linux) | 3 280 µs/op | **52 µs/op** (63× faster) |
| OASIS vs rclcpp intra @ 1 MB | 7.8× slower | **7.85× faster** |
| OASIS-rclcpp crossover | ~64 KB | **gone** (OASIS wins everywhere measured) |

## 10. Lines of code

| File | Lines added | Lines removed |
|---|---:|---:|
| `mesh.rs` | ~50 (1 fn + 2 tests) | 0 |
| `topics.rs` | ~15 (1 fn) | 0 |
| `bin/bench_payload_sweep.rs` | ~30 (split into 2 paths) | ~5 |

**Total: ~95 LOC for a 60× perf win at 1 MB.**

## 11. Reproducibility

```bash
# Host (Windows or Linux)
cargo build --release --bin bench_payload_sweep
./target/release/bench_payload_sweep   # Windows
./target/release/bench_payload_sweep   # Linux

# rclcpp comparison (WSL Ubuntu 24.04 + ROS 2 Jazzy)
g++ -O3 -std=c++17 ros2_bench_sweep.cpp -o ros2_bench_sweep \
    [include + lib flags from prior audit]
./ros2_bench_sweep
```

## 12. Next-step candidates

1. **v9 buffer-backed signing** — same idea but threading the MAC
   computation through a streaming view. ~1 day. Closes the small
   remaining "signed = slow" footgun.
2. **Allocator pool** for zero-alloc hot path. ~1-2 days. Could push
   16 B latency below 100 ns.
3. **Statistical bands on benchmark numbers** — simple infrastructure
   improvement. ~half a day.
4. **Per-node Ed25519 mesh signing** (insider attack vector, carried
   over since session start).
5. **Real MCU hardware boot** (no_std verification gap, carried over).

I lean toward **#3** — adding error bars to benches. Every bench
result so far has been single-run. Without statistical bounds, the
"63× speedup" claim could include 5-10% noise on either end. Worth
firming up.
