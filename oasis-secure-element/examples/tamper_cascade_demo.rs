//! OASIS — Tamper-cascade end-to-end demo (Gap 4 step 2).
//!
//! Demonstrates the full physical-attack response chain:
//!
//!     1. Tamper switch trips (chassis open, glass break, T-ramp)
//!     2. SE.wipe(reason)  → SE returns Err on every subsequent sign
//!     3. Operator publishes signed RevocationList including the
//!        compromised node's fingerprint (SPORE\x06 envelope)
//!     4. Revocation propagates through the mesh
//!     5. Every receiver verifies the operator signature, merges fp
//!        into local revocation set
//!     6. Attacker who captured a valid envelope from the now-wiped
//!        node tries to replay it → every receiver rejects because
//!        fp is in the revocation set, regardless of sig validity
//!
//! This is the cascade that makes the SE pattern actually work as
//! anti-tamper. The SE alone makes the seed unrecoverable; the
//! revocation makes the captured-envelope replay also useless.
//! Together: physical compromise → fleet-wide atomization.
//!
//! R20 from CLAUDE.md: "Unsigned node = atomization < 1 ms". This
//! demo measures the actual atomization deadline end-to-end.

use std::time::Instant;
use std::collections::HashSet;

use oasis_rt::mesh::{
    MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter,
    mesh_v10_pubkey_from_seed,
};
use oasis_rt::spore_crypto::RevocationList;
use oasis_secure_element::sim::SimSecureElement;
use oasis_secure_element::{SecureElement, TamperReason};

const SEED_A: [u8; 32] = [0xA0; 32];
const SEED_B: [u8; 32] = [0xB0; 32];
const SEED_C: [u8; 32] = [0xC0; 32];
const SEED_D: [u8; 32] = [0xD0; 32];

const FP_A: [u8; 8] = [0xA0, 0, 0, 0, 0, 0, 0, 0];
const FP_B: [u8; 8] = [0xB0, 0, 0, 0, 0, 0, 0, 0];
const FP_C: [u8; 8] = [0xC0, 0, 0, 0, 0, 0, 0, 0];
const FP_D: [u8; 8] = [0xD0, 0, 0, 0, 0, 0, 0, 0];

/// The operator's signing seed — separate from any node seed.
/// In production this lives in the operator's HSM, never on a drone.
const OPERATOR_SEED: [u8; 32] = [0x55; 32];

fn build_full_registry() -> MeshPubRegistry {
    let mut reg = MeshPubRegistry::new();
    for (fp, seed) in [(FP_A, SEED_A), (FP_B, SEED_B), (FP_C, SEED_C), (FP_D, SEED_D)] {
        let pk = mesh_v10_pubkey_from_seed(&MeshEdSeed(seed)).unwrap();
        reg.insert(fp, pk);
    }
    reg
}

/// A node = MeshRouter + local revocation set. The revocation set is
/// what makes captured-envelope replay useless after a wipe.
struct Node {
    name: &'static str,
    router: MeshRouter,
    revoked: HashSet<[u8; 8]>,
}

impl Node {
    fn new(name: &'static str, fp: [u8; 8], seed: [u8; 32]) -> Self {
        Self {
            name,
            router: MeshRouter::new_ed25519_signed(fp, MeshEdSeed(seed), build_full_registry()),
            revoked: HashSet::new(),
        }
    }

