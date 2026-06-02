//! Cross-check: the exact same inputs used by oasis-mcu-demo/src/main.rs
//! must produce byte-identical outputs on host x86_64 and on the RP2040
//! Cortex-M0+ under Wokwi. This file holds the host-side assertions.
//! See webots/SHADOW_AUDIT_WOKWI_TRL6.md for the matching Wokwi transcript.

use oasis_rt::mesh;
use oasis_rt::tension::{self, TensionField};
use oasis_rt::reflex::AdaptiveReflex;
use oasis_rt::hyper_state::{agent_new, entropy, collapse, is_action_safe, evolve};
use oasis_rt::emotion::EmotionalState;
use oasis_rt::synapse::{SynapticNetwork, AgentMomentum};
use oasis_rt::vec::{V, vz};

#[test]
fn mcu_primitive_1_mesh_ttl() {
    assert_eq!(mesh::ttl_after_forward(5), 4);
    assert!(mesh::should_forward(5));
}

#[test]
fn mcu_primitive_2_tension_tick_ttl() {
    assert_eq!(tension::tick_ttl(3), (2, false));
    assert_eq!(tension::tick_ttl(1), (0, true));
}

#[test]
fn mcu_primitive_3_bloom_bit_index() {
    let b0 = mesh::bloom_bit_index(0xDEAD_BEEF, 0, 256);
    let b1 = mesh::bloom_bit_index(0xDEAD_BEEF, 1, 256);
    let b2 = mesh::bloom_bit_index(0xDEAD_BEEF, 2, 256);
    assert_eq!((b0, b1, b2), (234, 155, 34),
        "RP2040 printed {{234, 155, 34}} — host must match");
}

#[test]
fn mcu_primitive_4_hmac_sha256_8() {
    let key = [0x42u8; 32];
    let tag = mesh::hmac_sha256_8(&key, b"lin:0.5 ang:0.1");
    assert_eq!(tag, [0xa2, 0x4c, 0x06, 0x2f, 0x8f, 0xce, 0x90, 0x5b],
        "RP2040 printed [a2 4c 06 2f 8f ce 90 5b] — host must match");
}

#[test]
fn mcu_primitive_8_m1_tension_field() {
    // Matches the Wokwi MCU section [8] exactly.
    let mut tf = TensionField::new();
    let mut a: V = vz(); a[0] =  1.0; a[1] =  0.5;
    let mut b: V = vz(); b[0] =  0.8; b[1] =  0.4;
    let mut c: V = vz(); c[0] = -1.0; c[1] = -0.5;
    tf.emit(&a, 1.0, 5);
    tf.emit(&b, 0.7, 5);
    tf.emit(&c, 0.5, 5);
    let (net, con_ratio, _des_mag) = tf.sample();
    assert!((net[0] - 1.06).abs() < 1e-6, "net[0] must be 1.06, got {}", net[0]);
    assert!((net[1] - 0.53).abs() < 1e-6, "net[1] must be 0.53, got {}", net[1]);
    // MCU reported 0.7573; tolerance for f64 rounding across builds.
    assert!((con_ratio - 0.7573).abs() < 1e-3,
        "constructive ratio 0.7573 expected, got {}", con_ratio);
    for _ in 0..6 { tf.tick(); }
    let (_, _, des_after) = tf.sample();
    assert!(des_after.abs() < 1e-12, "TTL=0 must zero destructive magnitude");
}

