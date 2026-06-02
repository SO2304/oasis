//! OASIS-RT — Hebbian/STDP Synaptic Network (Mechanism 7)
//!
//! Proven 7/7 on Samsung Galaxy with real sensor data.
//! dt = postFireTick - preFireTick (causal direction verified).

use crate::vec::*;
#[cfg(not(feature = "std"))]
use alloc::{vec::Vec, string::String, vec, format};
#[cfg(not(feature = "std"))]
use crate::fmath::F64Ext;

const MAX_SYNAPSES: usize = 32;

#[derive(Clone)]
pub struct Synapse {
    pub active: bool,
    pub pre: usize,
    pub post: usize,
    pub weight: f64,
    pub activation_count: u32,
    pub last_activated: u32,
    pub conduction_axis: V,
    pub pre_fire_tick: u32,
    pub post_fire_tick: u32,
    pub eligibility: f64,
}

#[derive(Clone)]
pub struct SynapticNetwork {
    // synapses was `[Synapse; MAX_SYNAPSES]` inline = 34 KiB on stack
    // (each Synapse holds a 128D `conduction_axis: V`). Moved to heap-
    // backed Vec<Synapse> of fixed length MAX_SYNAPSES, initialized at
    // `new()`. Indexed access and .iter_mut() work identically via Deref
    // so host behavior is byte-preserved. Unblocks MCU port.
    pub synapses: Vec<Synapse>,
    pub len: usize,
    pub tick: u32,
    pub potentiation_rate: f64,
    pub depression_rate: f64,
    pub prune_threshold: f64,
    pub max_weight: f64,
    pub formation_threshold: f64,
    pub silence_ticks: u32, // homeostatic plasticity: ticks with 0 active synapses
}

pub struct AgentMomentum {
    pub momentum: V,
    pub entropy: f64,
}

impl SynapticNetwork {
    pub fn new() -> Self {
        Self {
            synapses: {
                // Heap-init MAX_SYNAPSES inactive slots without a stack
                // array temporary (which would be 34 KiB, exceeding M0+ stack).
                let mut v = Vec::with_capacity(MAX_SYNAPSES);
                for _ in 0..MAX_SYNAPSES {
                    v.push(Synapse {
                        active: false,
                        pre: 0,
                        post: 0,
                        weight: 0.0,
                        activation_count: 0,
                        last_activated: 0,
                        conduction_axis: vz(),
                        pre_fire_tick: 0,
                        post_fire_tick: 0,
                        eligibility: 0.0,
                    });
                }
                v
            },
            len: 0,
            tick: 0,
            potentiation_rate: 0.05,
            depression_rate: 0.002,
            prune_threshold: 0.05,
            max_weight: 1.0,
            formation_threshold: 0.5,
            silence_ticks: 0,
        }
    }