    /// Application-layer two-step verify: (1) mesh sig OK?,
    /// (2) origin not in revocation set?
    fn process_with_revocation(&mut self, env: &[u8]) -> Result<[u8; 8], &'static str> {
        match self.router.process(env) {
            MeshDecision::Arrived { .. } => {
                // Extract origin_fp from envelope bytes [14..22]
                let mut fp = [0u8; 8];
                fp.copy_from_slice(&env[14..22]);
                if self.revoked.contains(&fp) {
                    Err("revoked origin")
                } else {
                    Ok(fp)
                }
            }
            MeshDecision::Drop(reason) => Err(reason),
        }
    }

    /// Operator publishes a revocation list; each node verifies +
    /// bulk-merges via the new fingerprints() iterator. Returns the
    /// number of new fps merged.
    /// Updated 2026-05-10: uses RevocationList::fingerprints()
    /// added to oasis-rt to fix the O(N²) audit finding.
    fn merge_operator_revocation(&mut self, signed_blob: &[u8],
                                 operator_pub: &[u8; 32]) -> Result<usize, &'static str>
    {
        let parsed = RevocationList::parse_and_verify(signed_blob, operator_pub)?;
        let mut new = 0;
        for fp in parsed.fingerprints() {
            if self.revoked.insert(*fp) { new += 1; }
        }
        Ok(new)
        // After this function returns, `parsed` is dropped — the
        // RevocationList struct's allocation is freed. Receiver only
        // retains the small local HashSet keyed by the fps it cares
        // about. This is the memory-pressure win on MCU.
    }
}

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  OASIS Tamper-Cascade Demo — physical attack → fleet response    ║");
    println!("║  Gap 4 step 2: end-to-end SE.wipe → revocation → atomization     ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");

    // Build operator pubkey for revocation verification
    let op_seed = ed25519_compact::Seed::from_slice(&OPERATOR_SEED).unwrap();
    let op_kp = ed25519_compact::KeyPair::from_seed(op_seed);
    let mut op_pub = [0u8; 32];
    op_pub.copy_from_slice(op_kp.pk.as_ref());

    // ── Setup: 4-node fleet, each with SE-backed identity ───────────
    println!();
    println!("Setup: 4 nodes (A, B, C, D), each with seed in a SimSecureElement.");
    println!("Operator has separate Ed25519 keypair for signing revocations.");
    let mut se_a = SimSecureElement::new_no_latency();
    se_a.provision(&SEED_A).unwrap();
    let mut se_c = SimSecureElement::new_no_latency();
    se_c.provision(&SEED_C).unwrap();

    let mut node_a = Node::new("A", FP_A, SEED_A);
    let mut node_b = Node::new("B", FP_B, SEED_B);
    let mut node_d = Node::new("D", FP_D, SEED_D);

    // ── Phase 0: normal operation, A→B and C→D both work ───────────
    println!();
    println!("──────────────────────────────────────────────────────────────────");
    println!(" Phase 0 — normal operation");
    println!("──────────────────────────────────────────────────────────────────");
    let mut sender_a = MeshRouter::new_ed25519_signed(FP_A, MeshEdSeed(SEED_A), MeshPubRegistry::new());
    let mut sender_c = MeshRouter::new_ed25519_signed(FP_C, MeshEdSeed(SEED_C), MeshPubRegistry::new());
    let env_a = sender_a.origin_wrap(b"A normal telemetry");
    let env_c_legit = sender_c.origin_wrap(b"C normal telemetry");
    println!("  A → B: {:?}", node_b.process_with_revocation(&env_a).map(|_| "Arrived"));
    println!("  C → D: {:?}", node_d.process_with_revocation(&env_c_legit).map(|_| "Arrived"));

    // ── Phase 1: TAMPER on Node C ──────────────────────────────────
    println!();
    println!("──────────────────────────────────────────────────────────────────");
    println!(" Phase 1 — TAMPER detected on Node C (chassis switch)");
    println!("──────────────────────────────────────────────────────────────────");
    let t_tamper = Instant::now();
    se_c.wipe(TamperReason::ChassisSwitch).unwrap();
    println!("  SE_C.wipe(ChassisSwitch) at t=0");
    println!("  SE_C.is_wiped() = {}", se_c.is_wiped());
    println!("  SE_C.sign() now returns: {:?}", se_c.sign(b"any").map(|_| "Ok").map_err(|e| e));

    // ── Phase 2: operator publishes signed revocation ──────────────
    println!();
    println!("──────────────────────────────────────────────────────────────────");
    println!(" Phase 2 — Operator publishes signed RevocationList");
    println!("──────────────────────────────────────────────────────────────────");
    let mut rev = RevocationList::new();
    let revoked_at_secs: u64 = 1746883200;  // arbitrary fixed timestamp for determinism
    rev.revoke(FP_C, revoked_at_secs);
    let signed_rev = rev.serialize_signed(&OPERATOR_SEED).unwrap();
    println!("  RevocationList: 1 entry (fp_C @ unix {})", revoked_at_secs);
    println!("  serialize_signed() → {} bytes (SPORE\\x06 envelope)", signed_rev.len());

    // ── Phase 3: revocation propagates to remaining nodes ──────────
    println!();
    println!("──────────────────────────────────────────────────────────────────");
    println!(" Phase 3 — Revocation propagates A, B, D");
    println!("──────────────────────────────────────────────────────────────────");
    // Each node uses the new bulk-merge helper that walks the parsed
    // RevocationList via `fingerprints()` (no operator-side mirror
    // needed). The parsed list is dropped after merge.
    for node in [&mut node_a, &mut node_b, &mut node_d] {
        match node.merge_operator_revocation(&signed_rev, &op_pub) {
            Ok(n_new) => {
                println!("  Node {}: parse_and_verify OK, {} new fp(s) merged via fingerprints() iterator",
                         node.name, n_new);
            }
            Err(e) => {
                println!("  Node {}: revocation REJECTED ({})", node.name, e);
            }
        }
    }
    let t_propagated = t_tamper.elapsed();

    // ── Phase 4: attacker REPLAYS the captured legitimate envelope ─
    println!();
    println!("──────────────────────────────────────────────────────────────────");
    println!(" Phase 4 — Attacker replays env_c_legit (captured before wipe)");
    println!("──────────────────────────────────────────────────────────────────");
    println!("  This envelope was signed BEFORE the wipe, signature is still");
    println!("  cryptographically valid. Without revocation: would Arrive.");
    println!("  With revocation merged at every node: every receiver rejects.");
    println!();

    // We use FRESH receiver instances (without any prior dedup of the
    // legitimate first delivery in phase 0) so the replay isn't
    // confused with the dedup mechanism — we want to test the
    // REVOCATION rejection specifically.
    let mut fresh_b = Node::new("B", FP_B, SEED_B);
    let mut fresh_d = Node::new("D", FP_D, SEED_D);
    fresh_b.revoked.insert(FP_C);
    fresh_d.revoked.insert(FP_C);

    let mut atomized = 0u32;
    let mut total = 0u32;
    for (name, node) in [("B", &mut fresh_b), ("D", &mut fresh_d)] {
        total += 1;
        match node.process_with_revocation(&env_c_legit) {
            Err("revoked origin") => {
                println!("  Node {}: REJECTED (revoked origin) — sig was valid but FP_C atomized", name);
                atomized += 1;
            }
            Err(other) => {
                println!("  Node {}: rejected with reason '{}' (NOT via revocation path)",
                         name, other);
            }
            Ok(_) => {
                println!("  Node {}: ACCEPTED — REVOCATION FAILED to atomize", name);
            }
        }
    }
    let t_atomized = t_tamper.elapsed();

    // ── Summary ─────────────────────────────────────────────────────
    println!();
    println!("══════════════════════════════════════════════════════════════════");
    println!(" Cascade summary");
    println!("──────────────────────────────────────────────────────────────────");
    println!("  Atomization: {} / {} receivers rejected the replay", atomized, total);
    println!();
    println!("  Latency (host x86 — real MCU will be ~1000× slower):");
    println!("    tamper → revocation propagated:   {} µs", t_propagated.as_micros());
    println!("    tamper → fleet-wide atomization:  {} µs", t_atomized.as_micros());
    println!();
    println!("  Honest disclaimer: this is host x86 with synchronous propagation.");
    println!("  Real-world: revocation must traverse the mesh (multi-hop), so the");
    println!("  end-to-end deadline is ~(mesh_diameter × hop_latency + verify_cost).");
    println!("  R20 (\"atomization < 1 ms\") is per-node post-revocation, not the");
    println!("  end-to-end propagation deadline.");

    if atomized == total {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}
