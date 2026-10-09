# OASIS — Strict shadow audit (post-parameters round)

**2026-04-22.** This doc is deliberately ruthless. Prior audits leaned
marketing. Here I list only real shipped facts + real unfixed gaps.
No "honest pitch" section.

---

## 1. What actually shipped this round

- `parameters.rs` (~300 LOC) — typed Bool/Int/Float/String/Bytes server,
  declare-then-set pattern, numeric clamping via `NumericDescriptor`,
  monotonic `version` counter. BTreeMap-backed for determinism + Kani.
- 11 unit tests + 5 Kani proofs VERIFIED in < 1.1 s each.
- `get_float` latency on a 50-key server: **203 ns/op** (dominated by
  `format!()` allocating the key in the bench — real hot path with
  pre-built `&str` would be faster; not measured).
- Registered `pub mod parameters;` in `lib.rs`.

## 2. Scoreboard honnête

| Metric | Pre | Post |
|---|---|---|
| Lib tests | 374 | **385** (+11) |
| Kani proofs | 56 | **61** (+5) |
| Kani failures | 0 | 0 |
| Lib modules | 32 | **33** |
| rclcpp core matched | 7/7 | 7/7 (+parameters is rclcpp's 8th) |

**Note:** I count "7/7 core rclcpp primitives" as pub/sub/services/actions/
tf/timer (what every node uses). Parameters bring us to **8/8** of what
rclcpp considers its full node-level API surface.

## 3. Gaps that this round did NOT close

### 3.1 Parameters — real rclcpp Parameter feature set missing

| rclcpp feature | OASIS parameters | Honest verdict |
|---|---|---|
| Declare with type + default | ✅ | matched |
| Set with type check | ✅ | matched |
| Numeric range descriptor | ✅ min/max only | rclcpp has `step`, we don't |
| Read-only parameters | ❌ | no `read_only` flag yet |
| Parameter events (`/parameter_events`) | ❌ | `version` counter is a poor-man's version; no pub/sub broadcast |
| `declare_parameter` from YAML | ❌ | parser absent |
| Dynamic reconfigure callback | ❌ | no `add_on_set_callback` |
| Namespaced parameters (`foo.bar.baz`) | ⚠️ | string key works but no hierarchy semantics |
| CLI `ros2 param set` equivalent | ❌ | no CLI shell |
| Parameter persistence | ❌ | caller's problem, documented |

**Reality:** OASIS parameters are ~30% of rclcpp Parameter feature set.
The core contract (declare + typed set/get + bounds) is covered; the
ecosystem plumbing (YAML, events, reconfigure, CLI) is not.

### 3.2 Kani proofs only cover the clamp helpers

`clamp_float` + `clamp_int` are proven. **Nothing proven about:**
- version monotonicity (trivially true by `wrapping_add`, but not proven)
- type-mismatch rejection (unit test only)
- declare-then-declare rejection (unit test only)
- BTreeMap insertion correctness (out of reach — see § 5)

I explicitly did not add a Kani proof for `version_monotonic` because
`wrapping_add(1)` would trivially pass but mislead ("monotonic" actually
wraps at u64::MAX — after 5 × 10^11 years of 1-Hz updates, so irrelevant
in practice but not literally monotonic).

### 3.3 mesh.rs — weaknesses the current 6 Kani proofs do NOT address

User selected mesh.rs. Strict review:

1. **Dedup eviction = replay window.** `DEFAULT_DEDUP_CAP = 4096`. A node
   seeing 100 pkts/sec from 40 peers fills this in ~10 s. Past that, an
   attacker who recorded an evicted envelope can replay it and the node
   will re-forward it as fresh. The test `dedup_cache_evicts_oldest`
   literally documents this as "evicted msg_id should be accepted as
   fresh (documented limit)". Documented ≠ fixed.

2. **No authenticity on `origin_fp`.** Attacker can spoof any `origin_fp`
   in the mesh header. This means:
   - Dedup DoS: attacker floods 4096 msg_ids "from victim", evicting
     victim's legit traffic from dedup caches of peers.
   - The inner payload's v5+ signature protects *content* but not the
     mesh routing fabric.

3. **`tx_counter` not persisted.** Comment says "caller's responsibility."
   No caller in the current codebase persists it. Reboot → counter = 0 →
   potential msg_id collision with in-flight messages.

4. **Flood amplification grows as O(N × TTL).** For 200 drones at TTL=8,
   each origin broadcast spawns up to 1600 retransmissions before dedup
   kicks in. The 200-drone sim reports 84.8% dedup drops — meaning 15.2%
   are legitimate retransmissions = ~240 per broadcast. Acceptable for a
   small swarm, scales poorly past N ≈ 500.

5. **HashSet in hot path.** `seen_set: HashSet<u64>` uses the default
   SipHash-1-3 hasher. For u64 keys, FxHash or AHash is ~2× faster. Not
   critical but low-hanging.

6. **Echo guard is fingerprint-equality only.** Doesn't prevent ping-pong
   between two misconfigured nodes with different fps — dedup bails us
   out but under heavy dedup churn the bound degrades.

None of these are Kani-provable regressions; they're architectural /
threat-model gaps. The 6 existing mesh proofs (TTL decrement, forward
decision, bounded hops, msg_id no-panic, total forwards bounded, header
offsets) cover the wire-format layer, not the routing robustness layer.

### 3.4 Transcendental-dependent proofs still on the wishlist

Still untouched across the codebase:
- `wrap_angle` in `transforms.rs` (uses `atan2`, Kani can't)
- STDP `exp()` in `synapse.rs::update` (Kani can't)
- SHA-256 determinism in `topics.rs` / `spore_crypto.rs` (SAT-hard)
- SplitMix64 bijection in `mesh.rs::origin_msg_id` (SAT-hard)
- ChaCha20-Poly1305 security properties (out of scope — RFC 8439 compliance only)

These are covered by test vectors + RFC compliance, not Kani.

## 4. What OASIS still doesn't have vs ROS 2 (pre-1.0 honest list)

- **No IDL codegen.** Messages are `&[u8]`. rclcpp's `.msg` / `.action`
  files auto-generate C++ structs with serde. OASIS asks users to hand-roll.
- **No distributed discovery.** rclcpp has DDS node discovery over multicast;
  OASIS nodes must know peers' fingerprints via manual pairing.
- **No QoS profiles.** rclcpp offers 10+ QoS knobs (durability, reliability,
  history, deadline). OASIS has best-effort only.
- **No lifecycle.** rclcpp has `Lifecycle Nodes` (configure → activate →
  deactivate → shutdown). OASIS has nothing.
- **No rosbag.** No record/replay tooling.
- **No rviz.** No visualization.
- **No driver ecosystem.** ROS 2 has thousands of community packages for
  cameras, LIDARs, IMUs, robotic arms. OASIS has zero-driver auto-detection
  for phone sensors + MAVLink/PX4 — that's it.
- **No multi-language.** rclcpp has C++/Python/Rust/.NET bindings with IDL
  shared. OASIS is Rust-only.

## 5. Kani proofs — what's actually provable vs what we claim

Out of the **61 verified proofs**, breakdown by rigor level:

- **37 proofs = pure scalar arithmetic** (clamps, bounds, saturating math,
  wire header offsets). Genuinely formally verified.
- **13 proofs = envelope roundtrip on bounded payload sizes** (topics,
  services, actions, mesh header). Prove parse(build(x)) == x for small x.
- **6 proofs = termination / loop bounds** (mesh TTL, timer coalescing).
  Genuine but bounded to ≤ 16 iterations.
- **5 proofs = "no panic under any input"** (msg_id generation, etc.)
  Useful but weak — proves absence of panics, not functional correctness.

What we **don't** have and should stop claiming we do:
- End-to-end crypto security (ChaCha20-Poly1305 soundness, Ed25519 forgery
  resistance — we rely on the audited RustCrypto + ed25519-compact crates).
- Concurrency correctness (everything is single-threaded by design; no
  Send/Sync proofs).
- Integer overflow freedom in the stateful modules (`hyper_state.rs`,
  `efference.rs`, `synapse.rs::update`). We prove scalar invariants, not
  state evolution.

## 6. Cumulative Kani breakdown (61 total)

| Module | Count | Notes |
|---|---|---|
| hyper_state | 4 | stateful (monotonic, boundary, determinism, adversarial) |
| efference | 3 | mixed scalar + stateful |
| branching | 1 | scalar |
| emotion | 3 | scalar + 1 stateful |
| morpho | 2 | scalar |
| dreams | 2 | scalar |
| world_model | 3 | scalar |
| mesh | 6 | wire + termination |
| topics | 2 | wire roundtrip |
| services | 4 | wire roundtrip |
| actions | 4 | wire roundtrip |
| hal | 5 | clamp + geofence (strongest safety proof in the repo) |
| spinal | 3 | geometry (well-formed, disjoint, classify) |
| transforms | 5 | SE(2) identity / inverse |
| timers | 5 | pure per-timer scheduling |
| synapse | 5 | scalar plasticity math |
| **parameters** | **5** | **clamp helpers** |
| **TOTAL** | **61** | 0 failures, 0 claimed false |

## 7. Where to go next (prioritized by impact / cost)

1. **Fix the mesh dedup eviction replay window.** Concrete:
   add a Bloom filter as a second-level dedup, or require a timestamp
   header with MAX_AGE rejection. ~1 day, 2–3 new Kani proofs possible.

2. **Sign the mesh header.** Prepend an Ed25519 signature over the 25-byte
   mesh header (or require that `origin_fp` match the inner v5 signature's
   sender_fp). Closes the spoofing vector. ~2 days.

3. **YAML parameter file loader.** Cheapest DX win for ROS 2 parity.
   `serde_yaml` already a dep candidate. ~half a day.

4. **Typed message derive macro** — still the biggest open DX gap. ~1 week.

5. **Persistence layer** for `tx_counter` + parameter snapshot. ~1 day.

## 8. Things this audit does NOT cover (disclosed)

- I did not rerun real-hardware tests on the Samsung S23 FE for this
  round.
- I did not re-verify the PX4 SITL 4-waypoint mission.
- I did not check whether the 3h23 daemon run still reproduces.
- The 200-drone sim stats (84.8% dedup, 60-80% reach) were not re-run.
- All benchmark numbers are single-laptop, Windows 11, release mode,
  unloaded system. No A/B vs real ROS 2 on matched hardware.
