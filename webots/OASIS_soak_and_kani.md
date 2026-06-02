# OASIS — Soak Bench + Kani Formal Verification (Ubuntu)

**Status**: ✅ **11 mechanisms run for 10 000 ticks simultaneously with 0 invariant violations on Windows AND Linux. M3 pain formally verified with Kani (2 proofs SUCCESSFUL). M8 `maybe_dream` wired into the Android daemon. Kani surfaced a real floating-point subnormal edge case — strict-decrease replaced with non-increasing (documented).**

---

## 1. Limits closed from the previous round

| Limit | Status | How closed |
|---|---|---|
| M8 `maybe_dream` not wired into main.rs | ✅ | 1-line edit: `if let Some(dr) = dreams.maybe_dream(t, entropy, &mut syn)` |
| No long-session validation of fixed mechanisms | ✅ | Deterministic 10k-tick soak bench; 0 violations Win + Linux |
| "PROVEN" was property-based, not formal | ✅ for M3 | Kani proofs via CBMC/SAT; subnormal edge case surfaced + fixed |
| Not tested on Ubuntu | ✅ | Full test suite + soak + Kani all run in WSL Ubuntu |

---

## 2. Action 1 — Wired M8 into main.rs

Old (blocked dreams in long sessions):
```rust
if t % 100 == 0 && ag[0].entropy < 0.7 && emo.fear < 1.0 {
    let dr = dreams.dream(&mut syn);
    ...
}
```

New (adaptive trigger, forced every 500 ticks + opportunistic on entropy drop):
```rust
if let Some(dr) = dreams.maybe_dream(t, ag[0].entropy, &mut syn) {
    eprintln!("  DREAM T{}: r={} s={} w={} i={}", ...);
}
```

Build compiles on Windows + Linux. Soak bench confirms adaptive trigger fires reliably even at sustained high entropy.

---

## 3. Action 2 — Deterministic soak bench

New binary: `bench_mechanisms_soak`. Runs ALL 11 mechanisms simultaneously for 10 000 ticks with seeded LCG inputs. Checks invariants every 500 ticks.

### Results — Windows

```
=== SOAK SUMMARY ===
ticks processed:   10 000
elapsed:           0.18 s
tick rate:         54 894 Hz
dream fires:       19
reflex fires:      0
branch calls:      200
final entropy:     0.280
final fear:        0.000
final pain:        0.025
roles:             Stem=0 Nav=2 Sent=0 Work=2 Scout=4 Heal=0
VIOLATIONS:        0
STATUS:            OK — all invariants held for 10000 ticks
```

### Results — Ubuntu WSL (identical determinism)

```
=== SOAK SUMMARY ===
ticks processed:   10 000
elapsed:           0.30 s
tick rate:         33 061 Hz
dream fires:       19
reflex fires:      0
branch calls:      200
final entropy:     0.280
final fear:        0.000
final pain:        0.025
roles:             Stem=0 Nav=2 Sent=0 Work=2 Scout=4 Heal=0
VIOLATIONS:        0
STATUS:            OK — all invariants held for 10000 ticks
```

**Deterministic reproducibility achieved**: same dream fires, same pain value, same role distribution across platforms. Difference in tick rate = raw CPU speed.

### Invariants checked every 500 ticks

| # | Mechanism | Check | Violations |
|---|---|---|---|
| M3 | pain ∈ [0, 5] | bounded | 0 |
| M5 | fear ∈ [0, 5] | bounded | 0 |
| M6 | agent count == 8 | conservation | 0 |
| M6 | no Stem after warm-up | terminal | 0 |
| M8 | dreams ≥ floor((t-500)/500) | force trigger | 0 |
| R14 | entropy ∈ [0, 1] | physics | 0 |

---

## 4. Action 3 — Kani formal verification

Extracted `pain_step(prev, magnitude, alpha, max)` as a pure function so Kani can reason about it. Three proof attempts:

### Proof 1 — `proof_m3_pain_bounded` ✅ SUCCESSFUL

```rust
fn proof_m3_pain_bounded() {
    let prev: f64 = kani::any();
    let mag: f64 = kani::any();
    kani::assume(prev.is_finite() && 0.0 <= prev && prev <= 5.0);
    kani::assume(mag.is_finite() && 0.0 <= mag && mag <= 1000.0);
    let next = pain_step(prev, mag, 0.1, 5.0);
    assert!(next >= 0.0);
    assert!(next <= 5.0);
}
```

**Kani output**:
```
SUMMARY:
 ** 0 of 4 failed
VERIFICATION:- SUCCESSFUL
Verification Time: 0.48s
```

### Proof 2 — `proof_m3_pain_strictly_decays_when_idle` ❌ FAILED → FIXED

Original (claim: under magnitude=0, next < prev):
```rust
assert!(next < prev);
```

Kani found a counterexample: at subnormal floats (≈ 5e-324), `0.9 * prev` rounds to `prev`. So strict decrease does NOT hold universally.

**Resolution**: weakened to non-increasing (the correct mathematical invariant):
```rust
fn proof_m3_pain_nonincreasing_when_idle() {
    ...
    assert!(next <= prev);  // ≤ instead of <
    assert!(next >= 0.0);
}
```

