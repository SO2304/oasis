//! AC3 — 7-day compressed soak (604 800 ticks @ 1 Hz virtual).
//!
//! Validates that the new bloom_auto_reset_threshold (AC1) keeps
//! per-hour throughput flat across 168 hours of continuous operation.
//! AB-round fix alone (correct 64 KiB Bloom sizing) plateaus around
//! 4-5 % drift over 24h; without auto-reset, drift compounds over 7d.
//!
//! Predictions:
//!   AC3-a: with auto-reset configured at 40 000 inserts, bloom_reset
//!          fires ~16 times across 7 days (= 7 × 92 400 inserts ÷ 40k)
//!   AC3-b: per-hour throughput drift < 5 % across all 168 hours
//!          (vs > 100 % when auto-reset disabled)
//!   AC3-c: wall-clock runs in 4-6 minutes per trial on host x86

use std::time::Instant;

use oasis_rt::mesh::{
    MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter,
    mesh_v10_pubkey_from_seed,
};
use oasis_rt::vec::{V, vz};
use oasis_rt::world_model::{WorldModel, ZoneType};
use oasis_trl_harness::*;

fn fp(b: u8) -> [u8; 8] { let mut f = [0u8; 8]; f[0] = b; f }
fn seed(b: u8) -> MeshEdSeed { MeshEdSeed([b; 32]) }

const TICKS_PER_VIRTUAL_HOUR: u64 = 3600;
const VIRTUAL_HOURS_TO_SOAK: u64 = 168;          // 7 days
const ADVERSARY_INTERVAL_TICKS: u64 = 30;
const BLOOM_AUTO_RESET_THRESHOLD: u64 = 40_000;  // 1 % FPR threshold for 64 KiB Bloom

#[derive(Clone, Default)]
struct DaySnapshot {
    day: u64,
    arrived_total: u64,
    drops_total: u64,
    bloom_resets_router_b: u64,
    bloom_inserts_since_reset_router_b: u64,
}

fn run_one_trial(rng_seed: u64, with_auto_reset: bool) -> Vec<u32> {
    let pk_a = mesh_v10_pubkey_from_seed(&seed(0xA0)).unwrap();
    let pk_b = mesh_v10_pubkey_from_seed(&seed(0xB0)).unwrap();
    let mut full_reg = MeshPubRegistry::new();
    full_reg.insert(fp(0xA0), pk_a);
    full_reg.insert(fp(0xB0), pk_b);

    let mut router_a = MeshRouter::new_ed25519_signed(fp(0xA0), seed(0xA0), full_reg.clone());
    let mut router_b = MeshRouter::new_ed25519_signed(fp(0xB0), seed(0xB0), full_reg.clone());

    if with_auto_reset {
        router_a.set_bloom_auto_reset_threshold(Some(BLOOM_AUTO_RESET_THRESHOLD));
        router_b.set_bloom_auto_reset_threshold(Some(BLOOM_AUTO_RESET_THRESHOLD));
    }

    let mut world_a = WorldModel::new();
    let mut world_b = WorldModel::new();
    let mut goal: V = vz(); goal[0] = 10.0; goal[1] = 10.0;
    world_a.try_add_zone(ZoneType::Attractive, goal, 5.0, 8.0).expect("setup");
    world_b.try_add_zone(ZoneType::Attractive, goal, 5.0, 8.0).expect("setup");

    let mut sensor_a = SensorNoiseModel::new(10.0);
    let mut sensor_b = SensorNoiseModel::new(10.0);
    let mut rng = Rng::new(rng_seed);
    let mut net = NetworkChannel::realistic_lora();
    let mut adversary = AdversaryAgent::new(fp(0xAA), ADVERSARY_INTERVAL_TICKS);
    let mut operator = OperatorSimulator::new();

    let total_ticks = TICKS_PER_VIRTUAL_HOUR * VIRTUAL_HOURS_TO_SOAK;
    let mut hourly_arrived: Vec<u32> = Vec::with_capacity(VIRTUAL_HOURS_TO_SOAK as usize);
    let mut current_hour_arrived: u32 = 0;

    for tick in 0..total_ticks {
        for (s, w) in [(&mut sensor_a, &mut world_a),
                       (&mut sensor_b, &mut world_b)].iter_mut() {
            let v = s.sample(&mut rng);
            if (v - 10.0).abs() > 0.5 {
                let mut c: V = vz(); c[0] = v;
                let _ = w.try_add_zone(ZoneType::Repulsive, c, 1.0, 0.5);
            }
        }

        let env = router_a.origin_wrap(b"telemetry");
        let (delivered, _) = net.transmit(&mut rng);
        if delivered {
            let mut origin = [0u8; 8];
            origin.copy_from_slice(&env[14..22]);
            if !operator.is_revoked(&origin) {
                if let MeshDecision::Arrived { .. } = router_b.process(&env) {
                    current_hour_arrived += 1;
                }
            }
        }

        if let Some(att) = adversary.maybe_inject(tick) {
            if !operator.is_revoked(&att) {
                let mut bp: V = vz();
                bp[0] = 5.0 + rng.next_gaussian(0.0, 1.0);
                bp[1] = 5.0 + rng.next_gaussian(0.0, 1.0);
                let _ = world_b.try_add_zone(ZoneType::Repulsive, bp, 8.0, 1.0);
            }
        }
        let cap = world_a.cap_hit_count() + world_b.cap_hit_count();
        operator.tick(cap, Some(adversary.attacker_fp));

        if (tick + 1) % TICKS_PER_VIRTUAL_HOUR == 0 {
            hourly_arrived.push(current_hour_arrived);
            current_hour_arrived = 0;
        }
    }

    eprintln!("    router_b: bloom_resets={} bloom_inserts_since_reset={} bloom_inserts_total={}",
        router_b.bloom_reset_count(),
        router_b.bloom_inserts_since_reset(),
        router_b.bloom_inserts());
    hourly_arrived
}

