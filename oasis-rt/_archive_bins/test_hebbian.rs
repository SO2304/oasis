//! OASIS — Ruthless Hebbian/STDP Validation
//!
//! 7 tests, each designed to PROVE or DESTROY a specific claim:
//!
//! TEST 1: FORMATION — co-activation > threshold → synapse forms
//! TEST 2: NON-FORMATION — co-activation < threshold → NO synapse
//! TEST 3: POTENTIATION — repeated co-activation → weight increases
//! TEST 4: DEPRESSION — inactivity → weight decays toward zero
//! TEST 5: PRUNING — prolonged inactivity → synapse deleted
//! TEST 6: STDP DIRECTIONALITY — pre-before-post → LTP, reverse → LTD
//! TEST 7: REWARD CREDIT — reinforceByReward via eligibility trace
//!
//! Uses REAL sensor data to generate forces. No synthetic data.

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

// ─── Synapse (exact port of TypeScript) ─────────────────────

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

struct SynapticNetwork {
    synapses: Vec<Synapse>,
    tick: u32,
    // Config (matching TypeScript defaults)
    potentiation_rate: f64,
    depression_rate: f64,
    prune_threshold: f64,
    max_weight: f64,
    formation_threshold: f64,
}

#[derive(Debug)]
struct PlasticityEvent {
    kind: &'static str, // FORMED, POTENTIATED, DEPRESSED, PRUNED
    pre: usize,
    post: usize,
    weight_before: f64,
    weight_after: f64,
    tick: u32,
}

// Agent state for synapse update
struct AgentState {
    momentum: V,
    entropy: f64,
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

    fn find(&self, i: usize, j: usize) -> Option<usize> {
        let (a, b) = if i < j { (i, j) } else { (j, i) };
        self.synapses.iter().position(|s| s.pre == a && s.post == b)
    }

    /// Exact port of TypeScript SynapticNetwork.update()
    fn update(&mut self, agents: &[AgentState]) -> Vec<PlasticityEvent> {
        self.tick += 1;
        let mut events = Vec::new();
        let n = agents.len();

        // Phase 1: co-activation
        for i in 0..n {
            for j in (i + 1)..n {
                let mom_a = vn(&agents[i].momentum);
                let mom_b = vn(&agents[j].momentum);

                if mom_a < 0.01 && mom_b < 0.01 {
                    continue;
                }

                let co_act = if mom_a > 0.01 && mom_b > 0.01 { vcos(&agents[i].momentum, &agents[j].momentum) } else { 0.0 };

                let avg_entropy = (agents[i].entropy + agents[j].entropy) / 2.0;
                let plasticity = 1.0 - (avg_entropy - 0.4).abs() * 2.0;
                let eff_plasticity = plasticity.max(0.1);

                if let Some(idx) = self.find(i, j) {
                    let syn = &mut self.synapses[idx];
                    let w_before = syn.weight;

                    // STDP: read dt BEFORE updating (fixed sign convention)
                    let dt = syn.post_fire_tick as i64 - syn.pre_fire_tick as i64;
                    if mom_a > 0.1 {
                        syn.pre_fire_tick = self.tick;
                    }
                    if mom_b > 0.1 {
                        syn.post_fire_tick = self.tick;
                    }

                    let stdp_tau = 5.0_f64;
                    let stdp = if dt == 0 {
                        1.0
                    } else if dt > 0 {
                        (-(dt as f64) / stdp_tau).exp()
                    } else {
                        -0.5 * ((dt as f64) / stdp_tau).exp()
                    };

                    if co_act.abs() > 0.1 {
                        let delta = co_act * self.potentiation_rate * eff_plasticity * (1.0 + stdp);
                        syn.weight = (syn.weight + delta).clamp(-self.max_weight, self.max_weight);
                        syn.activation_count += 1;
                        syn.last_activated = self.tick;
                        syn.eligibility = (syn.eligibility + 0.3).min(1.0);

                        if (syn.weight - w_before).abs() > 0.001 {
                            events.push(PlasticityEvent { kind: "POTENTIATED", pre: i, post: j, weight_before: w_before, weight_after: syn.weight, tick: self.tick });
                        }
                    }
                    syn.eligibility *= 0.9;
                } else if co_act.abs() > self.formation_threshold {
                    self.synapses.push(Synapse {
                        pre: i,
                        post: j,
                        weight: co_act * 0.3,
                        activation_count: 1,
                        last_activated: self.tick,
                        formed_at: self.tick,
                        conduction_axis: if mom_a > mom_b { agents[i].momentum } else { agents[j].momentum },
                        pre_fire_tick: if mom_a > 0.1 { self.tick } else { 0 },
                        post_fire_tick: if mom_b > 0.1 { self.tick } else { 0 },
                        eligibility: 0.3,
                    });
                    events.push(PlasticityEvent { kind: "FORMED", pre: i, post: j, weight_before: 0.0, weight_after: co_act * 0.3, tick: self.tick });
                }
            }
        }

        // Phase 2: depression + pruning
        let tick = self.tick;
        let dr = self.depression_rate;
        let pt = self.prune_threshold;
        let mut to_remove = Vec::new();
        for (idx, syn) in self.synapses.iter_mut().enumerate() {
            let since = tick - syn.last_activated;
            if since > 0 {
                let w_before = syn.weight;
                syn.weight *= 1.0 - dr * since as f64 * 0.1;
                if syn.weight.abs() < pt {
                    events.push(PlasticityEvent { kind: "PRUNED", pre: syn.pre, post: syn.post, weight_before: w_before, weight_after: 0.0, tick });
                    to_remove.push(idx);
                } else if (syn.weight - w_before).abs() > 0.001 {
                    events.push(PlasticityEvent { kind: "DEPRESSED", pre: syn.pre, post: syn.post, weight_before: w_before, weight_after: syn.weight, tick });
                }
            }
        }
        for idx in to_remove.into_iter().rev() {
            self.synapses.remove(idx);
        }

        events
    }

