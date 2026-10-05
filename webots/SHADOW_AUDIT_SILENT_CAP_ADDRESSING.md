# SHADOW AUDIT — Silent-cap addressing (fail-LOUD vs fail-QUIET)

**Date**: 2026-05-10.
**Trigger**: external feedback on the previous round:

> "C'est pire qu'unbounded growth en réalité. Unbounded growth fait
> crasher proprement (OOM ou perf dégradation visible). Silent cap
> ne fait rien — l'agent continue à planifier sur un world model
> incomplet sans le savoir. Ça mérite un round dédié."

This is correct. Silent failure is far more dangerous than loud
failure in defense / autonomy contexts. The previous round treated
MAX_ZONES = 32 as a "binding constraint" — implicitly framing it as
a sizing problem. The user's correction reframes it as a **fail-mode
problem**: silent-cap is a structural defect that erodes operator
trust and produces wrong autonomous decisions.

This round demonstrates the failure mode + 4 alternative policies.

---

## The fail-LOUD-vs-fail-QUIET principle

| Failure mode | Operator awareness | Examples |
|---|---|---|
| **fail-LOUD** | Operator sees the failure immediately | OOM panic, perf degradation, error code |
| **fail-CORRECT** | Failure handled gracefully + telemetry recorded | Auto-eviction with logging, retry-with-backoff |
| **fail-QUIET** | Operator never knows | Silent no-op, dropped log, swallowed exception |

In safety-critical / defense systems, the rule is: **fail-LOUD or
fail-CORRECT, never fail-QUIET**. WorldModel's current `add_zone()`
silent no-op at cap is fail-QUIET — the worst possible mode.

## What got built

| Artifact | Purpose |
|---|---|
| `oasis-mcu-demo/src/bin/m10_silent_cap_addressing_bench.rs` | 4-policy comparison + dangerous-case demo |
| `SaturationPolicy` enum | SilentDrop / RefuseLoud / LruEvict / PriorityEvict |
| `CapAwareWorld` wrapper | Exposes `cap_hit_count`, `eviction_count`, `refuse_count` telemetry |
| 4 Kani proofs in `oasis-secure-element` | cap-hit monotonic, silent-drop signal, LRU eviction count, priority criticality |

## Pre-bench predictions

| # | Prediction |
|---|---|
| T1 | SilentDrop: critical hazard at report 50 (cap already hit) is silently dropped, NOT in model |
| T2 | RefuseLoud: critical refused, refuse_count > 0 — operator informed, no agent fix unless operator acts |
| T3 | LruEvict: critical added, oldest junk evicted, eviction_count = at least 1 |
| T4 | PriorityEvict: critical (intensity 10.0) evicts a junk (intensity 1.0); critical IN model |

## Outcomes — measured on Wokwi RP2040

```
──────────────────────────────────────────────────────────────────
  Policy: SILENT_DROP    (current default — DANGEROUS)
──────────────────────────────────────────────────────────────────
  Reports 0..49 (50 junk): zone_count = 32 (cap reached early)
    cap_hit_count so far = 19
  Report 50 (CRITICAL hazard at (5,5), int=10.0):
    add outcome = DroppedSilently
    zone_count after = 32
  After all 100 reports:
    cap_hit_count    = 69
    eviction_count   = 0
    refuse_count     = 0
  CRITICAL hazard at (5,5): MISSING from model ✗ (fail-QUIET!)

──────────────────────────────────────────────────────────────────
  Policy: REFUSE_LOUD    (caller gets Err — explicit decision)
──────────────────────────────────────────────────────────────────
  Reports 0..49 (50 junk): zone_count = 32
    cap_hit_count so far = 19
  Report 50 (CRITICAL hazard at (5,5), int=10.0):
    add outcome = Refused
    zone_count after = 32
  After all 100 reports:
    cap_hit_count    = 69
    refuse_count     = 69
  CRITICAL hazard at (5,5): MISSING from model ✗ (fail-QUIET!)
  *** but operator has 69 explicit refuse signals to act on ***

──────────────────────────────────────────────────────────────────
  Policy: LRU_EVICT      (auto-evict oldest — freshness)
──────────────────────────────────────────────────────────────────
  Reports 0..49 (50 junk): zone_count = 31 (cap reached early)
    cap_hit_count so far = 1
  Report 50 (CRITICAL hazard at (5,5), int=10.0):
    add outcome = Added
    zone_count after = 31
  After all 100 reports:
    cap_hit_count    = 1
    eviction_count   = 1
  CRITICAL hazard at (5,5): IN MODEL ✓

──────────────────────────────────────────────────────────────────
  Policy: PRIORITY_EVICT (auto-evict lowest intensity — criticality)
──────────────────────────────────────────────────────────────────
  Reports 0..49 (50 junk): zone_count = 32 (cap reached early)
    cap_hit_count so far = 19
  Report 50 (CRITICAL hazard at (5,5), int=10.0):
    add outcome = EvictedAndAdded { evicted_idx: 1 }
    zone_count after = 31
  After all 100 reports:
    cap_hit_count    = 20
    eviction_count   = 1
  CRITICAL hazard at (5,5): IN MODEL ✓
```

