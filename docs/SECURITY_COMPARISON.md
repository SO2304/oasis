# Mesh security comparison — OASIS v0B vs Reticulum / Meshtastic / Thread

**Claim:** OASIS mesh v0B verifies origin, content, freshness and network of every
message **at each relay**, and rejects forged/modified/replayed/foreign messages
at the first hop — even after a relay reboot. This doc compares that to three
widely used meshes.

## Evidence policy (read this first)

- **OASIS cells** are backed by a test or log I produced in this repo and can
  reproduce: the `v0b_*` tests in [oasis-rt/src/mesh/tests.rs](../oasis-rt/src/mesh/tests.rs),
  the Kani proofs in [oasis-rt/src/mesh/kani_proofs.rs](../oasis-rt/src/mesh/kani_proofs.rs),
  and (Phase 3) the silicon logs under `evidence/silicon/`.
- **Competitor cells** summarise behaviour from the sources cited in
  `prompts/MESH_V0B_SECURITY.md`. I have **not** independently checked those
  repositories in this session, so each is marked **[non vérifié]** — the
  citation is the prompt author's, not mine. Where I rely only on general
  protocol knowledge I write **[général, non vérifié]**. Treat competitor rows
  as "to confirm against their source", not as established fact.

## Scenario matrix

| Attack | OASIS v0B | Reticulum | Meshtastic | Thread |
|---|---|---|---|---|
| **Payload modified in flight** | Rejected at every relay — payload is bound via `SHA-256(payload)` in the Ed25519-signed data. Test `v0b_content_swap_rejected_real_still_passes`, Kani `proof_v0b_preimage_binds_payload_digest`. | Relays don't crypto-check data packets; integrity only end-to-end **[non vérifié: RNS/Transport.py l.2018-2045]** | Channel AES-CTR gives confidentiality; integrity/auth of broadcast content not per-hop verified **[non vérifié]** | Network-key MIC gives link integrity to anyone holding the shared key **[général, non vérifié]** |
| **Message suppression (keep sig, swap content, poison dedup)** | Defeated: forged content fails the signature *before* any dedup/counter state is touched, so the real message still arrives. Test `v0b_content_swap_rejected_real_still_passes`. | N/A in the same form (no per-packet sig to keep) but no per-hop content auth either **[non vérifié]** | **[non vérifié]** | **[non vérifié]** |
| **Forged origin (signed with another key)** | Rejected — relay looks up the claimed origin's registered Ed25519 key. Test `v0b_forged_origin_rejected`. | Announces are validated; data packets have no source address by design **[non vérifié: validate_announce]** | DMs/admin signed since 2.5, verified at destination not per-hop **[non vérifié]** | Any network-key holder is "authentic" to the link **[général, non vérifié]** |
| **Immediate replay** | Rejected by the per-origin counter window. Test `v0b_immediate_replay_rejected`. | Short-term dedup cache **[non vérifié: Transport.py l.2123-2160]** | **[non vérifié]** | Frame counter **[général, non vérifié]** |
| **Replay after relay reboot** | Rejected — counter high-water mark is **persisted** and restored on boot. Test `v0b_replay_after_reboot_rejected` (+ Phase 3 silicon T8). | RAM dedup cache lost on restart ⇒ old packets re-accepted **[non vérifié]** | **[non vérifié]** | Depends on counter persistence **[général, non vérifié]** |
| **Replay after dedup-cache reset/overflow** | Rejected — the counter window is independent of the RAM Bloom. Test `v0b_replay_after_bloom_reset_rejected`. | Bounded cache ⇒ replay possible past the window **[non vérifié]** | **[non vérifié]** | **[général, non vérifié]** |
| **Message from another network** | Rejected before signature — `network_id` filter + the id is in the signed data. Test `v0b_foreign_network_rejected`. | Network segregation differs (no signed network id per packet) **[non vérifié]** | Different channel key ⇒ undecryptable, but relayed first **[non vérifié]** | Different network key **[général, non vérifié]** |
| **Revoked node** | Rejected at the first hop, before signature. Test `v0b_revoked_origin_rejected`. | **[non vérifié]** | **[non vérifié]** | Rekey the whole network **[général, non vérifié]** |
| **TTL/hops tampering** | Bounded on receipt (`ttl, hops, ttl+hops ≤ MAX_TTL`); hard anti-amplification is accept-once via the signed counter, not ttl. Test `v0b_ttl_inflation_bounded`. | Hop limit enforced; unsigned **[non vérifié]** | Hop limit; unsigned **[non vérifié]** | Mesh-local hop limits **[général, non vérifié]** |
| **Invalid-then-valid (state pollution)** | Verify-before-remember: a rejected message mutates no state. Test `v0b_invalid_then_valid_accepted`; Kani `proof_v0b_parse_never_panics_*`. | **[non vérifié]** | **[non vérifié]** | **[non vérifié]** |
| **Reorder within window** | Accepted exactly once (128-bit window). Test `v0b_reorder_within_window_accept_once`. | **[non vérifié]** | **[non vérifié]** | **[général, non vérifié]** |

## What OASIS pays for this

- **No sender anonymity.** `origin_fp` is in clear and every origin must be
  pre-registered (an Ed25519 pubkey per node). Reticulum deliberately omits
  source addresses for anonymity; OASIS makes the opposite trade.
- **~176 ms Ed25519 verify per hop on Cortex-M0+** (v0A silicon baseline;
  v0B adds one SHA-256 of the payload — see the PC bench `bench_mesh_v0b` and
  Phase 3 on-chip numbers). v0B is for low-frequency authenticated broadcasts,
  not kHz control loops — v9 HMAC remains the choice for high-rate intra-swarm
  traffic.
- **Persistence cost.** A per-origin counter high-water mark must survive reboot
  (flash-lease strategy, see MESH_V0B_SPEC.md §5).

## Cost numbers

**PC (K=10, median ± half-spread)** — raw log:
`evidence/mesh_v0b/2026-10-06/bench_mesh_v0b.log` (Windows laptop, release):

| op | v0A | v0B | delta |
|---|---:|---:|---:|
| origin_wrap (sign), 15 B | 250 058 ns ±4.3% | 251 596 ns ±25% | +0.6% |
| verify, 15 B | 122 945 ns ±2.0% | 125 585 ns ±1.3% | +2.1% |
| origin_wrap (sign), 1 KB | 250 542 ns ±1.7% | 250 915 ns ±2.7% | +0.1% |
| verify, 1 KB | 125 744 ns ±4.4% | 127 414 ns ±2.1% | +1.3% |

v0B costs essentially the same as v0A: Ed25519 dominates, and binding the
payload adds only one SHA-256 (~2-3 µs, ≈1%), flat even at 1 KB. **Security was
bought at ~1% of the per-message crypto cost, not a redesign of the hot path.**

**On-chip (3× RP2040, K=5):** Phase 3 — pending hardware reflash + the physical
power-cut for the persistence (T8) replay test. No on-chip number is entered
here until its raw log exists under `evidence/silicon/`.
