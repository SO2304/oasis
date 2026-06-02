//! OASIS — payload-size sweep for intra-process topic dispatch.
//!
//! Same workload as bench_full_stack's "topic + mesh wrap + dispatch"
//! pipeline but parameterized over payload size. Numbers feed the
//! A/B-vs-rclcpp audit's payload sweep.
//!
//! Two paths measured side-by-side:
//!   "OLD: wrap_topic + origin_wrap"  — 2 allocations, 2 payload copies
//!   "NEW: origin_wrap_with builder"  — 1 allocation, 1 payload copy
//!
//! Each measurement is repeated `K_REPEATS` times. The bench reports
//! median, min, and max ns/op so single-run noise is visible. Median
//! is the headline number; (max-min)/2 is the half-spread error bar.
//!
//! Added 2026-04-22 in response to the prior audit's request for
//! statistical bands so callers can distinguish "real difference"
//! from "measurement noise."

use oasis_rt::{topics, mesh};
use std::time::Instant;

const K_REPEATS: usize = 10;

fn run_old(payload_size: usize, n: u32) -> f64 {
    let mut router = topics::TopicRouter::new();
    fn noop(_h: u64, _p: &[u8]) {}
    router.subscribe("/oasis_ab", noop);
    let mut mesh_router = mesh::MeshRouter::new([1u8; 8]);
    let payload: Vec<u8> = (0..payload_size).map(|i| (i & 0xFF) as u8).collect();

    for _ in 0..(n / 10).max(1) {
        let topic_env = topics::wrap_topic("/oasis_ab", &payload);
        let _ = mesh_router.origin_wrap(&topic_env);
        router.dispatch(&topic_env).unwrap();
    }
    let start = Instant::now();
    for _ in 0..n {
        let topic_env = topics::wrap_topic("/oasis_ab", &payload);
        let _ = mesh_router.origin_wrap(&topic_env);
        router.dispatch(&topic_env).unwrap();
    }
    start.elapsed().as_nanos() as f64 / n as f64
}

fn run_new(payload_size: usize, n: u32) -> f64 {
    let mut router = topics::TopicRouter::new();
    fn noop(_h: u64, _p: &[u8]) {}
    router.subscribe("/oasis_ab", noop);
    let mut mesh_router = mesh::MeshRouter::new([1u8; 8]);
    let topic_hash = topics::hash_topic("/oasis_ab");
    let payload: Vec<u8> = (0..payload_size).map(|i| (i & 0xFF) as u8).collect();

    for _ in 0..(n / 10).max(1) {
        let inner_len = topics::TOPIC_HEADER_LEN + payload.len();
        let mesh_env = mesh_router.origin_wrap_with(inner_len, |buf| {
            topics::write_topic_envelope_into(buf, topic_hash, &payload);
        }).unwrap();
        let inner = mesh::inner_slice(&mesh_env);
        router.dispatch(inner).unwrap();
    }
    let start = Instant::now();
    for _ in 0..n {
        let inner_len = topics::TOPIC_HEADER_LEN + payload.len();
        let mesh_env = mesh_router.origin_wrap_with(inner_len, |buf| {
            topics::write_topic_envelope_into(buf, topic_hash, &payload);
        }).unwrap();
        let inner = mesh::inner_slice(&mesh_env);
        router.dispatch(inner).unwrap();
    }
    start.elapsed().as_nanos() as f64 / n as f64
}

#[derive(Debug)]
struct Stat {
    median: f64,
    min: f64,
    max: f64,
    half_spread_pct: f64,
}

fn stats(samples: &mut [f64]) -> Stat {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = samples[samples.len() / 2];
    let min = samples[0];
    let max = samples[samples.len() - 1];
    let half_spread_pct = (max - min) / (2.0 * median) * 100.0;
    Stat { median, min, max, half_spread_pct }
}

fn measure<F: FnMut(usize, u32) -> f64>(name: &str, payload: usize, n: u32, mut f: F) -> Stat {
    let mut samples = Vec::with_capacity(K_REPEATS);
    for _ in 0..K_REPEATS {
        samples.push(f(payload, n));
    }
    let s = stats(&mut samples);
    let _ = name;
    s
}

fn main() {
    println!("OASIS payload sweep, K={} repeats per measurement", K_REPEATS);
    println!("(median ns/op ± half-spread; smaller spread = more reliable)\n");
    println!("  {:>10}  {:>20}  {:>20}  {:>10}",
             "payload", "OLD median (min-max)", "NEW median (min-max)", "speedup");

    for &(name, size, n) in &[
        ("16 B",     16,         100_000u32),
        ("1 KB",     1024,        50_000u32),
        ("64 KB",    64 * 1024,    5_000u32),
        ("1 MB",     1024 * 1024,    500u32),
    ] {
        let old = measure("OLD", size, n, run_old);
        let new = measure("NEW", size, n, run_new);
        let speedup = old.median / new.median;
        // Worst-case (min OLD / max NEW) and best-case (max OLD / min NEW).
        let worst_speedup = old.min / new.max;
        let best_speedup  = old.max / new.min;
        println!("  {:>10}  {:>9.0} ({:>4.0}-{:>4.0})  {:>9.0} ({:>4.0}-{:>4.0})  {:>5.2}x [{:>4.2}-{:>4.2}]",
                 name,
                 old.median, old.min, old.max,
                 new.median, new.min, new.max,
                 speedup, worst_speedup, best_speedup);
    }

    // Spread report — how noisy is the bench itself?
    println!("\nNoise check (NEW path half-spread % of median):");
    for &(name, size, n) in &[
        ("16 B",     16,         100_000u32),
        ("1 KB",     1024,        50_000u32),
        ("64 KB",    64 * 1024,    5_000u32),
        ("1 MB",     1024 * 1024,    500u32),
    ] {
        let new = measure("NEW", size, n, run_new);
        println!("  {:>10}  median={:>9.0} ns  half-spread=±{:>5.1}%",
                 name, new.median, new.half_spread_pct);
    }
}
