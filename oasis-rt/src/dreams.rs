//! OASIS-RT — Dream Consolidation (Mechanism 8)
//!
//! When idle: replay experiences, strengthen good synapses,
//! weaken bad ones, imagine counterfactuals.

use crate::synapse::SynapticNetwork;
use crate::vec::*;
#[cfg(not(feature = "std"))]
use alloc::{vec::Vec, string::String, vec, format};

const MAX_EXPERIENCES: usize = 64;
const MAX_TRAJECTORY: usize = 16;

#[derive(Clone)]
pub struct ExperienceTrace {
    pub trajectory: [V; MAX_TRAJECTORY],
    pub entropy_trajectory: [f64; MAX_TRAJECTORY],
    pub len: usize,
    pub outcome: f64, // [-1, 1]: positive = good, negative = bad
    pub tick: u32,
    pub active: bool,
}

pub struct DreamResult {
    pub replayed: u32,
    pub strengthened: u32,
    pub weakened: u32,
    pub imagined: u32,
}

pub struct DreamEngine {
    experiences: Vec<ExperienceTrace>,
    dream_count: u32,
    /// EMA of recent entropy. `maybe_dream` uses this to trigger on RELATIVE
    /// entropy drops (quiescence) rather than an absolute threshold. Fixes the
    /// long-run daemon bug where entropy floated at ~0.64 and a hard threshold
    /// of 0.5 blocked dreams forever.
    entropy_ema: f64,
    ema_alpha: f64,
    last_dream_tick: u32,
    /// Hard ceiling: dream at least this often regardless of entropy.
    max_interval: u32,
    /// Opportunistic trigger: current entropy <= ema - relative_drop.
    relative_drop: f64,
    /// Minimum ticks between opportunistic triggers.
    min_interval: u32,
}

impl DreamEngine {
    pub fn new() -> Self {
        Self {
            experiences: Vec::new(),
            dream_count: 0,
            entropy_ema: 0.5,
            ema_alpha: 0.02,
            last_dream_tick: 0,
            max_interval: 500,
            relative_drop: 0.1,
            min_interval: 50,
        }
    }