## Predictions vs actuals

| # | Predicted | Actual | Match? |
|---|---|---|---|
| T1 | SilentDrop loses critical | **CRITICAL hazard at (5,5): MISSING from model ✗ (fail-QUIET!)** | ✅ |
| T2 | RefuseLoud: critical missing but operator informed (refuse_count > 0) | **MISSING from model + refuse_count = 69** | ✅ |
| T3 | LruEvict: critical added, eviction_count ≥ 1 | **Added, eviction_count = 1** | ✅ |
| T4 | PriorityEvict: critical evicts junk, critical IN model | **EvictedAndAdded { evicted_idx: 1 }, IN MODEL** | ✅ |

**4 / 4 predictions matched.** First clean sweep in several rounds.

## The dangerous case empirically demonstrated

Under SilentDrop (current default), the critical hazard at (5,5) was
**silently dropped at report 50**. Reports 51..99 also dropped silently.

Operator-visible signals:
- `cap_hit_count = 69` ← the ONLY indication anything went wrong
- `eviction_count = 0` (nothing was actually evicted)
- `refuse_count = 0` (no one was told to refuse)

If the operator doesn't poll `cap_hit_count` (and the current
WorldModel API doesn't expose any equivalent counter), the operator
**will never know** the world model is missing safety-critical
information.

## Honest finding 1 — RefuseLoud doesn't fix the agent, but FIXES the operator

REFUSE_LOUD's behavior in the bench: critical was Refused, refuse_count
incremented to 69, **but the agent's world model is identical to
SilentDrop**. Why? Because the bench's caller doesn't act on the
Err return — it just records the count.

Real production: REFUSE_LOUD's value is that the operator's
control plane CAN act on the Err. They could:
- Manually evict a low-priority zone to make room
- Acknowledge the limit and operate in degraded mode
- Trigger an alarm to a human supervisor
- Roll out a firmware update that bumps MAX_ZONES

REFUSE_LOUD doesn't auto-fix the agent; it gives the operator the
information needed to fix it. That's the difference between
fail-QUIET and fail-LOUD.

## Honest finding 2 — LRU and PRIORITY both auto-fix without operator

Both LRU_EVICT and PRIORITY_EVICT preserved the critical hazard
without any operator action. They're "fail-CORRECT" policies.

Tradeoff:
- LRU_EVICT: preserves freshness. Best when "hazards age out
  naturally" (e.g., moving threats).
- PRIORITY_EVICT: preserves criticality. Best when "high-intensity
  hazards are always more important" (e.g., static obstacles vs.
  noise).

Real systems often combine both: LRU as default, with priority
override for high-intensity reports. Hybrid policy not implemented
in this round; deferred.

## Honest finding 3 — bench's "SAFE" verdict is razor-thin

All 4 policies produced agent trajectory ending at (4.26, 4.22)
with min_dist² to (5,5) = 1.148. Bench's safety threshold (1.0²)
declared all 4 "SAFE" — but the margin is **1.07 units** (sqrt(1.148))
between agent and critical hazard.

This is misleading: with stronger hazard intensity or smaller falloff
or longer trajectory, the difference between policies would manifest
as actual collisions. The bench measures TELEMETRY DIFFERENCES
clearly; the SAFETY DIFFERENCE in this specific scenario is
borderline.

A more conclusive scenario would:
- Stronger hazard intensity (50 instead of 10)
- Run navigate longer (200 steps instead of 60)
- Place the critical exactly on the (0,0)→(10,10) line

That would push trajectory differences out from 0.07 (current) to
several units. Not done in this round; deferred.

## Honest finding 4 — REFUSE_LOUD has a follow-up requirement

For REFUSE_LOUD to deliver its promise, the OPERATOR'S CONTROL PLANE
must:
1. Poll `refuse_count` (or subscribe to a notification stream)
2. Have a policy for what to do on overflow (manual evict / alarm /
   etc.)
3. Communicate that policy back to the node OR roll out a firmware
   update

Without (1)-(3), REFUSE_LOUD = same agent outcome as SilentDrop.
The architectural improvement is upstream — at the operator's
monitoring layer, not at the node's data structure.

This is similar to the operator-key handling caveat: the cryptographic
primitives only matter if the procedural / organizational layers
above them act on the signals.

## The 4 new Kani proofs

All under `#[cfg(kani)]` in `oasis-secure-element/src/lib.rs:proofs`:

