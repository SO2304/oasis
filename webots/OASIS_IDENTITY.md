# OASIS — What this project actually is

**2026-04-22.** Corrective document after a session drift. The last
~15 rounds measured OASIS heavily against ROS 2 rclcpp and cited
"28× faster than rclcpp intra-process" as the headline number. **That
framing is wrong.** OASIS is not competing with ROS 2. They overlap
in one sub-layer (pub/sub primitives) and diverge on everything else.
This document restores the actual positioning.

---

## What OASIS is

### 1. A bio-inspired adaptive kernel
Eleven mechanisms modeled on nervous-system primitives, running as
one continuous process:

- **M1 Tension field** — agents communicate via force vectors in a shared
  latent space, never direct calls (rule R2).
- **M2 HyperState + R14 entropy gate** — each agent is a continuous
  128-dim vector; discrete states (CREATED, RUNNING, etc.) are
  anchor-projections of that vector. R14 is a formally-proven safety
  gate: if entropy > 0.95, physical action is blocked.
- **M3 Efference copy / predictive processing** — every motor command
  produces an internal prediction; actual sensor input is compared, and
  the divergence updates a pain memory.
- **M4 Temporal branching** — fork N candidate futures, simulate via
  gradient descent, collapse to the fittest.
- **M5 Emotional gain modulation** — 5 emotions (fear, curiosity,
  frustration, calm, saturated) that reshape thresholds on other
  mechanisms. Fear ≤ 5× saturation (Kani-proven).
- **M6 Morphogenesis** — agents specialize into roles (Stem, Navigator,
  Sentinel, Worker, Scout, Healer) driven by field needs.
- **M7 Hebbian / STDP synapses** — co-activation strengthens a link;
  pre-before-post strengthens further; weights bounded, decay guarantees
  non-runaway (Kani-proven).
- **M8 Dream consolidation** — during idle, replay experiences; strong
  synapses reinforced, weak pruned.
- **M9 Reflex arc** — hardwired condition-action rules on raw telemetry
  BEFORE any deliberative processing. Reaction < 100 µs.
- **M10 Non-Euclidean world model** — entities are continuous pressure
  zones, not geometric obstacles. Navigation = gradient descent through
  the combined field.
- **M11 Federated resonance** — peers exchange signed digests over UDP
  multicast; accepted entries update the local tension field.

These are not "features on top of a middleware." They are the kernel.
The middleware primitives (topics/services/actions/timers) exist
because they're useful plumbing for the daemon to communicate with
other processes — not because OASIS's point is pub/sub.

### 2. A "living" daemon
The `main.rs` loop does not die. If a sensor fails, entropy rises, R14
gates unsafe actions, vitality degrades from Healthy → Degraded →
Critical → Dead. If the device is stressed, fear rises, synapses
damped, motor output attenuated. If idle, dreams fire, pruning
happens.

Validated: 3h23 continuous run on a Samsung Galaxy S23 FE, 121 290
ticks, zero crash. The daemon *suffers, adapts, continues.* That's
the first-order design property, not tick-rate.

### 3. Cryptographically-authenticated mesh by default
Spore wire format 1 → 0A (13 versions), security monotonically
increasing:

- v1-v2: unauthenticated baseline + FEC
- v3: AEAD with pre-shared key (external attacker excluded)
- v4: ECDH forward secrecy (past-traffic compromise limited)
- v5: Noise-KK (dual DH = sender authenticated)
- v6: Signed operator revocation envelope
- v7: Monotonic counter + 128-bit replay window
- v8: Multi-hop mesh + Bloom dedup
- v9: Mesh + HMAC-SHA256-8 (external-attacker resistance on mesh header)
- v0A: **Mesh + Ed25519 per-node signature** (insider-resistant — a
  node with the shared PSK still cannot forge other senders)

This trajectory was built across many sessions, not retrofitted.
Other robotics middlewares treat security as optional (DDS Security
exists but is rarely deployed); OASIS makes it a wire-format contract.

### 4. Formally verified where possible
**69 Kani proofs across 17 modules.** Zero failures. Examples:

- **HAL geofence**: position outside bounding box ⇒ (force, torque,
  velocity) all zero, bit-exactly, provably.
- **R14 entropy gate**: signal > threshold ⇒ action blocked, across
  all finite float inputs.
- **Synapse weight bounds**: Hebbian reinforce clamp is provably
  monotonic under any reward/rate/eligibility.
- **Mesh TTL**: termination proven — TTL reaches 0 in ≤ initial_ttl
  hops, no infinite forwarding.
- **Bloom filter primitives**: bit-set-then-test correctness, OR
  idempotence.

No ROS 2 component ships formal proofs at this density. This is not
a competitive advantage — it's a different posture.

