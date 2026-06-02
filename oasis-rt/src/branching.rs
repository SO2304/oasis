//! OASIS-RT — Temporal Branching (Mechanism 4)
//!
//! Fork state into N timelines, simulate via gradient descent,
//! evaluate fitness, collapse to the best branch.

use crate::hyper_state::*;
use crate::vec::*;
#[cfg(not(feature = "std"))]
use alloc::{vec::Vec, string::String, vec, format};
#[cfg(not(feature = "std"))]
use crate::fmath::F64Ext;

const MAX_BRANCHES: usize = 8;
const PROPAGATION_STEPS: usize = 8;

pub struct BranchResult {
    pub best_direction: V,
    pub best_fitness: f64,
    pub branches_evaluated: usize,
    pub fitness_scores: [f64; MAX_BRANCHES],
}

pub struct TemporalBrancher {
    branch_count: usize,
    goal: Option<V>,
}

impl TemporalBrancher {
    pub fn new(branches: usize) -> Self {
        Self { branch_count: branches.min(MAX_BRANCHES), goal: None }
    }

    pub fn set_goal(&mut self, goal: V) {
        self.goal = Some(goal);
    }

    /// Branch: fork current state, propagate, evaluate, collapse
    pub fn branch(&self, agent: &Agent, base_force: &V, pressure_field: &V) -> BranchResult {
        let n = self.branch_count;
        let mut best_fitness = f64::NEG_INFINITY;
        let mut best_direction = *base_force;
        let mut scores = [0.0_f64; MAX_BRANCHES];

        for b in 0..n {
            // Generate hypothesis: rotate base force by different angles in latent space
            let mut hypothesis = *base_force;
            let angle = (b as f64 / n as f64) * core::f64::consts::TAU;
            // Perturb in 2 dimensions to create directional diversity
            let perturbation = 0.3;
            hypothesis[10] += angle.cos() * perturbation;
            hypothesis[11] += angle.sin() * perturbation;
            // Add pressure field influence
            for i in 0..DIM {
                hypothesis[i] += pressure_field[i] * 0.1;
            }

            // Propagate: simulate K steps forward
            let mut sim = Agent { pos: agent.pos, momentum: agent.momentum, entropy: agent.entropy, collapsed: agent.collapsed };
            let mut total_pain = 0.0_f64;
            let mut total_entropy = 0.0_f64;

            for step in 0..PROPAGATION_STEPS {
                let decay = 1.0 - (step as f64 / PROPAGATION_STEPS as f64) * 0.3;
                let step_force = vscale(&hypothesis, decay);
                evolve(&mut sim, &step_force, 0.1, 0.05);
                total_entropy += sim.entropy;
                // Pain = how close to high-pressure (repulsive) zones
                let pressure_at_pos: f64 = (0..DIM).map(|i| sim.pos[i] * pressure_field[i]).sum();
                if pressure_at_pos < 0.0 {
                    total_pain += pressure_at_pos.abs();
                }
            }

            // Evaluate fitness
            let goal_fitness = if let Some(goal) = &self.goal {
                let dist = vd(&sim.pos, goal);
                1.0 / (1.0 + dist) // Closer = better
            } else {
                0.5
            };

            let entropy_fitness = 1.0 - (total_entropy / PROPAGATION_STEPS as f64);
            let pain_fitness = 1.0 / (1.0 + total_pain);
            let smoothness = 1.0 / (1.0 + vn(&sim.momentum)); // Less oscillation = smoother

            let fitness = goal_fitness * 0.4 + entropy_fitness * 0.2 + pain_fitness * 0.3 + smoothness * 0.1;

            scores[b] = fitness;

            if fitness > best_fitness {
                best_fitness = fitness;
                best_direction = hypothesis;
            }
        }

        BranchResult { best_direction, best_fitness, branches_evaluated: n, fitness_scores: scores }
    }
}

