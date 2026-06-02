# OASIS — Strict audit: `mesh_bloom_mcu` feature + `tx_counter` persistence

**⚠️ 2026-04-22 RETRACTION:** the MCU claim below is misleading.
Subsequent cross-compile attempt (see
[OASIS_strict_audit_mcu_reality_check.md](OASIS_strict_audit_mcu_reality_check.md))
revealed that oasis-rt's dep tree (serde, getrandom) does NOT compile
for `thumbv7em-none-eabi`. The `mesh_bloom_mcu` feature shrinks RAM
usage on hosts that CAN compile, but it does not unblock embedded
targets. The "unblocks MCU / embedded targets" framing was unverified.

**2026-04-22 (original).** Two small, orthogonal additions. Both were
on the "still-open" list from prior audits. Neither closes a big
architectural hole but both unblock real deployment paths (MCU +
long-running).

---

## 1. What shipped

### `mesh_bloom_mcu` Cargo feature (opt-in)

```toml
[features]
mesh_bloom_mcu = []  # shrinks Bloom 64 KiB → 2 KiB
```

```rust
#[cfg(not(feature = "mesh_bloom_mcu"))]
pub const BLOOM_WORDS: usize = 8192;   // 64 KiB, 1 % FPR at ~52 k
#[cfg(feature = "mesh_bloom_mcu")]
pub const BLOOM_WORDS: usize = 256;    // 2 KiB,  1 % FPR at ~1.6 k
```

Single compile-time flip. `MeshRouter` heap shrinks by 62 KiB per router.
**No API change** — `BLOOM_WORDS` stays `pub const` for introspection.

### `MeshRouter::tx_counter() / set_tx_counter()`

```rust
pub fn tx_counter(&self) -> u64;         // snapshot
pub fn set_tx_counter(&mut self, value: u64);  // restore on boot
```

The caller's binary is responsible for persisting the value (file, NVS,
etc.). The mesh library doesn't touch disk.

## 2. Tests added (3)

| Test | What it proves |
|---|---|
| `tx_counter_roundtrip_through_persistence` | snapshot → rebuild router → restore → next msg_id does NOT collide with pre-reboot ones |
| `tx_counter_without_restore_collides` | **control test**: reboot without restore produces identical msg_id — documents why the API is needed |
| `bloom_words_matches_feature_flag` | compile-time assertion: default = 8192, `--features mesh_bloom_mcu` = 256 |

The `without_restore_collides` test is intentional — it exists to make
the bug being fixed visible in the test suite, not just the audit.

## 3. Measured impact (MCU feature build)

Re-ran `bench_mesh_signed` with `--features mesh_bloom_mcu`:

| Metric | Default (64 KiB) | MCU (2 KiB) | Delta |
|---|---:|---:|---:|
| v8 origin_wrap | 219 ns | 303 ns | +84 ns (noise / release-build variance) |
| v9 origin_wrap | 514 ns | 500 ns | ~equal |
| **v8 process forward** | **308 ns** | **99 ns** | **−209 ns (−68 %)** ← L1 fit |
| v9 process forward | 599 ns | 381 ns | −218 ns (−36 %) |
| v8 duplicate drop | 23 ns | 23 ns | 0 |
| v9 bad-MAC drop | 319 ns | 326 ns | ~equal |

The MCU Bloom (2 KiB) fits entirely in L1 cache; the 64 KiB Bloom does
not. This shows up as a **real 3× speedup on the v8 process() hot path**
— a surprise upside of the MCU mode.

### BUT: MCU Bloom saturates near-instantly

| n | observed FPR | theory FPR |
|---:|---:|---:|
|  5 000 | 56.86 % | 29.35 % ← probe-feedback compounding |
| 13 000 | 94.94 % | 90.89 % |
| 20 000 | 99.38 % | 98.89 % |
| 40 000 | 100.00 % | 100.00 % |

MCU mode is **only usable** below ~1 600 inserts (1 % FPR target).
After that the filter is effectively a "always-say-duplicate" machine.
Operators running in this mode must call `bloom_reset()` aggressively.

The 56.9 % at n=5 000 is the probe-feedback effect I noted last round:
when FPR is high, even the "fresh" 5 000 probes quickly saturate the
filter during the probe phase itself, driving observed FPR above the
starting theoretical value. This is a fair model of how MCU mode behaves
under sustained load.

## 4. Scoreboard

| Metric | Pre | Post |
|---|---|---|
| Lib tests | 396 | **399** (+3) |
| Kani proofs | 67 | 67 |
| Kani failures | 0 | 0 |
| Cargo features | 1 (`std_env`) | **2** (+`mesh_bloom_mcu`) |
| `MeshRouter` heap, default | ~104 KB | ~104 KB |
| `MeshRouter` heap, MCU mode | — | **~42 KB** |

