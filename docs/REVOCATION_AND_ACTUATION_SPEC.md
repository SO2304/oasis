# Signed mesh revocation (E) and actuation gate (F) — specification (Part A)

Prompt: `prompts/OASIS_REVOCATION_GATE_ATTACKSUITE.md`. Branch `oasis-e-f-attacks`,
based on `claude/eloquent-ptolemy-oojjn0` @ `08bc8bf` (mesh v0B + strict mode + sender
counter lease, all silicon-verified). **More secure, not faster.** Nothing here changes
v0B or any earlier wire format; everything is additive.

## 0. What already exists (verified in the tree, 2026-10-06)

| Piece | Where | Verified | Limit that matters here |
|---|---|---|---|
| `RevocationList`, `OASREV` v1, `SPORE\x06` wrapper | `spore_crypto.rs:497-667` | ✅ | signs `magic‖version‖count‖(fp,ts)*` — **no epoch, no `network_id`, no domain tag** |
| Process-global revocation list | `spore.rs:149-176` | ✅ | `std` only (`OnceLock<Mutex>`), `spore` module gated on `std_env` |
| Operator authority, single or k-of-n | `oasis-operator-key` (`single_from_seed`, `multisig_from_seeds`, `verify_authorization(msg, &[(Pub,Sig)])`, `apply_transition`) | ✅ | **`no_std`, builds for `thumbv6m`** — usable on the chip |
| Local relay revocation | `mesh.rs` `revoke()` / `is_revoked()` | ✅ | unsigned, not propagated, not persisted; checked **before** the v0B signature |
| R14 gate | `hyper_state.rs:125` `is_action_safe(agent, t) = agent.entropy < t` | ✅ | pure; fails closed on NaN (`NaN < t` is false) |
| Physical limits | `hal.rs:43` `clamp_command(force, torque, velocity, pos, &PhysicalConstraints) -> ClampResult` | ✅ | returns `clamped_*` / `geofence_breach` flags; `KillSwitch` is `std` only |
| Sender lease | `tx_lease.rs` | ✅ | counters survive reboot on both sides (precondition of this prompt) |

Consequence: `OASREV` cannot carry epoch/network semantics without changing v6, which
rule 6 forbids. E gets its own content format; the operator crate is reused as is.

---

## Part E — signed mesh revocation

### E.1 Carrier: a v0B content type, not a new envelope

A revocation travels as the **payload of an ordinary v0B envelope**, identified by
an inner 4-byte content magic `ORV1`. Justification:

- **Transport is solved already.** v0B gives origin authentication, `network_id`
  binding, accept-once anti-replay, strict mode and TTL flooding, all silicon-proven.
  A dedicated envelope would duplicate all of it.
- **Authority is separate from transport.** The operator signature sits *inside*
  the content, so **any** registered node can carry or re-originate a list. That is
  what makes catch-up (E.5) work, and it needs no operator presence in the mesh.
- **Zero change to v0B.** Relays inspect the payload of envelopes v0B already
  accepted; `process_v0b` is untouched.

### E.2 Content format `ORV1` (little-endian)

```
"ORV1" (4) | network_id (8) | epoch u64 (8) | issued_at u64 (8) | count u16 (2)
| fp[8] × count | nsig u8 (1) | (signer_pub[32] ‖ sig[64]) × nsig
```

**Signed message** (domain-separated):

```
"OASIS-REVOKE-v1" ‖ network_id ‖ epoch ‖ issued_at ‖ count ‖ fp × count
```

- Fingerprints must be **strictly ascending** (canonical: one encoding per set, no
  duplicates). Unsorted or duplicated → rejected.
- `issued_at` is the operator's wall clock: **informational only**. Ordering comes from
  `epoch`, never from a clock.
- Signatures are checked by `OperatorAuthority::verify_authorization(msg, &[(pub, sig)])`,
  single key or k-of-n.
- **Size.** One signature: 127 + 8·n bytes. The test firmware's frame limit
  (`MAX_ENV = 300`, so payload ≤ 201) would allow only 9 fingerprints and **no**
  2-of-3 list. Part C raises `MAX_ENV` to 512 (payload ≤ 413): single signature up to
  35 fingerprints, 2-of-3 up to 23. Cap: `MAX_REVOKED = 16` on the MCU profile.

