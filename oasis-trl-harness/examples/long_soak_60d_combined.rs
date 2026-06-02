//! AF4 — 60-day soak combining auto-reset (AC1) + dashboard (AE).
//!
//! The combined production pattern:
//!   - auto-reset @ 40 000 inserts handles routine cycle cleanup
//!     (threshold 40k = 76.9% of 52k capacity, below dashboard's 80% alert)
//!   - dashboard polls bloom_health_snapshot() hourly and treats
//!     capacity_alert() as an anomaly signal
//!
//! Predictions:
//!   AF4-a: ZERO capacity alerts across 60 virtual days (because
//!          auto-reset fires before the dashboard would have seen 80%).
//!   AF4-b: ~120 auto-resets fired (matching AD theory: 86 400 × 0.924 × 60 / 40 001 ≈ 120).
//!   AF4-c: drift over 60 days < 5% (validates AF3 prediction at production threshold).
//!   AF4-d: wall-clock ~28 min (= 60/30 × 14 min = 28; AG-errata
//!          fixes a "30/30 × 14 × 2" typo here — same answer 28).
//!
//! Also exercises AF2 (serialize_topic_v1): the dashboard receives
//! the serialized 36-byte record (round-tripped via deserialize) to
//! prove the wire format works end-to-end.

use std::time::Instant;

use oasis_rt::mesh::{
    bloom_capacity_estimate_1pct_fpr, mesh_v10_pubkey_from_seed, BloomHealthSnapshot, MeshDecision,
    MeshEdSeed, MeshPubRegistry, MeshRouter,
};
use oasis_rt::vec::{vz, V};
use oasis_rt::world_model::{WorldModel, ZoneType};
use oasis_trl_harness::*;

fn fp(b: u8) -> [u8; 8] {
    let mut f = [0u8; 8];
    f[0] = b;
    f
}
fn seed(b: u8) -> MeshEdSeed {
    MeshEdSeed([b; 32])
}