### 5. MCU-to-phone-to-laptop spectrum
```
cargo build --target thumbv7em-none-eabi --lib --no-default-features --features mesh_bloom_mcu --release
# 0 errors, .o files produced for Cortex-M4F
```
Same kernel on:
- Cortex-M4F (verified to compile; hardware boot unverified)
- Android via Termux (3h23 real run)
- Windows/Linux laptop (427/427 tests)

The bio-mechanism code path is deterministic across all three —
proven this session by the K=10 soak bench equality check.

---

## What OASIS is NOT

### Not a ROS 2 replacement
ROS 2 ships:
- DDS multi-language bindings
- A vast driver/sim ecosystem (rviz, rosbag, Nav 2, MoveIt, ros2_control)
- QoS profiles, lifecycle nodes, parameter events, dynamic reconfigure
- A community + tooling moat built over 15 years

OASIS has NONE of these. A roboticist writing a new pipeline with
camera + LIDAR + URDF + planning stack should use ROS 2. There is no
version of OASIS where "switch from ROS 2 to OASIS" makes sense for
that workload.

### Not flight-certified, not externally audited, not production-ready
Pre-1.0 research kernel. Zero external crypto audit. No DO-178C, no
IEC 62304, no certified hardware. One successful PX4 SITL 4-waypoint
mission (altitude ±6 cm) is the only autonomous flight demo.

### Not a fast-pub/sub middleware
The "28× faster than rclcpp intra-process at 16 B, 3.75× at 1 MB" is
a real measurement (see CLAUDE.md for banded numbers) but it's a
side-effect of being a function-call-level kernel vs a DDS-bridging
executor. OASIS didn't optimize for that — it ended up there because
executor-less dispatch is what a bio-mechanism kernel needs anyway.

**Stop citing that number as the pitch.** Cite the bio-kernel, the
formal proofs, the living-daemon property, the cryptographic mesh.

### Not a product
No users besides the developer. No deployments besides the author's
laptop and phone. The "calibrated performance numbers" are
machine-local measurements, not field data from a customer drone.

---

## Where OASIS and ROS 2 actually differ

| Axis | OASIS | ROS 2 |
|---|---|---|
| **Primary purpose** | Bio-inspired adaptive kernel | Middleware for robotics composition |
| **Communication model** | Tension field (latent-space forces) + pub/sub | Topics / services / actions as primary API |
| **Security** | v3-v0A chain — mandatory where it matters | DDS Security (optional, rarely deployed) |
| **Formal verification** | 69 Kani proofs on safety invariants | None shipped |
| **Lifecycle model** | Vitality states (Healthy → Dead), organic | Managed lifecycle nodes (configure → activate) |
| **Hardware adaptivity** | R14 entropy gate + reflex arc + morphogenesis | Software only; hardware via drivers |
| **Failure posture** | Suffers and adapts (fear, pain memory) | Exits on error; supervisor restarts |
| **Code size** | ~16 KLOC lib | ~1 MLOC |
| **Deploy target** | MCU + phone + laptop (same kernel) | Linux + some QNX |
| **Ecosystem** | None (research artifact) | ROS PKG index (thousands of pkgs) |

OASIS and ROS 2 are orthogonal. The A/B numbers say "if you happen to
need just pub/sub, OASIS is fast." They don't say "OASIS is the better
robotics middleware."

---

## What this identity doc is for

1. **Correct future framing in new audits.** The ROS 2 numbers stay in
   CLAUDE.md as incidental measurement; they should not lead.
2. **Anchor the session pattern.** When picking the next feature, prefer
   items that strengthen the bio-kernel / formal-verification / mesh-
   security / living-daemon axes. Deprioritize generic-middleware items
   (typed messages derive macro, lifecycle manager clone, etc.) that
   would only make OASIS "a slower ROS 2 with weirder primitives."
3. **Honesty about scope.** OASIS is a specialist research kernel with
   real engineering discipline (69 Kani proofs, MCU compile, 427
   tests). It is not trying to replace ROS 2, and the one-liner that
   pretends it is should be removed from anywhere I've put it.

---

## Carried-forward items that match this identity

High-identity-fit next steps:
- **Real MCU hardware boot** — close the "compile ≠ runs" gap. Proves
  the "MCU-to-laptop spectrum" claim on hardware.
- **v0A key rotation protocol** — deepens the cryptographic mesh story.
- **More Kani proofs on bio-mechanisms** (M1 tension, M9 reflex, M11
  federation currently at 0 proofs each) — deepens the formal-
  verification story.
- **Real phone re-run** — revalidate the 3h23 daemon claim with the
  current post-session codebase.

Low-identity-fit (skippable):
- Typed messages derive macro (DX catch-up with rclcpp)
- rosbag-equivalent playback
- rviz-equivalent GUI

These three WOULD make OASIS feel more like ROS 2 — and that's not
what OASIS is. Skip.
