//! OASIS — Phone→PC Propagation Proof
//! Loads phone's real pain memories + tests federation propagation.
//! No live sensor needed. Pure data contagion test.

use oasis_rt::dreams::DreamEngine;
use oasis_rt::emotion::EmotionalState;
use oasis_rt::federation::FederatedMesh;
use oasis_rt::hyper_state::agent_new;
use oasis_rt::synapse::{AgentMomentum, SynapticNetwork};
use oasis_rt::vec::*;

fn main() {
    eprintln!("╔═══════════════════════════════════════════════╗");
    eprintln!("║  OASIS — Phone→PC Propagation Proof           ║");
    eprintln!("╚═══════════════════════════════════════════════╝\n");

    // 1. Load phone's real pain memories
    let mut phone_emo = EmotionalState::new();
    let pain_path = if std::path::Path::new("/tmp/phone-pain.bin").exists() {
        "/tmp/phone-pain.bin"
    } else if std::path::Path::new("/sdcard/oasis-pain.bin").exists() {
        "/sdcard/oasis-pain.bin"
    } else {
        eprintln!("No pain file found");
        return;
    };

    let loaded = phone_emo.load_pain(pain_path).unwrap_or(0);
    eprintln!("  T1: Loaded {} phone pain memories from {}", loaded, pain_path);
    let t1 = loaded > 0;

    // 2. PC agent feels phone's fear (contagion via position proximity)
    let pc_agent = agent_new(3);
    phone_emo.update(&pc_agent.pos, pc_agent.entropy, 1);
    let pc_fear = phone_emo.fear;
    eprintln!("  T2: PC fear from phone trauma = {:.2}%", pc_fear * 100.0);
    let t2 = pc_fear > 0.0;

    // 3. Build PC synapses + harvest into federation
    let mut syn = SynapticNetwork::new();
    syn.formation_threshold = 0.3;
    let mom = AgentMomentum {
        momentum: {
            let mut m = vz();
            m[10] = 1.0;
            m[11] = 0.5;
            m
        },
        entropy: 0.4,
    };
    for _ in 0..15 {
        syn.update(&[mom.clone(), mom.clone(), mom.clone()]);
    }
    let syn_count = syn.count();
    eprintln!("  T3: PC synapses formed = {}", syn_count);
    let t3 = syn_count >= 2;

    // 4. Federation: harvest PC synapses → digests
    let mut fed = FederatedMesh::new();
    let events: Vec<(usize, usize, f64, f64, &str)> = syn.synapses[..syn.len]
        .iter()
        .filter(|s| s.active && s.weight.abs() > 0.03)
        .map(|s| (s.pre, s.post, 0.0, s.weight, "POTENTIATED"))
        .collect();
    let ents = [0.4, 0.4, 0.4];
    let harvested = fed.harvest(&events, &syn.synapses[..syn.len], &ents);
    eprintln!("  T4: Harvested {} digests from {} events", harvested, events.len());
    let t4 = harvested > 0;

    // 5. Save PC digests → file (simulates PC broadcasting to phone)
    let save_path = "/tmp/pc-memory.bin";
    let saved = fed.save(save_path).is_ok();
    let digest_count = fed.digest_count();
    eprintln!("  T5: Saved {} digests to {}", digest_count, save_path);
    let t5 = saved && digest_count > 0;

    // 6. Simulate phone loading PC's digests (foreign merge with trust 0.5)
    let mut phone_fed = FederatedMesh::new();
    let merged = phone_fed.merge_foreign(save_path, 0.5).unwrap_or(0);
    eprintln!("  T6: Phone merged {} foreign digests (trust=0.5)", merged);
    let t6 = merged > 0;

    // 7. Phone propagates PC's experience to its own synapses
    let mut phone_syn = SynapticNetwork::new();
    phone_syn.formation_threshold = 0.3;
    let phone_mom = AgentMomentum {
        momentum: {
            let mut m = vz();
            m[10] = 0.9;
            m[11] = 0.6;
            m
        },
        entropy: 0.5,
    };
    for _ in 0..10 {
        phone_syn.update(&[phone_mom.clone(), phone_mom.clone()]);
    }
    let w_before: Vec<f64> = phone_syn.synapses[..phone_syn.len].iter().filter(|s| s.active).map(|s| s.weight).collect();
    let propagated = phone_fed.propagate(&mut phone_syn.synapses[..phone_syn.len], 2, &[0.5, 0.5]);
    let w_after: Vec<f64> = phone_syn.synapses[..phone_syn.len].iter().filter(|s| s.active).map(|s| s.weight).collect();
    let changed = w_before.iter().zip(&w_after).filter(|(a, b)| (*a - *b).abs() > 0.0001).count();
    eprintln!("  T7: Propagated to {} synapses, {} weight changes", propagated, changed);
    let t7 = propagated > 0 || changed > 0;

    // 8. Dream consolidation on phone using merged experience
    let mut dreams = DreamEngine::new();
    for i in 0..10 {
        let outcome = if phone_emo.fear < 0.5 { 0.3 } else { -0.5 };
        let mut p2 = pc_agent.pos;
        p2[10] += 0.01 * i as f64;
        dreams.record(&[pc_agent.pos, p2], &[0.4, 0.5], outcome, i);
    }
    let dr = dreams.dream(&mut phone_syn);
    eprintln!("  T8: Dream: replayed={} strengthened={} weakened={} imagined={}", dr.replayed, dr.strengthened, dr.weakened, dr.imagined);
    let t8 = dr.replayed > 0;

    // SCORECARD
    eprintln!("\n╔═══════════════════════════════════════════════╗");
    eprintln!("║  PROPAGATION SCORECARD                        ║");
    eprintln!("╚═══════════════════════════════════════════════╝\n");
    let tests = [
        (t1, "T1 Phone pain loaded"),
        (t2, "T2 PC feels phone fear"),
        (t3, "T3 PC synapses formed"),
        (t4, "T4 PC digests harvested"),
        (t5, "T5 PC memory saved"),
        (t6, "T6 Phone merges PC digests"),
        (t7, "T7 Resonance propagates"),
        (t8, "T8 Dream consolidates"),
    ];
    let mut score = 0;
    for (pass, name) in &tests {
        eprintln!("  {} {}", if *pass { "✓" } else { "✗" }, name);
        if *pass {
            score += 1;
        }
    }
    eprintln!("\n  SCORE: {}/8", score);
    eprintln!("  Pain: {} memories | Fear: {:.1}% | Digests: {} | Propagated: {}", loaded, pc_fear * 100.0, digest_count, propagated);
    std::fs::remove_file(save_path).ok();
    if score == 8 {
        eprintln!("\n  ══ PROPAGATION PROVED ══");
    }
    eprintln!("\nDone.");
}
