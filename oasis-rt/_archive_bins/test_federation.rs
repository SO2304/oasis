//! OASIS — Federated Neural Mesh Validation
//!
//! 5 tests to PROVE or DESTROY the Federated Synaptic Resonance:
//!
//! TEST 1: HARVEST — significant plasticity → experience digest emitted
//! TEST 2: RESONANCE — aligned synapses receive sympathetic updates
//! TEST 3: REJECTION — misaligned synapses are NOT affected
//! TEST 4: TRUST GATING — low trust blocks resonance propagation
//! TEST 5: COLLECTIVE ADVANTAGE — federated learns faster than isolated

use serde::Deserialize;
use std::process::Command;

const DIM: usize = 128;
type V = [f64; DIM];

#[inline(always)]
fn vz() -> V {
    [0.0; DIM]
}
#[inline(always)]
fn vn(v: &V) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}
#[inline(always)]
fn vnorm(v: &V) -> V {
    let n = vn(v);
    if n < 1e-12 {
        return vz();
    }
    let mut r = vz();
    for i in 0..DIM {
        r[i] = v[i] / n;
    }
    r
}
#[inline(always)]
fn vcos(a: &V, b: &V) -> f64 {
    let (mut d, mut na, mut nb) = (0.0, 0.0, 0.0);
    for i in 0..DIM {
        d += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let dn = na.sqrt() * nb.sqrt();
    if dn < 1e-12 {
        0.0
    } else {
        d / dn
    }
}

// ─── Synapse (from test_hebbian.rs) ────────────────────────

#[derive(Clone)]
struct Synapse {
    pre: usize,
    post: usize,
    weight: f64,
    activation_count: u32,
    last_activated: u32,
    formed_at: u32,
    conduction_axis: V,
    pre_fire_tick: u32,
    post_fire_tick: u32,
    eligibility: f64,
}

struct AgentState {
    momentum: V,
    entropy: f64,
}

#[derive(Debug)]
struct PlasticityEvent {
    kind: &'static str,
    pre: usize,
    post: usize,
    weight_before: f64,
    weight_after: f64,
    tick: u32,
}

struct SynapticNetwork {
    synapses: Vec<Synapse>,
    tick: u32,
    potentiation_rate: f64,
    depression_rate: f64,
    prune_threshold: f64,
    max_weight: f64,
    formation_threshold: f64,
}

impl SynapticNetwork {
    fn new() -> Self {
        Self {
            synapses: Vec::new(),
            tick: 0,
            potentiation_rate: 0.05,
            depression_rate: 0.002,
            prune_threshold: 0.05,
            max_weight: 1.0,
            formation_threshold: 0.5,
        }
    }

    fn update(&mut self, agents: &[AgentState]) -> Vec<PlasticityEvent> {
        self.tick += 1;
        let mut events = Vec::new();
        let n = agents.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (ma, mb) = (vn(&agents[i].momentum), vn(&agents[j].momentum));
                if ma < 0.01 && mb < 0.01 {
                    continue;
                }
                let co = if ma > 0.01 && mb > 0.01 { vcos(&agents[i].momentum, &agents[j].momentum) } else { 0.0 };
                let avg_e = (agents[i].entropy + agents[j].entropy) / 2.0;
                let plast = (1.0 - (avg_e - 0.4).abs() * 2.0).max(0.1);

                if let Some(s) = self.synapses.iter_mut().find(|s| s.pre == i && s.post == j) {
                    let wb = s.weight;
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
                            events.push(PlasticityEvent { kind: "POTENTIATED", pre: i, post: j, weight_before: wb, weight_after: s.weight, tick: self.tick });
                        }
                    }
                    s.eligibility *= 0.9;
                } else if co.abs() > self.formation_threshold {
                    let axis = if ma > mb { vnorm(&agents[i].momentum) } else { vnorm(&agents[j].momentum) };
                    self.synapses.push(Synapse {
                        pre: i,
                        post: j,
                        weight: co * 0.3,
                        activation_count: 1,
                        last_activated: self.tick,
                        formed_at: self.tick,
                        conduction_axis: axis,
                        pre_fire_tick: if ma > 0.1 { self.tick } else { 0 },
                        post_fire_tick: if mb > 0.1 { self.tick } else { 0 },
                        eligibility: 0.3,
                    });
                    events.push(PlasticityEvent { kind: "FORMED", pre: i, post: j, weight_before: 0.0, weight_after: co * 0.3, tick: self.tick });
                }
            }
        }

        let tick = self.tick;
        let (dr, pt) = (self.depression_rate, self.prune_threshold);
        let mut to_rm = Vec::new();
        for (idx, s) in self.synapses.iter_mut().enumerate() {
            let since = tick - s.last_activated;
            if since > 0 {
                let wb = s.weight;
                s.weight *= 1.0 - dr * since as f64 * 0.1;
                if s.weight.abs() < pt {
                    to_rm.push(idx);
                    events.push(PlasticityEvent { kind: "PRUNED", pre: s.pre, post: s.post, weight_before: wb, weight_after: 0.0, tick });
                }
            }
        }
        for idx in to_rm.into_iter().rev() {
            self.synapses.remove(idx);
        }
        events
    }
}

