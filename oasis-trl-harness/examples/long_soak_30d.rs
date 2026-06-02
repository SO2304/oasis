//! AD3 — 30-day compressed soak (2 592 000 ticks).
//!
//! Validates AC1 prediction: "~20 minutes wall-clock and ~64 resets
//! at 40k threshold". With auto-reset enabled, throughput should
//! stay flat across all 720 hours.
//!
//! Reports only key checkpoints (day 1, 7, 14, 21, 30) to keep
//! output small; full per-hour series would be 720 lines.

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
const VIRTUAL_HOURS_TO_SOAK: u64 = 720;          // 30 days
const ADVERSARY_INTERVAL_TICKS: u64 = 30;
const AUTO_RESET_THRESHOLD: u64 = 40_000;

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  AD3 — 30-day compressed soak (2 592 000 ticks)                  ║");
    println!("║  auto-reset @ {} inserts                                         ║", AUTO_RESET_THRESHOLD);
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    let pk_a = mesh_v10_pubkey_from_seed(&seed(0xA0)).unwrap();
    let pk_b = mesh_v10_pubkey_from_seed(&seed(0xB0)).unwrap();
    let mut full_reg = MeshPubRegistry::new();
    full_reg.insert(fp(0xA0), pk_a);
    full_reg.insert(fp(0xB0), pk_b);

    let mut router_a = MeshRouter::new_ed25519_signed(fp(0xA0), seed(0xA0), full_reg.clone());
    let mut router_b = MeshRouter::new_ed25519_signed(fp(0xB0), seed(0xB0), full_reg.clone());
    router_a.set_bloom_auto_reset_threshold(Some(AUTO_RESET_THRESHOLD));
    router_b.set_bloom_auto_reset_threshold(Some(AUTO_RESET_THRESHOLD));

    let mut world_a = WorldModel::new();
    let mut world_b = WorldModel::new();
    let mut goal: V = vz(); goal[0] = 10.0; goal[1] = 10.0;
    world_a.try_add_zone(ZoneType::Attractive, goal, 5.0, 8.0).expect("setup");
    world_b.try_add_zone(ZoneType::Attractive, goal, 5.0, 8.0).expect("setup");

    let mut sensor_a = SensorNoiseModel::new(10.0);
    let mut sensor_b = SensorNoiseModel::new(10.0);
    let mut rng = Rng::new(20260511);
    let mut net = NetworkChannel::realistic_lora();
    let mut adversary = AdversaryAgent::new(fp(0xAA), ADVERSARY_INTERVAL_TICKS);
    let mut operator = OperatorSimulator::new();

    let total_ticks = TICKS_PER_VIRTUAL_HOUR * VIRTUAL_HOURS_TO_SOAK;
    let trial_start = Instant::now();

    // Checkpoint hours: 1, 168 (1d), 336 (2w), 504 (3w), 720 (30d)
    let checkpoint_hours: [u64; 5] = [1, 168, 336, 504, 720];
    let mut throughputs: Vec<u32> = vec![0; checkpoint_hours.len()];
    let mut current_hour_arrived: u32 = 0;
    let mut current_virtual_hour: u64 = 1;
    let mut reset_count_history: Vec<u64> = Vec::new();

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
            for (i, &h) in checkpoint_hours.iter().enumerate() {
                if current_virtual_hour == h {
                    throughputs[i] = current_hour_arrived;
                    reset_count_history.push(router_b.bloom_reset_count());
                    let elapsed_min = trial_start.elapsed().as_secs() / 60;
                    println!("    hour {:>4}: env/h={:>4}, router_b resets={:>3} (elapsed: {} min)",
                        h, current_hour_arrived,
                        router_b.bloom_reset_count(), elapsed_min);
                }
            }
            current_hour_arrived = 0;
            current_virtual_hour += 1;
        }
    }

    let duration_ms = trial_start.elapsed().as_millis();
    println!();
    println!("    Total wall clock: {} ms ({} min)", duration_ms, duration_ms / 60_000);
    println!("    router_b final state:");
    println!("      bloom_resets:              {}", router_b.bloom_reset_count());
    println!("      bloom_inserts_since_reset: {}", router_b.bloom_inserts_since_reset());
    println!("      bloom_inserts (total):     {}", router_b.bloom_inserts());

    let hour_1 = throughputs[0];
    let hour_720 = throughputs[checkpoint_hours.len() - 1];
    let drift = (hour_720 as f64 - hour_1 as f64) * 100.0 / hour_1.max(1) as f64;

    println!();
    println!("══════════════════════════════════════════════════════════════════");
    println!(" AD3 verdict");
    println!("══════════════════════════════════════════════════════════════════");
    println!("    hour 1:   {} env/h", hour_1);
    println!("    hour 720: {} env/h", hour_720);
    println!("    drift over 30 days: {:+.2}%", drift);
    println!("    auto-resets fired: {} (predicted ~64 per AC1)", router_b.bloom_reset_count());

    let wall_min = duration_ms / 60_000;
    let drift_ok = drift.abs() < 5.0;
    let wall_ok = wall_min >= 10 && wall_min <= 40;       // AC1 said ~20 min
    let resets_ok = router_b.bloom_reset_count() >= 50
                 && router_b.bloom_reset_count() <= 80;
    println!();
    if drift_ok && wall_ok && resets_ok {
        println!("    [PASS] AC1 prediction CONFIRMED on all 3 axes:");
        println!("      - drift < 5%      ({:+.2}%)", drift);
        println!("      - wall ~20 min    ({} min)", wall_min);
        println!("      - resets 50-80    ({})", router_b.bloom_reset_count());
        std::process::exit(0);
    } else {
        eprintln!("    [PARTIAL] AC1 prediction needs honest re-statement:");
        eprintln!("      - drift < 5%:    {} ({:+.2}%)", if drift_ok { "✓" } else { "✗" }, drift);
        eprintln!("      - wall 10-40 min:{} ({} min)", if wall_ok { "✓" } else { "✗" }, wall_min);
        eprintln!("      - resets 50-80:  {} ({})", if resets_ok { "✓" } else { "✗" }, router_b.bloom_reset_count());
        std::process::exit(1);
    }
}
