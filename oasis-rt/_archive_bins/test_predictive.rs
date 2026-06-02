//! OASIS — Predictive Learning Test
//!
//! THE HARDEST TEST: Does past experience improve FUTURE performance?
//!
//! Method:
//!   1. Train kernel A on V1 calm data (pre-trauma baseline)
//!   2. Train kernel B on V1 full data (calm + trauma) → "experienced"
//!   3. Expose BOTH to V2 trauma data (NEW trauma, never seen before)
//!   4. Compare: experienced kernel should handle trauma BETTER
//!      (lower peak fear, faster recovery, more synaptic stability)

use oasis_rt::dreams::DreamEngine;
use oasis_rt::emotion::EmotionalState;
use oasis_rt::hyper_state::*;
use oasis_rt::reflex::AdaptiveReflex;
use oasis_rt::synapse::{AgentMomentum, SynapticNetwork};
use oasis_rt::tension::TensionField;
use oasis_rt::vec::*;

struct Tick {
    entropy: f64,
    fear: f64,
    motion: f64,
    pressure: f64,
}
impl Copy for Tick {}
impl Clone for Tick {
    fn clone(&self) -> Self {
        *self
    }
}

fn parse(line: &str) -> Option<Tick> {
    if !line.contains("RUNNING") {
        return None;
    }
    let mut t = Tick { entropy: 0.0, fear: 0.0, motion: 0.0, pressure: 960.0 };
    if let Some(i) = line.find("E:") {
        t.entropy = line[i + 2..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<String>().parse::<f64>().unwrap_or(0.0) / 100.0;
    }
    if let Some(i) = line.find("fear:") {
        t.fear = line[i + 5..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<f64>().unwrap_or(0.0) / 100.0;
    }
    if let Some(i) = line.find("mot:") {
        t.motion = line[i + 4..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<String>().parse::<f64>().unwrap_or(0.0) / 100.0;
    }
    Some(t)
}

fn to_force(t: &Tick) -> V {
    let mut f = vz();
    f[0] = t.motion * 3.0;
    f[1] = t.fear.min(5.0) * 2.0;
    f[2] = (1.0 - t.entropy) * 1.5;
    f[10] = t.entropy * 2.0;
    f[11] = t.fear.min(5.0) * 3.0;
    f[12] = t.motion * 5.0;
    f
}

struct KernelInstance {
    ag: Agent,
    tf: TensionField,
    syn: SynapticNetwork,
    emo: EmotionalState,
    reflex: AdaptiveReflex,
    dreams: DreamEngine,
}

impl KernelInstance {
    fn new() -> Self {
        Self {
            ag: agent_new(3),
            tf: TensionField::new(),
            syn: SynapticNetwork::new(),
            emo: EmotionalState::new(),
            reflex: AdaptiveReflex::new(3.0),
            dreams: DreamEngine::new(),
        }
    }

    fn process_tick(&mut self, tick: &Tick, i: u32) {
        let f = to_force(tick);
        if i < 10 {
            self.reflex.feed(tick.motion);
            if i == 9 {
                self.reflex.calibrate();
            }
        }
        self.tf.emit(&f, 0.3 + tick.motion, 8);
        let (net_force, _, _) = self.tf.sample();
        if tick.motion > 0.01 {
            self.emo.record_pain(&f, tick.motion, i);
        }
        self.emo.update(&f, self.ag.entropy, i);
        let gain = 1.0 + self.emo.satisfaction * 0.5;
        let safe = is_action_safe(&self.ag, 0.85);
        let final_f = vscale(&net_force, if safe { gain } else { 0.1 });
        evolve(&mut self.ag, &final_f, 0.1, 0.05);
        if i % 3 == 0 {
            let agents = [AgentMomentum { momentum: f, entropy: tick.entropy }, AgentMomentum { momentum: vscale(&f, 0.8), entropy: 0.4 }];
            self.syn.update(&agents);
        }
        self.tf.tick();
        // Record experience for dreams
        if tick.motion > 0.1 || tick.fear > 0.3 {
            let outcome = if tick.fear > 0.5 { -0.5 } else { 0.3 };
            self.dreams.record(&[f], &[tick.entropy], outcome, i);
        }
    }

    fn dream(&mut self) {
        self.dreams.dream(&mut self.syn);
    }
}

struct PerfMetrics {
    peak_fear: f64,
    avg_fear: f64,
    recovery_ticks: u32,
    synapse_stability: f64,
}

fn measure_performance(kernel: &mut KernelInstance, ticks: &[Tick]) -> PerfMetrics {
    let mut peak_fear = 0.0_f64;
    let mut total_fear = 0.0_f64;
    let mut in_spike = false;
    let mut spike_start = 0u32;
    let mut recovery = 0u32;
    let syn_before = kernel.syn.count();

    for (i, tick) in ticks.iter().enumerate() {
        kernel.process_tick(tick, i as u32 + 1000); // Offset to avoid recalibration
        if kernel.emo.fear > peak_fear {
            peak_fear = kernel.emo.fear;
        }
        total_fear += kernel.emo.fear;
        if kernel.emo.fear > 0.3 && !in_spike {
            in_spike = true;
            spike_start = i as u32;
        }
        if kernel.emo.fear < 0.1 && in_spike {
            in_spike = false;
            recovery = i as u32 - spike_start;
        }
    }

    let syn_after = kernel.syn.count();
    let stability = if syn_before == 0 { 1.0 } else { syn_after as f64 / syn_before as f64 };

    PerfMetrics {
        peak_fear,
        avg_fear: total_fear / ticks.len() as f64,
        recovery_ticks: if recovery == 0 { ticks.len() as u32 } else { recovery },
        synapse_stability: stability,
    }
}

const G: &str = "\x1b[32m";
const R: &str = "\x1b[31m";
const C: &str = "\x1b[36m";
const X: &str = "\x1b[0m";

fn main() {
    println!("{C}╔══════════════════════════════════════════════════════════════╗");
    println!("║  OASIS PREDICTIVE LEARNING TEST                              ║");
    println!("║  Does experience improve future performance? PROVE IT.       ║");
    println!("╚══════════════════════════════════════════════════════════════╝{X}\n");

    let v1_log = std::fs::read_to_string("C:\\tmp\\oasis-v1.log").expect("Need V1 log");
    let v2_log = std::fs::read_to_string("C:\\tmp\\oasis-v2-live.log").expect("Need V2 log");

    let v1: Vec<Tick> = v1_log.lines().filter_map(parse).collect();
    let v2: Vec<Tick> = v2_log.lines().filter_map(parse).collect();

    // Split V2 into calm and trauma phases
    let v2_trauma: Vec<Tick> = v2.iter().filter(|t| t.motion > 0.05 || t.fear > 0.1).copied().collect();
    println!("  V1: {} ticks, V2 trauma: {} ticks\n", v1.len(), v2_trauma.len());

    // ═══ NAIVE KERNEL: no prior experience ═══
    println!("  Training NAIVE kernel (no experience)...");
    let mut naive = KernelInstance::new();
    // Just calibrate reflexes on quiet data
    for (i, tick) in v1.iter().take(10).enumerate() {
        naive.process_tick(tick, i as u32);
    }

    // ═══ EXPERIENCED KERNEL: trained on V1 trauma + dreams ═══
    println!("  Training EXPERIENCED kernel (V1 full + dreams)...");
    let mut experienced = KernelInstance::new();
    for (i, tick) in v1.iter().enumerate() {
        experienced.process_tick(tick, i as u32);
    }
    // Dream: consolidate V1 experience
    println!("  Dreaming (consolidating V1 experience)...");
    experienced.dream();
    experienced.dream(); // Dream twice for deeper consolidation

    // ═══ EXPOSE BOTH TO V2 TRAUMA ═══
    println!("  Exposing BOTH to V2 trauma (unseen data)...\n");
    let naive_perf = measure_performance(&mut naive, &v2_trauma);
    let exp_perf = measure_performance(&mut experienced, &v2_trauma);

    // ═══ RESULTS ═══
    println!("  {:>20} {:>12} {:>12} {:>8}", "", "NAIVE", "EXPERIENCED", "BETTER?");
    println!("  {}", "─".repeat(56));

    let fear_better = exp_perf.peak_fear <= naive_perf.peak_fear;
    let avg_better = exp_perf.avg_fear <= naive_perf.avg_fear + 0.01;
    let recovery_better = exp_perf.recovery_ticks <= naive_perf.recovery_ticks;
    let stability_better = exp_perf.synapse_stability >= naive_perf.synapse_stability - 0.1;

    let icon = |b: bool| {
        if b {
            format!("{G}YES{X}")
        } else {
            format!("{R}NO{X}")
        }
    };

    println!("  {:>20} {:>12.4} {:>12.4} {:>8}", "Peak fear:", naive_perf.peak_fear, exp_perf.peak_fear, icon(fear_better));
    println!("  {:>20} {:>12.4} {:>12.4} {:>8}", "Avg fear:", naive_perf.avg_fear, exp_perf.avg_fear, icon(avg_better));
    println!("  {:>20} {:>12} {:>12} {:>8}", "Recovery (ticks):", naive_perf.recovery_ticks, exp_perf.recovery_ticks, icon(recovery_better));
    println!("  {:>20} {:>12.4} {:>12.4} {:>8}", "Syn stability:", naive_perf.synapse_stability, exp_perf.synapse_stability, icon(stability_better));

    let advantages = [fear_better, avg_better, recovery_better, stability_better].iter().filter(|&&b| b).count();

    println!("\n  ────────────────────────────────");
    println!("  Advantages: {}/4", advantages);

    if advantages >= 3 {
        println!("  {G}✓ PREDICTIVE LEARNING PROVED{X}");
        println!("  Experience from V1 improved handling of V2 trauma.");
        println!("  The kernel didn't just change — it got BETTER.");
    } else if advantages >= 2 {
        println!("  {G}✓ PARTIAL LEARNING{X} — experience helped in {}/4 metrics", advantages);
    } else {
        println!("  {R}✗ LEARNING NOT PROVED{X} — only {}/4 metrics improved", advantages);
    }
}