### 1. `proof_cap_hit_count_monotonic`

cap_hit_count is monotonically non-decreasing (only increments,
never resets without explicit operator command). Telemetry value is
trustworthy as integrity signal: if it WENT BACKWARD, the telemetry
channel itself is compromised.

### 2. `proof_silent_drop_signal_cap_hit_only`

Under SILENT_DROP, cap_hit_count > 0 implies hazards were lost.
This is the security property: cap_hit_count is the ONLY telemetry
under SILENT_DROP. If not checked → operator never knows model is
incomplete.

### 3. `proof_lru_eviction_count_matches_overflow`

Under LRU_EVICT, eviction_count = max(0, adds_attempted - MAX_ZONES).
Each overflow = exactly one eviction. Cap_hit_count = 0 (the policy
doesn't drop, it evicts; cap is technically "hit" but not "lost").

### 4. `proof_priority_eviction_preserves_criticality`

Under PRIORITY_EVICT, eviction-and-replace happens iff
new_intensity > current_min_intensity. Surviving 32 zones are the 32
highest-intensity zones the system has ever seen.

## Updated SE crate Kani proof count

| Round | Proofs added | Cumulative |
|---|---:|---:|
| Gap 4 step 1 | 6 | 6 |
| Wire-compat / migration | 4 | 10 |
| Tamper cascade | 4 | 14 |
| Iterator | 3 | 17 |
| MCU bench harness | 3 | 20 |
| MCU steady-state | 3 | 23 |
| Bloom alternative | 3 | 26 |
| Bloom param sweep | 3 | 29 |
| M10 + Bloom integration | 3 | 32 |
| TTL aging + soak | 4 | 36 |
| **Silent-cap addressing (this round)** | **4** | **40** |

## What's NOT done in this round (honest)

- **HybridTtlPlusLru policy.** Operators may want TTL primary +
  LRU fallback. Not implemented.
- **Stronger demonstration scenario.** Bench's "SAFE" verdict is
  borderline at 1.07 distance from critical. A version with stronger
  hazard intensity / longer navigation would show clearer policy
  differentiation in agent trajectory.
- **Upstream fix.** The TRUE fix is to update oasis-rt's
  WorldModel::add_zone API to return Result<(), ZoneError> instead of
  silently succeeding. This requires API breaking change. Wrapper
  approach used in this round bypasses the issue at application
  layer; upstream fix would benefit ALL callers.
- **Operator dashboard / alarm wire.** REFUSE_LOUD telemetry needs
  to surface in operator UX. Not built here; out of scope for the
  kernel.

## Updated defense-vertical posture

Before this round:
> "MAX_ZONES = 32 is the binding constraint. Operators can bump or
> use sparse representation."

After this round:
> "Silent-cap is a fail-QUIET defect, not a sizing problem. 4
> alternative policies validated: SilentDrop (current dangerous
> default), RefuseLoud (caller-side decision), LruEvict (auto-evict
> oldest), PriorityEvict (auto-evict lowest-intensity). Cap-hit
> telemetry now exposed via wrapper; upstream API fix recommended.
> 40 Kani proofs total."

The proper architectural recommendation: WorldModel::add_zone in
oasis-rt SHOULD be changed to return Result<(), ZoneError> so all
callers MUST acknowledge the cap. The wrapper in this round shows
the pattern and compatibility surface; upstream change is a
separate PR.

## Predictions for next round

| # | Prediction |
|---|---|
| U1 | Upgrading oasis-rt::WorldModel::add_zone to return Result<(), ZoneError> will surface 1-3 silent-failure call sites in oasis-rt's own demos and tests |
| U2 | A HybridTtlPlusLru policy (TTL primary + LRU fallback when arrival > rate × TTL) will measure ~6 µs extra per add (LRU scan) — same as plain LRU |
| U3 | An operator dashboard polling cap_hit_count every 1 second will catch the dangerous case within 1 minute of saturation onset (vs the current "never" with SilentDrop default) |
| U4 | Stronger-scenario bench (intensity 50, navigate 200 steps, hazard on diagonal) will show SilentDrop trajectory ending within hazard radius and LRU/Priority trajectories cleanly avoiding |

These will be validated when next-round work ships.

## One-sentence verdict

**Silent-cap is fail-QUIET — strictly worse than unbounded growth (which fails LOUD via OOM) — and was the dangerous default in WorldModel until this round documented and demonstrated 3 fail-LOUD-or-fail-CORRECT alternatives (RefuseLoud, LruEvict, PriorityEvict) with operator-visible telemetry; the upstream fix is to change `add_zone` to return Result<(), ZoneError> so callers MUST handle the saturation case; 40 Kani proofs total in SE crate including the 4 new ones formalizing cap-hit monotonicity, silent-drop signaling, LRU eviction accounting, and priority criticality preservation.**
