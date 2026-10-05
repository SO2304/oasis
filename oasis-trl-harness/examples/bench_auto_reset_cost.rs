//! AD1 — microbenchmark: per-call cost of MeshRouter::remember() in
//! 5 modes. Validates AC3 prediction: "negligible per-call cost
//! increase" from auto-reset.
//!
//! Uses UNSIGNED v8 router so we measure ONLY the Bloom + dedup layer
//! cost (signed v0A would be dominated by Ed25519 sign at ~250 µs/op).
//! K=10 trials, banded medians, per-call cost in nanoseconds.

use oasis_rt::mesh::MeshRouter;
use std::time::Instant;

const N_PER_TRIAL: u64 = 1_000_000;
const K_TRIALS: usize = 11; // odd so median is well-defined
const WARMUP: usize = 2; // discard first 2 trials

fn fp(b: u8) -> [u8; 8] {
    let mut f = [0u8; 8];
    f[0] = b;
    f
}

fn build_router(threshold: Option<u64>) -> MeshRouter {
    let mut r = MeshRouter::new(fp(0xA0)); // unsigned v8 — pure dedup path
    r.set_bloom_auto_reset_threshold(threshold);
    r
}

fn time_one_trial(threshold: Option<u64>) -> u128 {
    let mut router = build_router(threshold);
    let start = Instant::now();
    for _ in 0..N_PER_TRIAL {
        let _ = router.origin_wrap(b"x");
    }
    let elapsed_ns = start.elapsed().as_nanos();
    elapsed_ns / N_PER_TRIAL as u128
}

fn run_k(threshold: Option<u64>, label: &str) -> u128 {
    let mut samples: Vec<u128> = (0..K_TRIALS).map(|_| time_one_trial(threshold)).collect();
    // Discard WARMUP slowest samples (cold cache, first-iteration jit, etc.).
    samples.sort();
    let trimmed: Vec<u128> = samples.iter().take(K_TRIALS - WARMUP).cloned().collect();
    let n = trimmed.len();
    let median = trimmed[n / 2];
    let min = trimmed[0];
    let max = trimmed[n - 1];
    let half_spread = (max - min) / 2;
    let pct = if median > 0 {
        half_spread * 100 / median
    } else {
        0
    };
    println!(
        "  {:<36} median={:>4} ns/call  range=[{}, {}]  ±{}%",
        label, median, min, max, pct
    );
    median
}

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  AD1 — auto-reset per-call cost (unsigned v8 hot path)           ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    println!(
        "  K=10 trials × {} origin_wrap iterations per trial.",
        N_PER_TRIAL
    );
    println!();

    let m_off = run_k(None, "auto-reset DISABLED (legacy):");
    let m_never = run_k(Some(N_PER_TRIAL * 2), "auto-reset CONFIG, never fires:");
    let m_freq = run_k(Some(40_000), "auto-reset @ 40k (~5 fires):");
    let m_aggr = run_k(Some(2_000), "auto-reset @ 2k  (~100 fires):");
    let m_ext = run_k(Some(100), "auto-reset @ 100 (~2000 fires):");

    println!();
    println!("──────────────────────────────────────────────────────────────────");
    println!(" AD1 verdict (relative to disabled baseline)");
    println!("──────────────────────────────────────────────────────────────────");
    let pct_never = (m_never as i128 - m_off as i128) * 100 / m_off as i128;
    let pct_freq = (m_freq as i128 - m_off as i128) * 100 / m_off as i128;
    let pct_aggr = (m_aggr as i128 - m_off as i128) * 100 / m_off as i128;
    let pct_ext = (m_ext as i128 - m_off as i128) * 100 / m_off as i128;
    println!("    config-only overhead:        {:+}%", pct_never);
    println!("    @ 40k (~5 resets in 200k):   {:+}%", pct_freq);
    println!("    @ 2k  (~100 resets):         {:+}%", pct_aggr);
    println!("    @ 100 (~2000 resets):        {:+}%", pct_ext);
    println!();
    if pct_never.abs() < 5 {
        println!("    [PASS-1] config-only overhead < 5% (one extra branch per call)");
    } else {
        println!(
            "    [WARN]   config-only overhead {:+}% — investigate",
            pct_never
        );
    }
    if pct_freq < 10 {
        println!("    [PASS-2] @ 40k threshold cost < 10% — AC3 \"negligible\" CONFIRMED");
    } else {
        println!(
            "    [FAIL]   @ 40k threshold cost {:+}% — AC3 prediction FALSIFIED",
            pct_freq
        );
    }
    println!();
    println!("  bench complete.");
}