// ─── Federated Neural Mesh (Rust port) ─────────────────────

struct ExperienceDigest {
    source: usize,
    axis: V,
    magnitude: f64,
    valence: f64, // +1 potentiation, -1 depression
    entropy: f64,
    tick: u32,
    reinforcements: u32,
}

struct FederatedMesh {
    pool: Vec<ExperienceDigest>,
    trust: Vec<(usize, usize, f64)>, // (from, to, trust_level)
    significance_threshold: f64,
    resonance_threshold: f64,
    resonance_gain: f64,
    default_trust: f64,
    tick: u32,
    sympathetic_count: u32,
    rejected_count: u32,
}

impl FederatedMesh {
    fn new() -> Self {
        Self {
            pool: Vec::new(),
            trust: Vec::new(),
            significance_threshold: 0.03,
            resonance_threshold: 0.3,
            resonance_gain: 0.02,
            default_trust: 0.5,
            tick: 0,
            sympathetic_count: 0,
            rejected_count: 0,
        }
    }

    fn get_trust(&self, from: usize, to: usize) -> f64 {
        self.trust.iter().find(|t| t.0 == from && t.1 == to).map(|t| t.2).unwrap_or(self.default_trust)
    }

    fn set_trust(&mut self, from: usize, to: usize, val: f64) {
        if let Some(t) = self.trust.iter_mut().find(|t| t.0 == from && t.1 == to) {
            t.2 = val.clamp(0.0, 1.0);
        } else {
            self.trust.push((from, to, val.clamp(0.0, 1.0)));
        }
    }

    /// HARVEST: extract significant learning events into digests
    fn harvest(&mut self, events: &[PlasticityEvent], synapses: &[Synapse], entropies: &[f64]) -> u32 {
        self.tick += 1;
        let mut count = 0;
        for ev in events {
            if ev.kind != "POTENTIATED" && ev.kind != "DEPRESSED" {
                continue;
            }
            let delta = (ev.weight_after - ev.weight_before).abs();
            if delta < self.significance_threshold {
                continue;
            }
            let syn = synapses.iter().find(|s| s.pre == ev.pre && s.post == ev.post);
            let axis = match syn {
                Some(s) if vn(&s.conduction_axis) > 0.01 => vnorm(&s.conduction_axis),
                _ => continue,
            };
            let entropy = if ev.pre < entropies.len() && ev.post < entropies.len() { (entropies[ev.pre] + entropies[ev.post]) / 2.0 } else { 0.5 };
            // Dedup: merge with existing similar digest
            let existing = self.pool.iter_mut().find(|d| d.source == ev.pre && vcos(&d.axis, &axis) > 0.9);
            if let Some(d) = existing {
                d.reinforcements += 1;
            } else {
                self.pool.push(ExperienceDigest {
                    source: ev.pre,
                    axis,
                    magnitude: delta,
                    valence: if ev.kind == "POTENTIATED" { 1.0 } else { -1.0 },
                    entropy,
                    tick: ev.tick,
                    reinforcements: 1,
                });
            }
            count += 1;
        }
        count
    }

