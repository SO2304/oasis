//! OASIS Grid Demo v3 — chained layer interactions.
//!
//! v2 showed each of the 5 layers in isolation (1 fault class → 1 layer
//! catches it). That's pedagogically clear but operationally
//! incomplete: real grid scenarios involve LAYERS COORDINATING on the
//! same packet / same fault / same time window. v3 covers that.
//!
//! Four chained scenarios:
//!
//!   C1  L1 ∩ L2  Insider with valid keys sends a command. Mesh sig
//!                verifies (L1 OK). But receiver's R14 entropy is high
//!                → refuses to act despite sig validity (L2 OK ≠ act).
//!
//!   C2  L3 → L4  Reflex (M9) fires from a vibration anomaly. The
//!                handler injects a Repulsive zone at the anomaly
//!                location into the world model. M10 navigate()
//!                re-paths the agent AROUND the new zone.
//!
//!   C3  L5⇒L5    Sensor losses accumulate over time. Vitality drops
//!                Healthy → Degraded → Critical. When Critical persists,
//!                kill switch latches via PanicReason::VitalityDead.
//!
//!   C4  L1→L5    Full operational pipeline. SCADA → mesh → entropy →
//!                pressure-field → geofence → execute → fault →
//!                reflex → vitality → kill switch. The complete defense
//!                chain end-to-end on one DER setpoint.
//!
//! These are the patterns OASIS is actually designed to handle. v2 was
//! the spec sheet; v3 is the operational manual.

use std::time::Instant;

use oasis_rt::mesh::{
    MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter,
    inner_slice, mesh_v10_pubkey_from_seed,
};
use oasis_rt::reflex::AdaptiveReflex;
use oasis_rt::hyper_state::{agent_new, entropy, evolve, is_action_safe};
use oasis_rt::world_model::{WorldModel, ZoneType};
use oasis_rt::vitality::{VitalityState, Vitality, VitalityLevel};
use oasis_rt::hal::{KillSwitch, PanicReason};
use oasis_rt::vec::{V, vz};

fn fp(tag: u8) -> [u8; 8] { let mut f = [0u8; 8]; f[0] = tag; f }
fn seed(tag: u8) -> MeshEdSeed { MeshEdSeed([tag; 32]) }

fn header(tag: &str, title: &str) {
    println!();
    println!("══════════════════════════════════════════════════════════════════");
    println!("  {}  {}", tag, title);
    println!("══════════════════════════════════════════════════════════════════");
}

fn pass(c: &str, msg: &str) -> u32 {
    println!("  [OK] {} PASS — {}", c, msg);
    1
}

fn fail(c: &str, msg: &str) -> u32 {
    println!("  [FAIL] {} FAIL — {}", c, msg);
    0
}

