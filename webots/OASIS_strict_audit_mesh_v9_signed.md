# OASIS — Strict shadow audit: SPORE\x09 signed mesh envelopes

**2026-04-22.** Closed the last architectural security hole flagged in
the prior strict audits: unauthenticated `origin_fp` in the mesh header.
This doc is deliberately honest about what v9 does and doesn't do.

---

## 1. What actually shipped

### New wire format SPORE\x09

```
[0..6]    "SPORE\x09"    magic (distinct from \x08)
[6..14]   msg_id         u64 LE
[14..22]  origin_fp      8 bytes
[22]      ttl            u8   — mutable on forward, NOT covered by MAC
[23..25]  hops_so_far    u16 LE — mutable on forward, NOT covered by MAC
[25..33]  mac_tag        8 bytes — HMAC-SHA256 truncated to 64 bits
[33..]    inner_payload
```

MAC is computed over `SPORE_V9_MAGIC || msg_id_le || origin_fp` — 22 bytes,
fixed length. Explicitly excludes TTL and hops so the tag survives every
hop's decrement/increment without recomputation.

### MAC construction

RFC 2104 HMAC-SHA256 truncated to 8 bytes, hand-rolled in ~15 lines using
the already-in-tree `sha2` crate. No new dep.

```rust
pub fn hmac_sha256_8(key: &[u8; 32], msg: &[u8]) -> [u8; 8] { ... }
pub fn mesh_v9_tag(key, msg_id, origin_fp) -> [u8; 8] { ... }
pub fn mesh_v9_verify(key, msg_id, origin_fp, got) -> bool { ... }  // constant-time
```

### Router API

```rust
MeshRouter::new(fp)                           // v8, unchanged
MeshRouter::new_signed(fp, MeshMacKey([u8;32])) // v9, new
router.is_signed() -> bool
```

### Policy rules (strictly enforced by `parse_and_verify`)

1. Signed router + incoming v9 → verify MAC; drop on bad MAC.
2. Signed router + incoming v8 → drop ("unsigned v8 rejected by signed router").
3. Unsigned router + incoming v9 → drop ("v9 received but router has no key").
4. Unsigned router + incoming v8 → accept (unchanged from pre-v9 behavior).

This means a swarm is either all-v8 or all-v9; mixed mode is a deliberate
dead end. Operators migrate everyone at once.

### Tests added (9)

| Test | What it proves |
|---|---|
| `v9_signed_roundtrip_origin_to_hop` | v9 envelope parses + forwards, MAC stable |
| `v9_tag_tampering_rejected` | flip 1 bit of tag → Drop("bad mesh mac") |
| `v9_origin_fp_spoofing_rejected` | change origin_fp bytes → Drop("bad mesh mac") — **the main attack closed** |
| `v9_wrong_key_rejected` | mismatched keys reject |
| `v9_signed_router_rejects_unsigned_v8` | no downgrade attack |
| `v9_unsigned_router_rejects_v9` | no mode confusion |
| `v9_ttl_decrement_preserves_mac_through_chain` | 3-hop chain, MAC valid at every hop |
| `v9_inner_slice_zero_copy_view` | v9 inner at correct offset |
| `hmac_sha256_8_matches_rfc_2104_shape` | deterministic, key-sensitive, message-sensitive |

### Kani proofs added (2 verified, 1 honestly dropped)

| Proof | Result |
|---|---|
| `proof_mesh_v9_magic_distinct_from_v8` | ✅ 0.30 s |
| `proof_mesh_v9_header_length_arithmetic` | ✅ 0.09 s |
| `proof_mesh_hmac_tag_length_is_8` | ⛔ 300 s timeout — Kani bit-blasts SHA-256 compression (~100 k CNF clauses). NOT massaged to pass. Code comment in the module documents this. |

## 2. Scoreboard

| Metric | Pre | Post |
|---|---|---|
| Lib tests | 387 | **396** (+9) |
| Mesh tests | 15 | **24** (+9) |
| Kani proofs VERIFIED | 65 | **67** (+2) |
| Kani proofs attempted but dropped | 2 | **3** (documented) |
| Wire format versions | 15 (v1..v0F) | **16** (v1..v0F, SPORE\x09 added on the mesh layer) |
| `MeshRouter` RAM | ~56 KB | ~56 KB (MAC key is 32 B, negligible) |

## 3. What this round closes

### ✅ `origin_fp` spoofing (external attacker)
Before v9, an attacker on the wire with no PSK could craft a valid v8
envelope with any `origin_fp` and inject it. Downstream nodes would
dedup on the attacker-chosen msg_id, poisoning caches against legitimate
traffic from the claimed origin. **Dedup-DoS vector.**

After v9 with `new_signed`, the MAC binds `origin_fp` to knowledge of
the 32-byte MAC key. External attackers (no key) cannot produce a valid
tag; their envelopes are dropped before dedup insertion.

### ✅ Hop-by-hop tag stability
MAC excludes TTL/hops so the tag remains valid after every forward. No
per-hop signing machinery needed.

### ✅ No downgrade / mode confusion
Strict policy rules reject cross-mode envelopes.

## 4. What this round does NOT close — honest list

### 🔴 Insider attacks (shared-key model)
Every node in a v9 swarm holds the SAME 32-byte MAC key. Any compromised
node can forge envelopes claiming ANY `origin_fp`. The PSK is a swarm
admission token, not a per-node identity. **This is the expected
property of a shared-secret MAC**, not a bug, but it means v9 closes
"external attacker" not "malicious insider".