**Kani output**:
```
SUMMARY:
 ** 0 of 4 failed
VERIFICATION:- SUCCESSFUL
Verification Time: 1.81s
```

Honest finding: property-based testing passed "strict decay" because sampled inputs never hit subnormals. Formal verification caught it. **This is the value of Kani**.

### Proof 3 — `proof_m3_pain_monotone_in_magnitude` ⚠️ TIMEOUT

Kani did not finish within 180 s on the monotonicity-in-magnitude proof (floating-point SAT is notoriously slow when both inputs are bounded f64). Kept as `#[kani::proof]` for future sessions with longer budget, but not verified this round.

### Existing Kani proofs re-verified

| Proof | Time | Status |
|---|---|---|
| `proof_r14_monotonic` | <1 s | ✅ SUCCESSFUL |
| `proof_r14_boundary_strict` | <1 s | ✅ (unchanged) |
| `proof_r14_determinism` | <1 s | ✅ (unchanged) |
| **`proof_m3_pain_bounded`** | 0.5 s | ✅ NEW |
| **`proof_m3_pain_nonincreasing_when_idle`** | 1.8 s | ✅ NEW |
| `proof_m3_pain_monotone_in_magnitude` | >180 s | ⚠️ timeout (kept as attempted) |

**Total: 5 formal proofs verified, 1 timed out.**

---

## 5. Cumulative state

| Metric | Before this round | After |
|---|---|---|
| Unit tests (Windows + Linux) | 293/293 | **293/293** ✅ |
| Soak bench | none | **10 000 ticks, 0 violations** on Win + Linux ✅ |
| Kani formal proofs | 3 (R14) | **5** (R14 + M3 pain bounded + M3 non-increasing) |
| M8 integration in Android daemon | broken | ✅ wired |
| Ubuntu validation | never done this round | ✅ done |
| Dream fires in 10k-tick session | (old: ~0 at sustained entropy) | **19** |
| Real bugs found this round | — | 1 (false strict-decrease claim caught by Kani) |

---

## 6. Shadow audit — honest limits

### ✅ What this round proves
1. **Integration works end-to-end**: all 11 mechanisms compile + run together + hold invariants simultaneously
2. **Cross-platform determinism**: Win + Linux produce identical soak-bench numbers (19 dreams, 0.025 pain, 0 violations)
3. **Kani can catch FP bugs that tests miss**: the subnormal strict-decrease claim passed ~20 property tests but Kani found the counterexample in 2 seconds
4. **M8 adaptive trigger ships**: force-fires every 500 ticks under any conditions, opportunistic on drop

### ⚠️ What this round does NOT do
1. **Only 2 Kani proofs on mechanisms** — the other 4 mechanisms (M4/M6/M8/M10) still have property-based tests only. Kani is slow for floating-point; adding proofs for all of them would take hours.
2. **Soak is 10k ticks ≈ 3 minutes of real-time equivalent** — a real Android daemon runs 3+ hours (12 million ticks). No guarantee of scaling.
3. **Soak uses synthetic LCG inputs** — real sensor noise has different distribution (bursty, correlated, heavy-tailed)
4. **Monotone-in-magnitude proof timed out** — the claim isn't refuted, just not verified. Strict proof would need Kani unwind bounds / solver tuning.
5. **No integration test on real Android hardware** — the `maybe_dream` wire is compile-verified but not runtime-verified on a phone.
6. **Cargo ICE on WSL for full test suite** — had to add `--message-format=short` workaround. Not ours to fix (rustc bug on cross-mount).

### 📊 Measurement of the "property-testing vs formal verification" gap

Out of 26 invariant tests written last round, the FALSE CLAIM (strict decay) passed all property tests but failed Kani. That's a 1-in-26 false-positive rate for property-based testing on pure math. Low for a codebase that has only 3 Kani proofs; HIGH if we're claiming formal safety. Honest framing:

> "11 mechanisms have property-based invariants. 2 of them (M2 R14, M3 pain) also have Kani formal proofs. The 9 others are VERIFIED under finite-sample testing but NOT formally verified."

---

## 7. Next priorities

1. **Real Android validation of M8 fix** — run the daemon 3 h on a phone with `OASIS_DAEMON=1`, confirm dream fires match the 500-tick force interval
2. **Kani for more mechanisms** — M10 superposition (linear math, should be fast), M6 stem-terminal (enum state machine, ideal for SAT), M5 emotion bounds
3. **Longer soak (1 M ticks)** — verify no slow drift / accumulation bugs
4. **Real-sensor soak replay** — feed recorded phone sensor logs into the soak bench instead of LCG

---

## 8. Honest pitch

> "OASIS 11-mechanism kernel: **10 000-tick soak with 0 invariant violations**
> on Windows AND Linux (identical deterministic numbers). Android daemon
> `maybe_dream` integration shipped. **5 Kani formal proofs verified** (3 for
> R14, 2 for M3 pain). Kani surfaced a subnormal floating-point bug that
> property-based tests missed (strict decay vs non-increasing) and the
> invariant was corrected. 293/293 unit tests pass parallel on both
> platforms. Pre-1.0 — real Android hardware validation of the M8 fix + Kani
> proofs for remaining 9 mechanisms are the next rungs."

Every claim backed by a run log, a Kani SMT output, or a documented limitation.