    /// Record an experience trace (trajectory + outcome)
    pub fn record(&mut self, trajectory: &[V], entropies: &[f64], outcome: f64, tick: u32) {
        let len = trajectory.len().min(MAX_TRAJECTORY);
        let mut traj = [vz(); MAX_TRAJECTORY];
        let mut etraj = [0.0; MAX_TRAJECTORY];
        for i in 0..len {
            traj[i] = trajectory[i];
            etraj[i] = entropies.get(i).copied().unwrap_or(0.5);
        }

        if self.experiences.len() >= MAX_EXPERIENCES {
            // Evict lowest-magnitude outcome
            if let Some(idx) = self
                .experiences
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.outcome.abs().partial_cmp(&b.1.outcome.abs()).unwrap())
                .map(|(i, _)| i)
            {
                self.experiences[idx] = ExperienceTrace { trajectory: traj, entropy_trajectory: etraj, len, outcome, tick, active: true };
                return;
            }
        }
        self.experiences.push(ExperienceTrace { trajectory: traj, entropy_trajectory: etraj, len, outcome, tick, active: true });
    }

    /// Dream: consolidate when idle. Returns what was learned.
    pub fn dream(&mut self, synapses: &mut SynapticNetwork) -> DreamResult {
        self.dream_count += 1;
        let mut result = DreamResult { replayed: 0, strengthened: 0, weakened: 0, imagined: 0 };

        // Sort by outcome magnitude (most impactful first)
        self.experiences.sort_by(|a, b| b.outcome.abs().partial_cmp(&a.outcome.abs()).unwrap_or(core::cmp::Ordering::Equal));

        // Replay top experiences
        let replay_count = self.experiences.len().min(10);
        for i in 0..replay_count {
            let exp = &self.experiences[i];
            if !exp.active || exp.len < 2 {
                continue;
            }

            result.replayed += 1;

            // Reinforce synapses connected to agents involved
            // Positive outcome → strengthen, negative → weaken
            let reward = exp.outcome * 0.3;
            for agent_idx in 0..synapses.len.min(32) {
                if !synapses.synapses[agent_idx].active {
                    continue;
                }
                let s = &mut synapses.synapses[agent_idx];
                let old_w = s.weight;
                s.weight = (s.weight + reward * s.eligibility).clamp(-1.0, 1.0);
                if s.weight > old_w + 0.001 {
                    result.strengthened += 1;
                }
                if s.weight < old_w - 0.001 {
                    result.weakened += 1;
                }
            }
        }

        // Imagine: generate counterfactual by inverting a negative experience
        for exp in &self.experiences {
            if !exp.active || exp.outcome >= 0.0 {
                continue;
            }
            if exp.len < 2 {
                continue;
            }
            // "What if the opposite had happened?"
            // This pre-conditions synapses for the alternative
            for agent_idx in 0..synapses.len.min(32) {
                if !synapses.synapses[agent_idx].active {
                    continue;
                }
                let s = &mut synapses.synapses[agent_idx];
                // Slight nudge in opposite direction of the bad outcome
                s.weight = (s.weight - exp.outcome * 0.05 * s.eligibility).clamp(-1.0, 1.0);
            }
            result.imagined += 1;
            if result.imagined >= 5 {
                break;
            }
        }

        result
    }

    pub fn experience_count(&self) -> usize {
        self.experiences.iter().filter(|e| e.active).count()
    }

    pub fn dream_count(&self) -> u32 {
        self.dream_count
    }

    /// Feed current entropy; update EMA. Does NOT trigger a dream.
    pub fn observe_entropy(&mut self, current_entropy: f64) {
        self.entropy_ema = self.entropy_ema * (1.0 - self.ema_alpha)
                         + current_entropy * self.ema_alpha;
    }

    /// Decide adaptively if we should dream now. Rules:
    /// 1. FORCE: (tick - last_dream_tick) >= max_interval → dream regardless
    /// 2. OPPORTUNISTIC: current_entropy + relative_drop <= ema
    ///    AND (tick - last_dream_tick) >= min_interval → dream
    /// 3. Otherwise: don't dream.
    ///
    /// This fixes the "absolute threshold blocks forever" bug. Guarantees:
    /// - At least one dream every `max_interval` ticks (upper-bounded latency)
    /// - Dreams preferentially triggered at quiescence (ents drop below mean)
    pub fn should_dream(&self, tick: u32, current_entropy: f64) -> bool {
        should_dream_pure(
            tick, self.last_dream_tick, current_entropy,
            self.entropy_ema, self.max_interval, self.min_interval,
            self.relative_drop,
        )
    }

    /// Combined entry point: observe entropy, check trigger, run dream + return.
    /// Returns None if no trigger fired.
    pub fn maybe_dream(&mut self, tick: u32, current_entropy: f64,
                      synapses: &mut SynapticNetwork) -> Option<DreamResult>
    {
        self.observe_entropy(current_entropy);
        if !self.should_dream(tick, current_entropy) {
            return None;
        }
        let r = self.dream(synapses);
        self.last_dream_tick = tick;
        Some(r)
    }

    #[cfg(test)]
    pub fn entropy_ema(&self) -> f64 { self.entropy_ema }
}

