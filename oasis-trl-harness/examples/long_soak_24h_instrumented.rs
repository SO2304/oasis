//! AA1 round — instrument long_soak_24h to identify the dominant
//! Drop reason responsible for the per-hour throughput regression
//! discovered in Z4 (3 350 env/hour at hour 1 → 0 by hour 12).
//!
//! Predictions for this round:
//!   AA1: dominant Drop reason will be "duplicate" (Bloom dedup
//!        false positive saturation) at hours 12+
//!   AA2: fix is one of (a) bloom_reset() periodically,
//!        (b) tx_counter persistence, (c) larger Bloom
//!   AA4: fix is < 50 LOC in oasis-rt::mesh
//!
//! This bench runs ONE seed for the full 86 400 ticks but tags every
//! Drop reason and reports per-hour buckets. That's enough to falsify
//! AA1 if e.g. the dominant reason turns out to be unknown sender or
//! parse error rather than dedup.

use std::collections::BTreeMap;
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
const VIRTUAL_HOURS_TO_SOAK: u64 = 24;
const ADVERSARY_INTERVAL_TICKS: u64 = 30;

#[derive(Default, Clone)]
struct DropReasonHistogram {
    by_reason: BTreeMap<&'static str, u32>,
}

impl DropReasonHistogram {
    fn record(&mut self, reason: &'static str) {
        *self.by_reason.entry(reason).or_insert(0) += 1;
    }
    fn total(&self) -> u32 { self.by_reason.values().sum() }
    fn dominant(&self) -> Option<(&'static str, u32)> {
        self.by_reason.iter().max_by_key(|(_, v)| *v).map(|(k, v)| (*k, *v))
    }
}