    fn reinforce_by_reward(&mut self, agent: usize, reward: f64) -> u32 {
        let mut modified = 0;
        for syn in &mut self.synapses {
            if syn.pre != agent && syn.post != agent {
                continue;
            }
            if syn.eligibility < 0.01 {
                continue;
            }
            let delta = reward * syn.eligibility * self.potentiation_rate;
            syn.weight = (syn.weight + delta).clamp(-self.max_weight, self.max_weight);
            modified += 1;
        }
        modified
    }
}

// ─── Sensor ─────────────────────────────────────────────────

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

fn read_accel_gyro() -> Option<([f64; 3], [f64; 3])> {
    let o = Command::new("termux-sensor").args(["-s", "LSM6DSVTR Accelerometer,LSM6DSVTR Gyroscope", "-n", "1"]).output().ok()?;
    let d: SD = serde_json::from_slice(&o.stdout).ok()?;
    let a = d.a.as_ref()?;
    let g = d.g.as_ref()?;
    Some(([*a.values.first()?, *a.values.get(1)?, *a.values.get(2)?], [*g.values.first()?, *g.values.get(1)?, *g.values.get(2)?]))
}

fn accel_to_momentum(accel: [f64; 3], gyro: [f64; 3], scale_factor: f64) -> V {
    let mut m = vz();
    m[10] = accel[0] * scale_factor;
    m[11] = accel[1] * scale_factor;
    m[12] = (accel[2] - 9.81) * scale_factor;
    m[13] = gyro[0] * scale_factor * 3.0;
    m[14] = gyro[1] * scale_factor * 3.0;
    m[15] = gyro[2] * scale_factor * 3.0;
    m
}

// ─── Test framework ─────────────────────────────────────────

const G: &str = "\x1b[32m";
const R: &str = "\x1b[31m";
const Y: &str = "\x1b[33m";
const C: &str = "\x1b[36m";
const X: &str = "\x1b[0m";