/// Pure boolean function extracted from `should_dream` so Kani can verify it.
/// Returns true iff a dream should fire given the trigger conditions.
#[inline]
pub fn should_dream_pure(
    tick: u32, last_dream_tick: u32,
    current_entropy: f64, entropy_ema: f64,
    max_interval: u32, min_interval: u32,
    relative_drop: f64,
) -> bool {
    let since = tick.saturating_sub(last_dream_tick);
    if since >= max_interval { return true; }
    if since >= min_interval && current_entropy + relative_drop <= entropy_ema {
        return true;
    }
    false
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: if (tick - last_dream_tick) >= max_interval, should_dream is ALWAYS true.
    /// This is the force-trigger guarantee: dream latency is upper-bounded by max_interval.
    #[kani::proof]
    fn proof_m8_force_trigger_upper_bound() {
        let tick: u32 = kani::any();
        let last: u32 = kani::any();
        let max: u32 = kani::any();
        let min: u32 = kani::any();
        let e_cur: f64 = kani::any();
        let e_ema: f64 = kani::any();
        let drop: f64 = kani::any();
        // Reasonable input constraints (u32 unbounded, just assume some sanity)
        kani::assume(e_cur.is_finite() && e_ema.is_finite() && drop.is_finite());
        kani::assume(tick >= last && tick - last >= max); // the force condition
        let decision = should_dream_pure(tick, last, e_cur, e_ema, max, min, drop);
        assert!(decision, "force interval reached but should_dream returned false");
    }

    /// PROVE: if since == 0, should_dream is ALWAYS false (no immediate re-trigger).
    /// Guards against accidental re-dream on the same tick after `maybe_dream` updated last_dream_tick.
    #[kani::proof]
    fn proof_m8_no_retrigger_at_same_tick() {
        let tick: u32 = kani::any();
        let last = tick; // same tick
        let e_cur: f64 = kani::any();
        let e_ema: f64 = kani::any();
        let drop: f64 = kani::any();
        let max: u32 = kani::any();
        let min: u32 = kani::any();
        kani::assume(e_cur.is_finite() && e_ema.is_finite() && drop.is_finite());
        // Require max >= 1 and min >= 1 so "since 0" doesn't trigger either branch
        kani::assume(max >= 1 && min >= 1);
        let decision = should_dream_pure(tick, last, e_cur, e_ema, max, min, drop);
        assert!(!decision, "same-tick retrigger must be blocked");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synapse::{AgentMomentum, SynapticNetwork};

    fn setup_net_with_synapse() -> SynapticNetwork {
        let mut net = SynapticNetwork::new();
        let mut m = vz();
        m[10] = 1.0;
        m[11] = 0.5;
        let mom = AgentMomentum { momentum: m, entropy: 0.4 };
        for _ in 0..5 {
            net.update(&[mom.clone(), mom.clone()]);
        }
        net
    }

    #[test]
    fn records_experiences() {
        let mut engine = DreamEngine::new();
        let traj = [vz(), vz()];
        engine.record(&traj, &[0.5, 0.4], 0.8, 1);
        assert_eq!(engine.experience_count(), 1);
    }

    #[test]
    fn dream_strengthens_on_positive() {
        let mut engine = DreamEngine::new();
        let mut net = setup_net_with_synapse();
        let w_before = net.synapses[0].weight;
        let traj = [vz(), vz()];
        engine.record(&traj, &[0.5, 0.4], 0.9, 1); // positive outcome
        let result = engine.dream(&mut net);
        assert!(result.replayed > 0);
        assert!(result.strengthened > 0);
    }

    #[test]
    fn dream_weakens_on_negative() {
        let mut engine = DreamEngine::new();
        let mut net = setup_net_with_synapse();
        let traj = [vz(), vz()];
        engine.record(&traj, &[0.5, 0.4], -0.9, 1); // negative outcome
        let result = engine.dream(&mut net);
        assert!(result.replayed > 0);
        assert!(result.weakened > 0 || result.imagined > 0);
    }

    #[test]
    fn counterfactual_imagination() {
        let mut engine = DreamEngine::new();
        let mut net = setup_net_with_synapse();
        let traj = [vz(), vz()];
        engine.record(&traj, &[0.5, 0.4], -0.8, 1);
        let result = engine.dream(&mut net);
        assert!(result.imagined > 0, "should imagine counterfactuals for negative experiences");
    }

    #[test]
    fn evicts_weakest_when_full() {
        let mut engine = DreamEngine::new();
        let traj = [vz(), vz()];
        for i in 0..MAX_EXPERIENCES {
            engine.record(&traj, &[0.5], i as f64 * 0.01, i as u32);
        }
        assert_eq!(engine.experience_count(), MAX_EXPERIENCES);
        // Add a strong one — should evict weakest
        engine.record(&traj, &[0.5], 1.0, 999);
        assert_eq!(engine.experience_count(), MAX_EXPERIENCES);
    }

    // ───── M8 adaptive-trigger invariants (FIX bug: absolute threshold blocks forever) ──

    /// Invariant 1 — at sustained high entropy (0.65, above old 0.5 threshold),
    /// dreams STILL fire within max_interval ticks. Old code: zero dreams forever.
    #[test]
    fn invariant_dreams_fire_at_sustained_high_entropy() {
        let mut engine = DreamEngine::new();
        let mut net = setup_net_with_synapse();
        let traj = [vz(), vz()];
        engine.record(&traj, &[0.6], 0.5, 0);
        let mut dream_ticks = 0u32;
        let entropy = 0.65; // WOULD block old absolute-threshold trigger
        // 2000 ticks with max_interval=500 ⇒ forced fires at t=500, 1000, 1500, 2000
        for t in 0..=2000u32 {
            if engine.maybe_dream(t, entropy, &mut net).is_some() {
                dream_ticks += 1;
            }
        }
        assert!(dream_ticks >= 4,
            "expected ≥4 dreams at high entropy over 2000 ticks (max_interval=500), got {}",
            dream_ticks);
    }

    /// Invariant 2 — at stable entropy, EMA tracks it (EMA is an unbiased estimator).
    #[test]
    fn invariant_ema_tracks_stable_entropy() {
        let mut engine = DreamEngine::new();
        let stable = 0.65;
        for _ in 0..500 {
            engine.observe_entropy(stable);
        }
        assert!((engine.entropy_ema() - stable).abs() < 0.01,
            "EMA should converge to stable input {}, got {}", stable, engine.entropy_ema());
    }

    /// Invariant 3 — opportunistic trigger fires on entropy DROP relative to mean.
    #[test]
    fn invariant_opportunistic_triggers_on_entropy_drop() {
        let mut engine = DreamEngine::new();
        let mut net = setup_net_with_synapse();
        let traj = [vz(), vz()];
        engine.record(&traj, &[0.5], 0.5, 0);
        // Warm the EMA to ~0.7
        for t in 0..200u32 {
            engine.observe_entropy(0.7);
            // Run at least min_interval ticks so opportunistic is eligible
            if t >= 60 {
                engine.maybe_dream(t, 0.7, &mut net); // at-mean, should NOT trigger
            }
        }
        let last_before = engine.dream_count;
        // Now drop entropy to 0.4 (below ema 0.7 by > relative_drop 0.1) → TRIGGER
        let triggered = engine.maybe_dream(250, 0.4, &mut net);
        assert!(triggered.is_some(),
            "entropy drop from ema=0.7 to 0.4 MUST trigger opportunistic dream");
        assert!(engine.dream_count > last_before);
    }

    /// Invariant 4 — min_interval rate-limits: two entropy drops 10 ticks apart
    /// don't both trigger.
    #[test]
    fn invariant_min_interval_rate_limits_opportunistic() {
        let mut engine = DreamEngine::new();
        let mut net = setup_net_with_synapse();
        let traj = [vz(), vz()];
        engine.record(&traj, &[0.5], 0.5, 0);
        // Warm EMA
        for _ in 0..200 { engine.observe_entropy(0.7); }
        // First drop triggers
        assert!(engine.maybe_dream(100, 0.4, &mut net).is_some());
        // Second drop 10 ticks later — too soon (min_interval = 50)
        assert!(engine.maybe_dream(110, 0.4, &mut net).is_none(),
            "min_interval must rate-limit consecutive opportunistic triggers");
        // Third drop 60 ticks later — allowed
        assert!(engine.maybe_dream(161, 0.4, &mut net).is_some());
    }

    /// Invariant 5 — after a forced dream, last_dream_tick is updated so
    /// subsequent calls don't re-trigger until the next interval boundary.
    #[test]
    fn invariant_force_trigger_updates_last_dream_tick() {
        let mut engine = DreamEngine::new();
        let mut net = setup_net_with_synapse();
        let traj = [vz(), vz()];
        engine.record(&traj, &[0.5], 0.5, 0);
        // Default max_interval=500. At t=500 we should get a force trigger.
        engine.maybe_dream(500, 0.9, &mut net);
        // Immediately after, another call must NOT re-trigger
        assert!(engine.maybe_dream(501, 0.9, &mut net).is_none());
        // But 500 ticks later, force again
        assert!(engine.maybe_dream(1001, 0.9, &mut net).is_some());
    }
}