    /// PROPAGATE: apply resonance to aligned synapses of OTHER agents
    fn propagate(&mut self, synapses: &mut [Synapse], agent_count: usize, entropies: &[f64]) -> u32 {
        let mut affected = 0;
        let mut to_remove = Vec::new();

        for di in 0..self.pool.len() {
            let digest = &self.pool[di];
            if self.tick.saturating_sub(digest.tick) > 50 {
                to_remove.push(di);
                continue;
            }

            for si in 0..synapses.len() {
                let syn = &synapses[si];
                // Don't resonate with source agent's own synapses
                if syn.pre == digest.source || syn.post == digest.source {
                    continue;
                }
                if vn(&syn.conduction_axis) < 0.01 {
                    continue;
                }

                let alignment = vcos(&digest.axis, &syn.conduction_axis);
                if alignment.abs() < self.resonance_threshold {
                    self.rejected_count += 1;
                    continue;
                }

                let trust = self.get_trust(digest.source, syn.pre);
                if trust < 0.1 {
                    continue;
                }

                let entropy_match = if syn.pre < entropies.len() { 1.0 - (entropies[syn.pre] - digest.entropy).abs() } else { 0.5 };

                let boost = (1.0 + (digest.reinforcements as f64).log2()).min(3.0);
                let weight_delta = alignment * digest.valence * digest.magnitude * trust * entropy_match * boost * self.resonance_gain;

                synapses[si].weight = (synapses[si].weight + weight_delta).clamp(-1.0, 1.0);
                self.sympathetic_count += 1;
                affected += 1;
            }

            if digest.reinforcements <= 1 {
                to_remove.push(di);
            }
        }

        to_remove.sort_unstable();
        to_remove.dedup();
        for idx in to_remove.into_iter().rev() {
            if idx < self.pool.len() {
                self.pool.remove(idx);
            }
        }
        affected
    }
}

// ─── Sensor ────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct SV {
    values: Vec<f64>,
}
#[derive(Deserialize, Default)]
struct SD {
    #[serde(rename = "LSM6DSVTR Accelerometer")]
    a: Option<SV>,
    #[serde(rename = "LSM6DSVTR Gyroscope")]
    g: Option<SV>,
}

fn read_sensors() -> Option<([f64; 3], [f64; 3])> {
    let o = Command::new("termux-sensor").args(["-s", "LSM6DSVTR Accelerometer,LSM6DSVTR Gyroscope", "-n", "1"]).output().ok()?;
    let d: SD = serde_json::from_slice(&o.stdout).ok()?;
    let a = d.a.as_ref()?;
    let g = d.g.as_ref()?;
    Some(([*a.values.first()?, *a.values.get(1)?, *a.values.get(2)?], [*g.values.first()?, *g.values.get(1)?, *g.values.get(2)?]))
}

fn make_momentum(accel: [f64; 3], gyro: [f64; 3], scale: f64, offset: usize) -> V {
    let mut m = vz();
    m[offset] = accel[0] * scale;
    m[offset + 1] = accel[1] * scale;
    m[offset + 2] = (accel[2] - 9.81) * scale;
    m[offset + 3] = gyro[0] * scale * 3.0;
    m[offset + 4] = gyro[1] * scale * 3.0;
    m[offset + 5] = gyro[2] * scale * 3.0;
    m
}

// ─── Test framework ────────────────────────────────────────

const G: &str = "\x1b[32m";
const R: &str = "\x1b[31m";
const Y: &str = "\x1b[33m";
const C: &str = "\x1b[36m";
const X: &str = "\x1b[0m";

struct TR {
    name: &'static str,
    pass: bool,
    details: Vec<String>,
}
fn pass(n: &'static str, d: Vec<String>) -> TR {
    TR { name: n, pass: true, details: d }
}
fn fail(n: &'static str, d: Vec<String>) -> TR {
    TR { name: n, pass: false, details: d }
}