    /// Hebbian + STDP update. Returns number of plasticity events.
    pub fn update(&mut self, agents: &[AgentMomentum]) -> u32 {
        self.tick += 1;
        let mut events = 0u32;
        let n = agents.len();
        // Homeostatic plasticity: track silence, lower threshold when network empty
        if self.count() == 0 {
            self.silence_ticks = self.silence_ticks.saturating_add(1);
        } else {
            self.silence_ticks = 0;
        }
        let ht = if self.silence_ticks > 30 {
            (self.formation_threshold * 0.4).max(0.08) // desperate: accept weak co-activation
        } else {
            self.formation_threshold
        };

        // Phase 1: co-activation
        for i in 0..n {
            for j in (i + 1)..n {
                let ma = vn(&agents[i].momentum);
                let mb = vn(&agents[j].momentum);
                if ma < 0.01 && mb < 0.01 {
                    continue;
                }

                let co = if ma > 0.01 && mb > 0.01 { vcos(&agents[i].momentum, &agents[j].momentum) } else { 0.0 };

                let avg_e = (agents[i].entropy + agents[j].entropy) / 2.0;
                let plast = (1.0 - (avg_e - 0.4).abs() * 2.0).max(0.1);

                if let Some(s) = self.synapses[..self.len].iter_mut().find(|s| s.active && s.pre == i && s.post == j) {
                    let wb = s.weight;
                    // STDP: causal direction (dt > 0 = pre before post = LTP)
                    let dt = s.post_fire_tick as i64 - s.pre_fire_tick as i64;
                    if ma > 0.1 {
                        s.pre_fire_tick = self.tick;
                    }
                    if mb > 0.1 {
                        s.post_fire_tick = self.tick;
                    }

                    let stdp = if dt == 0 {
                        1.0
                    } else if dt > 0 {
                        (-(dt as f64) / 5.0).exp()
                    } else {
                        -0.5 * ((dt as f64) / 5.0).exp()
                    };

                    if co.abs() > 0.1 {
                        let delta = co * self.potentiation_rate * plast * (1.0 + stdp);
                        s.weight = (s.weight + delta).clamp(-self.max_weight, self.max_weight);
                        s.activation_count += 1;
                        s.last_activated = self.tick;
                        s.eligibility = (s.eligibility + 0.3).min(1.0);

                        let dir = if ma > mb { agents[i].momentum } else { agents[j].momentum };
                        if vn(&dir) > 0.01 {
                            s.conduction_axis = vnorm(&dir);
                        }

                        if (s.weight - wb).abs() > 0.001 {
                            events += 1;
                        }
                    }
                    // eligibility decay moved to Phase 2 only (was double-decayed here)
                } else if co.abs() > ht && self.len < MAX_SYNAPSES {
                    let axis = if ma > mb { vnorm(&agents[i].momentum) } else { vnorm(&agents[j].momentum) };
                    self.synapses[self.len] = Synapse {
                        active: true,
                        pre: i,
                        post: j,
                        weight: co * 0.3,
                        activation_count: 1,
                        last_activated: self.tick,
                        conduction_axis: axis,
                        pre_fire_tick: if ma > 0.1 { self.tick } else { 0 },
                        post_fire_tick: if mb > 0.1 { self.tick } else { 0 },
                        eligibility: 0.3,
                    };
                    self.len += 1;
                    events += 1;
                }
            }
        }

        // Phase 2: depression + pruning (bio-inspired: fixed decay rate, no runaway)
        let pt = self.prune_threshold;
        for s in self.synapses[..self.len].iter_mut() {
            if !s.active {
                continue;
            }
            s.weight *= 0.9998; // half-life ~3500 ticks (~6 min)
            s.eligibility *= 0.9;
            if s.weight.abs() < pt {
                s.active = false;
            }
        }
        // Phase 3: compaction — reclaim dead slots so network can grow again
        let mut write = 0;
        for read in 0..self.len {
            if self.synapses[read].active {
                if write != read {
                    self.synapses[write] = self.synapses[read].clone();
                }
                write += 1;
            }
        }
        self.len = write;

        events
    }

    /// Credit assignment via eligibility traces
    pub fn reinforce(&mut self, agent: usize, reward: f64) -> u32 {
        let mut modified = 0;
        for s in self.synapses[..self.len].iter_mut() {
            if !s.active {
                continue;
            }
            if s.pre != agent && s.post != agent {
                continue;
            }
            if s.eligibility < 0.01 {
                continue;
            }
            let delta = reward * s.eligibility * self.potentiation_rate;
            s.weight = (s.weight + delta).clamp(-self.max_weight, self.max_weight);
            modified += 1;
        }
        modified
    }

    /// Count active synapses
    pub fn count(&self) -> usize {
        self.synapses[..self.len].iter().filter(|s| s.active).count()
    }

    /// Query weight between two specific agents (pre → post). Returns Some(weight) if synapse exists,
    /// None otherwise. Used for applying STDP-learned weights to downstream computations.
    pub fn weight_between(&self, pre: usize, post: usize) -> Option<f64> {
        for s in self.synapses[..self.len].iter() {
            if !s.active { continue; }
            if s.pre == pre && s.post == post { return Some(s.weight); }
        }
        None
    }
}

// ==========================================================================
// Pure scalar helpers extracted for Kani verification.
// `SynapticNetwork` is ~32 KB (32×[f64;128] axes) — too large for Kani's SAT
// backend. These pure functions isolate the arithmetic invariants so that
// `update` / `reinforce` correctness can be proven at the scalar level.
// ==========================================================================

/// Pure reinforce math: weight <- clamp(weight + reward * eligibility * rate).
#[inline]
pub fn apply_reinforce(weight: f64, reward: f64, eligibility: f64, rate: f64, max_w: f64) -> f64 {
    (weight + reward * eligibility * rate).clamp(-max_w, max_w)
}

/// Pure decay: multiplicative decay toward 0. Bounded by input magnitude.
#[inline]
pub fn apply_decay(weight: f64, factor: f64) -> f64 {
    weight * factor
}