const TICKS_PER_VIRTUAL_HOUR: u64 = 3600;
const VIRTUAL_HOURS_TO_SOAK: u64 = 1440; // 60 days
const ADVERSARY_INTERVAL_TICKS: u64 = 30;
const AUTO_RESET_THRESHOLD: u64 = 40_000;

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  AF4 — 60-day combined deployment (auto-reset + dashboard)       ║");
    println!("║  Expected: ZERO dashboard alerts (auto-reset handles routine)    ║");
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
    let mut rng = Rng::new(20260512);
    let mut net = NetworkChannel::realistic_lora();
    let mut adversary = AdversaryAgent::new(fp(0xAA), ADVERSARY_INTERVAL_TICKS);
    let mut operator = OperatorSimulator::new();

    let total_ticks = TICKS_PER_VIRTUAL_HOUR * VIRTUAL_HOURS_TO_SOAK;
    let trial_start = Instant::now();

    // Checkpoints: day 1, 15, 30, 45, 60
    let checkpoint_hours: [u64; 5] = [24, 360, 720, 1080, 1440];
    let mut hour_1_throughput: u32 = 0;
    let mut hour_1440_throughput: u32 = 0;
    let mut dashboard_alerts: u64 = 0;
    let mut last_alert_hour: Option<u64> = None;
    let mut current_hour_arrived: u32 = 0;
    let mut current_virtual_hour: u64 = 1;
    // AF2 wire-format roundtrip — exercise serialize/deserialize each polling tick.
    let mut wire_roundtrips_ok: u64 = 0;
    let mut wire_roundtrips_failed: u64 = 0;

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
            // Dashboard polls hourly + AF2 wire roundtrip exercise
            let snap = router_b.bloom_health_snapshot();
            let wire = snap.serialize_topic_v1();
            let cap_est = snap.bloom_capacity_estimate_1pct_fpr;
            match BloomHealthSnapshot::deserialize_topic_v1(&wire, cap_est) {
                Some(restored) if restored == snap => wire_roundtrips_ok += 1,
                _ => wire_roundtrips_failed += 1,
            }
            if snap.capacity_alert() {
                dashboard_alerts += 1;
                if last_alert_hour.is_none() {
                    last_alert_hour = Some(current_virtual_hour);
                }
            }
            if current_virtual_hour == 1 {
                hour_1_throughput = current_hour_arrived;
            }
            if current_virtual_hour == VIRTUAL_HOURS_TO_SOAK {
                hour_1440_throughput = current_hour_arrived;
            }

            for &h in &checkpoint_hours {
                if current_virtual_hour == h {
                    let elapsed_min = trial_start.elapsed().as_secs() / 60;
                    println!("    hour {:>4}: env/h={:>4}, resets={:>4}, alerts={:>3}, cons_milli={:>4}, elapsed: {:>3} min",
                        h, current_hour_arrived,
                        router_b.bloom_reset_count(),
                        dashboard_alerts,
                        snap.capacity_consumed_milli(),
                        elapsed_min);
                }
            }
            current_hour_arrived = 0;
            current_virtual_hour += 1;
        }
    }

    let duration_ms = trial_start.elapsed().as_millis();
    let drift = (hour_1440_throughput as f64 - hour_1_throughput as f64) * 100.0
        / hour_1_throughput.max(1) as f64;
    let auto_resets = router_b.bloom_reset_count();
    let final_snap = router_b.bloom_health_snapshot();

    println!();
    println!("══════════════════════════════════════════════════════════════════");
    println!(" AF4 verdict");
    println!("══════════════════════════════════════════════════════════════════");
    println!(
        "    Total wall clock: {} ms ({} min)",
        duration_ms,
        duration_ms / 60_000
    );
    println!("    Dashboard alerts fired:  {}", dashboard_alerts);
    println!("    Auto-resets fired:       {}", auto_resets);
    println!("    AF2 wire roundtrips OK:  {}", wire_roundtrips_ok);
    println!("    AF2 wire roundtrips bad: {}", wire_roundtrips_failed);
    println!(
        "    Final capacity_consumed_milli: {}",
        final_snap.capacity_consumed_milli()
    );
    println!();
    println!("    hour    1:  {} env/h", hour_1_throughput);
    println!("    hour 1440:  {} env/h", hour_1440_throughput);
    println!("    drift over 60 days: {:+.2}%", drift);
    println!();
    println!(
        "    capacity estimate (build): {} inserts",
        bloom_capacity_estimate_1pct_fpr()
    );
    println!("    auto_reset threshold:      {}", AUTO_RESET_THRESHOLD);
    println!();

    let af4a_zero_alerts = dashboard_alerts == 0;
    let af4b_resets_in_band = auto_resets >= 100 && auto_resets <= 140;
    let af4c_drift_ok = drift.abs() < 5.0;
    let af4d_wall_ok = duration_ms / 60_000 >= 15 && duration_ms / 60_000 <= 50;
    let wire_ok = wire_roundtrips_failed == 0;

    println!(
        "  AF4-a (zero alerts):     {} (got {})",
        if af4a_zero_alerts { "✓" } else { "✗" },
        dashboard_alerts
    );
    println!(
        "  AF4-b (resets 100-140):  {} (got {})",
        if af4b_resets_in_band { "✓" } else { "✗" },
        auto_resets
    );
    println!(
        "  AF4-c (drift < 5%):      {} (got {:+.2}%)",
        if af4c_drift_ok { "✓" } else { "✗" },
        drift
    );
    println!(
        "  AF4-d (wall 15-50 min):  {} (got {} min)",
        if af4d_wall_ok { "✓" } else { "✗" },
        duration_ms / 60_000
    );
    println!(
        "  AF2  (wire roundtrips):  {} ({} ok / {} bad)",
        if wire_ok { "✓" } else { "✗" },
        wire_roundtrips_ok,
        wire_roundtrips_failed
    );
    println!();
    if af4a_zero_alerts && af4b_resets_in_band && af4c_drift_ok && af4d_wall_ok && wire_ok {
        println!("    [PASS] combined deployment validated at 60-day scale");
        println!("           auto-reset prevents dashboard alerts; production pattern proven");
        std::process::exit(0);
    } else {
        eprintln!("    [PARTIAL] check axes above");
        std::process::exit(1);
    }
}