fn main() {
    println!("{C}╔══════════════════════════════════════════════════════════════╗");
    println!("║  OASIS — Federated Neural Mesh Validation                    ║");
    println!("║  5 tests. Collective learning. No mercy.                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝{X}\n");

    println!("Reading sensors...");
    let (accel, gyro) = match read_sensors() {
        Some(v) => v,
        None => {
            println!("{R}FATAL: Cannot read sensors{X}");
            std::process::exit(1);
        }
    };
    println!("  Accel: [{:.3}, {:.3}, {:.3}]", accel[0], accel[1], accel[2]);
    println!("  Gyro:  [{:.5}, {:.5}, {:.5}]\n", gyro[0], gyro[1], gyro[2]);

    let mut results: Vec<TR> = Vec::new();

    // ═══════════════════════════════════════════════════════
    // TEST 1: HARVEST
    // ═══════════════════════════════════════════════════════
    println!("{Y}── TEST 1: Harvest ──{X}");
    println!("  Claim: significant plasticity events produce experience digests");
    {
        let mut net = SynapticNetwork::new();
        let mut mesh = FederatedMesh::new();
        let mom = make_momentum(accel, gyro, 1.0, 10);
        let agents = vec![AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }];
        // Run several ticks to build up weight changes
        let mut total_harvested = 0u32;
        for _ in 0..10 {
            let events = net.update(&agents);
            let h = mesh.harvest(&events, &net.synapses, &[0.4, 0.4]);
            total_harvested += h;
        }

        if total_harvested > 0 && !mesh.pool.is_empty() {
            let d = &mesh.pool[0];
            results.push(pass(
                "T1: Harvest",
                vec![
                    format!("Harvested {} significant events", total_harvested),
                    format!("Pool size: {} digest(s)", mesh.pool.len()),
                    format!("Digest: axis_norm={:.4}, mag={:.4}, valence={:.0}", vn(&d.axis), d.magnitude, d.valence),
                    format!("Reinforcements: {}", d.reinforcements),
                ],
            ));
        } else {
            results.push(fail("T1: Harvest", vec![format!("Harvested: {}, Pool: {}", total_harvested, mesh.pool.len())]));
        }
    }

    // ═══════════════════════════════════════════════════════
    // TEST 2: SYMPATHETIC RESONANCE
    // ═══════════════════════════════════════════════════════
    println!("{Y}── TEST 2: Sympathetic Resonance ──{X}");
    println!("  Claim: aligned synapses receive sympathetic weight updates");
    {
        let mut net = SynapticNetwork::new();
        let mut mesh = FederatedMesh::new();
        // 4 agents: pair (0,1) and pair (2,3) with SAME momentum direction
        let mom = make_momentum(accel, gyro, 1.0, 10);
        let agents4 =
            vec![AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }];

        // Build synapses for both pairs
        for _ in 0..5 {
            net.update(&agents4);
        }

        // Record weight of synapse (2,3) BEFORE federation
        let w23_before = net.synapses.iter().find(|s| s.pre == 2 && s.post == 3).map(|s| s.weight).unwrap_or(0.0);

        // Harvest events from the learning
        for _ in 0..5 {
            let events = net.update(&agents4);
            mesh.harvest(&events, &net.synapses, &[0.4, 0.4, 0.4, 0.4]);
        }

        // Propagate resonance
        let affected = mesh.propagate(&mut net.synapses, 4, &[0.4, 0.4, 0.4, 0.4]);
        let w23_after = net.synapses.iter().find(|s| s.pre == 2 && s.post == 3).map(|s| s.weight).unwrap_or(0.0);

        let delta = w23_after - w23_before;

        if affected > 0 && delta.abs() > 0.0001 {
            results.push(pass(
                "T2: Sympathetic Resonance",
                vec![
                    format!("Synapse (2,3) weight: {:.4} → {:.4} (delta: {:.4})", w23_before, w23_after, delta),
                    format!("Affected {} synapse(s) via resonance", affected),
                    format!("Sympathetic: {}, Rejected: {}", mesh.sympathetic_count, mesh.rejected_count),
                ],
            ));
        } else {
            results.push(fail("T2: Sympathetic Resonance", vec![format!("Affected: {}, delta: {:.6}", affected, delta), format!("Pool: {}, Synapses: {}", mesh.pool.len(), net.synapses.len())]));
        }
    }

    // ═══════════════════════════════════════════════════════
    // TEST 3: REJECTION (misaligned)
    // ═══════════════════════════════════════════════════════
    println!("{Y}── TEST 3: Rejection ──{X}");
    println!("  Claim: misaligned synapses are NOT affected by resonance");
    {
        let mut net = SynapticNetwork::new();
        let mut mesh = FederatedMesh::new();
        // Pair (0,1) learns in one direction, pair (2,3) in ORTHOGONAL direction
        let mom_a = make_momentum(accel, gyro, 1.0, 10); // dims 10-15
        let mut mom_b = vz();
        mom_b[50] = 1.0;
        mom_b[51] = 0.5;
        mom_b[52] = 0.8; // dims 50-52, orthogonal

        // Build synapse for pair (0,1)
        for _ in 0..8 {
            let events = net.update(&[
                AgentState { momentum: mom_a, entropy: 0.4 },
                AgentState { momentum: mom_a, entropy: 0.4 },
                AgentState { momentum: mom_b, entropy: 0.4 },
                AgentState { momentum: mom_b, entropy: 0.4 },
            ]);
            mesh.harvest(&events, &net.synapses, &[0.4; 4]);
        }

        // Record (2,3) weight before
        let w23_before = net.synapses.iter().find(|s| s.pre == 2 && s.post == 3).map(|s| s.weight).unwrap_or(0.0);

        // Only propagate digests from agent 0 (which learned in dim 10-15)
        // to agent 2's synapses (which are in dim 50-52)
        let rejected_before = mesh.rejected_count;
        mesh.propagate(&mut net.synapses, 4, &[0.4; 4]);

        let w23_after = net.synapses.iter().find(|s| s.pre == 2 && s.post == 3).map(|s| s.weight).unwrap_or(0.0);

        let axis_cos = if let (Some(s01), Some(s23)) = (net.synapses.iter().find(|s| s.pre == 0 && s.post == 1), net.synapses.iter().find(|s| s.pre == 2 && s.post == 3)) {
            vcos(&s01.conduction_axis, &s23.conduction_axis)
        } else {
            0.0
        };

        let delta = (w23_after - w23_before).abs();
        let new_rejections = mesh.rejected_count - rejected_before;

        if delta < 0.001 && new_rejections > 0 {
            results.push(pass(
                "T3: Rejection",
                vec![
                    format!("Orthogonal axis cosine: {:.4} < threshold 0.3", axis_cos),
                    format!("Weight unchanged: {:.4} → {:.4} (delta: {:.6})", w23_before, w23_after, delta),
                    format!("Rejected {} resonance(s)", new_rejections),
                ],
            ));
        } else {
            results.push(fail("T3: Rejection", vec![format!("Axis cosine: {:.4}, delta: {:.6}, rejections: {}", axis_cos, delta, new_rejections)]));
        }
    }

    // ═══════════════════════════════════════════════════════
    // TEST 4: TRUST GATING
    // ═══════════════════════════════════════════════════════
    println!("{Y}── TEST 4: Trust Gating ──{X}");
    println!("  Claim: low trust blocks resonance propagation");
    {
        let mut net = SynapticNetwork::new();
        let mut mesh = FederatedMesh::new();
        let mom = make_momentum(accel, gyro, 1.0, 10);
        let agents =
            vec![AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }];

        // Build synapses
        for _ in 0..8 {
            let events = net.update(&agents);
            mesh.harvest(&events, &net.synapses, &[0.4; 4]);
        }

        // Set trust from agent 0 to agent 2 to ZERO
        mesh.set_trust(0, 2, 0.0);
        mesh.set_trust(1, 2, 0.0);

        let w23_before = net.synapses.iter().find(|s| s.pre == 2 && s.post == 3).map(|s| s.weight).unwrap_or(0.0);

        let symp_before = mesh.sympathetic_count;
        mesh.propagate(&mut net.synapses, 4, &[0.4; 4]);

        let w23_after = net.synapses.iter().find(|s| s.pre == 2 && s.post == 3).map(|s| s.weight).unwrap_or(0.0);

        // Check: agent 0's experience should NOT affect synapse (2,3) due to zero trust
        let delta = (w23_after - w23_before).abs();
        let symp_from_0 = mesh.sympathetic_count - symp_before;

        // With trust=0.0 from 0→2 and 1→2, resonance from 0,1 should not affect 2,3
        if delta < 0.001 {
            results.push(pass(
                "T4: Trust Gating",
                vec![format!("Trust(0→2) = 0.0, Trust(1→2) = 0.0"), format!("Weight unchanged: {:.4} → {:.4}", w23_before, w23_after), format!("Zero-trust blocks resonance completely ✓")],
            ));
        } else {
            results.push(fail("T4: Trust Gating", vec![format!("Delta: {:.6} (should be ~0)", delta), format!("Sympathetic events: {}", symp_from_0)]));
        }
    }

    // ═══════════════════════════════════════════════════════
    // TEST 5: COLLECTIVE ADVANTAGE
    // ═══════════════════════════════════════════════════════
    println!("{Y}── TEST 5: Collective Advantage ──{X}");
    println!("  Claim: federated agents learn faster than isolated");
    {
        let mom = make_momentum(accel, gyro, 1.0, 10);

        // Scenario A: ISOLATED — two separate networks, no federation
        let mut net_iso = SynapticNetwork::new();
        let agents2 = vec![AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }];
        for _ in 0..5 {
            net_iso.update(&agents2);
        }
        let w_iso = net_iso.synapses.first().map(|s| s.weight).unwrap_or(0.0);

        // Scenario B: FEDERATED — 4 agents, mesh propagates experience
        let mut net_fed = SynapticNetwork::new();
        let mut mesh = FederatedMesh::new();
        let agents4 =
            vec![AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }];
        for _ in 0..5 {
            let events = net_fed.update(&agents4);
            mesh.harvest(&events, &net_fed.synapses, &[0.4; 4]);
            mesh.propagate(&mut net_fed.synapses, 4, &[0.4; 4]);
        }
        let w_fed = net_fed.synapses.iter().find(|s| s.pre == 2 && s.post == 3).map(|s| s.weight).unwrap_or(0.0);

        let advantage = w_fed - w_iso;

        if w_fed > w_iso && advantage > 0.001 {
            results.push(pass(
                "T5: Collective Advantage",
                vec![
                    format!("Isolated weight after 5 ticks: {:.4}", w_iso),
                    format!("Federated weight after 5 ticks: {:.4}", w_fed),
                    format!("Advantage: +{:.4} ({:.1}% faster)", advantage, (advantage / w_iso.abs().max(0.001)) * 100.0),
                    format!("Federation accelerates learning ✓"),
                ],
            ));
        } else {
            results.push(fail("T5: Collective Advantage", vec![format!("Isolated: {:.4}, Federated: {:.4}, Advantage: {:.4}", w_iso, w_fed, advantage)]));
        }
    }

    // ═══════════════════════════════════════════════════════
    // SCORECARD
    // ═══════════════════════════════════════════════════════
    println!("\n{C}╔══════════════════════════════════════════════════════════════╗");
    println!("║         FEDERATED NEURAL MESH SCORECARD                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝{X}\n");

    let passed = results.iter().filter(|r| r.pass).count();
    let total = results.len();

    for r in &results {
        let icon = if r.pass { format!("{G}✓ PASS{X}") } else { format!("{R}✗ FAIL{X}") };
        println!("  {} {}", icon, r.name);
        for d in &r.details {
            println!("    {}", d);
        }
        println!();
    }

    println!("  ────────────────────────────────");
    println!("  {G}PASSED: {}{X} / {}", passed, total);
    if passed < total {
        println!("  {R}FAILED: {}{X} / {}", total - passed, total);
    }
    println!("  Score: {}%\n", (passed * 100) / total);
}