### E.3 Relay state and rules

State, persisted: `epoch: u64`, `revoked: sorted set of fp`, `blob`: the last
accepted signed `ORV1` content, kept so the relay can re-serve it.

Rules, in this order. **Nothing is mutated before step 5.**

| # | Check | On failure |
|---|---|---|
| 1 | The carrying v0B envelope is accepted by `process_v0b`, strict mode | dropped by v0B, unchanged |
| 2 | `ORV1` parses; `network_id` == mine; canonical list; `count ≤ MAX_REVOKED` | `Reject(Malformed)` |
| 3 | `epoch > known` (`== known` → `Duplicate`; `< known` → `Reject(Rollback)`) | no state change; **never forwarded** |
| 4 | Operator signature valid (single or k-of-n) | `Reject(BadOperatorSig)` |
| 5 | New list ⊇ current `revoked` set (revocation is permanent) | `Reject(Shrink)` |
| 6 | **Persist** `(epoch, revoked, blob)` to flash, two alternating sectors (torn-write safe, as `tx_lease`) | if not durable: `Reject(PersistFailed)`, nothing applied |
| 7 | Apply: `router.revoke(fp)` for every fp (existing additive v0B API) | — |
| 8 | Forward the carrying envelope **once** (only on a fresh epoch) | — |

**Why permanent (rule 5).** Un-revoking a fingerprint would need a router API to
remove entries (a v0B change) and would re-trust a key that was declared compromised.
A recovered device is **re-keyed**: it gets a new fingerprint. Rule 5 also blunts a
mis-issued list: it can add victims but never silently drop a known-compromised node.

**Effect on traffic.** Once step 7 runs, every v0B envelope from a revoked origin is
dropped with `origin revoked` **before** its Ed25519 check (existing v0B order), on
every relay that holds the epoch.

The whole decision is a **pure function**:
`revocation_transition(state, parsed, sig_ok) -> (RevDecision, Option<RevState>)`.
It returns `None` on every reject, so a rejected message provably leaves state
unchanged (Kani, Part B).

### E.4 Restart

On boot the relay reads both sectors, takes the highest valid `(epoch, list)` and
re-applies `router.revoke` **before processing any envelope**. This is the same
ordering already used for the v0B counter window.

### E.5 Catch-up for relays that missed the broadcast

- Every node originates an **epoch beacon** `OEP1 ‖ epoch` (v0B, `ttl = 0`, so
  neighbours only) at boot, every `BEACON_PERIOD = 60 s`, and whenever its epoch
  changes.
- A node that hears a beacon with a **lower** epoch than its own re-originates its
  stored `blob` in a new v0B envelope from itself. The operator signature inside is
  unchanged. Rate limit: once per `(neighbour, epoch)` per period.
- The lagging node applies it through the normal rules (E.3). No operator presence
  is needed.

### E.6 What revocation does not cover (stated, not solved)

- **Exposure window.** Between the broadcast and a relay's catch-up, a relay that
  missed the list still forwards the revoked node. The window is bounded by
  reconnection + one beacon period; a partitioned segment stays exposed until it
  reconnects.
- **The operator key itself.** A compromised quorum can revoke everyone (DoS), or
  jump `epoch` to `u64::MAX` and freeze future revocations. k-of-n raises the bar;
  nothing here removes the risk. Re-rooting is `apply_transition`'s job, outside this
  prompt.
- **Physical extraction.** With SWD access to a node, its own key and persisted state
  can be read or rewritten (no secure element).

---

## Part F — actuation gate

### F.1 Command format `OAC1` (a v0B content type, little-endian)

```
"OAC1" (4) | actuator_id u16 | cmd_seq u32 | boot_id u64 | deadline_ms u64
| force f32 | torque f32 | velocity f32 | pos [f32; 3]          = 54 bytes
```

### F.2 Time without a shared clock

Nodes have no synchronised clock, so a sender-side timestamp means nothing to the
actuator. The deadline is therefore expressed in the **actuator's own time base**:

