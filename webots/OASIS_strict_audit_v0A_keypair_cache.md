# OASIS — Strict audit: v0A KeyPair caching (prediction miss documented)

**2026-04-22.** Follow-up to the session-seal audit. Cached
`ed25519_compact::KeyPair` in `MeshRouter` to skip the
`KeyPair::from_seed` derivation on every signing call. **Predicted
3× speedup; measured 1.48×.** Honest disclosure below.

---

## 1. Pre-audit prediction vs actual measurement

| | Predicted | Actual | Accuracy |
|---|---:|---:|---|
| Sign latency drop | 370 µs → ~100 µs (~3× speedup) | **370 µs → 250 µs (1.48× speedup)** | **off by 2×** |
| Verify latency | unchanged | 129 µs (unchanged) | ✅ |
| Cross-platform consistency | Windows + Linux similar | Linux 250 µs, Windows 252 µs | ✅ |

### Why the miss

I assumed `KeyPair::from_seed` was THE expensive operation and `sign()`
was fast. Actual cost structure of `ed25519-compact`:
- `KeyPair::from_seed`: one EC scalar multiplication (derive pubkey)
- `SecretKey::sign`: ALSO derives the public key internally (for the
  deterministic nonce construction in RFC 8032 Ed25519)

Caching `KeyPair` eliminates ONE of the TWO internal pubkey derivations,
not all of them. That gives ~1.5× speedup, not 3×.

**Lesson:** next time I estimate a crypto optimization, I'll read the
library's sign() source path instead of assuming architectural
knowledge. Same lesson as the pre-v0A audit where I predicted 30-100 µs
sign and got 370 µs. Predictions keep being optimistic; measurements
keep being sobering.

## 2. Measured on Linux WSL2

```
                                    Pre-cache     Post-cache    Speedup
v0A origin_wrap (sign):           369 887 ns      249 673 ns    1.48×
v0A process (verify):             129 283 ns      129 552 ns    1.00×  (unchanged as expected)
```

Verify didn't change because verification was always a pure function
call; the cache refactor only touched the sign path.

## 3. Cross-platform numbers

| | Linux | Windows |
|---|---:|---:|
| v0A sign (pre-cache) | 369 887 ns | 365 689 ns |
| v0A sign (post-cache) | 249 673 ns | 251 701 ns |
| v0A verify | 129 552 ns | 126 594 ns |

**Cross-platform consistency is real.** Ed25519 math is the bottleneck,
not OS-specific machinery. Sub-1% variance between Linux and Windows.

## 4. Impact on the v0A usage story

Pre-cache numbers:
- 100 pkt/s with v0A sign: 37 ms/s = 3.7% CPU
- 1 kHz: 370 ms/s = 37% CPU (too much)

Post-cache numbers:
- 100 pkt/s with v0A sign: **25 ms/s = 2.5% CPU**
- 1 kHz: **250 ms/s = 25% CPU** (still a lot, but now viable with 2-core systems)
- 10 Hz (authority broadcasts): 2.5 ms/s = 0.025% CPU

The refactor marginally expands the "viable" frequency band for v0A
but doesn't change the fundamental "low-frequency only" positioning.

## 5. What shipped (code)

- New pure helper `mesh_v10_sign_with_kp(kp, msg_id, origin_fp)` —
  skips seed→kp derivation.
- `MeshRouter::ed_seed: Option<MeshEdSeed>` replaced with
  `ed_keypair: Option<ed25519_compact::KeyPair>`. Derivation happens
  once at router construction.
- `origin_wrap_with_ttl` v0A path uses the cached `KeyPair`.
- Old `mesh_v10_sign(seed, ...)` retained as convenience wrapper
  (now calls `mesh_v10_sign_with_kp` after a one-shot derivation).

## 6. Verification

```
cargo test --lib --release:  427/427 pass  (unchanged from session-seal)
mesh tests:                   39/39 pass   (incl. all 10 v0A tests)
```

Zero regression. All 10 pre-existing v0A tests (`v10_signed_roundtrip`,
`v10_sig_tampering`, `v10_spoofed_origin_fp`, etc.) continue to pass —
the cached keypair produces byte-identical signatures to the
seed-derived path.

## 7. Scoreboard delta (from session-seal)

| Metric | Session-seal | This round |
|---|---:|---:|
| Lib tests | 427 | 427 (unchanged) |
| Kani proofs | 69 | 69 |
| v0A sign (Linux) | 370 µs | **250 µs** (1.48× faster) |
| v0A verify | 129 µs | 129 µs (unchanged) |

## 8. What this round does NOT do

### 🔴 Verify path not optimized
Ed25519 verify also does internal pubkey handling. Caching on the
verify side would require pre-computing per-sender "expanded"
verification structures (batch verify, aggregate keys). Not done.

### 🔴 Batch signing / batch verify
RustCrypto's `ed25519-dalek` supports batch verify at 2-5× speedup
for N signatures. `ed25519-compact` does not. A batch-capable
backend swap would close another factor but introduces a new dep.

### 🔴 No statistical bands on v0A bench
v0A bench still runs N=1000 single-shot. The 1.48× number has
implicit noise. Recent payload-sweep pattern (K=10 repeats + median
± half-spread) should eventually apply here. Not done.

### 🔴 Hardware-accelerated Ed25519
Some ARM Cortex-M chips have Ed25519 hardware accelerators
(STM32WL, nRF52840). OASIS doesn't use them — relies on software
impl. On MCU without hw accel, expect 10-100× slower than laptop
figures.

### 🔴 Prediction calibration unfixed
I keep over-predicting OASIS wins. Session history:
- Pre-v0A: predicted 30-100 µs sign → actual 370 µs (3-12× off)
- Post-v0A cache: predicted 3× speedup → actual 1.48× (2× off)
- Buffer-backed refactor: predicted "close the gap" → actual "reverse it 60×" (huge beat)

Net: predictions have variance in both directions. Should use an
inline micro-bench to inform predictions instead of intuition.

## 9. The honest framing update

Prior session-seal said:
> "v0A origin_wrap + Ed25519 sign: 370 µs/op ... 870× slower than v9."

Updated:
> "v0A origin_wrap + Ed25519 sign: **250 µs/op** (Linux laptop,
>  2026-04-22 post-KeyPair-cache). **586× slower than v9**. Still
>  in the 'low-frequency authority broadcasts' regime. Hot control
>  loops should stay on v9."

## 10. Next-step candidates (unchanged)

Infrastructure:
1. Statistical bands on v0A bench.
2. `ed25519-dalek` backend swap for batch verify.
3. Key rotation protocol.

Features (carried from session-seal):
4. Typed messages derive macro.
5. Real MCU hardware boot.
6. Per-node revocation broadcast integration.

## 11. Self-critique

**The real value of this round:** the audit pattern forced me to
publicly document a missed prediction. 1.48× isn't the 3× I promised
in the session-seal; readers need to know.

**The real optimization value:** 1.48× is not nothing. Going from
370 µs to 250 µs per envelope means v0A works at ~4 kHz instead of
~2.7 kHz with equal CPU budget. For swarm-broadcast use cases
(leader commands at 10-100 Hz), this pushes the per-peer update
budget from ~3k sends/s to ~4k sends/s.

**The real cost:** predictions should be measured, not intuited.
I'll bench before publishing a "~Nx speedup" number next time.
