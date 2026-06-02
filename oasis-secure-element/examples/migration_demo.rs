//! OASIS — Progressive migration demo.
//!
//! Demonstrates that wire-format compatibility between in-process
//! signing and SE-backed signing makes a **fleet upgrade trivial**:
//! nodes can be migrated one at a time, in any order, with mixed-mode
//! operation throughout. No flag day, no atomic upgrade, no
//! coordinated rollback risk.
//!
//! Setup: 4-node mesh fleet (NODE_A/B/C/D).
//! Each node has a fixed Ed25519 seed (provisioned at commissioning).
//! At each phase, some nodes hold the seed in process, some hold it
//! behind a SimSecureElement. Every node sends one envelope; every
//! other node verifies it.
//!
//! Phases:
//!   Phase 0 — all 4 in-process (legacy state)
//!   Phase 1 — 1 of 4 migrated to SE
//!   Phase 2 — 2 of 4 migrated to SE
//!   Phase 3 — 3 of 4 migrated to SE
//!   Phase 4 — all 4 migrated (target state)
//!
//! At every phase: 4 senders × 3 receivers = 12 cross-verifications.
//! All 12 must Arrive at every phase. Total = 60 across 5 phases.
//!
//! Throughput per phase: dominated by the slowest signer. As more
//! nodes move to SE, max sustainable mesh sign rate drops. The
//! operator chooses the migration cadence based on the throughput
//! reduction they can absorb.

use std::time::Instant;

use oasis_rt::mesh::{
    mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter,
};
use oasis_secure_element::sim::SimSecureElement;
use oasis_secure_element::{sign_v10_envelope_via_se, SecureElement};

const SEEDS: [[u8; 32]; 4] = [[0xA0; 32], [0xB0; 32], [0xC0; 32], [0xD0; 32]];
const NAMES: [&str; 4] = ["A", "B", "C", "D"];

fn fp(b: u8) -> [u8; 8] {
    [b, 0, 0, 0, 0, 0, 0, 0]
}
fn fps() -> [[u8; 8]; 4] {
    [fp(0xA0), fp(0xB0), fp(0xC0), fp(0xD0)]
}

/// Signer for one node. In-process holds the seed in a MeshRouter;
/// SE-backed delegates the sign step to a SimSecureElement (with the
/// realistic 60 ms ATECC608B latency).
enum Signer {
    InProcess(MeshRouter),
    Se(SimSecureElement, MeshRouter), // SE for sig, MeshRouter for envelope framing
}

impl Signer {
    /// Build the v0A envelope: header + payload, with the signature
    /// produced by EITHER in-process or SE path. Wire-format identical.
    fn build_envelope(&mut self, payload: &[u8]) -> (Vec<u8>, u128) {
        match self {
            Signer::InProcess(router) => {
                let t0 = Instant::now();
                let env = router.origin_wrap(payload);
                (env, t0.elapsed().as_micros())
            }
            Signer::Se(se, router) => {
                // Build envelope normally (gives us msg_id, fp, header,
                // and an in-process sig we'll OVERWRITE with the SE sig
                // to demonstrate byte-equivalence).
                let t0 = Instant::now();
                let mut env = router.origin_wrap(payload);
                let mut msg_id_bytes = [0u8; 8];
                msg_id_bytes.copy_from_slice(&env[6..14]);
                let msg_id = u64::from_le_bytes(msg_id_bytes);
                let mut origin_fp = [0u8; 8];
                origin_fp.copy_from_slice(&env[14..22]);
                // SE-sign (this is the slow part — 60 ms simulated I2C+chip)
                let sig = sign_v10_envelope_via_se(se, msg_id, origin_fp).unwrap();
                // Overwrite envelope sig bytes [25..89] with SE-produced sig.
                // If wire-compat holds, the receiver's verify is unchanged.
                env[25..89].copy_from_slice(&sig);
                (env, t0.elapsed().as_micros())
            }
        }
    }
}

fn make_signer(idx: usize, use_se: bool) -> Signer {
    let seed = MeshEdSeed(SEEDS[idx]);
    let router = MeshRouter::new_ed25519_signed(fps()[idx], seed.clone(), MeshPubRegistry::new());
    if use_se {
        let mut se = SimSecureElement::new(); // realistic 60 ms latency
        se.provision(&SEEDS[idx]).unwrap();
        Signer::Se(se, router)
    } else {
        Signer::InProcess(router)
    }
}

/// Build a fleet-wide registry: every node knows every other node's pubkey.
fn full_registry() -> MeshPubRegistry {
    let mut reg = MeshPubRegistry::new();
    for i in 0..4 {
        let pk = mesh_v10_pubkey_from_seed(&MeshEdSeed(SEEDS[i])).unwrap();
        reg.insert(fps()[i], pk);
    }
    reg
}