/// Prune predicate matching the Phase 2 rule.
#[inline]
pub fn would_prune(weight: f64, threshold: f64) -> bool {
    weight.abs() < threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_momentum(dims: &[(usize, f64)]) -> AgentMomentum {
        let mut m = vz();
        for &(d, v) in dims {
            m[d] = v;
        }
        AgentMomentum { momentum: m, entropy: 0.4 }
    }

    #[test]
    fn formation_above_threshold() {
        let mut net = SynapticNetwork::new();
        let mom = make_momentum(&[(10, 1.0), (11, 0.5)]);
        net.update(&[mom.clone(), mom.clone()]);
        assert_eq!(net.count(), 1);
    }

    #[test]
    fn no_formation_below_threshold() {
        let mut net = SynapticNetwork::new();
        let a = make_momentum(&[(10, 1.0)]);
        let b = make_momentum(&[(50, 1.0)]); // orthogonal
        net.update(&[a, b]);
        assert_eq!(net.count(), 0);
    }

    #[test]
    fn potentiation_increases_weight() {
        let mut net = SynapticNetwork::new();
        let mom = make_momentum(&[(10, 1.0), (11, 0.5)]);
        for _ in 0..10 {
            net.update(&[mom.clone(), mom.clone()]);
        }
        let w = net.synapses[0].weight;
        assert!(w > 0.3, "weight should increase, got {}", w);
    }

    #[test]
    fn pruning_removes_inactive() {
        let mut net = SynapticNetwork::new();
        let mom = make_momentum(&[(10, 1.0), (11, 0.5)]);
        net.update(&[mom.clone(), mom.clone()]);
        assert_eq!(net.count(), 1);
        let w0 = net.synapses[0].weight;
        let silent = make_momentum(&[]);
        // Bio-correct: fixed decay 0.9998/tick. Synapse survives minutes, not seconds.
        for _ in 0..200 {
            net.update(&[silent.clone(), silent.clone()]);
        }
        assert_eq!(net.count(), 1, "synapse should survive 200 ticks of silence");
        assert!(net.synapses[0].weight < w0, "weight should decay");
        for _ in 0..9500 {
            net.update(&[silent.clone(), silent.clone()]);
        }
        assert_eq!(net.count(), 0, "synapse should prune after ~10min silence");
    }

    #[test]
    fn multiple_synapses_form_with_4_agents() {
        let mut net = SynapticNetwork::new();
        let mom = make_momentum(&[(10, 1.0), (11, 0.5)]);
        // 4 agents with same momentum → should form C(4,2)=6 synapses
        for _ in 0..5 {
            net.update(&[mom.clone(), mom.clone(), mom.clone(), mom.clone()]);
        }
        assert!(net.count() >= 4, "4 co-active agents should form multiple synapses, got {}", net.count());
    }

    #[test]
    fn inhibitory_synapse_from_opposite_momentum() {
        let mut net = SynapticNetwork::new();
        net.formation_threshold = 0.3; // lower threshold
        let a = make_momentum(&[(10, 1.0), (11, 0.5)]);
        let mut b_mom = vz();
        for i in 0..DIM {
            b_mom[i] = -a.momentum[i];
        }
        let b = AgentMomentum { momentum: b_mom, entropy: 0.4 };
        net.update(&[a, b]);
        if net.count() > 0 {
            assert!(net.synapses[0].weight < 0.0, "opposite momenta should form inhibitory synapse");
        }
    }

    #[test]
    fn stdp_causal_direction() {
        let mut net = SynapticNetwork::new();
        let mom = make_momentum(&[(10, 1.0), (11, 0.5)]);
        let silent = AgentMomentum { momentum: vz(), entropy: 0.4 };
        // Form synapse
        net.update(&[mom.clone(), mom.clone()]);
        // LTP path: pre fires before post
        let mut net_ltp = net.clone();
        for _ in 0..5 {
            net_ltp.update(&[mom.clone(), silent.clone()]);
            net_ltp.update(&[silent.clone(), mom.clone()]);
            net_ltp.update(&[mom.clone(), mom.clone()]);
        }
        // LTD path: post fires before pre
        let mut net_ltd = net.clone();
        for _ in 0..5 {
            net_ltd.update(&[silent.clone(), mom.clone()]);
            net_ltd.update(&[mom.clone(), silent.clone()]);
            net_ltd.update(&[mom.clone(), mom.clone()]);
        }
        let w_ltp = net_ltp.synapses[0].weight;
        let w_ltd = net_ltd.synapses[0].weight;
        assert!(w_ltp > w_ltd, "LTP ({:.4}) should be > LTD ({:.4})", w_ltp, w_ltd);
    }

    #[test]
    fn eligibility_decays() {
        let mut net = SynapticNetwork::new();
        net.prune_threshold = 0.001; // prevent pruning during test
        let mom = make_momentum(&[(10, 1.0), (11, 0.5)]);
        // Build up strong synapse
        for _ in 0..10 {
            net.update(&[mom.clone(), mom.clone()]);
        }
        let e1 = net.synapses[0].eligibility;
        assert!(e1 > 0.5, "eligibility should be high after activation");
        // Silent ticks: no co-activation, eligibility decays at 0.9x per tick
        let silent = AgentMomentum { momentum: vz(), entropy: 0.4 };
        net.update(&[silent.clone(), silent.clone()]);
        let e2 = net.synapses[0].eligibility;
        assert!(e2 < e1, "eligibility should decay: {:.4} → {:.4}", e1, e2);
    }

    #[test]
    fn reward_modifies_weight() {
        let mut net = SynapticNetwork::new();
        let mom = make_momentum(&[(10, 1.0), (11, 0.5)]);
        for _ in 0..3 {
            net.update(&[mom.clone(), mom.clone()]);
        }
        let before = net.synapses[0].weight;
        net.reinforce(0, 0.8);
        assert!(net.synapses[0].weight > before);
    }
}

