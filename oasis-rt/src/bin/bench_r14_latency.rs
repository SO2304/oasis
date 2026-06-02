//! OASIS-RT — R14 gate latency benchmark.
//!
//! Measures: from fault injection (entropy spike) → R14 gate fires (action blocked).
//! Output: CSV to stdout with columns fault_id, latency_ns, detected.
//!
//! Comparable metric for PX4: failsafe trigger latency from sensor fault.
//! PX4 typical: 50-200 ms from dropout to EKF2 detection to failsafe → ~100ms median.
//! OASIS claim to test: can R14 detect within ONE tick (~32 ms frame period).
//!
//! Usage:
//!   cargo run --release --bin bench_r14_latency > r14_latency.csv
//!   awk -F, 'NR>1 {sum+=$2; n++} END{print "mean_latency_us=", sum/n/1000}' r14_latency.csv

use oasis_rt::hyper_state::*;
use oasis_rt::vitality::{Vitality, VitalityState};
use std::time::Instant;

fn main() {
    println!("fault_id,latency_ns,signal,threshold,blocked");

    let mut rng_seed = 12345u64;
    let mut simple_rand = || {
        rng_seed ^= rng_seed << 13;
        rng_seed ^= rng_seed >> 7;
        rng_seed ^= rng_seed << 17;
        (rng_seed as f64 / u64::MAX as f64).abs()
    };

    const N_FAULTS: usize = 1000;
    let threshold = 0.95;
    let mut all_latencies: Vec<u128> = Vec::with_capacity(N_FAULTS);
    let mut blocked_count = 0usize;

    for fault_id in 0..N_FAULTS {
        // Fresh agent (state anchor cycles through 0..9 — AN has 9 rows)
        let mut agent = agent_new(fault_id % 9);
        let mut vit = VitalityState::new();

        // 10 benign ticks
        for _ in 0..10 {
            inject_sensory(&mut agent, 0.1 + 0.05 * simple_rand());
        }

        // Fault: high-novelty sample that should push entropy over threshold
        let fault_magnitude = 0.85 + 0.15 * simple_rand();

        // Measure: time from injection to R14 decision.
        // Use the COMBINED signal (entropy + vitality_contribution) per drone_bridge logic.
        let t0 = Instant::now();
        inject_sensory(&mut agent, fault_magnitude);
        vit.update(&[(false, Vitality::Important)]); // sonar fault
        let signal = (agent.entropy + vit.entropy_contribution).min(1.0);
        let blocked = signal > threshold;
        let latency = t0.elapsed().as_nanos();

        println!("{},{},{:.4},{:.4},{}", fault_id, latency, signal, threshold, blocked);
        all_latencies.push(latency);
        if blocked {
            blocked_count += 1;
        }
    }

    // Summary stats over all N_FAULTS samples. Emitted as comments so CSV
    // parsers ignore them. Part of the hygiene round — no single-number
    // claims without min/median/max.
    all_latencies.sort_unstable();
    let n = all_latencies.len();
    let min = all_latencies[0];
    let p50 = all_latencies[n / 2];
    let p95 = all_latencies[(n * 95) / 100];
    let p99 = all_latencies[(n * 99) / 100];
    let max = all_latencies[n - 1];
    let mean: f64 = all_latencies.iter().sum::<u128>() as f64 / n as f64;
    eprintln!();
    eprintln!("# R14 gate latency summary (N={}):", n);
    eprintln!("#   min  = {} ns", min);
    eprintln!("#   mean = {:.0} ns", mean);
    eprintln!("#   p50  = {} ns", p50);
    eprintln!("#   p95  = {} ns", p95);
    eprintln!("#   p99  = {} ns", p99);
    eprintln!("#   max  = {} ns", max);
    eprintln!("#   blocked (signal > threshold): {} / {} ({:.1}%)", blocked_count, n, 100.0 * blocked_count as f64 / n as f64);
}
