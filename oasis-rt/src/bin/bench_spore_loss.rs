//! OASIS — Spore loss-tolerance benchmark
//!
//! Simulates packet loss in software and measures successful message recovery.
//! No actual network — uses fragment_v2 + Reassembler in-process.
//!
//! Output: success rate at loss percentages 0%, 10%, 20%, 30%, 40%, 50%
//! across (no-FEC, FEC, FEC+repeat=2, FEC+repeat=3) configurations.
//!
//! Run: cargo run --release --bin bench_spore_loss
//!
//! ── Hygiene note (2026-04-22) ──
//! This bench runs N=200 independent trials per (config, loss%) cell and
//! reports the resulting success rate. Internal N=200 already averages
//! away per-trial drop-sequence randomness. No outer K-repeats loop was
//! added during the session's statistical-bands hygiene round: the
//! output is a success fraction, not a latency — bands-over-medians
//! semantics don't apply cleanly. To get inter-run variance, re-run the
//! binary manually; expect ~±2% on low-loss cells, ~±5% on 50%-loss.

use oasis_rt::federation::FederatedMesh;
use oasis_rt::spore::{fragment_v2, parse_chunk_v2, Reassembler};
use oasis_rt::vec::*;

const TRIALS_PER_CELL: usize = 200;
const SEED_BASE: u32 = 0x9E3779B1;

fn main() {
    println!("OASIS spore loss-tolerance bench (n={} trials per cell)", TRIALS_PER_CELL);
    println!();

    let mesh = build_mesh();
    let payload = mesh.serialize_to_vec();
    println!("Payload: {} bytes serialized", payload.len());

    let configs = [
        ("v2 no-FEC,        repeat=1", false, 1u8),
        ("v2 +XOR-parity,   repeat=1", true,  1u8),
        ("v2 +XOR-parity,   repeat=2", true,  2u8),
        ("v2 +XOR-parity,   repeat=3", true,  3u8),
    ];
    let losses = [0.0, 0.10, 0.20, 0.30, 0.40, 0.50];

    println!();
    print!("{:32}", "config");
    for l in &losses { print!(" {:>5.0}%", l * 100.0); }
    println!();
    println!("{}", "-".repeat(32 + 6 * losses.len() + losses.len()));

    for (label, fec, repeat) in &configs {
        print!("{:32}", label);
        for &loss in &losses {
            let success = trial(&payload, *fec, *repeat, loss);
            print!(" {:>5.1}%", success * 100.0);
        }
        println!();
    }

    println!();
    println!("Bandwidth multipliers (bytes sent vs raw payload):");
    for (label, fec, repeat) in &configs {
        let chunks = fragment_v2(&payload, 0, *fec);
        let bytes_sent: usize = chunks.iter().map(|c| c.len()).sum::<usize>() * (*repeat as usize);
        println!("  {:32} {:.2}x  ({} bytes for {} payload)",
            label, bytes_sent as f64 / payload.len() as f64, bytes_sent, payload.len());
    }
}

fn trial(payload: &[u8], fec: bool, repeat: u8, loss_rate: f64) -> f64 {
    let mut successes = 0;
    let mut rng_state = SEED_BASE;
    for trial_i in 0..TRIALS_PER_CELL {
        rng_state = rng_state.wrapping_mul(0x9E3779B1).wrapping_add(trial_i as u32);
        if reassemble_with_loss(payload, fec, repeat, loss_rate, &mut rng_state) {
            successes += 1;
        }
    }
    successes as f64 / TRIALS_PER_CELL as f64
}

fn reassemble_with_loss(
    payload: &[u8],
    fec: bool,
    repeat: u8,
    loss_rate: f64,
    rng_state: &mut u32,
) -> bool {
    let chunks = fragment_v2(payload, 1, fec);
    // Build reassembler from first chunk header.
    let (msg_id, total, _, flags, _) = parse_chunk_v2(&chunks[0]).unwrap();
    let mut reass = Reassembler::new(msg_id, total, flags);

    // Simulated transmission: each chunk is sent `repeat` times. For each copy,
    // drop with probability `loss_rate`. Receiver dedups by chunk_idx.
    for _ in 0..repeat {
        for pkt in &chunks {
            *rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let r = (*rng_state as f64) / (u32::MAX as f64);
            if r < loss_rate { continue; }
            let (_, _, idx, _, payload_chunk) = parse_chunk_v2(pkt).unwrap();
            if let Some(recovered) = reass.feed(idx, payload_chunk) {
                return recovered == payload;
            }
        }
    }
    false
}

fn build_mesh() -> FederatedMesh {
    // Force a payload that requires 3+ chunks (multi-fragment regime).
    let mut fed = FederatedMesh::new();
    for i in 0..40u32 {
        let mut a = vz();
        for d in 0..16usize {
            a[d * 8 % 128] = ((i + d as u32) as f64) / 100.0;
        }
        fed.pool_push_test(a, 0.5, 1.0, 0.4);
    }
    fed
}