**To close the insider case:** switch to per-node Ed25519 signatures.
~2x the tag size (16 vs 8 bytes), ~1 ms signing vs ~1 µs MAC, plus a
pubkey registry. **Not done this round.**

### 🔴 64-bit tag has finite security
2^32 birthday bound against forgery under continuous attacker effort.
At 100 pkt/s attacker rate, ~1.3 years. OK for research, marginal for
critical multi-year deployments. 128-bit (16-byte) tag would be strict
RFC-recommended. **Not done this round** — kept 8 bytes to minimize
per-envelope overhead on lossy radio.

### 🔴 No key rotation mechanism
`MeshMacKey` is set at construction and never changes. For long-running
swarms, caller must tear down and rebuild the router to rotate keys.
**No graceful rotation protocol.** Not done this round.

### 🔴 Key derivation is the caller's problem
`MeshMacKey` is `pub struct MeshMacKey(pub [u8; 32])` — zero validation.
Caller is expected to derive it via HKDF from the swarm PSK or v5+
session key, but OASIS provides no helper. If the caller passes a weak
key, MAC strength degrades silently.

### 🔴 Still carried over from prior audits
- `tx_counter` not persisted across reboots. **Not touched.**
- Bloom false-positive silent drop (0.3–1 % FPR). **Not touched.**
- O(N × TTL) flood amplification. **Not touched.**
- SipHash-1-3 on u64 keys (suboptimal). **Not touched.**
- Real-hardware re-validation (3h23 S23 FE, PX4 SITL). **Not run.**
- 200-drone sim with v9. **Not run** — sim still uses unsigned routers.

## 5. New risks this round introduces

### ⚠️ MAC compute adds ~3 µs per origin_wrap + verify
HMAC-SHA256 on 22 bytes + sha2 on 1×64-byte block = ~2 SHA-256 compressions.
On a modern CPU ~1-2 µs each. Not benchmarked against the existing
`process()` hot path, but expected to dominate. For 1 kHz drone control
loops emitting at 100 pkt/s from one origin, that's 300 µs/s = 0.03 %
CPU. Fine. On MCU (no SHA hw), could be 10-50× slower — not measured.

### ⚠️ Doubled parser surface
`parse_and_verify` must correctly handle 4 combinations (signed/unsigned
router × v8/v9 envelope). Test coverage is 9/9 paths but static analysis
could find a case I missed — e.g., a v9 envelope with exactly 25 bytes
(truncated before the tag). That path is caught by the `envelope.len()
< header_len` check after the magic match; tested in `malformed_envelopes_drop_cleanly`
for v8 only, NOT re-tested explicitly for v9 truncation. **Gap.**

### ⚠️ "Unsigned v8 rejected" error string leaks mode
A signed router's rejection message tells the attacker they're in v9
territory. Minor info leak — the attacker already knows the swarm's
PSK-or-nothing posture from a single probe.

### ⚠️ No replay of v9 into v8 router scenarios tested
If a v9 origin somehow sends to a mixed peer that accepts v8, the peer
drops with the "v9 has no key" message. No weird intermediate state
tested where routers silently fall back.

## 6. What Kani does NOT prove here — stated plainly

OASIS cannot formally prove, via Kani, ANY of:
- HMAC-SHA256 is a secure PRF.
- 8-byte truncation preserves security.
- `mesh_v9_verify` is constant-time.
- The RFC 2104 construction is correctly implemented.

All four depend on SHA-256 compression, which is SAT-intractable. OASIS
relies on:
- RustCrypto's `sha2` crate (well-audited, constant-time).
- The RFC 2104 specification being correct (40-year peer-reviewed).
- Unit tests confirming the shape of our construction (9 new ones).
- Compiler to preserve the XOR-accumulator constant-time property.

This is a **trust delegation**, not a formal proof. I state it explicitly
because the "65 Kani proofs VERIFIED" headline should NOT be read as
"OASIS crypto is formally verified."

## 7. Updated cumulative Kani breakdown (67 total)

| Module | Count |
|---|---|
| hyper_state | 4 |
| efference | 3 |
| branching | 1 |
| emotion | 3 |
| morpho | 2 |
| dreams | 2 |
| world_model | 3 |
| **mesh** | **12** (6 base + 4 bloom + 2 v9) |
| topics | 2 |
| services | 4 |
| actions | 4 |
| hal | 5 |
| spinal | 3 |
| transforms | 5 |
| timers | 5 |
| synapse | 5 |
| parameters | 5 |
| **TOTAL** | **67** |

## 8. What the next session should pick

Unchanged from the previous strict audit's list, minus mesh-signing
which is now done:

- **`tx_counter` persistence** — wire it into one binary (cheapest).
- **Benchmark `process()` v8 vs v9 delta** — measurement, not code.
- **200-drone sim re-run with v9** — validate FPR/reach under signed mode.
- **Typed messages derive macro** — biggest DX gap vs ROS 2.
- **Per-node Ed25519 signatures** — closes the insider case left open here.

## 9. Disclosed non-coverage

- No key rotation path.
- No HKDF helper for deriving `MeshMacKey` from an existing PSK.
- No v9 fuzz corpus.
- `parse_and_verify` truncation-near-header-boundary explicitly tested
  only for v8; v9 short-envelope path covered by the generic length check
  but not by a targeted test.
- No CPU benchmark of origin_wrap/process v8 vs v9.
- No MCU CPU/memory characterization.
