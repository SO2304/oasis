//! OASIS — Deterministic 10k-tick soak bench for all 11 mechanisms (K=10 banded).
//!
//! Purpose: validate that mechanism-level invariants hold SIMULTANEOUSLY
//! over a long synthetic session. Complements per-module property tests.
//!
//! Inputs are deterministic (seeded LCG) → functional outputs identical
//! across runs. K=10 repeats measure TICK-RATE band only. Any divergence
//! in functional output across repeats is a bug (checked; assertion).
//!
//! Invariants checked every 500 ticks + at end:
//!   M3 pain   ∈ [0, 5]                        (bounded)
//!   M5 fear   ∈ [0, 5]                        (bounded)
//!   M6 count  == initial_count                (conserved)
//!   M6 Stem   == 0 after warm-up              (terminal)
//!   M8 dreams ≥ floor((t-500)/500)            (force trigger fires)
//!   R14 entropy always in [0, 1]              (physics invariant)
//!
//! Usage: cargo run --release --bin bench_mechanisms_soak

use oasis_rt::branching::TemporalBrancher;
use oasis_rt::dreams::DreamEngine;
use oasis_rt::efference::ReflectionEngine;
use oasis_rt::emotion::EmotionalState;
use oasis_rt::hyper_state::*;
use oasis_rt::morpho::{MorphoEngine, Role};
use oasis_rt::reflex::AdaptiveReflex;
use oasis_rt::synapse::{AgentMomentum, SynapticNetwork};
use oasis_rt::tension::TensionField;
use oasis_rt::vec::*;
use oasis_rt::world_model::{WorldModel, ZoneType};

const TOTAL_TICKS: u32 = 10_000;
const CHECK_EVERY: u32 = 500;
const N_AGENTS: usize = 8;
const K_REPEATS: usize = 10;

/// Deterministic functional outputs — must be identical across K repeats
/// (seeded LCG, same input sequence). Divergence = bug.
#[derive(Debug, Clone, PartialEq)]
struct SoakOutcome {
    dream_fires: u32,
    reflex_fires: u32,
    branch_calls: u32,
    violations: u32,
    final_entropy_bits: u64,       // f64 bits for eq check
    final_fear_bits: u64,
    final_pain_bits: u64,
    roles: [usize; 6],             // Stem, Nav, Sent, Work, Scout, Heal
}

