//! OASIS — Mesh routing throughput bench (K=10 banded).
//!
//! Measures:
//!   1. origin_wrap     — allocation + encode cost
//!   2. process (&[u8]) — receive-side parse + dedup + forward
//!   3. process_owned   — zero-extra-copy variant
//!   4. forward chain   — end-to-end latency through 8 hops
//!
//! Each measurement repeated K=10 times. Median ns/op + min-max range
//! + half-spread %. Added 2026-04-22 as part of the engineering-hygiene
//! round — no single-shot bench numbers in the repo.

use oasis_rt::mesh::{inner_slice, MeshDecision, MeshRouter, FP_LEN};
use std::time::Instant;

const K_REPEATS: usize = 10;

fn median_min_max(samples: &mut [f64]) -> (f64, f64, f64) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (samples[samples.len() / 2], samples[0], samples[samples.len() - 1])
}

fn report(label: &str, samples: &mut [f64], unit: &str, per_op_scale: f64) {
    let (median, min, max) = median_min_max(samples);
    let half = (max - min) / (2.0 * median) * 100.0;
    println!(
        "  {:<22} {:>7.2} {:<8} ({:>6.2}-{:>6.2}) ±{:>4.1}%  {:>10.0} {}/sec",
        label,
        median / per_op_scale,
        unit,
        min / per_op_scale,
        max / per_op_scale,
        half,
        1e9 / median,
        if unit.contains("µs/chain") { "chains" } else { "ops" }
    );
}

fn main() {
    const N: u32 = 100_000;
    println!("OASIS mesh bench — K={} repeats, N={} per repeat", K_REPEATS, N);
    println!();

    // 1. origin_wrap
    let mut samples: Vec<f64> = (0..K_REPEATS).map(|_| {
        let mut r = MeshRouter::new([1u8; FP_LEN]);
        let payload = vec![0u8; 500];
        let start = Instant::now();
        for _ in 0..N {
            let _env = r.origin_wrap(&payload);
        }
        start.elapsed().as_nanos() as f64 / N as f64
    }).collect();
    report("origin_wrap", &mut samples, "µs/op", 1000.0);

    // 2. process(&[u8])
    let mut samples: Vec<f64> = (0..K_REPEATS).map(|_| {
        let mut origin = MeshRouter::new([1u8; FP_LEN]);
        let payload = vec![0u8; 500];
        let env = origin.origin_wrap(&payload);
        let mut hop = MeshRouter::with_config([2u8; FP_LEN], 8, N as usize * 2);
        let start = Instant::now();
        for i in 0..N {
            let mut e = env.clone();
            e[6..14].copy_from_slice(&(i as u64).to_le_bytes());
            let _ = hop.process(&e);
        }
        start.elapsed().as_nanos() as f64 / N as f64
    }).collect();
    report("process(&[u8])", &mut samples, "µs/op", 1000.0);

    // 3. process_owned
    let mut samples: Vec<f64> = (0..K_REPEATS).map(|_| {
        let mut origin = MeshRouter::new([1u8; FP_LEN]);
        let payload = vec![0u8; 500];
        let env = origin.origin_wrap(&payload);
        let mut hop = MeshRouter::with_config([2u8; FP_LEN], 8, N as usize * 2);
        let start = Instant::now();
        for i in 0..N {
            let mut e = env.clone();
            e[6..14].copy_from_slice(&(i as u64).to_le_bytes());
            let _ = hop.process_owned(e);
        }
        start.elapsed().as_nanos() as f64 / N as f64
    }).collect();
    report("process_owned", &mut samples, "µs/op", 1000.0);

    // 4. 9-hop chain
    const CHAIN: u32 = 10_000;
    let mut samples: Vec<f64> = (0..K_REPEATS).map(|_| {
        let payload = vec![0xABu8; 500];
        let mut origin = MeshRouter::new([0u8; FP_LEN]);
        let mut hops: Vec<MeshRouter> = (1..=9u8)
            .map(|i| MeshRouter::new([i; FP_LEN]))
            .collect();
        let start = Instant::now();
        for i in 0..CHAIN {
            let mut env = origin.origin_wrap(&payload);
            env[6..14].copy_from_slice(&(i as u64).to_le_bytes());
            for h in hops.iter_mut() {
                match h.process_owned(env) {
                    MeshDecision::Arrived { envelope, .. } => { env = envelope; }
                    other => panic!("unexpected hop decision: {:?}", other),
                }
            }
            let _ = inner_slice(&env);
        }
        start.elapsed().as_nanos() as f64 / CHAIN as f64
    }).collect();
    report("9-hop chain", &mut samples, "µs/chain", 1000.0);

    println!();
    println!("K={} repeats per measurement. Numbers are median + (min-max).", K_REPEATS);
}
