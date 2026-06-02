//! OASIS Grid Demo v2 — five coordinated defense layers, not just mesh.
//!
//! The previous demo only showed M11 (Ed25519 mesh signing). That's
//! one layer of OASIS. The thing OASIS actually is — and what
//! distinguishes it from every other secure-mesh library — is the
//! **coordination of five graceful-degradation layers**:
//!
//!    L1  M11 mesh signing      catches network-borne attacks
//!    L2  R14 entropy gate      refuses action when world is unclear
//!    L3  M9  reflex arc        fires fast on sensor surprise
//!    L4  M10 pressure field    navigates AWAY from danger zones
//!    L5  Vitality + Kill switch graceful degradation, last-resort halt
//!
//! Each scenario below names the layer that catches the fault class.
//! The coordination is the product. Each layer alone exists in the
//! literature; OASIS's contribution is they all run together on a
//! 278 KB Rust kernel that fits a $3 MCU.
//!
//! Audience: a grid-ops engineer running this as a 30-second sanity
//! check. Output is verbose enough to follow, deterministic, prints
//! a clear PASS/FAIL per scenario at the end.

use std::time::Instant;

use oasis_rt::mesh::{
    MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter,
    mesh_v10_pubkey_from_seed,
};
use oasis_rt::reflex::AdaptiveReflex;
use oasis_rt::hyper_state::{agent_new, entropy, evolve, is_action_safe};
use oasis_rt::world_model::{WorldModel, ZoneType};
use oasis_rt::vitality::{VitalityState, Vitality, VitalityLevel};
use oasis_rt::hal::{KillSwitch, PanicReason};
use oasis_rt::vec::{V, vz};

fn fp(tag: u8) -> [u8; 8] {
    let mut f = [0u8; 8]; f[0] = tag; f
}

fn seed(tag: u8) -> MeshEdSeed { MeshEdSeed([tag; 32]) }

fn print_banner() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  OASIS Grid Demo v2 — 5 coordinated defense layers               ║");
    println!("║  L1 mesh sign  L2 entropy gate  L3 reflex  L4 pressure  L5 halt  ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
}

fn header(layer: &str, scenario: &str) {
    println!("──────────────────────────────────────────────────────────────────");
    println!(" [{}]  {}", layer, scenario);
    println!("──────────────────────────────────────────────────────────────────");
}

fn pass(scenario: u32, msg: &str) -> u32 {
    println!("  [OK] Scenario {} PASS — {}", scenario, msg);
    println!();
    1
}

fn fail(scenario: u32, msg: &str) -> u32 {
    println!("  [FAIL] Scenario {} FAIL — {}", scenario, msg);
    println!();
    0
}