fn run_soak(verbose: bool) -> (SoakOutcome, std::time::Duration) {
    let mut agents: Vec<Agent> = (0..N_AGENTS).map(agent_new).collect();
    let mut tension = TensionField::new();
    let mut syn = SynapticNetwork::new();
    let mut emo = EmotionalState::new();
    let mut reflex_g = AdaptiveReflex::new(2.0);
    let mut reflex_m = AdaptiveReflex::new(2.0);
    let mut morpho = MorphoEngine::new();
    let mut eff = ReflectionEngine::new();
    let mut dreams = DreamEngine::new();
    let mut world = WorldModel::new();

    for _ in 0..N_AGENTS { morpho.register(); }

    let mut obs = vz(); obs[10] = 3.0;
    world.try_add_zone(ZoneType::Repulsive, obs, 2.0, 0.5).expect("setup must not exceed cap");
    let mut tgt = vz(); tgt[10] = 8.0;
    world.try_add_zone(ZoneType::Attractive, tgt, 3.0, 0.3).expect("setup must not exceed cap");

    let mut dream_fires = 0u32;
    let mut reflex_fires = 0u32;
    let mut branch_calls = 0u32;
    let mut violations = 0u32;

    let mut rng_state: u64 = 0xCAFEBABE_CAFEBABE;
    let mut rnd = || -> f64 {
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((rng_state >> 32) as f64) / (u32::MAX as f64)
    };

    let start = std::time::Instant::now();

    for t in 0..TOTAL_TICKS {
        let accel = (rnd() - 0.5) * 2.0;
        let gyro = (rnd() - 0.5) * 1.0;

        let mut emit_force = vz();
        emit_force[10 + (t as usize % 4)] = (rnd() - 0.5) * 0.4;
        tension.emit(&emit_force, 0.5, 10);

        for a in agents.iter_mut() {
            let mut f = vz();
            f[10] = (rnd() - 0.5) * 0.2;
            evolve(a, &f, 0.1, 0.05);
        }
        let entropy = agents[0].entropy;

        emo.update(&agents[0].pos, entropy, t);

        let moms: Vec<AgentMomentum> = agents.iter()
            .map(|a| AgentMomentum { momentum: a.momentum, entropy: a.entropy })
            .collect();
        syn.update(&moms);

        if reflex_g.check(gyro.abs()) { reflex_fires += 1; }
        if reflex_m.check(accel.abs()) { reflex_fires += 1; }

        if t % 20 == 0 {
            let cmd = (rnd() - 0.5) * 0.5;
            let pos = vz();
            let effort = vz();
            eff.predict(0, &pos, &effort, cmd, 1.0);
            let mut actual = vz();
            actual[0] = cmd * 0.5 * 0.016;
            eff.reflect(0, &actual, &effort);

            let entropies: Vec<f64> = agents.iter().map(|a| a.entropy).collect();
            let momenta: Vec<f64> = agents.iter().map(|a| vn(&a.momentum)).collect();
            let threat = emo.fear.min(1.0);
            let unknown = (entropy + 0.2).min(1.0);
            let healing = emo.frustration.min(0.5);
            morpho.differentiate(&entropies, &momenta, threat, unknown, healing, true);
        }

        if t % 50 == 0 {
            let mut brancher = TemporalBrancher::new(4);
            let mut goal = vz(); goal[10] = 5.0;
            brancher.set_goal(goal);
            let base = vz();
            let _r = brancher.branch(&agents[0], &base, &vz());
            branch_calls += 1;
        }

        if let Some(_dr) = dreams.maybe_dream(t, entropy, &mut syn) {
            dream_fires += 1;
        }
        if t % 10 == 0 {
            let outcome = if emo.fear < 0.1 { 0.5 } else { -emo.fear.min(1.0) };
            dreams.record(&[agents[0].pos], &[entropy], outcome, t);
        }

        if t % 100 == 0 {
            let mut goal = vz(); goal[10] = 8.0;
            let _path = world.navigate(&agents[0].pos, &goal, 20);
        }

        if t > 0 && t % CHECK_EVERY == 0 {
            let pain = eff.get_pain(0);
            if !(0.0..=5.0).contains(&pain) { violations += 1; }
            if emo.fear > 5.0 + 1e-9 || emo.fear < 0.0 { violations += 1; }
            let count: usize = [Role::Stem, Role::Navigator, Role::Sentinel,
                               Role::Worker, Role::Scout, Role::Healer]
                .iter().map(|&r| morpho.count_by_role(r)).sum();
            if count != N_AGENTS { violations += 1; }
            if t > 100 && morpho.count_by_role(Role::Stem) > 0 { violations += 1; }
            let expected_min = if t >= 500 { (t - 500) / 500 } else { 0 };
            if dream_fires < expected_min { violations += 1; }
            for a in agents.iter() {
                if !(0.0..=1.0).contains(&a.entropy) { violations += 1; }
            }
            if verbose {
                println!("  [T{}] ok (dreams={} reflex={} branch={} fear={:.3} pain={:.3} ent={:.3})",
                    t, dream_fires, reflex_fires, branch_calls, emo.fear, pain, entropy);
            }
        }
    }

    let elapsed = start.elapsed();
    let outcome = SoakOutcome {
        dream_fires,
        reflex_fires,
        branch_calls,
        violations,
        final_entropy_bits: agents[0].entropy.to_bits(),
        final_fear_bits: emo.fear.to_bits(),
        final_pain_bits: eff.get_pain(0).to_bits(),
        roles: [
            morpho.count_by_role(Role::Stem),
            morpho.count_by_role(Role::Navigator),
            morpho.count_by_role(Role::Sentinel),
            morpho.count_by_role(Role::Worker),
            morpho.count_by_role(Role::Scout),
            morpho.count_by_role(Role::Healer),
        ],
    };
    (outcome, elapsed)
}