#[derive(Default)]
struct HourBucket {
    virtual_hour: u64,
    arrived: u32,
    drops: DropReasonHistogram,
    bloom_inserts_router_b: u64,
    tx_counter_router_a: u64,
}

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  AA1 — instrumented 24h soak: Drop-reason histogram per hour     ║");
    println!("║  Goal: identify root cause of throughput regression              ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    let pk_a = mesh_v10_pubkey_from_seed(&seed(0xA0)).unwrap();
    let pk_b = mesh_v10_pubkey_from_seed(&seed(0xB0)).unwrap();
    let pk_c = mesh_v10_pubkey_from_seed(&seed(0xC0)).unwrap();
    let mut full_reg = MeshPubRegistry::new();
    full_reg.insert(fp(0xA0), pk_a);
    full_reg.insert(fp(0xB0), pk_b);
    full_reg.insert(fp(0xC0), pk_c);

    let mut router_a = MeshRouter::new_ed25519_signed(fp(0xA0), seed(0xA0), full_reg.clone());
    let mut router_b = MeshRouter::new_ed25519_signed(fp(0xB0), seed(0xB0), full_reg.clone());

    let mut world_a = WorldModel::new();
    let mut world_b = WorldModel::new();
    let mut world_c = WorldModel::new();
    let mut goal: V = vz(); goal[0] = 10.0; goal[1] = 10.0;
    world_a.try_add_zone(ZoneType::Attractive, goal, 5.0, 8.0).expect("setup");
    world_b.try_add_zone(ZoneType::Attractive, goal, 5.0, 8.0).expect("setup");
    world_c.try_add_zone(ZoneType::Attractive, goal, 5.0, 8.0).expect("setup");

    let mut sensor_v_a = SensorNoiseModel::new(10.0);
    let mut sensor_v_b = SensorNoiseModel::new(10.0);
    let mut sensor_v_c = SensorNoiseModel::new(10.0);

    let mut rng = Rng::new(20260511);
    let mut net = NetworkChannel::realistic_lora();
    let mut adversary = AdversaryAgent::new(fp(0xAA), ADVERSARY_INTERVAL_TICKS);
    let mut operator = OperatorSimulator::new();

    let total_ticks = TICKS_PER_VIRTUAL_HOUR * VIRTUAL_HOURS_TO_SOAK;
    let mut buckets: Vec<HourBucket> = Vec::with_capacity(VIRTUAL_HOURS_TO_SOAK as usize);
    let mut current = HourBucket::default();
    current.virtual_hour = 1;

    let trial_start = Instant::now();
    let mut delivered_total: u64 = 0;

    for tick in 0..total_ticks {
        for (sensor, world) in [(&mut sensor_v_a, &mut world_a),
                                 (&mut sensor_v_b, &mut world_b),
                                 (&mut sensor_v_c, &mut world_c)].iter_mut() {
            let v = sensor.sample(&mut rng);
            if (v - 10.0).abs() > 0.5 {
                let mut center: V = vz();
                center[0] = v;
                let _ = world.try_add_zone(ZoneType::Repulsive, center, 1.0, 0.5);
            }
        }

        // Mesh transmit A → B with full Drop instrumentation
        let env = router_a.origin_wrap(b"telemetry");
        let (delivered, _) = net.transmit(&mut rng);
        if delivered {
            delivered_total += 1;
            let mut origin = [0u8; 8];
            origin.copy_from_slice(&env[14..22]);
            if !operator.is_revoked(&origin) {
                match router_b.process(&env) {
                    MeshDecision::Arrived { .. } => current.arrived += 1,
                    MeshDecision::Drop(reason) => current.drops.record(reason),
                }
            } else {
                current.drops.record("operator-revoked");
            }
        }

        if let Some(attacker_fp) = adversary.maybe_inject(tick) {
            if !operator.is_revoked(&attacker_fp) {
                let mut bad_pos: V = vz();
                bad_pos[0] = 5.0 + rng.next_gaussian(0.0, 1.0);
                bad_pos[1] = 5.0 + rng.next_gaussian(0.0, 1.0);
                let _ = world_b.try_add_zone(ZoneType::Repulsive, bad_pos, 8.0, 1.0);
            }
        }

        let combined_cap = world_a.cap_hit_count() + world_b.cap_hit_count() + world_c.cap_hit_count();
        operator.tick(combined_cap, Some(adversary.attacker_fp));

        if (tick + 1) % TICKS_PER_VIRTUAL_HOUR == 0 {
            current.bloom_inserts_router_b = router_b.bloom_inserts();
            current.tx_counter_router_a = router_a.tx_counter();
            buckets.push(std::mem::take(&mut current));
            current.virtual_hour = (tick + 1) / TICKS_PER_VIRTUAL_HOUR + 1;
        }
    }

    let total_wall_ms = trial_start.elapsed().as_millis();
    println!("  total wall clock: {} ms", total_wall_ms);
    println!("  total ticks:      {}", total_ticks);
    println!("  total delivered:  {}", delivered_total);
    println!();
    println!("  Per-hour breakdown — arrived vs dominant Drop reason:");
    println!();
    println!("  {:>4}  {:>7}  {:>7}  {:>10}  {:>22}  {:>10}  {:>12}",
        "hour", "arrived", "drops", "delivered", "dominant_drop", "tx_ctr_a", "bloom_ins_b");
    for b in &buckets {
        let dominant = b.drops.dominant().map(|(r, n)| format!("{}({})", r, n))
            .unwrap_or_else(|| "—".to_string());
        println!("  {:>4}  {:>7}  {:>7}  {:>10}  {:>22}  {:>10}  {:>12}",
            b.virtual_hour, b.arrived, b.drops.total(),
            b.arrived + b.drops.total(),
            dominant, b.tx_counter_router_a, b.bloom_inserts_router_b);
    }

    println!();
    println!("  Aggregate Drop-reason histogram across 24 hours:");
    let mut agg = DropReasonHistogram::default();
    for b in &buckets {
        for (r, n) in &b.drops.by_reason {
            *agg.by_reason.entry(r).or_insert(0) += n;
        }
    }
    let total_drops = agg.total() as f64;
    let mut sorted: Vec<_> = agg.by_reason.iter().collect();
    sorted.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (r, n) in sorted {
        println!("    {:>22} : {:>8}  ({:5.2}%)",
            r, n, 100.0 * (*n as f64) / total_drops);
    }

    println!();
    println!("──────────────────────────────────────────────────────────────────");
    println!("  AA1 verdict:");
    println!("──────────────────────────────────────────────────────────────────");
    if let Some((reason, _)) = agg.dominant() {
        println!("  Dominant Drop reason: \"{}\"", reason);
        match reason {
            "duplicate" => println!("    → AA1 prediction CONFIRMED: Bloom-layer dedup is the cause."),
            "unknown sender" => println!("    → AA1 prediction FALSIFIED: pubkey-registry mismatch is the cause."),
            "operator-revoked" => println!("    → AA1 prediction FALSIFIED: operator over-revoked router_a."),
            "bad mesh magic" | "mesh envelope too short" => {
                println!("    → AA1 prediction FALSIFIED: malformed envelopes — encoding bug somewhere.");
            }
            other => println!("    → AA1 prediction FALSIFIED: unexpected reason \"{}\".", other),
        }
    } else {
        println!("  No drops recorded — but throughput still dropped? Investigate.");
    }
}