struct TestResult {
    name: &'static str,
    pass: bool,
    details: Vec<String>,
}
fn pass(name: &'static str, details: Vec<String>) -> TestResult {
    TestResult { name, pass: true, details }
}
fn fail(name: &'static str, details: Vec<String>) -> TestResult {
    TestResult { name, pass: false, details }
}

// ─── Main ───────────────────────────────────────────────────

fn main() {
    println!("{C}╔══════════════════════════════════════════════════════════════╗");
    println!("║  OASIS — Ruthless Hebbian/STDP Synaptic Validation           ║");
    println!("║  7 tests. Real sensor data. No mercy.                        ║");
    println!("╚══════════════════════════════════════════════════════════════╝{X}\n");

    // Read real sensor data for force generation
    println!("📡 Reading real sensor data...");
    let (accel, gyro) = match read_accel_gyro() {
        Some(v) => v,
        None => {
            println!("{R}FATAL: Cannot read sensors{X}");
            std::process::exit(1);
        }
    };
    println!("  Accel: [{:.3}, {:.3}, {:.3}]", accel[0], accel[1], accel[2]);
    println!("  Gyro:  [{:.5}, {:.5}, {:.5}]\n", gyro[0], gyro[1], gyro[2]);

    let mut results: Vec<TestResult> = Vec::new();

    // ═══════════════════════════════════════════════════════
    // TEST 1: FORMATION
    // ═══════════════════════════════════════════════════════
    println!("{Y}── TEST 1: Synapse Formation ──{X}");
    println!("  Claim: co-activation > 0.5 cosine similarity → synapse forms");
    {
        let mut net = SynapticNetwork::new();
        // Two agents with IDENTICAL momentum → cosine = 1.0
        let mom = accel_to_momentum(accel, gyro, 1.0);
        let agents = vec![
            AgentState { momentum: mom, entropy: 0.4 },
            AgentState { momentum: mom, entropy: 0.4 }, // identical
        ];
        let events = net.update(&agents);
        let formed = events.iter().filter(|e| e.kind == "FORMED").count();

        if formed == 1 && net.synapses.len() == 1 {
            let w = net.synapses[0].weight;
            results.push(pass(
                "T1: Formation",
                vec![format!("Synapse formed: weight={:.4}", w), format!("cosine(mom_A, mom_B) = 1.0 > threshold 0.5 ✓"), format!("Initial weight = coActivation * 0.3 = {:.4} ✓", w)],
            ));
        } else {
            results.push(fail("T1: Formation", vec![format!("Expected 1 synapse, got {}", net.synapses.len())]));
        }
    }

    // ═══════════════════════════════════════════════════════
    // TEST 2: NON-FORMATION
    // ═══════════════════════════════════════════════════════
    println!("{Y}── TEST 2: Non-Formation ──{X}");
    println!("  Claim: |co-activation| < 0.5 → NO synapse; opposite → inhibitory");
    {
        // Part A: Opposite momenta → cosine=-1.0, |cosine|=1.0 > 0.5
        // This SHOULD form an INHIBITORY synapse (weight < 0). Correct behavior.
        let mut net = SynapticNetwork::new();
        let mom_a = accel_to_momentum(accel, gyro, 1.0);
        let mut mom_b = vz();
        for i in 0..DIM {
            mom_b[i] = -mom_a[i];
        }

        let agents = vec![AgentState { momentum: mom_a, entropy: 0.4 }, AgentState { momentum: mom_b, entropy: 0.4 }];
        let events = net.update(&agents);
        let formed = events.iter().filter(|e| e.kind == "FORMED").count();
        let cos = vcos(&mom_a, &mom_b);
        let inhibitory = net.synapses.first().map(|s| s.weight < 0.0).unwrap_or(false);

        // Part B: Orthogonal momenta → cosine≈0, |cosine| < 0.5 → NO synapse
        let mut mom_c = vz();
        mom_c[50] = 1.0;
        mom_c[51] = 1.0;
        let agents2 = vec![AgentState { momentum: mom_a, entropy: 0.4 }, AgentState { momentum: mom_c, entropy: 0.4 }];
        let mut net2 = SynapticNetwork::new();
        net2.update(&agents2);
        let cos2 = vcos(&mom_a, &mom_c);
        let no_ortho_synapse = net2.synapses.is_empty();

        if formed == 1 && inhibitory && no_ortho_synapse {
            results.push(pass(
                "T2: Non-Formation",
                vec![
                    format!("Opposite momenta: inhibitory synapse (weight={:.4}) ✓", net.synapses[0].weight),
                    format!("cosine={:.4}, |cosine|={:.4} > 0.5 → inhibitory ✓", cos, cos.abs()),
                    format!("Orthogonal: cosine={:.4}, no synapse formed ✓", cos2),
                ],
            ));
        } else {
            results
                .push(fail("T2: Non-Formation", vec![format!("Opposite: formed={}, inhibitory={}", formed, inhibitory), format!("Orthogonal: cosine={:.4}, synapses={}", cos2, net2.synapses.len())]));
        }
    }

    // ═══════════════════════════════════════════════════════
    // TEST 3: POTENTIATION
    // ═══════════════════════════════════════════════════════
    println!("{Y}── TEST 3: Potentiation ──{X}");
    println!("  Claim: repeated co-activation → weight monotonically increases");
    {
        let mut net = SynapticNetwork::new();
        let mom = accel_to_momentum(accel, gyro, 1.0);
        let agents = vec![AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }];

        let mut weights: Vec<f64> = Vec::new();
        for t in 0..20 {
            net.update(&agents);
            if let Some(s) = net.synapses.first() {
                weights.push(s.weight);
            }
        }

        let monotonic = weights.windows(2).all(|w| w[1] >= w[0] - 1e-9);
        let increased = weights.last().unwrap_or(&0.0) > weights.first().unwrap_or(&0.0);
        let w0 = weights.first().copied().unwrap_or(0.0);
        let wn = weights.last().copied().unwrap_or(0.0);

        if monotonic && increased && weights.len() >= 15 {
            results.push(pass(
                "T3: Potentiation",
                vec![
                    format!("Weight: {:.4} → {:.4} over {} ticks", w0, wn, weights.len()),
                    format!("Monotonic increase: ✓"),
                    format!("Growth: {:.1}x", wn / w0.max(0.001)),
                    format!("Trajectory: {}", weights.iter().map(|w| format!("{:.3}", w)).collect::<Vec<_>>().join(" → ")),
                ],
            ));
        } else {
            results.push(fail("T3: Potentiation", vec![format!("Monotonic: {}, Increased: {}, Ticks: {}", monotonic, increased, weights.len()), format!("Weights: {:?}", weights)]));
        }
    }

    // ═══════════════════════════════════════════════════════
    // TEST 4: DEPRESSION
    // ═══════════════════════════════════════════════════════
    println!("{Y}── TEST 4: Depression ──{X}");
    println!("  Claim: inactivity → weight decays toward zero");
    {
        let mut net = SynapticNetwork::new();
        let mom = accel_to_momentum(accel, gyro, 1.0);

        // Form synapse with 5 ticks of co-activation
        for _ in 0..5 {
            net.update(&[AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }]);
        }
        let weight_before = net.synapses.first().map(|s| s.weight).unwrap_or(0.0);

        // Now agents stop moving (zero momentum) for 20 ticks
        let mut weights_decay: Vec<f64> = vec![weight_before];
        for _ in 0..20 {
            net.update(&[AgentState { momentum: vz(), entropy: 0.4 }, AgentState { momentum: vz(), entropy: 0.4 }]);
            if let Some(s) = net.synapses.first() {
                weights_decay.push(s.weight);
            } else {
                weights_decay.push(0.0); // pruned
                break;
            }
        }

        let wn = *weights_decay.last().unwrap_or(&0.0);
        let decayed = wn < weight_before;
        let monotonic_decay = weights_decay.windows(2).all(|w| w[1] <= w[0] + 1e-9);

        if decayed && monotonic_decay {
            results.push(pass(
                "T4: Depression",
                vec![format!("Weight: {:.4} → {:.4}", weight_before, wn), format!("Decay: {:.1}% of original", (wn / weight_before) * 100.0), format!("Monotonic decay: ✓")],
            ));
        } else {
            results.push(fail("T4: Depression", vec![format!("Before: {:.4}, After: {:.4}", weight_before, wn), format!("Decayed: {}, Monotonic: {}", decayed, monotonic_decay)]));
        }
    }

    // ═══════════════════════════════════════════════════════
    // TEST 5: PRUNING
    // ═══════════════════════════════════════════════════════
    println!("{Y}── TEST 5: Pruning ──{X}");
    println!("  Claim: prolonged inactivity → synapse deleted when weight < 0.05");
    {
        let mut net = SynapticNetwork::new();
        let mom = accel_to_momentum(accel, gyro, 1.0);

        // Form synapse
        net.update(&[AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }]);
        let initial_count = net.synapses.len();

        // Decay until pruned
        let mut ticks_to_prune = 0u32;
        for t in 0..500 {
            net.update(&[AgentState { momentum: vz(), entropy: 0.4 }, AgentState { momentum: vz(), entropy: 0.4 }]);
            if net.synapses.is_empty() {
                ticks_to_prune = t + 1;
                break;
            }
        }

        if initial_count == 1 && net.synapses.is_empty() {
            results.push(pass("T5: Pruning", vec![format!("Synapse pruned after {} ticks of inactivity ✓", ticks_to_prune), format!("Threshold: weight < {}", net.prune_threshold)]));
        } else {
            let w = net.synapses.first().map(|s| s.weight).unwrap_or(0.0);
            results.push(fail("T5: Pruning", vec![format!("Initial: {}, After 500 ticks: {}, weight={:.6}", initial_count, net.synapses.len(), w)]));
        }
    }

    // ═══════════════════════════════════════════════════════
    // TEST 6: STDP DIRECTIONALITY
    // ═══════════════════════════════════════════════════════
    println!("{Y}── TEST 6: STDP Directionality ──{X}");
    println!("  Claim: pre-before-post → LTP (strengthen), post-before-pre → LTD (weaken)");
    {
        let mom = accel_to_momentum(accel, gyro, 1.0);
        let silent = vz();

        // Scenario A: Agent 0 fires FIRST, then Agent 1 fires next tick
        // pre fires before post → dt > 0 → LTP → stronger weight
        let mut net_ltp = SynapticNetwork::new();
        // First: both fire to form synapse
        net_ltp.update(&[AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }]);
        let w_formed = net_ltp.synapses.first().map(|s| s.weight).unwrap_or(0.0);

        // Now alternate: A fires alone, then B fires alone
        for _ in 0..5 {
            // A fires, B silent → preFireTick updated
            net_ltp.update(&[AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: silent, entropy: 0.4 }]);
            // B fires, A silent → postFireTick updated, dt = preFireTick - postFireTick > 0 → LTP
            net_ltp.update(&[AgentState { momentum: silent, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }]);
            // Both fire → apply STDP with dt > 0
            net_ltp.update(&[AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }]);
        }
        let w_ltp = net_ltp.synapses.first().map(|s| s.weight).unwrap_or(0.0);

        // Scenario B: Agent 1 fires FIRST, then Agent 0
        // post fires before pre → dt < 0 → LTD → weaker weight
        let mut net_ltd = SynapticNetwork::new();
        net_ltd.update(&[AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }]);

        for _ in 0..5 {
            // B fires first, A silent → postFireTick updated
            net_ltd.update(&[AgentState { momentum: silent, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }]);
            // A fires, B silent → preFireTick updated, dt = preFireTick - postFireTick < 0 → LTD
            net_ltd.update(&[AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: silent, entropy: 0.4 }]);
            // Both fire → apply STDP with dt < 0
            net_ltd.update(&[AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }]);
        }
        let w_ltd = net_ltd.synapses.first().map(|s| s.weight).unwrap_or(0.0);

        println!("  LTP path weight: {:.4} → {:.4} (pre-before-post)", w_formed, w_ltp);
        println!("  LTD path weight: {:.4} → {:.4} (post-before-pre)", w_formed, w_ltd);
        println!("  Delta: LTP={:.4}, LTD={:.4}", w_ltp - w_formed, w_ltd - w_formed);

        if w_ltp > w_ltd {
            results.push(pass(
                "T6: STDP Directionality",
                vec![
                    format!("LTP weight: {:.4} (pre→post causal)", w_ltp),
                    format!("LTD weight: {:.4} (post→pre anti-causal)", w_ltd),
                    format!("LTP > LTD: {:.4} > {:.4} ✓", w_ltp, w_ltd),
                    format!("Difference: {:.4} ({:.1}%)", w_ltp - w_ltd, ((w_ltp - w_ltd) / w_ltd.abs().max(0.001)) * 100.0),
                    format!("STDP creates directional learning ✓"),
                ],
            ));
        } else {
            results.push(fail("T6: STDP Directionality", vec![format!("LTP={:.4} should be > LTD={:.4}", w_ltp, w_ltd), format!("STDP did NOT create directional difference")]));
        }
    }

    // ═══════════════════════════════════════════════════════
    // TEST 7: REWARD CREDIT ASSIGNMENT
    // ═══════════════════════════════════════════════════════
    println!("{Y}── TEST 7: Reward Credit Assignment ──{X}");
    println!("  Claim: reinforceByReward modifies weights via eligibility trace");
    {
        let mut net = SynapticNetwork::new();
        let mom = accel_to_momentum(accel, gyro, 1.0);

        // Form and activate synapse
        for _ in 0..3 {
            net.update(&[AgentState { momentum: mom, entropy: 0.4 }, AgentState { momentum: mom, entropy: 0.4 }]);
        }
        let w_before = net.synapses.first().map(|s| s.weight).unwrap_or(0.0);
        let elig_before = net.synapses.first().map(|s| s.eligibility).unwrap_or(0.0);

        // Apply positive reward to agent 0
        let modified = net.reinforce_by_reward(0, 0.8);
        let w_reward = net.synapses.first().map(|s| s.weight).unwrap_or(0.0);

        // Apply negative reward (pain)
        let w_before_pain = w_reward;
        net.reinforce_by_reward(0, -0.5);
        let w_pain = net.synapses.first().map(|s| s.weight).unwrap_or(0.0);

        if modified > 0 && w_reward > w_before && w_pain < w_before_pain {
            results.push(pass(
                "T7: Reward Credit",
                vec![
                    format!("Eligibility trace: {:.4}", elig_before),
                    format!("Positive reward: {:.4} → {:.4} (delta: +{:.4})", w_before, w_reward, w_reward - w_before),
                    format!("Negative reward: {:.4} → {:.4} (delta: {:.4})", w_before_pain, w_pain, w_pain - w_before_pain),
                    format!("Reward strengthens, pain weakens ✓"),
                    format!("Modified {} synapse(s) via eligibility trace ✓", modified),
                ],
            ));
        } else {
            results.push(fail(
                "T7: Reward Credit",
                vec![format!("Modified: {}, w_before={:.4}, w_reward={:.4}, w_pain={:.4}", modified, w_before, w_reward, w_pain), format!("Eligibility: {:.4}", elig_before)],
            ));
        }
    }

    // ═══════════════════════════════════════════════════════
    // SCORECARD
    // ═══════════════════════════════════════════════════════
    println!("\n{C}╔══════════════════════════════════════════════════════════════╗");
    println!("║           HEBBIAN/STDP VALIDATION SCORECARD                  ║");
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