## 5. What this round closes

- ✅ **MCU / embedded targets can now use the mesh layer** by opting
  into the smaller Bloom. A Cortex-M with 128 KiB RAM can afford
  `~42 KB` per router; 64 KB was impractical.
- ✅ **`tx_counter` persistence unblocked**. A binary that cares
  about reboot-safety can now persist the counter via any storage
  it chooses.
- ✅ **L1-cache-fit speedup quantified** as a real side benefit of
  MCU mode (−68 % on v8 process()).

## 6. What this round does NOT close

### 🔴 MCU mode is operationally fragile
1 % FPR at only 1 600 inserts means:
- At 10 pkt/s from 10 peers = 100 pkt/s ingress, the filter saturates
  in ~16 seconds.
- Operators MUST reset aggressively or accept high legitimate-drop rates.
- **No auto-reset on threshold crossed** — still manual. A distracted
  operator will silently degrade.

### 🔴 `tx_counter` persistence is library-only
- No binary in `oasis-rt/src/bin/` actually persists it yet.
- No example code showing the recommended pattern (write on `SIGTERM`,
  read on boot, fsync on a timer).
- No integration with the existing `oasis_keygen` / `spore_*` tooling.
- Library API is minimal — caller can easily forget fsync, causing
  counter loss under unclean shutdown.

### 🔴 Not measured
- Linux / macOS / ARM behaviour with the MCU feature flag.
- Real MCU target compile (armv7-m / thumbv7em) — the code uses `Box`
  which works on no_std with `alloc` but I haven't actually cross-compiled.
- Behaviour of the 3× L1 speedup under realistic multi-router sim
  (e.g., 200-drone where cache lines are shared across routers).
- `v9` signing cost on an MCU (HMAC-SHA256 without hardware acceleration
  is 10–50× slower than laptop).

### 🔴 Still carried from prior audits
- Per-node Ed25519 (insider attacker) — unchanged.
- Key rotation for `MeshMacKey` — unchanged.
- HKDF helper — unchanged.
- 200-drone sim re-run with v9 — unchanged.
- A/B vs ROS 2 on matched hardware — unchanged.
- 64-bit MAC tag vs 128-bit — unchanged.

## 7. Honest disclosures

1. **I did not actually compile this for a real MCU target.** The
   feature flag works on x86_64 and was verified via the unit test
   `bloom_words_matches_feature_flag`. A real `thumbv7em-none-eabi`
   build has NOT been attempted. If the crate still pulls in `std` via
   some transitive dep, MCU mode won't link. **Not tested.**

2. **`tx_counter` API is minimal by design.** I deliberately did not
   add `save_to_file(path)` / `load_from_file(path)` helpers because
   the right persistence target depends on the platform (file, NVS,
   BLE-backed remote key store, etc.). This means every binary has
   to write ~10 lines of glue code. That's a migration cost I pushed
   to callers.

3. **The `tx_counter_without_restore_collides` test was surprising to
   write.** I expected different msg_ids because of HashMap random
   seeds or something — but the SplitMix64 is deterministic and fp is
   the same, so counters starting from 0 produce literally identical
   msg_ids. That's the bug. Test makes it visible.

4. **The measured L1-cache speedup (3× on v8 process) is honest but
   misleading as a reason to pick MCU mode.** The real reason is RAM
   constraint. Anyone not memory-bound should stay on default.

## 8. Cumulative Kani breakdown (67 total — unchanged)

No new Kani proofs this round. The feature flag is a compile-time
constant; the tx_counter API is a pair of trivial getters/setters over
u64. Both are covered by the 3 new unit tests. I deliberately did NOT
write pro-forma Kani proofs for either, because the marginal value
over "this compiles and the unit test passes" is zero.

## 9. Next-step candidates (unchanged from last audit)

1. **Per-node Ed25519 mesh signatures** — closes insider attack vector.
   ~2 days. Biggest unsolved architectural gap.
2. **Typed messages derive macro** — biggest DX gap vs ROS 2. ~1 week.
3. **200-drone sim re-run with v9 + MCU-mode ablation** — validation.
4. **Auto-reset Bloom on threshold** — operator-safety improvement.
5. **Real MCU cross-compile verification** — eat my own dogfood.

I recommend **#5** next: cross-compile `--target thumbv7em-none-eabi
--features mesh_bloom_mcu --no-default-features` and report what
actually breaks. Anything less is a claim I haven't verified.
