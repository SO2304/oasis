//! AA1-followup probe — investigate why Bloom froze at 7472 inserts
//! when the theoretical 1% FPR threshold is ~52 000 inserts.
//!
//! Hypotheses:
//!   H1. msg_id values from origin_msg_id collide for sequential counters
//!   H2. bloom_bit_index produces a small image set
//!   H3. There's a feedback loop where each rejection prevents insertion,
//!       making FPR rise faster than expected
//!
//! This probe is purely a diagnostic and does not need realistic envelopes.

use oasis_rt::mesh::{
    bloom_bit_index, bloom_contains, bloom_insert, origin_msg_id, BLOOM_BITS, BLOOM_HASHES,
    BLOOM_WORDS,
};
use std::collections::{BTreeSet, HashMap};

fn fp(b: u8) -> [u8; 8] {
    let mut f = [0u8; 8];
    f[0] = b;
    f
}

fn main() {
    println!();
    println!("BLOOM_BITS    = {}", BLOOM_BITS);
    println!("BLOOM_WORDS   = {}", BLOOM_WORDS);
    println!("BLOOM_HASHES  = {}", BLOOM_HASHES);
    println!();

    let fpa = fp(0xA0);

    // ── H1: msg_id uniqueness for sequential counters ─────────────
    let n_probes = 100_000u64;
    let mut msg_ids: BTreeSet<u64> = BTreeSet::new();
    for ctr in 1..=n_probes {
        msg_ids.insert(origin_msg_id(fpa, ctr));
    }
    println!("H1 — msg_id uniqueness:");
    println!(
        "  produced {} msg_ids from {} unique counters",
        msg_ids.len(),
        n_probes
    );
    println!("  collisions: {}", n_probes - msg_ids.len() as u64);
    println!();

    // ── H2: bloom_bit_index image diversity ───────────────────────
    let mut bit_positions: HashMap<u64, u32> = HashMap::new();
    for ctr in 1..=n_probes {
        let m = origin_msg_id(fpa, ctr);
        for k in 0..BLOOM_HASHES as u64 {
            let bit = bloom_bit_index(m, k, BLOOM_BITS as u64);
            *bit_positions.entry(bit).or_insert(0) += 1;
        }
    }
    let total_inserts_attempted = (n_probes * BLOOM_HASHES as u64) as usize;
    let unique_positions = bit_positions.len();
    let max_collisions = bit_positions.values().max().copied().unwrap_or(0);
    let mean_collisions = total_inserts_attempted as f64 / unique_positions as f64;
    println!("H2 — bloom_bit_index image:");
    println!(
        "  total bit-positions attempted: {}",
        total_inserts_attempted
    );
    println!("  unique positions hit:          {}", unique_positions);
    println!(
        "  expected uniform (n*k/m):      ~{}",
        total_inserts_attempted.min(BLOOM_BITS)
    );
    println!("  mean collisions per bit:       {:.2}", mean_collisions);
    println!("  max collisions on one bit:     {}", max_collisions);
    println!();

    // ── H3: simulate Bloom saturation feedback ────────────────────
    println!("H3 — Bloom saturation simulation (insert-only, no rejection):");
    let mut bloom = vec![0u64; BLOOM_WORDS].into_boxed_slice();
    let mut last_density_pct = -1i32;
    let mut frozen_at: Option<u64> = None;
    let _ = frozen_at; // for realistic-feedback variant below
    for ctr in 1..=n_probes {
        let m = origin_msg_id(fpa, ctr);
        bloom_insert(&mut bloom, m, BLOOM_HASHES);
        if ctr.is_power_of_two() || ctr % 10_000 == 0 {
            let set: usize = bloom.iter().map(|w| w.count_ones() as usize).sum();
            let pct = (set as f64 * 100.0 / BLOOM_BITS as f64) as i32;
            if pct != last_density_pct {
                println!("  ctr={:>6}: bits set = {} ({}%)", ctr, set, pct);
                last_density_pct = pct;
            }
        }
    }

    // ── H4: simulate REJECTION feedback ───────────────────────────
    // Mirrors the actual harness logic: only insert if NOT seen.
    println!();
    println!("H4 — Bloom WITH realistic rejection feedback:");
    let mut bloom2 = vec![0u64; BLOOM_WORDS].into_boxed_slice();
    let mut accepted = 0u64;
    let mut rejected = 0u64;
    let mut last_status_at = 0u64;
    for ctr in 1..=n_probes {
        let m = origin_msg_id(fpa, ctr);
        if bloom_contains(&bloom2, m, BLOOM_HASHES) {
            rejected += 1;
        } else {
            accepted += 1;
            bloom_insert(&mut bloom2, m, BLOOM_HASHES);
        }
        if ctr - last_status_at >= 5_000 {
            let set: usize = bloom2.iter().map(|w| w.count_ones() as usize).sum();
            println!(
                "  ctr={:>6}: accepted={} rejected={} bits_set={} ({:.4}%)",
                ctr,
                accepted,
                rejected,
                set,
                set as f64 * 100.0 / BLOOM_BITS as f64
            );
            last_status_at = ctr;
        }
    }
    println!();
    println!("  Final: accepted={} rejected={}", accepted, rejected);
    println!(
        "  Acceptance ratio: {:.6}",
        accepted as f64 / n_probes as f64
    );
    if accepted < 50_000 && rejected > 50_000 {
        println!("  ⚠️  Bloom froze well below theoretical capacity.");
    } else {
        println!("  Bloom behaved as theory predicts.");
    }
}