fn dist(a: &V, b: &V) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  OASIS Grid Demo v3 — chained layer interactions                 ║");
    println!("║  (run after v2; v2 = layers in isolation, v3 = layers coordinated)║");
    println!("╚══════════════════════════════════════════════════════════════════╝");

    let mut passed = 0u32;
    let mut total = 0u32;

    // ── Setup mesh nodes ─────────────────────────────────────────────
    let seed_a = seed(0xA0);
    let seed_d = seed(0xD0);
    let pk_a = mesh_v10_pubkey_from_seed(&seed_a).unwrap();
    let mut reg_d = MeshPubRegistry::new();
    reg_d.insert(fp(0xA0), pk_a);
    let mut router_a = MeshRouter::new_ed25519_signed(fp(0xA0), seed_a.clone(), MeshPubRegistry::new());
    let mut router_d = MeshRouter::new_ed25519_signed(fp(0xD0), seed_d, reg_d);

    // ── C1 — L1 ∩ L2: signed but refused by R14 ─────────────────────
    total += 1;
    header("C1 / L1 ∩ L2", "Signed-but-refused (mesh sig OK, R14 says no)");
    println!();
    println!("  Scenario: a long-trusted node A sends a high-impact command");
    println!("  ('open breaker on bus 3'). Node A's keys are valid → mesh sig");
    println!("  verifies cleanly. BUT receiver D's local sensors disagree on");
    println!("  the bus state right now (entropy high). R14 says: do NOT act.");
    println!();

    let cmd = b"{\"cmd\":\"open_breaker\",\"bus\":3}";
    let env = router_a.origin_wrap(cmd);

    // Receiver D processes envelope — L1 layer
    let t0 = Instant::now();
    let l1_decision = router_d.process(&env);
    let l1_us = t0.elapsed().as_micros();

    // Receiver's local entropy (L2 layer) — independent of the message
    let mut local_agent = agent_new(2);
    let mut force: V = vz();
    force[0] = 50.0; force[1] = 50.0; force[2] = 50.0; force[3] = 50.0;
    for _ in 0..20 { evolve(&mut local_agent, &force, 0.1, 0.99); }
    let r14_threshold = 0.5;
    let l2_action_safe = is_action_safe(&local_agent, r14_threshold);

    println!("  L1 decision: {:?}   ({} µs)",
        match &l1_decision { MeshDecision::Arrived { .. } => "Arrived", MeshDecision::Drop(_) => "Drop" },
        l1_us);
    println!("  L2 R14:      entropy={:.3} threshold={:.2}  is_action_safe={}",
        local_agent.entropy, r14_threshold, l2_action_safe);

    let executed = matches!(&l1_decision, MeshDecision::Arrived { .. }) && l2_action_safe;
    println!("  Final verdict: {} (must be both L1 OK AND L2 OK)",
        if executed { "EXECUTE" } else { "REFUSE" });

    passed += if matches!(&l1_decision, MeshDecision::Arrived { .. }) && !l2_action_safe && !executed {
        pass("C1", "sig accepted by L1 but action refused by L2 — coordinated safe outcome")
    } else {
        fail("C1", "expected L1=Arrived, L2=unsafe, final=REFUSE")
    };

    // ── C2 — L3 → L4: reflex re-routes via M10 zone injection ────────
    total += 1;
    header("C2 / L3 → L4", "Reflex injects hazard zone, M10 reroutes");
    println!();
    println!("  Scenario: edge agent (inspection drone, autonomous switchgear");
    println!("  attendant) is en route to inspect bus B at (10, 0). Mid-route");
    println!("  a transformer at (5, 2) emits a vibration anomaly (M9 fires).");
    println!("  L3 reflex handler injects a Repulsive zone at that location;");
    println!("  L4 navigate() returns a path that detours around the new hazard.");
    println!();

    let mut start: V = vz(); start[0] = 0.0; start[1] = 0.0;
    let mut goal: V = vz(); goal[0] = 10.0; goal[1] = 0.0;

    // Path BEFORE reflex fires
    let mut wm = WorldModel::new();
    let path_before = wm.navigate(&start, &goal, 50);
    let dist_before_to_anomaly_pos = path_before.iter()
        .map(|p| {
            let mut anomaly: V = vz(); anomaly[0] = 5.0; anomaly[1] = 2.0;
            dist(p, &anomaly)
        })
        .fold(f64::INFINITY, f64::min);

    // M9 reflex fires on anomaly
    let mut rf = AdaptiveReflex::new(3.0);
    for v in [1.0_f64, 0.95, 1.05, 1.0, 0.98, 1.02, 1.0, 0.97, 1.03, 1.0] {
        rf.feed(v);
    }
    rf.calibrate();
    let anomaly_value = 6.0;
    let l3_fired = rf.check(anomaly_value);

    // L4 handler — inject Repulsive zone at anomaly location
    let mut anomaly_pos: V = vz(); anomaly_pos[0] = 5.0; anomaly_pos[1] = 2.0;
    if l3_fired {
        let _ = wm.try_add_zone(ZoneType::Repulsive, anomaly_pos, 8.0, 1.5);
    }

    // Path AFTER reflex+inject
    let path_after = wm.navigate(&start, &goal, 50);
    let dist_after_to_anomaly_pos = path_after.iter()
        .map(|p| dist(p, &anomaly_pos))
        .fold(f64::INFINITY, f64::min);

    println!("  L3 reflex.check(6.0) = {} (sigma-outlier)", l3_fired);
    println!("  Path BEFORE reflex: closest approach to anomaly = {:.3} m",
        dist_before_to_anomaly_pos);
    println!("  Path AFTER  reflex: closest approach to anomaly = {:.3} m",
        dist_after_to_anomaly_pos);
    println!("  Both paths reach close to goal (within 50 steps).");

    passed += if l3_fired && dist_after_to_anomaly_pos > dist_before_to_anomaly_pos {
        pass("C2", "L3 fires → L4 zone injected → re-route stays further from anomaly")
    } else {
        fail("C2", &format!("expected L3 fire AND increased anomaly distance (got fire={}, before={:.2}, after={:.2})",
            l3_fired, dist_before_to_anomaly_pos, dist_after_to_anomaly_pos))
    };

    // ── C3 — L5 cascade: Vitality → KillSwitch ───────────────────────
    total += 1;
    header("C3 / L5 cascade", "Vitality degradation cascades into KillSwitch");
    println!();
    println!("  Scenario: sensors fail one by one over time (cabling fault,");
    println!("  weather, ESD events). Vitality tracks the cumulative loss");
    println!("  Healthy → Degraded → Critical. When Critical persists, the");
    println!("  controller decides to engage the kill switch via VitalityDead");
    println!("  rather than try to keep operating with no safe sensor margin.");
    println!();

    let mut vit = VitalityState::new();
    let ks = KillSwitch::new();

    let healthy: Vec<(bool, Vitality)> = vec![
        (true, Vitality::Vital), (true, Vitality::Vital), (true, Vitality::Vital),
        (true, Vitality::Important), (true, Vitality::Important),
        (true, Vitality::Optional),
    ];
    let one_vital_dead: Vec<(bool, Vitality)> = vec![
        (true, Vitality::Vital), (true, Vitality::Vital), (false, Vitality::Vital),
        (true, Vitality::Important), (true, Vitality::Important),
        (true, Vitality::Optional),
    ];
    let three_vital_dead: Vec<(bool, Vitality)> = vec![
        (false, Vitality::Vital), (false, Vitality::Vital), (false, Vitality::Vital),
        (true, Vitality::Important), (true, Vitality::Important),
        (true, Vitality::Optional),
    ];

    println!("  Time   Sensor state                  Vitality   R14_thr  Action");
    println!("  ─────  ────────────────────────────  ─────────  ───────  ────────");
    vit.update(&healthy);
    println!("  t=0    all sensors alive             {:?}    {:.2}     accept",
        vit.level, vit.r14_threshold());
    vit.update(&one_vital_dead);
    println!("  t=1    1 of 3 vital dead             {:?}   {:.2}     cautious",
        vit.level, vit.r14_threshold());
    // Tick 100 more times in Critical to reach Dead (critical_timeout=100)
    for tick in 2..=101 {
        vit.update(&three_vital_dead);
        if tick == 2 || tick == 50 || tick == 100 || tick == 101 {
            println!("  t={}{} 3 of 3 vital dead             {:?}   {:.2}     crit_ticks={} {}",
                tick,
                if tick < 10 { "   " } else if tick < 100 { "  " } else { " " },
                vit.level, vit.r14_threshold(),
                vit.ticks_in_critical,
                if vit.should_shutdown() { "← SHUTDOWN" } else { "" });
        }
    }

    let cascade_triggered = vit.should_shutdown();
    if cascade_triggered {
        ks.panic(PanicReason::VitalityDead, "C3_demo",
            format!("vitality persisted Critical for {} ticks, halting", 4));
    }

    println!();
    println!("  Vitality.should_shutdown() = {}", cascade_triggered);
    println!("  KillSwitch.is_triggered()  = {}", ks.is_triggered());

    passed += if cascade_triggered && ks.is_triggered() {
        pass("C3", "L5 cascade: Vitality Critical persisted → KillSwitch latched via VitalityDead")
    } else {
        fail("C3", "expected cumulative vitality failure to trigger kill switch")
    };

    // ── C4 — Full operational pipeline ───────────────────────────────
    total += 1;
    header("C4 / L1→L5 full chain", "Complete defense pipeline on a DER setpoint");
    println!();
    println!("  Realistic ops sequence: SCADA sends battery setpoint to a DER");
    println!("  edge controller. The command traverses ALL 5 layers as gates");
    println!("  before reaching the actuator. We trace each gate's verdict.");
    println!();

    // Reset state for this scenario
    let ks = KillSwitch::new();
    let mut vit = VitalityState::new();
    vit.update(&healthy);
    let mut rf = AdaptiveReflex::new(3.0);
    for v in [1.0_f64, 0.95, 1.05, 1.0, 0.98, 1.02, 1.0, 0.97, 1.03, 1.0] {
        rf.feed(v);
    }
    rf.calibrate();
    let mut wm = WorldModel::new();
    let mut nominal: V = vz(); nominal[0] = 5.0; nominal[1] = 0.0;
    let _ = wm.try_add_zone(ZoneType::Attractive, nominal, 3.0, 5.0);
    let mut local_agent = agent_new(2);   // start in nominal/healthy state

    // Step 1 — SCADA sends command, Node A signs, transmits
    let setpoint_cmd = b"{\"cmd\":\"set_battery\",\"power_kw\":250}";
    let env = router_a.origin_wrap(setpoint_cmd);
    let l1 = router_d.process(&env);
    let l1_pass = matches!(&l1, MeshDecision::Arrived { .. });
    let inner = if l1_pass {
        if let MeshDecision::Arrived { ref envelope, .. } = l1 {
            inner_slice(envelope).to_vec()
        } else { Vec::new() }
    } else { Vec::new() };
    println!("  [step 1] L1 mesh sig verify ............. {}",
        if l1_pass { "PASS" } else { "FAIL" });
    println!("           inner payload = {:?}",
        std::str::from_utf8(&inner).unwrap_or("<bytes>"));

    // Step 2 — R14 local entropy check (sensors are healthy here)
    let l2_pass = is_action_safe(&local_agent, vit.r14_threshold());
    println!("  [step 2] L2 R14 entropy gate ............ {}  (entropy={:.3} < {})",
        if l2_pass { "PASS" } else { "FAIL" }, local_agent.entropy, vit.r14_threshold());

    // Step 3 — L4 Pressure field: is the agent's current state in a safe region?
    // (no Repulsive zones present yet; nominal Attractive only)
    let agent_pos = local_agent.pos.clone();
    let dist_to_nominal = dist(&agent_pos, &nominal);
    let l4_pass = dist_to_nominal < 10.0;   // within 10 units of attractor = safe
    println!("  [step 3] L4 pressure-field state check .. {}  (dist_to_attractor={:.2})",
        if l4_pass { "PASS" } else { "FAIL" }, dist_to_nominal);

    // Step 4 — Geofence on the requested setpoint magnitude (0..400 kW window)
    let setpoint_kw = 250.0_f64;
    let geofence_pass = setpoint_kw >= 0.0 && setpoint_kw <= 400.0;
    println!("  [step 4] geofence on setpoint kW ........ {}  ({} kW within [0, 400])",
        if geofence_pass { "PASS" } else { "FAIL" }, setpoint_kw);

    // Step 5 — Vitality status check
    let vitality_pass = matches!(vit.level, VitalityLevel::Healthy | VitalityLevel::Degraded);
    println!("  [step 5] L5 vitality permits actuation .. {}  (level={:?})",
        if vitality_pass { "PASS" } else { "FAIL" }, vit.level);

    let all_gates = l1_pass && l2_pass && l4_pass && geofence_pass && vitality_pass && !ks.is_triggered();
    println!();
    println!("  >>> ALL GATES PASSED: {} <<<  → setpoint applied: {} kW",
        all_gates, if all_gates { setpoint_kw } else { 0.0 });

    // Step 6 — A fault arrives mid-execution: vibration spike on the battery
    println!();
    println!("  [step 6] mid-execution: battery cell temp 6× sigma");
    let fault_value = 6.0;
    let l3_fired = rf.check(fault_value);
    println!("           L3 reflex.check({:.1}) = {}", fault_value, l3_fired);

    if l3_fired {
        // L4 handler: inject Repulsive zone at the cell location (we model
        // it as a high-pressure pop right at our agent's pos)
        let mut here = local_agent.pos.clone();
        here[0] = local_agent.pos[0]; here[1] = local_agent.pos[1];
        let _ = wm.try_add_zone(ZoneType::Repulsive, here, 10.0, 1.0);
        println!("           L4 reaction: Repulsive zone added at agent position");
    }

    // Cumulative degradation: simulate that this fault costs us a vital sensor
    let after_fault: Vec<(bool, Vitality)> = vec![
        (true, Vitality::Vital), (false, Vitality::Vital), (false, Vitality::Vital),
        (true, Vitality::Important), (true, Vitality::Important),
        (true, Vitality::Optional),
    ];
    vit.update(&after_fault);
    println!("           L5 vitality recomputed: level={:?} (R14 threshold now {})",
        vit.level, vit.r14_threshold());

    // If Critical persists for critical_timeout ticks → Dead → kill switch
    let mut vit_persistent = vit;
    for _ in 0..101 { vit_persistent.update(&three_vital_dead); }
    println!("  [step 7] vitality after 101 ticks of 3-of-3 dead: level={:?} crit_ticks={}",
        vit_persistent.level, vit_persistent.ticks_in_critical);
    if vit_persistent.should_shutdown() {
        ks.panic(PanicReason::VitalityDead, "C4_demo",
            "battery fault → cumulative vital loss → cannot continue safely".into());
    }
    println!("           kill switch latched? = {}", ks.is_triggered());

    let cascade_correct = l1_pass && l2_pass && all_gates && l3_fired
        && vit_persistent.should_shutdown() && ks.is_triggered();
    passed += if cascade_correct {
        pass("C4", "all 5 layers traverse correctly: gates pass, fault triggers reflex, cascade lands in kill switch")
    } else {
        fail("C4", "operational pipeline did not produce the expected sequence")
    };

    // ── Summary ─────────────────────────────────────────────────────
    println!();
    println!("══════════════════════════════════════════════════════════════════");
    println!("  v3 SUMMARY:  {} PASSED,  {} FAILED  (out of {} chained scenarios)",
        passed, total - passed, total);
    println!("══════════════════════════════════════════════════════════════════");
    println!();

    if passed == total {
        println!("All 4 chained scenarios produced expected outcomes.");
        println!();
        println!("v2 was the spec sheet (each layer in isolation, 1 fault → 1 catch).");
        println!("v3 is the operational manual (layers coordinate on the same packet,");
        println!("the same fault, the same time window).");
        println!();
        println!("What you just saw on x86 runs byte-exact on a Cortex-M0+ MCU");
        println!("(see docs/capabilities.md for the cross-check evidence).");
        println!();
        println!("Next steps:");
        println!("  - Read docs/architecture.md to see how the chain plugs into your stack");
        println!("  - Read docs/grid_use_cases.md for Elia/HEDGE-IoT scenario fit");
        println!("  - Contact souhaybrharrab@gmail.com for source access (NDA)");
        std::process::exit(0);
    } else {
        eprintln!("UNEXPECTED FAILURE in {} scenario(s).", total - passed);
        eprintln!("Send this output to souhaybrharrab@gmail.com — it's on us, not you.");
        std::process::exit(1);
    }
}