- The actuator originates a time beacon `OTM1 ‖ boot_id ‖ now_ms` (v0B-signed).
  `boot_id` is its **sender-lease resume point** at boot. Thanks to `tx_lease`, it
  strictly increases across reboots.
- A commander stamps `boot_id` and `deadline_ms = beacon.now_ms + validity` from the
  latest beacon it holds.
- The actuator checks `boot_id == current boot_id`, `now ≤ deadline`, and
  `deadline − now ≤ MAX_VALIDITY_MS`.
- **Tolerance:** zero clock skew by construction, because both values come from the
  same clock. The total delay a held-back command can exploit is bounded by
  `validity` + beacon age.
- **Default `validity` = 3 000 ms.** It must exceed the delivery path cost: about
  180 ms of Ed25519 verify per hop on the M0+, plus about 175 ms to sign.
- **A reboot of the actuator expires every outstanding command,** because `boot_id`
  changes.

### F.3 The rule

`actuation_decision(input) -> Decision` is a **pure function**: no I/O, no clock read,
no mutation. All values arrive as arguments. It returns `Act{setpoint}` only if
**every** condition holds. Otherwise it returns `Reject(reason)` for the **first**
failing one, in this order:

| # | Condition | Input | Reason |
|---|---|---|---|
| 1 | Full v0B verification succeeded (origin, content, freshness, network) | `v0b_ok: bool`, from `process_v0b` | `NotVerified` |
| 2 | Origin is a **command authority** for this `actuator_id` | provisioned table | `NotAuthorized` |
| 3 | Origin not revoked (re-checked at the actuator, which may hold a newer epoch) | `revoked: bool` | `Revoked` |
| 4 | Not expired: `boot_id` matches, `now ≤ deadline`, `deadline − now ≤ MAX_VALIDITY_MS` | `boot_id`, `now_ms` | `Expired` |
| 5 | R14: `is_action_safe(agent, threshold)` | `r14_safe: bool` | `R14Unsafe` |
| 6 | Within physical limits: `clamp_command(...)` raised **no** flag | `ClampResult` | `OutOfLimits` |
| 7 | **`cmd_seq > last_executed_seq[actuator_id]`** | per-actuator state | `StaleOrReplayed` |

- **Out-of-limit commands are rejected, not clamped** (condition 6, the choice the
  prompt asks for). Clamping would execute something the commander never asked for:
  "max thrust" instead of an impossible value. A command outside the envelope signals
  a faulty or hostile commander, and the safe response to that is to not move.
- **Condition 7 is added beyond the prompt's six.** v0B deliberately accepts
  reordering within 127 positions, which is right for messaging and wrong for
  actuators. An older setpoint delivered after a newer one would otherwise execute.
  Condition 7 also blocks replay at the application layer, independently of the v0B
  window. Because a reboot changes `boot_id` (condition 4), `last_executed_seq` can
  stay in RAM.
- **On `Act`:** the caller drives the actuator once and records `cmd_seq`.
- **On `Reject`:** no action of any kind, the reason is logged, and that reason's
  counter is incremented. There is no partial action.
- **Authority table (v1).** Provisioned at build or commissioning time. Distributing
  it as an operator-signed list, like `ORV1`, is left for later.

---

## Part B — tests and proofs to write

Tests (`rev_*`, `act_*`), one per attack row of the prompt:

| Test | Expected |
|---|---|
| `rev_wrong_key_rejected_state_unchanged` | `BadOperatorSig`, state unchanged |
| `rev_old_epoch_rejected` (`<` and `==`) | `Rollback` / `Duplicate`, not forwarded |
| `rev_tampered_byte_rejected` (exhaustive over the signed bytes) | rejected |
| `rev_then_revoked_origin_dropped_before_signature` | `origin revoked` at the first relay |
| `rev_persisted_across_reboot` | still rejected after restore from the simulated store |
| `rev_disconnected_relay_catches_up` | beacon → re-originated blob → applied → origin dropped |
| `rev_counter_revocation_by_revoked_node_rejected` | dropped (revoked origin), and `BadOperatorSig` even if re-carried |
| `rev_k_of_n_quorum` (k−1 signatures rejected, k accepted) | as stated |
| `rev_shrinking_list_rejected` | `Shrink` |
| `act_unauthorized_origin` / `act_r14_unsafe` / `act_expired` / `act_wrong_boot_id` / `act_replayed` / `act_reordered_older` / `act_out_of_limits` | `Reject(...)`, no action |
| `act_valid_executes_exactly_once` | `Act` once, then `StaleOrReplayed` |