fn main() {
    print_banner();
    println!("Setting up: 4-node substation mesh (NODE_A/B/C/D), edge agent");
    println!("at controllable DER, world model with one hazard zone.");
    println!();

    let mut passed = 0u32;
    let mut total = 0u32;

    // ── Mesh setup (used by L1 scenarios) ───────────────────────────
    let seed_a = seed(0xA0);
    let seed_c = seed(0xC0);
    let seed_d = seed(0xD0);
    let pk_a = mesh_v10_pubkey_from_seed(&seed_a).unwrap();
    let mut reg_d = MeshPubRegistry::new();
    reg_d.insert(fp(0xA0), pk_a);
    let mut router_a = MeshRouter::new_ed25519_signed(fp(0xA0), seed_a.clone(), MeshPubRegistry::new());
    let mut router_d = MeshRouter::new_ed25519_signed(fp(0xD0), seed_d, reg_d);

    // ── L1 — M11 MESH SIGNING ───────────────────────────────────────

    total += 1;
    header("L1 / M11", "S1: Normal signed telemetry from A to D");
    let reading = b"{\"bus\":\"A\",\"V\":230.4,\"I\":87.1,\"T\":42.3}";
    let env = router_a.origin_wrap(reading);
    let t0 = Instant::now();
    let decision = router_d.process(&env);
    let dt = t0.elapsed().as_micros();
    println!("  Ed25519 verify: {} µs   envelope: {} B", dt, env.len());
    passed += match decision {
        MeshDecision::Arrived { .. } => pass(1, "signature verified, telemetry accepted"),
        MeshDecision::Drop(r) => fail(1, &format!("unexpected Drop({:?})", r)),
    };

    total += 1;
    header("L1 / M11", "S2: Captured node C tries to spoof A's identity");
    let mut router_c_bad = MeshRouter::new_ed25519_signed(fp(0xC0), seed_c, MeshPubRegistry::new());
    let mut env = router_c_bad.origin_wrap(b"{\"bus\":\"A\",\"V\":999,\"I\":0}");
    env[14..22].copy_from_slice(&fp(0xA0));   // attacker overwrites origin_fp
    println!("  Attack: C signs with their key, claims fp=A in header");
    passed += match router_d.process(&env) {
        MeshDecision::Drop(r) => pass(2, &format!("rejected as {:?}", r)),
        MeshDecision::Arrived { .. } => fail(2, "spoof accepted — sig check broken"),
    };

    total += 1;
    header("L1 / M11", "S3: Replay of a previously valid envelope");
    let env_replay = router_a.origin_wrap(b"{\"bus\":\"A\",\"V\":230.5}");
    let _ = router_d.process(&env_replay);
    println!("  Attack: attacker re-injects the same bytes a second time");
    passed += match router_d.process(&env_replay) {
        MeshDecision::Drop(r) => pass(3, &format!("rejected as {:?}", r)),
        MeshDecision::Arrived { .. } => fail(3, "replay accepted — Bloom dedup broken"),
    };

    // ── L2 — R14 ENTROPY GATE ───────────────────────────────────────

    total += 1;
    header("L2 / R14", "S4: Action refused when sensor state is too uncertain");
    println!("  Operational scenario: SCADA wants to command a switch close.");
    println!("  Edge agent's sensor inputs are inconsistent (entropy = high).");
    println!("  R14: only act when uncertainty < threshold.");
    let mut agent = agent_new(2);
    let mut force: V = vz();
    force[0] = 50.0; force[1] = 50.0; force[2] = 50.0; force[3] = 50.0;
    for _ in 0..20 { evolve(&mut agent, &force, 0.1, 0.99); }
    let e = entropy(&agent.pos);
    let threshold = 0.5;
    let safe = is_action_safe(&agent, threshold);
    println!("  agent.entropy = {:.3}   threshold = {:.2}   safe = {}", e, threshold, safe);
    passed += if !safe {
        pass(4, "high entropy detected, switch-close command REFUSED")
    } else {
        fail(4, "R14 should have refused — sensor state was uncertain")
    };

    // ── L3 — M9 REFLEX ARC ──────────────────────────────────────────

    total += 1;
    header("L3 / M9", "S5: Reflex arc fires on a surprise sensor spike");
    println!("  Operational scenario: vibration sensor on transformer reads");
    println!("  baseline ~1.0 mm/s. A sudden 6.0 mm/s spike must trigger");
    println!("  immediate protective response without going through SCADA.");
    let mut rf = AdaptiveReflex::new(3.0);
    for v in [1.0_f64, 1.05, 0.95, 1.0, 1.02, 0.98, 1.01, 0.99, 1.03, 0.97] {
        rf.feed(v);
    }
    rf.calibrate();
    let pre  = rf.check(1.0);
    let fire = rf.check(6.0);
    println!("  baseline calibrated on 10 samples   threshold = mean ± 3σ");
    println!("  check(1.0 mm/s) = {} (no fire)      check(6.0 mm/s) = {} (FIRE)",
             pre, fire);
    passed += if fire && !pre {
        pass(5, "reflex arc fires only on sigma-outlier, baseline-suppressed")
    } else {
        fail(5, "reflex did not match expected fire/no-fire pattern")
    };

    // ── L4 — M10 PRESSURE FIELD NAVIGATION ──────────────────────────

    total += 1;
    header("L4 / M10", "S6: Edge agent navigates AWAY from a hazard zone");
    println!("  Operational scenario: a mobile edge node (drone inspection,");
    println!("  patrol robot) is operating near a known overheated cell.");
    println!("  M10 pressure field: hazard radiates Repulsive pressure;");
    println!("  navigate() returns a path that flows down the gradient");
    println!("  AWAY from the hazard, not into it.");
    let mut wm = WorldModel::new();
    let mut hazard_center: V = vz();
    hazard_center[0] = 0.0; hazard_center[1] = 0.0;
    let _ = wm.try_add_zone(ZoneType::Repulsive, hazard_center, 5.0, 2.0);
    let mut goal_center: V = vz();
    goal_center[0] = 10.0;
    let _ = wm.try_add_zone(ZoneType::Attractive, goal_center, 3.0, 5.0);

    let mut start: V = vz();
    start[0] = 1.5; start[1] = 0.5;       // start CLOSE to the hazard
    let path = wm.navigate(&start, &goal_center, 50);

    let dist = |a: &V, b: &V| ((a[0]-b[0]).powi(2) + (a[1]-b[1]).powi(2)).sqrt();
    let d_start_hazard = dist(&start, &hazard_center);
    let d_end_hazard   = dist(path.last().unwrap(), &hazard_center);
    let d_start_goal   = dist(&start, &goal_center);
    let d_end_goal     = dist(path.last().unwrap(), &goal_center);
    println!("  start=({:.2},{:.2})   hazard=(0,0)   goal=(10,0)   steps=50",
             start[0], start[1]);
    println!("  start→hazard: {:.2}  →  end→hazard: {:.2}  ({})",
             d_start_hazard, d_end_hazard,
             if d_end_hazard > d_start_hazard { "AWAY ✓" } else { "TOWARD ✗" });
    println!("  start→goal:   {:.2}  →  end→goal:   {:.2}  ({})",
             d_start_goal, d_end_goal,
             if d_end_goal < d_start_goal { "CLOSER ✓" } else { "FARTHER ✗" });
    passed += if d_end_hazard > d_start_hazard && d_end_goal < d_start_goal {
        pass(6, "gradient correctly avoids hazard AND advances toward goal")
    } else {
        fail(6, "navigate() did not produce a hazard-avoiding goal-approaching path")
    };

    // ── L5 — VITALITY GRACEFUL DEGRADATION ──────────────────────────

    total += 1;
    header("L5 / Vitality", "S7: Sensor loss triggers graceful capability downgrade");
    println!("  Operational scenario: 2 of 3 IMU axes stop reporting (cabling");
    println!("  fault, ESD, weather). Vitality detects vital-sensor loss and");
    println!("  downgrades to Critical — only reflexes allowed, zero actuation.");
    let mut vitality = VitalityState::new();
    let healthy_sensors: Vec<(bool, Vitality)> = vec![
        (true,  Vitality::Vital), (true,  Vitality::Vital), (true,  Vitality::Vital),
        (true,  Vitality::Important), (true, Vitality::Important),
        (true,  Vitality::Optional),
    ];
    vitality.update(&healthy_sensors);
    let level_before = vitality.level;

    let degraded_sensors: Vec<(bool, Vitality)> = vec![
        (true,  Vitality::Vital),                          // 1 of 3 vitals alive
        (false, Vitality::Vital), (false, Vitality::Vital),
        (true,  Vitality::Important), (true,  Vitality::Important),
        (true,  Vitality::Optional),
    ];
    vitality.update(&degraded_sensors);
    let level_after = vitality.level;
    let r14_before = 0.95;  // healthy default
    let r14_after = vitality.r14_threshold();
    println!("  Before: level = {:?}    R14 threshold = {}", level_before, r14_before);
    println!("  After:  level = {:?}    R14 threshold = {}", level_after, r14_after);
    println!("  diagnostic: {}", vitality.diagnostic());
    passed += match (level_before, level_after) {
        (VitalityLevel::Healthy, VitalityLevel::Critical) | (VitalityLevel::Healthy, VitalityLevel::Degraded) => {
            pass(7, "vitality correctly downgraded after vital-sensor loss")
        }
        _ => fail(7, &format!("expected Healthy → Degraded/Critical, got {:?} → {:?}",
            level_before, level_after)),
    };

    // ── L5 — KILL SWITCH (last resort) ──────────────────────────────

    total += 1;
    header("L5 / KillSwitch", "S8: Last-resort halt on geofence breach");
    println!("  Operational scenario: a control loop attempts to drive an");
    println!("  actuator beyond the authorized operational envelope.");
    println!("  Kill switch is the bottom-of-stack hardware-style halt:");
    println!("  once latched, no further command can re-enable.");
    let ks = KillSwitch::new();
    println!("  Initial state: triggered = {}", ks.is_triggered());
    let panicked = ks.panic(PanicReason::GeofenceBreach, "S8_demo",
        "actuator setpoint outside [vmin,vmax]".into());
    println!("  panic(GeofenceBreach) -> latched? {}", panicked);
    let still_locked = ks.is_triggered();
    println!("  After panic: triggered = {}", still_locked);
    passed += if panicked && still_locked {
        pass(8, "kill switch latched, all subsequent commands will refuse")
    } else {
        fail(8, "kill switch did not latch as expected")
    };

    // ── Summary ─────────────────────────────────────────────────────
    println!("══════════════════════════════════════════════════════════════════");
    println!("  SUMMARY:  {} PASSED,  {} FAILED  (out of {} scenarios)",
             passed, total - passed, total);
    println!("══════════════════════════════════════════════════════════════════");
    println!();

    if passed == total {
        println!("All 8 scenarios produced expected outcomes across the 5 layers:");
        println!();
        println!("  L1 / M11 mesh signing");
        println!("    [OK] normal telemetry accepted");
        println!("    [OK] insider spoof rejected");
        println!("    [OK] replay rejected (Bloom dedup)");
        println!("  L2 / R14 entropy gate");
        println!("    [OK] command refused when world state uncertain");
        println!("  L3 / M9 reflex arc");
        println!("    [OK] sigma-outlier detected, baseline suppressed");
        println!("  L4 / M10 pressure-field navigation");
        println!("    [OK] trajectory avoids hazard, approaches goal");
        println!("  L5 / Vitality + KillSwitch");
        println!("    [OK] sensor loss downgrades capability gracefully");
        println!("    [OK] geofence breach latches kill switch");
        println!();
        println!("OASIS coordinates these 5 layers in a single 278 KB Rust kernel,");
        println!("compiles to Cortex-M0+ ($3 MCU). What you just saw on x86 runs");
        println!("byte-exact on simulated MCU silicon — see docs/capabilities.md.");
        println!();
        println!("Next steps to evaluate OASIS for your grid deployment:");
        println!("  - docs/architecture.md   (15 min)");
        println!("  - docs/capabilities.md   (15 min)");
        println!("  - docs/grid_use_cases.md (15 min)");
        println!("  - docs/SHADOW_AUDIT.md   (the skeptic's view)");
        std::process::exit(0);
    } else {
        eprintln!("UNEXPECTED FAILURE in {} scenario(s).", total - passed);
        eprintln!("Send this output to souhaybrharrab@gmail.com — it's on us, not you.");
        std::process::exit(1);
    }
}
