//! AE-demo — operator dashboard that detects FPR drift BEFORE
//! throughput impact, using BloomHealthSnapshot.
//!
//! Story:
//!   - 3-day soak with auto-reset DELIBERATELY DISABLED to demonstrate
//!     the dashboard's alerting capability against a known failure mode.
//!   - Every virtual hour, dashboard polls bloom_health_snapshot().
//!   - When capacity_alert() (>= 80% of 1%-FPR capacity) fires, the
//!     dashboard issues an action recommendation BEFORE throughput
//!     degrades (the AB-audit baseline didn't go critical until ~100%).
//!
//! Predictions:
//!   AE-dash-1: alert fires by hour ~12 (52 000 × 0.8 / 3 320 ≈ 12.5)
//!   AE-dash-2: at the alert moment, throughput is still ~3 320 env/h
//!              (FPR < 1%), proving the alert is EARLY not REACTIVE.
//!   AE-dash-3: when the operator (this demo) applies bloom_reset()
//!              in response, the next hour's throughput stays flat.

use std::time::Instant;

use oasis_rt::mesh::{
    MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter,
    mesh_v10_pubkey_from_seed, BloomHealthSnapshot,
};
use oasis_rt::vec::{V, vz};
use oasis_rt::world_model::{WorldModel, ZoneType};
use oasis_trl_harness::*;

fn fp(b: u8) -> [u8; 8] { let mut f = [0u8; 8]; f[0] = b; f }
fn seed(b: u8) -> MeshEdSeed { MeshEdSeed([b; 32]) }

const TICKS_PER_VIRTUAL_HOUR: u64 = 3600;
const VIRTUAL_HOURS_TO_SOAK: u64 = 72;        // 3 days
const ADVERSARY_INTERVAL_TICKS: u64 = 30;

/// Dashboard's decision after polling a router's snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DashboardAction {
    /// Normal — no action.
    Ok,
    /// Capacity consumed >= 80% — recommend immediate reset.
    AlertResetNow,
    /// Already past 100% — too late, throughput will be impacted.
    Critical,
}

fn dashboard_assess(snap: &BloomHealthSnapshot) -> DashboardAction {
    let consumed = snap.capacity_consumed_milli();
    if consumed >= 1000 { DashboardAction::Critical }
    else if consumed >= 800 { DashboardAction::AlertResetNow }
    else { DashboardAction::Ok }
}

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  AE-demo — operator dashboard (detect drift BEFORE impact)       ║");
    println!("║  3-day soak, NO auto-reset; dashboard polls hourly snapshots     ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    let pk_a = mesh_v10_pubkey_from_seed(&seed(0xA0)).unwrap();
    let pk_b = mesh_v10_pubkey_from_seed(&seed(0xB0)).unwrap();
    let mut full_reg = MeshPubRegistry::new();
    full_reg.insert(fp(0xA0), pk_a);
    full_reg.insert(fp(0xB0), pk_b);

    let mut router_a = MeshRouter::new_ed25519_signed(fp(0xA0), seed(0xA0), full_reg.clone());
    let mut router_b = MeshRouter::new_ed25519_signed(fp(0xB0), seed(0xB0), full_reg.clone());
    // DELIBERATELY no set_bloom_auto_reset_threshold — we want the
    // dashboard to issue manual reset recommendations.

    let mut world_a = WorldModel::new();
    let mut world_b = WorldModel::new();
    let mut goal: V = vz(); goal[0] = 10.0; goal[1] = 10.0;
    world_a.try_add_zone(ZoneType::Attractive, goal, 5.0, 8.0).expect("setup");
    world_b.try_add_zone(ZoneType::Attractive, goal, 5.0, 8.0).expect("setup");

    let mut sensor_a = SensorNoiseModel::new(10.0);
    let mut sensor_b = SensorNoiseModel::new(10.0);
    let mut rng = Rng::new(20260512);
    let mut net = NetworkChannel::realistic_lora();
    let mut adversary = AdversaryAgent::new(fp(0xAA), ADVERSARY_INTERVAL_TICKS);
    let mut operator = OperatorSimulator::new();

    let total_ticks = TICKS_PER_VIRTUAL_HOUR * VIRTUAL_HOURS_TO_SOAK;
    let mut alert_hour: Option<u64> = None;
    let mut alert_throughput: u32 = 0;
    let mut current_hour_arrived: u32 = 0;
    let mut current_virtual_hour: u64 = 1;
    let mut dashboard_resets: u64 = 0;

    println!("  {:>4}  {:>7}  {:>10}  {:>5}  {:>14}", "hour", "env/h", "cons_milli", "alert", "dash_action");

    let _t0 = Instant::now();
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
            // Hourly poll
            let snap = router_b.bloom_health_snapshot();
            let action = dashboard_assess(&snap);
            let alert_str = if snap.capacity_alert() { "yes" } else { "no" };
            let action_str = match action {
                DashboardAction::Ok => "ok",
                DashboardAction::AlertResetNow => {
                    if alert_hour.is_none() {
                        alert_hour = Some(current_virtual_hour);
                        alert_throughput = current_hour_arrived;
                    }
                    "ALERT—reset!"
                }
                DashboardAction::Critical => "CRITICAL",
            };
            println!("  {:>4}  {:>7}  {:>10}  {:>5}  {:>14}",
                current_virtual_hour, current_hour_arrived,
                snap.capacity_consumed_milli(),
                alert_str, action_str);

            // Operator acts on the alert: issue manual reset
            if matches!(action, DashboardAction::AlertResetNow | DashboardAction::Critical) {
                router_b.bloom_reset();
                dashboard_resets += 1;
            }

            current_hour_arrived = 0;
            current_virtual_hour += 1;
        }
    }

    println!();
    println!("══════════════════════════════════════════════════════════════════");
    println!(" AE-demo verdict");
    println!("══════════════════════════════════════════════════════════════════");
    println!("  Dashboard-driven resets issued: {}", dashboard_resets);

    match alert_hour {
        Some(h) => {
            println!("  First alert raised at hour {}", h);
            println!("  Throughput at alert moment: {} env/h", alert_throughput);
            // Sanity bounds. Expected: alert ~ hour 12-14, throughput ~3300+
            let early = h <= 20;
            let throughput_healthy = alert_throughput >= 3000;
            if early && throughput_healthy {
                println!("  [PASS-1] alert fired EARLY (hour {} ≤ 20)", h);
                println!("  [PASS-2] throughput at alert was HEALTHY ({} env/h ≥ 3000)", alert_throughput);
                println!("  [PASS-3] dashboard detected drift BEFORE throughput impact");
                println!();
                println!("  AE1 prediction CONFIRMED: snapshot-based dashboard");
                println!("  detects FPR drift well before it impacts throughput.");
                std::process::exit(0);
            } else {
                eprintln!("  [PARTIAL] alert={} hour, throughput={} env/h", h, alert_throughput);
                eprintln!("    early?           {}", if early { "✓" } else { "✗" });
                eprintln!("    healthy at alert? {}", if throughput_healthy { "✓" } else { "✗" });
                std::process::exit(1);
            }
        }
        None => {
            eprintln!("  [FAIL] no alert raised in 72h — should have triggered");
            eprintln!("  This means the FPR-1% capacity was never reached.");
            std::process::exit(1);
        }
    }
}