Kani:
- `actuation_decision` returns `Act` ⇒ all seven conditions hold (symbolic inputs; the
  v0B result is an input, so Ed25519 is not unrolled).
- `r14_safe == false` ⇒ `Reject`, whatever the other inputs.
- `revocation_transition` never lowers `epoch`.
- A rejected revocation returns `None` and leaves state unchanged.

## Part C — silicon plan (summary)

`MAX_ENV` → 512 in `uart_mesh.rs` (test-harness constant). The operator seed lives on
the PC; A only carries the signed blob. C drives an observable GPIO (the on-board
LED, GP25) as the actuator. Tests follow the prompt's C.1–C.4, with `K` sent to all
boards first and per-packet sequence numbers. **One power-cut of B (C.2) — I will
ask before it.**

## Part D — attack suite: known risks before starting

- **Reticulum.** Pure Python, `pip install rns==<pinned>`, local instances over
  `TCPInterface`. Low install risk.
- **Zephyr Bluetooth Mesh in BabbleSim.** Needs a Linux toolchain. On this Windows
  PC that means WSL; the 3.3 GB WSL instance already struggles with Kani. **Installing
  it could exceed the prompt's one-hour budget.** If it does, I will stop and propose
  marking it "non exécuté" rather than accept a doubtful setup.
- **Meshtastic.** Optional, same one-hour rule.

## Decisions requested before code

1. **Revocation is permanent** (superset rule, re-key to recover).
2. **Out-of-limit commands rejected**, not clamped.
3. **A seventh condition** (`cmd_seq` strictly increasing per actuator).
4. **Authority table provisioned statically** in v1.
5. **Time base** = the actuator's own clock, via signed beacons with `boot_id` (§F.2).

## Status (2026-10-06, end of day)

The five decisions above were approved as proposed. Implemented on
`oasis-e-f-attacks`:

- **Part B:** `oasis-rt/src/mesh_revocation.rs` and `oasis-rt/src/actuation.rs`,
  with 28 tests plus 3 quorum tests in `oasis-operator-key`, and 6 Kani harnesses
  (verified individually in WSL: 13/16 on the first run, because the revocation harnesses' unwind bound was too small for `memcmp`; 16/16 after the fix, `evidence/kani/2026-10-06/`). Library tests went
  466 → 494, workspace 533, 0 failed; `no_std` `thumbv6m` builds.
- **Part C** on 3 RP2040 boards, stamp `6daa0bc`:
  `evidence/silicon/2026-10-06/ef/REPORT.md`.
- **Part D: deferred** (`attack-suite/DEFERRED.md`).

Deviations from the text above, found while building:

1. **`boot_id` uniqueness.** A boot that receives no command reserved no lease
   block, so two boots could share a `boot_id`. Firmware now forces one durable
   reservation at every boot. Verified on silicon: `boot_id` went 0 → 1279 across a
   power-cut, and a command stamped for the old boot was refused.
2. **NaN.** `hal::clamp_command` raises no flag on NaN or infinite inputs (all its
   comparisons are false). `actuation::command_within_limits` rejects non-finite
   values before consulting the clamp flags. `hal.rs` is unchanged.
3. **Frame size.** `MAX_ENV` stayed at 300. A single-key list is a 99-byte v0B
   header plus 127 + 8·n bytes of content: one fingerprint is 234 bytes on the
   wire, two are 242, and up to 9 fit. It must be raised to 512 before carrying
   k-of-n lists on the chip.
4. **Roles.** The rig is wired one way (A→B→C), so on silicon the revoked node is B
   and the enforcing relay is C (see the evidence report).
5. **Catch-up beacons** (E.5) exist in `oasis-rt` and are tested on the PC, but are
   not implemented in the test firmware.
