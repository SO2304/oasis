//! AD2 — auto-reset threshold sweep at 7-day scale.
//! Validates AC3 prediction: "threshold can be safely lowered to
//! 20 000 inserts" with negligible cost. Sweeps 5k → 80k.
//!
//! For each threshold, runs a single 7-day trial (604 800 ticks)
//! and reports per-hour drift + reset count + sample throughput.

use oasis_rt::mesh::{
    mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter,
};
use oasis_rt::vec::{vz, V};
use oasis_rt::world_model::{WorldModel, ZoneType};
use oasis_trl_harness::*;
use std::time::Instant;

fn fp(b: u8) -> [u8; 8] {
    let mut f = [0u8; 8];
    f[0] = b;
    f
}
fn seed(b: u8) -> MeshEdSeed {
    MeshEdSeed([b; 32])
}

const TICKS_PER_VIRTUAL_HOUR: u64 = 3600;
const VIRTUAL_HOURS_TO_SOAK: u64 = 168; // 7 days
const ADVERSARY_INTERVAL_TICKS: u64 = 30;

struct SweepResult {
    threshold: u64,
    hour_1_throughput: u32,
    hour_168_throughput: u32,
    drift_pct: f64,
    bloom_resets: u64,
    bloom_inserts_since_reset_final: u64,
    duration_ms: u128,
}

fn run_threshold_trial(threshold: u64) -> SweepResult {
    let pk_a = mesh_v10_pubkey_from_seed(&seed(0xA0)).unwrap();
    let pk_b = mesh_v10_pubkey_from_seed(&seed(0xB0)).unwrap();
    let mut full_reg = MeshPubRegistry::new();
    full_reg.insert(fp(0xA0), pk_a);
    full_reg.insert(fp(0xB0), pk_b);

    let mut router_a = MeshRouter::new_ed25519_signed(fp(0xA0), seed(0xA0), full_reg.clone());
    let mut router_b = MeshRouter::new_ed25519_signed(fp(0xB0), seed(0xB0), full_reg.clone());

    router_a.set_bloom_auto_reset_threshold(Some(threshold));
    router_b.set_bloom_auto_reset_threshold(Some(threshold));

    let mut world_a = WorldModel::new();
    let mut world_b = WorldModel::new();
    let mut goal: V = vz();
    goal[0] = 10.0;
    goal[1] = 10.0;
    world_a
        .try_add_zone(ZoneType::Attractive, goal, 5.0, 8.0)
        .expect("setup");
    world_b
        .try_add_zone(ZoneType::Attractive, goal, 5.0, 8.0)
        .expect("setup");

    let mut sensor_a = SensorNoiseModel::new(10.0);
    let mut sensor_b = SensorNoiseModel::new(10.0);
    let mut rng = Rng::new(20260511);
    let mut net = NetworkChannel::realistic_lora();
    let mut adversary = AdversaryAgent::new(fp(0xAA), ADVERSARY_INTERVAL_TICKS);
    let mut operator = OperatorSimulator::new();

    let total_ticks = TICKS_PER_VIRTUAL_HOUR * VIRTUAL_HOURS_TO_SOAK;
    let mut hour_1_throughput: u32 = 0;
    let mut hour_168_throughput: u32 = 0;
    let mut current_hour_arrived: u32 = 0;
    let mut current_virtual_hour: u64 = 1;

    let trial_start = Instant::now();
    for tick in 0..total_ticks {
        for (s, w) in [(&mut sensor_a, &mut world_a), (&mut sensor_b, &mut world_b)].iter_mut() {
            let v = s.sample(&mut rng);
            if (v - 10.0).abs() > 0.5 {
                let mut c: V = vz();
                c[0] = v;
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
            if current_virtual_hour == 1 {
                hour_1_throughput = current_hour_arrived;
            }
            if current_virtual_hour == VIRTUAL_HOURS_TO_SOAK {
                hour_168_throughput = current_hour_arrived;
            }
            current_hour_arrived = 0;
            current_virtual_hour += 1;
        }
    }

    let drift_pct = if hour_1_throughput > 0 {
        (hour_168_throughput as f64 - hour_1_throughput as f64) * 100.0 / hour_1_throughput as f64
    } else {
        0.0
    };

    SweepResult {
        threshold,
        hour_1_throughput,
        hour_168_throughput,
        drift_pct,
        bloom_resets: router_b.bloom_reset_count(),
        bloom_inserts_since_reset_final: router_b.bloom_inserts_since_reset(),
        duration_ms: trial_start.elapsed().as_millis(),
    }
}

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  AD2 — threshold sweep at 7-day scale (604 800 ticks per cell)  ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    let thresholds: [u64; 5] = [5_000, 10_000, 20_000, 40_000, 80_000];
    let mut results: Vec<SweepResult> = Vec::new();
    for &t in &thresholds {
        println!("──────────────────────────────────────────────────────────────────");
        println!(" Threshold = {} inserts", t);
        let r = run_threshold_trial(t);
        println!("    duration: {} ms", r.duration_ms);
        println!("    hour 1:     {} env/hour", r.hour_1_throughput);
        println!("    hour 168:   {} env/hour", r.hour_168_throughput);
        println!("    drift:      {:+.2}%", r.drift_pct);
        println!("    resets:     {}", r.bloom_resets);
        println!(
            "    inserts_since_last_reset: {}",
            r.bloom_inserts_since_reset_final
        );
        println!();
        results.push(r);
    }

    println!("══════════════════════════════════════════════════════════════════");
    println!(" Summary — threshold sweep results");
    println!("══════════════════════════════════════════════════════════════════");
    println!(
        "  {:>10} | {:>10} | {:>10} | {:>9} | {:>7} | {:>10}",
        "threshold", "hour 1", "hour 168", "drift%", "resets", "duration"
    );
    println!("  {}", "-".repeat(70));
    for r in &results {
        println!(
            "  {:>10} | {:>10} | {:>10} | {:>+8.2}% | {:>7} | {:>7} ms",
            r.threshold,
            r.hour_1_throughput,
            r.hour_168_throughput,
            r.drift_pct,
            r.bloom_resets,
            r.duration_ms
        );
    }

    println!();
    println!("──────────────────────────────────────────────────────────────────");
    println!(" AD2 verdict");
    println!("──────────────────────────────────────────────────────────────────");
    let all_under_5pct = results.iter().all(|r| r.drift_pct.abs() < 5.0);
    let lowest_threshold_safe = results
        .iter()
        .filter(|r| r.drift_pct.abs() < 5.0)
        .map(|r| r.threshold)
        .min();
    if all_under_5pct {
        println!("  [PASS] all thresholds 5k-80k keep 7-day drift < 5%");
        if let Some(min_t) = lowest_threshold_safe {
            println!("    safe threshold floor (this sweep): {} inserts", min_t);
            println!("    AC3 prediction (\"20k still works\"): CONFIRMED");
        }
        std::process::exit(0);
    } else {
        eprintln!("  [PARTIAL] some thresholds exceed 5% drift");
        if let Some(min_t) = lowest_threshold_safe {
            eprintln!("    safe floor: {} inserts", min_t);
        } else {
            eprintln!("    no threshold met the < 5% bar");
        }
        std::process::exit(1);
    }
}