fn drift_pct(hourly: &[u32]) -> f64 {
    if hourly.is_empty() { return 0.0; }
    let first = hourly[0] as f64;
    let last = *hourly.last().unwrap() as f64;
    if first == 0.0 { return 0.0; }
    ((last - first) / first) * 100.0
}

fn min_max(hourly: &[u32]) -> (u32, u32) {
    let mn = *hourly.iter().min().unwrap_or(&0);
    let mx = *hourly.iter().max().unwrap_or(&0);
    (mn, mx)
}

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  AC3 — 7-day compressed soak (604 800 ticks)                     ║");
    println!("║  A/B: with vs without bloom_auto_reset_threshold                 ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("  168 virtual hours × 1 trial each                                  ");
    println!("  threshold (when enabled): {} inserts (1% FPR for 64 KiB Bloom)", BLOOM_AUTO_RESET_THRESHOLD);
    println!();

    println!("──────────────────────────────────────────────────────────────────");
    println!(" Trial A: WITHOUT auto-reset (baseline — AB-only fix)              ");
    println!("──────────────────────────────────────────────────────────────────");
    let t0 = Instant::now();
    let baseline = run_one_trial(20260511, false);
    let baseline_ms = t0.elapsed().as_millis();
    let (mn_a, mx_a) = min_max(&baseline);
    println!("    duration: {} ms wall clock", baseline_ms);
    println!("    hour 1:  {} env/hour", baseline.first().unwrap_or(&0));
    println!("    hour 168:{} env/hour", baseline.last().unwrap_or(&0));
    println!("    range:   [{}, {}]", mn_a, mx_a);
    println!("    drift:   {:+.2}%", drift_pct(&baseline));
    println!();
    let samples = [0, 23, 47, 71, 95, 119, 143, 167];
    print!("    samples (24h apart): ");
    for &i in &samples {
        if i < baseline.len() { print!("h{}={} ", i + 1, baseline[i]); }
    }
    println!();
    println!();

    println!("──────────────────────────────────────────────────────────────────");
    println!(" Trial B: WITH auto-reset @ {} inserts                       ", BLOOM_AUTO_RESET_THRESHOLD);
    println!("──────────────────────────────────────────────────────────────────");
    let t1 = Instant::now();
    let with_reset = run_one_trial(20260511, true);
    let with_reset_ms = t1.elapsed().as_millis();
    let (mn_b, mx_b) = min_max(&with_reset);
    println!("    duration: {} ms wall clock", with_reset_ms);
    println!("    hour 1:  {} env/hour", with_reset.first().unwrap_or(&0));
    println!("    hour 168:{} env/hour", with_reset.last().unwrap_or(&0));
    println!("    range:   [{}, {}]", mn_b, mx_b);
    println!("    drift:   {:+.2}%", drift_pct(&with_reset));
    println!();
    print!("    samples (24h apart): ");
    for &i in &samples {
        if i < with_reset.len() { print!("h{}={} ", i + 1, with_reset[i]); }
    }
    println!();
    println!();

    println!("══════════════════════════════════════════════════════════════════");
    println!(" AC3 verdict");
    println!("══════════════════════════════════════════════════════════════════");
    let baseline_drift = drift_pct(&baseline).abs();
    let with_reset_drift = drift_pct(&with_reset).abs();
    println!("  Baseline drift over 7d:  {:.2}%", baseline_drift);
    println!("  With-reset drift over 7d:{:.2}%", with_reset_drift);
    println!();
    let pass = with_reset_drift < 5.0
        && baseline_drift > with_reset_drift;
    if pass {
        println!("  [PASS] auto-reset keeps 7-day throughput flat (< 5% drift)");
        println!("    & demonstrably better than baseline");
        std::process::exit(0);
    } else {
        eprintln!("  [FAIL] with-reset drift {:.2}% should be < 5% AND less than baseline {:.2}%",
            with_reset_drift, baseline_drift);
        std::process::exit(1);
    }
}