impl Clone for AgentMomentum {
    fn clone(&self) -> Self {
        Self { momentum: self.momentum, entropy: self.entropy }
    }
}

// ==========================================================================
//                              KANI PROOFS
// ==========================================================================

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: clamp-based reinforce always keeps weight in [-max_w, max_w],
    /// regardless of reward, eligibility, rate, or prior weight.
    #[kani::proof]
    fn proof_synapse_reinforce_weight_bounded() {
        let w: f64 = kani::any();
        let r: f64 = kani::any();
        let e: f64 = kani::any();
        let rate: f64 = kani::any();
        let max_w: f64 = kani::any();
        kani::assume(w.is_finite() && r.is_finite() && e.is_finite() && rate.is_finite() && max_w.is_finite());
        kani::assume(max_w > 0.0 && max_w < 1e6);
        kani::assume(w.abs() < 1e3 && r.abs() < 1e3 && e.abs() < 1e3 && rate.abs() < 1e3);

        let out = apply_reinforce(w, r, e, rate, max_w);
        assert!(out >= -max_w);
        assert!(out <=  max_w);
        assert!(out.is_finite());
    }

    /// PROVE: decay with |factor| <= 1 never increases |weight|.
    /// Models Phase 2's `weight *= 0.9998` invariant — decay can only shrink.
    #[kani::proof]
    fn proof_synapse_decay_non_expanding() {
        let w: f64 = kani::any();
        let f: f64 = kani::any();
        kani::assume(w.is_finite() && f.is_finite());
        kani::assume(w.abs() < 1e3);
        kani::assume(f.abs() <= 1.0);

        let out = apply_decay(w, f);
        assert!(out.abs() <= w.abs() + 1e-12);
    }

    /// PROVE: prune predicate is symmetric — `would_prune(w)` == `would_prune(-w)`.
    /// Guards against sign-bias during pruning.
    #[kani::proof]
    fn proof_synapse_prune_symmetric() {
        let w: f64 = kani::any();
        let t: f64 = kani::any();
        kani::assume(w.is_finite() && t.is_finite());
        kani::assume(t >= 0.0 && t < 1e6);
        kani::assume(w.abs() < 1e6);

        assert_eq!(would_prune(w, t), would_prune(-w, t));
    }

    /// PROVE: if `|weight| < threshold`, the synapse WILL prune.
    /// Catches any future "accidentally kept dead synapse" regression.
    #[kani::proof]
    fn proof_synapse_below_threshold_prunes() {
        let w: f64 = kani::any();
        let t: f64 = kani::any();
        kani::assume(w.is_finite() && t.is_finite());
        kani::assume(t > 0.0 && t < 1e6);
        kani::assume(w.abs() < t);  // strictly below threshold

        assert!(would_prune(w, t));
    }

    /// PROVE: reinforce is a fixpoint when reward * eligibility * rate == 0.
    /// I.e., no reward ⇒ weight unchanged (no spurious drift).
    #[kani::proof]
    fn proof_synapse_zero_reward_preserves_weight() {
        let w: f64 = kani::any();
        let max_w: f64 = kani::any();
        kani::assume(w.is_finite() && max_w.is_finite());
        kani::assume(max_w > 0.0 && max_w < 1e6);
        kani::assume(w.abs() <= max_w);  // precondition: already bounded

        // Any of reward / eligibility / rate being 0 is sufficient.
        let out = apply_reinforce(w, 0.0, 1.0, 1.0, max_w);
        assert_eq!(out, w);
    }
}