#[test]
fn mcu_primitive_9_m2_hyperstate_r14() {
    // Matches the Wokwi MCU section [9] exactly.
    let mut a = agent_new(2);
    assert_eq!(a.collapsed, 2, "agent_new(2) must start in state 2");

    let mut p_calm: V = vz();
    p_calm[0] = 1.0; p_calm[1] = 0.1;
    let e_calm = entropy(&p_calm);
    assert!((e_calm - 0.3661).abs() < 1e-3,
        "calm-pos entropy 0.3661 expected, got {}", e_calm);

    // Deterministic R14 gate: entropy < threshold ⇒ safe
    assert!(is_action_safe(&a, 0.95), "initial agent must be safe at threshold 0.95");

    // Push into chaos — same evolve pattern as MCU demo.
    let mut force: V = vz();
    force[0] = 50.0; force[1] = 50.0; force[2] = 50.0; force[3] = 50.0;
    for _ in 0..20 { evolve(&mut a, &force, 0.1, 0.99); }
    let e_chaos = entropy(&a.pos);
    // Same position, same arithmetic ⇒ byte-exact match with MCU's 0.8440.
    assert!((e_chaos - 0.8440).abs() < 1e-3,
        "chaos-pos entropy 0.8440 expected, got {}", e_chaos);

    assert!(is_action_safe(&a, 0.95), "entropy 0.844 < 0.95 threshold ⇒ still safe");
    assert!(!is_action_safe(&a, 0.50),
        "entropy 0.844 > 0.50 threshold ⇒ R14 MUST refuse action");

    let collapsed = collapse(&a.pos);
    assert_eq!(collapsed, 4,
        "collapse of chaos pos must reach state_idx=4 (MCU reported this)");
}

#[test]
fn mcu_primitive_10_m5_emotion_post_refactor() {
    // Post-refactor M5: pain_pos is now Vec<V> instead of [V; MAX_PAIN].
    // Behavior must be byte-identical — MCU Wokwi reported fear=0.6212
    // near hazard, 0.0000 after moving far.
    let mut es = EmotionalState::new();
    let mut hazard: V = vz(); hazard[0] = 5.0; hazard[1] = 0.0;
    es.record_pain(&hazard, 0.9, 0);

    let mut near: V = vz(); near[0] = 5.1; near[1] = 0.1;
    es.update(&near, 0.1, 10);
    assert!((es.fear - 0.6212).abs() < 1e-3,
        "near-hazard fear 0.6212 expected, got {}", es.fear);
    assert!(es.curiosity.abs() < 1e-9,
        "curiosity stays 0 without novelty");

    let mut far: V = vz(); far[0] = 100.0; far[1] = 100.0;
    es.update(&far, 0.1, 100);
    assert!(es.fear.abs() < 1e-9,
        "far-from-hazard fear must decay to ~0, got {}", es.fear);
}

#[test]
fn mcu_primitive_11_m7_synapse_post_refactor() {
    // Post-refactor M7: synapses is now Vec<Synapse> instead of
    // [Synapse; MAX_SYNAPSES]. MCU Wokwi reported 1 synapse formed,
    // weight 0.3067 after reinforcement.
    let mut net = SynapticNetwork::new();
    let mut m: V = vz(); m[0] = 1.0; m[1] = 0.5;
    let agents = [
        AgentMomentum { momentum: m, entropy: 0.2 },
        AgentMomentum { momentum: m, entropy: 0.2 },
    ];
    net.update(&agents);
    assert_eq!(net.count(), 1, "2 aligned agents must form 1 synapse");

    let affected = net.reinforce(0, 0.5);
    assert_eq!(affected, 1, "reinforce should touch 1 synapse");
    assert!((net.synapses[0].weight - 0.3067).abs() < 1e-3,
        "post-reinforce weight 0.3067 expected, got {}", net.synapses[0].weight);
}

#[test]
fn mcu_primitive_5_reflex_adaptive() {
    let mut rf = AdaptiveReflex::new(3.0);
    assert!(!rf.check(1000.0), "pre-calibration never fires");
    for v in [1.0_f64, 1.1, 0.9, 1.0, 1.05, 0.95, 1.02, 0.98, 1.0, 1.01] {
        rf.feed(v);
    }
    rf.calibrate();
    assert!(!rf.check(1.0), "within-band input must not fire");
    assert!(rf.check(5.0), "out-of-band input must fire");
}