fn median_min_max(samples: &mut [f64]) -> (f64, f64, f64) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (samples[samples.len() / 2], samples[0], samples[samples.len() - 1])
}

fn main() {
    println!("OASIS — 11-mechanism soak bench (K={} repeats, {} ticks each)",
             K_REPEATS, TOTAL_TICKS);
    println!("  agents: {}, check interval: {}", N_AGENTS, CHECK_EVERY);
    println!();

    // First run verbose (prints invariant-check every 500 ticks).
    // Subsequent K-1 runs silent (only timed).
    let (first_outcome, first_elapsed) = run_soak(true);
    println!();

    let mut all_elapsed = vec![first_elapsed.as_secs_f64()];
    let mut all_outcomes = vec![first_outcome.clone()];
    for k in 2..=K_REPEATS {
        let (outcome, elapsed) = run_soak(false);
        println!("  run {}: {:.2}s  (dreams={} reflex={} branch={} violations={})",
                 k, elapsed.as_secs_f64(),
                 outcome.dream_fires, outcome.reflex_fires,
                 outcome.branch_calls, outcome.violations);
        all_elapsed.push(elapsed.as_secs_f64());
        all_outcomes.push(outcome);
    }

    // Determinism check: all K functional outcomes MUST be identical.
    for (i, o) in all_outcomes.iter().enumerate() {
        if o != &first_outcome {
            println!();
            println!("⚠️  DETERMINISM VIOLATION at run {}:", i + 1);
            println!("   first: {:?}", first_outcome);
            println!("   this:  {:?}", o);
            std::process::exit(2);
        }
    }

    let (median, min, max) = median_min_max(&mut all_elapsed);
    let half = (max - min) / (2.0 * median) * 100.0;
    let median_hz = TOTAL_TICKS as f64 / median;
    let min_hz = TOTAL_TICKS as f64 / max;
    let max_hz = TOTAL_TICKS as f64 / min;

    println!();
    println!("=== SOAK SUMMARY (deterministic) ===");
    println!("ticks processed:    {}", TOTAL_TICKS);
    println!("dream fires:        {}", first_outcome.dream_fires);
    println!("reflex fires:       {}", first_outcome.reflex_fires);
    println!("branch calls:       {}", first_outcome.branch_calls);
    println!("final entropy:      {:.3}", f64::from_bits(first_outcome.final_entropy_bits));
    println!("final fear:         {:.3}", f64::from_bits(first_outcome.final_fear_bits));
    println!("final pain:         {:.3}", f64::from_bits(first_outcome.final_pain_bits));
    println!("roles:              Stem={} Nav={} Sent={} Work={} Scout={} Heal={}",
             first_outcome.roles[0], first_outcome.roles[1],
             first_outcome.roles[2], first_outcome.roles[3],
             first_outcome.roles[4], first_outcome.roles[5]);
    println!("violations:         {}", first_outcome.violations);
    println!();
    println!("=== K={} TIMING BAND ===", K_REPEATS);
    println!("elapsed median:     {:.3}s  ({:.3}-{:.3}s) ±{:.1}%",
             median, min, max, half);
    println!("tick rate median:   {:.0} Hz  ({:.0}-{:.0} Hz)", median_hz, min_hz, max_hz);
    println!("determinism check:  all {} runs produced identical functional output", K_REPEATS);
    if first_outcome.violations == 0 {
        println!("STATUS:             OK — all invariants held across {} runs × {} ticks",
                 K_REPEATS, TOTAL_TICKS);
        std::process::exit(0);
    } else {
        println!("STATUS:             FAIL — {} invariant violations",
                 first_outcome.violations);
        std::process::exit(1);
    }
}