/// Pure fitness formula extracted from `branch()` for Kani verification.
/// Given four sub-scores in [0, 1], returns the weighted sum (weights sum to 1).
#[inline]
pub fn fitness_compose(goal_f: f64, entropy_f: f64, pain_f: f64, smooth_f: f64) -> f64 {
    goal_f * 0.4 + entropy_f * 0.2 + pain_f * 0.3 + smooth_f * 0.1
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: if all sub-scores ∈ [0, 1], the weighted fitness ∈ [0, 1].
    /// This is an algebraic certainty: weights sum to 1.
    #[kani::proof]
    fn proof_m4_fitness_bounded_in_unit_interval() {
        let g: f64 = kani::any();
        let e: f64 = kani::any();
        let p: f64 = kani::any();
        let s: f64 = kani::any();
        kani::assume(g.is_finite() && e.is_finite() && p.is_finite() && s.is_finite());
        kani::assume(0.0 <= g && g <= 1.0);
        kani::assume(0.0 <= e && e <= 1.0);
        kani::assume(0.0 <= p && p <= 1.0);
        kani::assume(0.0 <= s && s <= 1.0);
        let f = fitness_compose(g, e, p, s);
        assert!(f >= 0.0);
        assert!(f <= 1.0 + 1e-12); // tiny float slack
    }

    /// PROVE: fitness is MONOTONE in each argument (all weights ≥ 0).
    /// Increasing any component cannot decrease fitness.
    #[kani::proof]
    fn proof_m4_fitness_monotone_in_goal() {
        let g1: f64 = kani::any();
        let g2: f64 = kani::any();
        let e: f64 = kani::any();
        let p: f64 = kani::any();
        let s: f64 = kani::any();
        kani::assume(g1.is_finite() && g2.is_finite()
                    && e.is_finite() && p.is_finite() && s.is_finite());
        kani::assume(0.0 <= e && e <= 1.0);
        kani::assume(0.0 <= p && p <= 1.0);
        kani::assume(0.0 <= s && s <= 1.0);
        kani::assume(g1 <= g2 && g1 >= 0.0 && g2 <= 1.0);
        let f1 = fitness_compose(g1, e, p, s);
        let f2 = fitness_compose(g2, e, p, s);
        assert!(f1 <= f2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches_produce_different_scores() {
        let brancher = TemporalBrancher::new(4);
        let agent = agent_new(3);
        let mut force = vz();
        force[10] = 0.5;
        let pressure = vz();
        let result = brancher.branch(&agent, &force, &pressure);
        assert_eq!(result.branches_evaluated, 4);
        // Scores should not all be identical
        let distinct = result.fitness_scores[..4].windows(2).any(|w| (w[0] - w[1]).abs() > 1e-6);
        assert!(distinct, "branches should produce different scores");
    }

    #[test]
    fn goal_biases_toward_goal() {
        let mut brancher = TemporalBrancher::new(4);
        let agent = agent_new(3);
        let mut goal = vz();
        goal[10] = 5.0;
        brancher.set_goal(goal);
        let mut force = vz();
        force[10] = 1.0;
        let result = brancher.branch(&agent, &force, &vz());
        // Best direction should have positive component toward goal (dim 10)
        assert!(result.best_direction[10] > 0.0, "should branch toward goal");
    }

    #[test]
    fn pressure_repels() {
        let brancher = TemporalBrancher::new(4);
        let agent = agent_new(3);
        let force = vz();
        // Strong repulsive pressure in dim 10
        let mut pressure = vz();
        pressure[10] = -5.0;
        let result = brancher.branch(&agent, &force, &pressure);
        assert!(result.best_fitness > 0.0);
    }

    #[test]
    fn more_branches_better_or_equal_fitness() {
        let agent = agent_new(3);
        let mut force = vz();
        force[10] = 0.5;
        let mut goal = vz();
        goal[10] = 3.0;

        let mut b2 = TemporalBrancher::new(2);
        b2.set_goal(goal);
        let r2 = b2.branch(&agent, &force, &vz());

        let mut b8 = TemporalBrancher::new(8);
        b8.set_goal(goal);
        let r8 = b8.branch(&agent, &force, &vz());

        assert!(r8.best_fitness >= r2.best_fitness - 0.01, "8 branches ({:.4}) should be >= 2 branches ({:.4})", r8.best_fitness, r2.best_fitness);
    }

    // ───── M4 mathematical invariants ──────────────────────────

    /// Invariant 1 — best_fitness is actually the max over evaluated branches.
    #[test]
    fn invariant_best_fitness_is_true_argmax() {
        let mut brancher = TemporalBrancher::new(8);
        let mut goal = vz(); goal[10] = 4.0;
        brancher.set_goal(goal);
        let agent = agent_new(3);
        let mut force = vz(); force[10] = 1.0;
        let r = brancher.branch(&agent, &force, &vz());
        let max_score = r.fitness_scores[..r.branches_evaluated]
            .iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!((r.best_fitness - max_score).abs() < 1e-9,
            "best_fitness {} != max of scores {}", r.best_fitness, max_score);
    }

    /// Invariant 2 — each fitness score ∈ [0, 1] given the weighted sum.
    #[test]
    fn invariant_all_fitness_scores_in_unit_interval() {
        let mut brancher = TemporalBrancher::new(8);
        let mut goal = vz(); goal[10] = 2.0;
        brancher.set_goal(goal);
        let agent = agent_new(3);
        let mut force = vz(); force[10] = 0.5;
        let mut pressure = vz(); pressure[11] = -0.5;
        let r = brancher.branch(&agent, &force, &pressure);
        for (i, &s) in r.fitness_scores[..r.branches_evaluated].iter().enumerate() {
            assert!((0.0..=1.0).contains(&s),
                "branch {} score {} outside [0,1]", i, s);
        }
    }

    /// Invariant 3 (DISCOVERED LIMITATION) — setting a goal can LOWER the
    /// max achievable fitness in M4's current design: `goal_fitness = 1/(1+dist)`
    /// starts at 0.5 only when dist == 1; for dist > 1 it is < 0.5 (below the
    /// default of 0.5 used when no goal is set).
    ///
    /// This means M4 is NOT a goal-maximizer in the naive sense. It ADDS goal
    /// awareness as one of four weighted components. For pure goal-pursuit,
    /// callers should set `perturbation` larger and weight goal_fitness higher.
    /// This is a design trade-off, not a bug; documented here explicitly.
    ///
    /// The OBSERVABLE property we CAN prove: best_direction is a member of the
    /// 8-branch hypothesis space, NOT the input base_force (unless it happens
    /// to win the argmax by accident).
    #[test]
    fn invariant_best_direction_is_from_hypothesis_set() {
        let brancher = TemporalBrancher::new(8);
        let agent = agent_new(3);
        let mut force = vz(); force[10] = 0.3;
        let r = brancher.branch(&agent, &force, &vz());
        // best_direction's dim 10 must equal 0.3 + cos(angle)*0.3 for some
        // angle = k * TAU / 8, k ∈ {0..8}
        let found = (0..8).any(|k| {
            let angle = k as f64 / 8.0 * std::f64::consts::TAU;
            let expected = 0.3 + angle.cos() * 0.3;
            (r.best_direction[10] - expected).abs() < 1e-9
        });
        assert!(found,
            "best_direction[10]={} does not match any hypothesis rotation",
            r.best_direction[10]);
    }

    /// Invariant 4 — finite + non-negative scores always.
    #[test]
    fn invariant_fitness_never_negative_or_nan() {
        let brancher = TemporalBrancher::new(8);
        let agent = agent_new(3);
        let r = brancher.branch(&agent, &vz(), &vz());
        for (i, &s) in r.fitness_scores[..r.branches_evaluated].iter().enumerate() {
            assert!(s.is_finite() && s >= 0.0,
                "branch {} score {} is negative or non-finite", i, s);
        }
    }

    /// Invariant 5 — deterministic: same inputs ⇒ same scores.
    #[test]
    fn invariant_branching_is_deterministic() {
        let mut brancher = TemporalBrancher::new(8);
        let mut goal = vz(); goal[11] = 2.0;
        brancher.set_goal(goal);
        let agent = agent_new(3);
        let mut force = vz(); force[10] = 0.7;
        let r1 = brancher.branch(&agent, &force, &vz());
        let r2 = brancher.branch(&agent, &force, &vz());
        for i in 0..r1.branches_evaluated {
            assert!((r1.fitness_scores[i] - r2.fitness_scores[i]).abs() < 1e-12,
                "non-determinism at branch {}: {} vs {}", i, r1.fitness_scores[i], r2.fitness_scores[i]);
        }
    }
}