fn run_phase(phase: u8, se_mask: [bool; 4]) -> (u32, u32, u128) {
    let modes: Vec<&str> = (0..4)
        .map(|i| if se_mask[i] { "SE" } else { "in-proc" })
        .collect();
    println!();
    println!("──────────────────────────────────────────────────────────────────");
    println!(
        "  Phase {} — A:{}  B:{}  C:{}  D:{}",
        phase, modes[0], modes[1], modes[2], modes[3]
    );
    println!("──────────────────────────────────────────────────────────────────");

    let mut signers: Vec<Signer> = (0..4).map(|i| make_signer(i, se_mask[i])).collect();

    let mut accepted = 0u32;
    let mut total = 0u32;
    let mut total_sign_us: u128 = 0;

    for sender_idx in 0..4 {
        let payload = format!("phase-{}-from-{}", phase, NAMES[sender_idx]);
        let (env, sign_us) = signers[sender_idx].build_envelope(payload.as_bytes());
        total_sign_us += sign_us;

        for receiver_idx in 0..4 {
            if receiver_idx == sender_idx {
                continue;
            }
            // Each receiver has the full fleet registry — production setup.
            let mut receiver = MeshRouter::new_ed25519_signed(
                fps()[receiver_idx],
                MeshEdSeed([0xFF; 32]), // receiver's own seed, not used for verify
                full_registry(),
            );
            total += 1;
            match receiver.process(&env) {
                MeshDecision::Arrived { .. } => accepted += 1,
                MeshDecision::Drop(reason) => {
                    println!(
                        "  [FAIL] {} → {}: Drop({:?})",
                        NAMES[sender_idx], NAMES[receiver_idx], reason
                    );
                }
            }
        }
    }

    let mean_sign_us = total_sign_us / 4; // 4 sends per phase
    println!("  cross-verifications: {} / {} accepted", accepted, total);
    println!(
        "  mean sign latency:   {} µs   (dominated by SE sleep when applicable)",
        mean_sign_us
    );
    println!(
        "  fleet sign rate:     {:.1} sign/s sustained per node",
        1_000_000.0 / mean_sign_us as f64
    );

    (accepted, total, total_sign_us)
}

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  OASIS Migration Demo — wire-compat enables progressive rollout  ║");
    println!("║  4-node fleet, 5 phases, 60 cross-verifications total            ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");

    let mut total_accepted = 0u32;
    let mut total_attempted = 0u32;
    let mut phase_throughputs: Vec<f64> = Vec::new();

    for (phase, mask) in [
        (0u8, [false, false, false, false]),
        (1, [true, false, false, false]),
        (2, [true, true, false, false]),
        (3, [true, true, true, false]),
        (4, [true, true, true, true]),
    ] {
        let (acc, tot, sign_us) = run_phase(phase, mask);
        total_accepted += acc;
        total_attempted += tot;
        let mean_per_send = (sign_us / 4) as f64;
        phase_throughputs.push(1_000_000.0 / mean_per_send);
    }

    // ── Summary ─────────────────────────────────────────────────
    println!();
    println!("══════════════════════════════════════════════════════════════════");
    println!("  Migration summary");
    println!("──────────────────────────────────────────────────────────────────");
    println!(
        "  Cross-verifications:   {} / {} ACCEPTED",
        total_accepted, total_attempted
    );
    println!();
    println!("  Phase   Mode breakdown        Sign throughput");
    println!("  ─────   ───────────────────   ────────────────");
    for (phase, tps) in phase_throughputs.iter().enumerate() {
        let n_se = match phase {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 3,
            _ => 4,
        };
        let label = format!("{}/4 SE-backed", n_se);
        println!("  {}        {:<20}  {:>8.1} sign/s/node", phase, label, tps);
    }
    println!();

    if total_accepted == total_attempted {
        println!("All cross-verifications succeeded. Wire-format compatibility");
        println!("means a single MeshRouter::process() codepath verifies envelopes");
        println!("regardless of whether the sender's seed lives in process memory");
        println!("or behind a Secure Element on the I2C bus. The receiver cannot");
        println!("distinguish — and doesn't need to.");
        println!();
        println!("Operationally:");
        println!("  - Migrate one node at a time during routine maintenance");
        println!("  - Mixed mode is stable indefinitely (no upgrade window)");
        println!("  - Per-node throughput drops as that node moves to SE");
        println!("  - Network-wide throughput dominated by slowest signer");
        println!("  - Rollback is per-node (re-flash with in-process build)");
        println!();
        println!("This is the load-bearing property that lets a TSO/grid operator");
        println!("roll out anti-tamper across thousands of nodes without a");
        println!("coordinated upgrade window.");
        std::process::exit(0);
    } else {
        eprintln!(
            "FAILURE: {} cross-verifications failed.",
            total_attempted - total_accepted
        );
        eprintln!("Wire-format compatibility is broken — investigate.");
        std::process::exit(1);
    }
}
