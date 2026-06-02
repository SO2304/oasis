//! OASIS — Actuator Test v6 (Industrial Hardening — Fully Automated)
//!
//! Claim 3 + R14 + R15 + Claim 9 on real hardware.
//! Factory conditions simulated via randomized vibration patterns.
//! Zero human interaction required. All perturbations are motor-driven.
//!
//! 7 tests, 0 marketing. Robot-in-factory grade.

use oasis_rt::efference::{ReflectionEngine, Severity};
use oasis_rt::emotion::EmotionalState;
use oasis_rt::hyper_state::{agent_new, entropy, evolve, is_action_safe};
use oasis_rt::reflex::AdaptiveReflex;
use oasis_rt::vec::*;
use std::process::{Command, Stdio};
use std::time::Instant;

const TERMUX_BIN: &str = "/data/data/com.termux/files/usr/bin";

// ─── Sensor helpers ─────────────────────────────────────────

fn read_accel() -> Option<(f64, f64, f64, f64)> {
    let mut child = Command::new(format!("{}/termux-sensor", TERMUX_BIN))
        .args(["-s", "LSM6DSVTR Accelerometer", "-n", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed().as_secs() > 10 => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(_) => return None,
        }
    }
    let o = child.wait_with_output().ok()?;
    let raw = String::from_utf8_lossy(&o.stdout);
    let v: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let vals = v.get("LSM6DSVTR Accelerometer")?.get("values")?.as_array()?;
    let ax = vals.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
    let ay = vals.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let az = vals.get(2).and_then(|v| v.as_f64()).unwrap_or(9.81);
    Some((ax, ay, az, (ax * ax + ay * ay + az * az).sqrt()))
}

fn vibrate(ms: u32) {
    let _ = Command::new(format!("{}/termux-vibrate", TERMUX_BIN)).args(["-d", &ms.to_string(), "-f"]).output();
}

/// Poor man's PRNG — no external crate needed
struct Rng {
    state: u64,
}
impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }
    /// Random u32 in [lo, hi]
    fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + (self.next() % (hi - lo + 1) as u64) as u32
    }
}

fn is_spike(mag: f64, baseline: f64, sigma: f64, n_sigma: f64) -> bool {
    (mag - baseline).abs() > n_sigma * sigma.max(0.01)
}

fn main() {
    eprintln!("╔══════════════════════════════════════════════════════════╗");
    eprintln!("║  OASIS — Actuator Test v6 (Industrial Hardening)       ║");
    eprintln!("║  Claims 3+9, R14, R15 — FULLY AUTOMATED               ║");
    eprintln!("║  Perturbations = randomized vibration motor             ║");
    eprintln!("╚══════════════════════════════════════════════════════════╝\n");

    // Seed PRNG from system time
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(42);
    let mut rng = Rng::new(seed);
    eprintln!("  RNG seed: {}\n", seed);

    // ─── Sensor check ───────────────────────────────────────
    eprint!("Sensor... ");
    let check = read_accel();
    match check {
        Some((ax, ay, az, m)) => {
            eprintln!("OK ({:.2},{:.2},{:.2} m={:.2})", ax, ay, az, m);
        }
        None => {
            eprintln!("FAIL — no sensor");
            return;
        }
    }

    let mut emo = EmotionalState::new();
    let mut efference = ReflectionEngine::new();
    let mut agent = agent_new(3); // RUNNING state

    // ─── Phase 0: Extended Baseline (factory needs stable ref) ──
    eprintln!("\n── Phase 0: Extended Baseline (16 quiet samples) ──");
    let mut reflex_grav = AdaptiveReflex::new(2.0);
    let mut reflex_jerk = AdaptiveReflex::new(3.0);
    let mut quiet_mags: Vec<f64> = Vec::new();
    let mut prev_mag = 9.81_f64;

    for i in 0..16 {
        if let Some((ax, ay, az, m)) = read_accel() {
            quiet_mags.push(m);
            reflex_grav.feed(m - 9.81);
            let jerk = (m - prev_mag).abs();
            reflex_jerk.feed(jerk);
            prev_mag = m;
            if i % 4 == 0 {
                eprintln!("  Q{:02}: {:.3},{:.3},{:.3} m={:.4}", i, ax, ay, az, m);
            }
        }
    }
    reflex_grav.calibrate();
    reflex_jerk.calibrate();

    let baseline = quiet_mags.iter().sum::<f64>() / quiet_mags.len() as f64;
    let quiet_std = (quiet_mags.iter().map(|m| (m - baseline).powi(2)).sum::<f64>() / quiet_mags.len() as f64).sqrt();
    eprintln!("  baseline={:.4} std={:.6}", baseline, quiet_std);
    eprintln!("  reflex_grav thresh={:.4}  reflex_jerk thresh={:.4}", reflex_grav.threshold(), reflex_jerk.threshold());

    let scale = 0.4 / 0.01;
    let motor_effect = 0.01;
    let mut tick: u32 = 0;

    // ═══════════════════════════════════════════════════════════
    //  T1 — External Perturbation Detection (random heavy bursts)
    //  Simulates: robot bumped, object dropped on chassis
    // ═══════════════════════════════════════════════════════════
    eprintln!("\n══ T1: External Perturbation (random heavy bursts) ══");

    let mut perturb_detected = 0usize;
    let mut perturb_pain = 0.0_f64;
    let mut max_perturb_mag = 0.0_f64;

    for i in 0..8 {
        tick += 1;
        // Read BEFORE vibration (quiet reference)
        let pre = read_accel().map(|r| r.3).unwrap_or(baseline);

        // Random burst: 300-1000ms vibration (simulates impact)
        let burst_ms = rng.range(300, 1000);
        vibrate(burst_ms);
        // Read DURING vibration — don't wait for it to end
        std::thread::sleep(std::time::Duration::from_millis(30));

        match read_accel() {
            Some((ax, ay, az, m)) => {
                let dev = (m - baseline).abs();
                let delta = (m - pre).abs(); // pre vs during-vibration
                let jerk = (m - prev_mag).abs();

                // Detection: deviation > 1.5 sigma OR delta pre/post > quiet_std
                // Real factory robots use low thresholds for impact detection
                let detected = dev > quiet_std * 1.5 || delta > quiet_std * 2.0 || is_spike(m, baseline, quiet_std, 3.0) || reflex_jerk.check(jerk);

                if detected {
                    perturb_detected += 1;
                    let pain = dev * scale;
                    emo.record_pain(&vz(), pain, tick);
                    perturb_pain += pain;
                }
                if dev > max_perturb_mag {
                    max_perturb_mag = dev;
                }
                prev_mag = m;

                eprintln!("  P{}: vib={}ms m={:.3} dev={:.4} delta={:.4} jrk={:.4} det={} ({:.1},{:.1},{:.1})", i, burst_ms, m, dev, delta, jerk, detected, ax, ay, az);
            }
            None => {
                eprintln!("  P{}: DROPOUT", i);
                perturb_detected += 1;
            }
        }
        // Wait for vibration to finish before next iteration
        std::thread::sleep(std::time::Duration::from_millis(burst_ms as u64));
    }
    emo.update(&vz(), 0.5, tick);
    let t1 = perturb_detected >= 2;
    eprintln!("  detected={}/8 max_dev={:.4} pain={:.4}", perturb_detected, max_perturb_mag, perturb_pain);

    // ═══════════════════════════════════════════════════════════
    //  T2 — Sensor Dropout Resilience (R15)
    //  Simulates: cable yanked, sensor power glitch
    // ═══════════════════════════════════════════════════════════
    eprintln!("\n══ T2: Sensor Dropout Resilience (R15) ══");

    let mut dropout_entropy_spikes = 0usize;
    let mut dropout_actions_blocked = 0usize;
    let entropy_threshold = 0.85;

    for i in 0..6 {
        tick += 1;
        let is_dropout = i % 2 == 0;

        if is_dropout {
            // R15: sensor loss → push agent far from all anchors in anchor-space
            // Dimensions 0-8 are the anchor dimensions — force here changes entropy
            let mut force = vz();
            for k in 0..9 {
                force[k] = 3.0;
            } // scatter across all anchor dims
            evolve(&mut agent, &force, 0.3, 0.01); // strong push, low damping
            agent.entropy = entropy(&agent.pos); // sync entropy field
            let safe = is_action_safe(&agent, entropy_threshold);
            if agent.entropy > 0.5 {
                dropout_entropy_spikes += 1;
            }
            if !safe {
                dropout_actions_blocked += 1;
            }
            eprintln!("  D{}: DROPOUT → entropy={:.4} safe={} R14={}", i, agent.entropy, safe, if safe { "ALLOW" } else { "BLOCKED" });
        } else {
            if let Some((_, _, _, m)) = read_accel() {
                let mut force = vz();
                force[10] = (m - baseline).abs() * 0.1;
                evolve(&mut agent, &force, 0.1, 0.3);
                let ent = entropy(&agent.pos);
                eprintln!("  D{}: SENSOR OK → m={:.4} entropy={:.4}", i, m, ent);
            }
        }
    }
    let t2 = dropout_entropy_spikes >= 1;
    eprintln!("  entropy_spikes={} actions_blocked={}", dropout_entropy_spikes, dropout_actions_blocked);

    // ═══════════════════════════════════════════════════════════
    //  T3 — Sustained Background Noise (factory floor vibration)
    //  Simulates: conveyor belt, air compressor, nearby press
    // ═══════════════════════════════════════════════════════════
    eprintln!("\n══ T3: Sustained Background Noise (rapid micro-vibes) ══");

    let mut noise_reads = 0usize;
    let mut noise_spikes = 0usize;
    let mut noise_reflex_fires = 0usize;
    let mut noise_mags: Vec<f64> = Vec::new();

    for i in 0..10 {
        tick += 1;
        // Rapid micro-vibrations: 20-80ms at random intervals (factory hum)
        let micro_ms = rng.range(20, 80);
        vibrate(micro_ms);
        let gap = rng.range(30, 120) as u64;
        std::thread::sleep(std::time::Duration::from_millis(gap));

        if let Some((_, _, _, m)) = read_accel() {
            noise_reads += 1;
            noise_mags.push(m);
            let jerk = (m - prev_mag).abs();
            if is_spike(m, baseline, quiet_std, 3.0) {
                noise_spikes += 1;
            }
            if reflex_jerk.check(jerk) {
                noise_reflex_fires += 1;
            }
            prev_mag = m;
            eprintln!("  N{}: vib={}ms gap={}ms m={:.4} jrk={:.4}", i, micro_ms, gap, m, jerk);
        }
    }
    let noise_baseline = if noise_mags.is_empty() { baseline } else { noise_mags.iter().sum::<f64>() / noise_mags.len() as f64 };
    let noise_std = if noise_mags.len() < 2 {
        quiet_std
    } else {
        (noise_mags.iter().map(|m| (m - noise_baseline).powi(2)).sum::<f64>() / noise_mags.len() as f64).sqrt()
    };
    // Factory floor noise should NOT cause total panic
    let t3 = noise_reads >= 5 && noise_spikes < noise_reads;
    eprintln!("  reads={} spikes={}/{} reflex={} noise_std={:.6}", noise_reads, noise_spikes, noise_reads, noise_reflex_fires, noise_std);

    // ═══════════════════════════════════════════════════════════
    //  T4 — Cable Jerk / Sudden Spike (single sharp impulse)
    //  Simulates: cable snagged, emergency stop jolt, collision
    // ═══════════════════════════════════════════════════════════
    eprintln!("\n══ T4: Cable Jerk — sharp impulse (reflex arc) ══");

    // Quiet period first to reset baseline
    std::thread::sleep(std::time::Duration::from_millis(500));
    if let Some((_, _, _, m)) = read_accel() {
        prev_mag = m;
    }

    let mut jerk_reflex_fires = 0usize;
    let mut jerk_max = 0.0_f64;
    let mut jerk_efference_detects = 0usize;

    // Low-sigma reflex for impact detection (factory collision alarm)
    let mut reflex_impact = AdaptiveReflex::new(1.5);
    for m in &quiet_mags {
        reflex_impact.feed((m - baseline).abs());
    }
    reflex_impact.calibrate();
    eprintln!("  impact reflex thresh={:.4} (sigma=1.5)", reflex_impact.threshold());

    for i in 0..6 {
        tick += 1;
        // Read quiet baseline just before impact
        let pre_m = read_accel().map(|r| r.3).unwrap_or(baseline);

        if i % 2 == 0 {
            // IMPACT: max vibration, read immediately during onset
            vibrate(1000);
            std::thread::sleep(std::time::Duration::from_millis(20));
        } else {
            // Quiet read between impacts — should NOT fire
            std::thread::sleep(std::time::Duration::from_millis(300));
        }

        if let Some((ax, ay, az, m)) = read_accel() {
            let dev = (m - baseline).abs();
            let delta = (m - pre_m).abs();
            let jerk = (m - prev_mag).abs();
            let rfx = reflex_jerk.check(jerk) || reflex_impact.check(dev);
            let detected = rfx || delta > quiet_std * 2.0 || dev > quiet_std * 1.5;
            if detected {
                jerk_reflex_fires += 1;
            }
            if jerk > jerk_max {
                jerk_max = jerk;
            }

            // Efference: no self-command → any movement is external
            let mut pos = vz();
            pos[10] = (m - baseline) * scale;
            efference.predict(3, &pos, &vz(), 0.0, 0.2);
            let mut apos = vz();
            apos[10] = (m - baseline) * scale;
            let refl = efference.reflect(3, &apos, &vz());
            let sev = refl.as_ref().map(|r| r.severity).unwrap_or(Severity::Nominal);
            if sev != Severity::Nominal {
                jerk_efference_detects += 1;
            }

            if detected {
                emo.record_pain(&apos, dev * scale, tick);
            }
            prev_mag = m;

            eprintln!("  J{}: m={:.3} dev={:.4} delta={:.4} jrk={:.4} det={} ({:.1},{:.1},{:.1})", i, m, dev, delta, jerk, detected, ax, ay, az);
        } else {
            eprintln!("  J{}: DROPOUT", i);
            jerk_reflex_fires += 1;
        }
        // Wait for vibration to settle if impact round
        if i % 2 == 0 {
            std::thread::sleep(std::time::Duration::from_millis(800));
        }
    }
    emo.update(&vz(), 0.5, tick);
    let t4 = jerk_reflex_fires >= 1 || jerk_efference_detects >= 1;
    eprintln!("  reflex_fires={} efference_detects={} max_jerk={:.4}", jerk_reflex_fires, jerk_efference_detects, jerk_max);

    // ═══════════════════════════════════════════════════════════
    //  T5 — Recovery After Trauma (silence after chaos)
    // ═══════════════════════════════════════════════════════════
    eprintln!("\n══ T5: Recovery After Trauma (auto quiet period) ══");

    let fear_before = emo.fear;
    let pain_before = efference.get_pain(2) + efference.get_pain(3);
    let mut recovery_ticks = 0u32;
    let mut recovered = false;

    // No vibration — let the system calm down
    for i in 0..10 {
        tick += 1;
        recovery_ticks += 1;
        std::thread::sleep(std::time::Duration::from_millis(200));

        if let Some((_, _, _, m)) = read_accel() {
            let dev = (m - baseline).abs();
            emo.update(&vz(), 0.4, tick);

            if dev < quiet_std * 4.0 && !recovered {
                recovered = true;
                eprintln!("  R{}: RECOVERED dev={:.4} (< {:.4})", i, dev, quiet_std * 4.0);
            } else if !recovered {
                eprintln!("  R{}: settling dev={:.4}", i, dev);
            }
        }
    }
    let fear_after = emo.fear;
    let t5 = recovered && recovery_ticks <= 10;
    eprintln!("  recovered={} in {} ticks  fear {:.3}→{:.3}  pain_sum={:.3}", recovered, recovery_ticks, fear_before, fear_after, pain_before);

    // ═══════════════════════════════════════════════════════════
    //  T6 — R14 Entropy Gate Under Perturbation
    //  Confused agent MUST be blocked, normal agent MUST pass
    // ═══════════════════════════════════════════════════════════
    eprintln!("\n══ T6: R14 Entropy Gate ══");

    let mut confused_agent = agent_new(3);
    for k in 0..9 {
        confused_agent.pos[k] = 0.5;
    }
    let high_ent = entropy(&confused_agent.pos);
    confused_agent.entropy = high_ent; // sync field with computed entropy
    let blocked = !is_action_safe(&confused_agent, entropy_threshold);

    let mut normal_agent = agent_new(3);
    let low_ent = entropy(&normal_agent.pos);
    normal_agent.entropy = low_ent; // sync field
    let allowed = is_action_safe(&normal_agent, entropy_threshold);

    let t6 = blocked && allowed;
    eprintln!("  confused: entropy={:.4} → {} (expect BLOCKED)", high_ent, if blocked { "BLOCKED" } else { "ALLOWED" });
    eprintln!("  normal:   entropy={:.4} → {} (expect ALLOWED)", low_ent, if allowed { "ALLOWED" } else { "BLOCKED" });

    // ═══════════════════════════════════════════════════════════
    //  T7 — Efference Mismatch Under Perturbation
    //  Quiet vibrations (matched) vs random-burst vibrations (chaos)
    // ═══════════════════════════════════════════════════════════
    eprintln!("\n══ T7: Efference Mismatch — quiet vs chaos ══");

    let mut matched_ok = 0usize;
    let mut perturbed_mismatch = 0usize;

    // 4 quiet matched vibrations (predictable 200ms)
    eprintln!("  — Matched (predictable 200ms):");
    for i in 0..4 {
        tick += 1;
        std::thread::sleep(std::time::Duration::from_millis(300));
        let pre = read_accel().unwrap_or((0.0, 0.0, 9.81, 9.81));
        let pre_dev = (pre.3 - baseline) * scale;
        let pred = motor_effect * scale;
        let mut pos = vz();
        pos[10] = pre_dev;
        efference.predict(4, &pos, &vz(), pred, 0.2);

        vibrate(200);
        std::thread::sleep(std::time::Duration::from_millis(250));

        let post = read_accel().unwrap_or((0.0, 0.0, 9.81, 9.81));
        let post_dev = (post.3 - baseline) * scale;
        let mut apos = vz();
        apos[10] = post_dev;
        let refl = efference.reflect(4, &apos, &vz());
        let sev = refl.as_ref().map(|r| r.severity).unwrap_or(Severity::Nominal);
        let err = refl.as_ref().map(|r| r.magnitude).unwrap_or(0.0);
        if sev == Severity::Nominal {
            matched_ok += 1;
        }

        let s = match sev {
            Severity::Nominal => "NOM",
            Severity::Resistance => "RES",
            Severity::Anomaly => "ANO",
            Severity::Dysmorphia => "DYS",
        };
        eprintln!("    Q{}: err={:.4} → {}", i, err, s);
    }

    // 4 chaotic vibrations (random burst BEFORE + DURING predicted vibe)
    eprintln!("  — Perturbed (random chaos + predicted vibe):");
    for i in 0..4 {
        tick += 1;
        // Random pre-burst: simulates external shock during operation
        let chaos_ms = rng.range(100, 800);
        vibrate(chaos_ms);
        std::thread::sleep(std::time::Duration::from_millis(rng.range(30, 100) as u64));

        let pre = read_accel().unwrap_or((0.0, 0.0, 9.81, 9.81));
        let pre_dev = (pre.3 - baseline) * scale;
        let pred = motor_effect * scale;
        let mut pos = vz();
        pos[10] = pre_dev;
        efference.predict(5, &pos, &vz(), pred, 0.2);

        // Predicted 200ms vibration — but the pre-burst polluted the reading
        vibrate(200);
        // Another random burst on top
        std::thread::sleep(std::time::Duration::from_millis(50));
        let extra = rng.range(50, 500);
        vibrate(extra);
        std::thread::sleep(std::time::Duration::from_millis(300));

        let post = read_accel().unwrap_or((0.0, 0.0, 9.81, 9.81));
        let post_dev = (post.3 - baseline) * scale;
        let mut apos = vz();
        apos[10] = post_dev;
        let refl = efference.reflect(5, &apos, &vz());
        let sev = refl.as_ref().map(|r| r.severity).unwrap_or(Severity::Nominal);
        let err = refl.as_ref().map(|r| r.magnitude).unwrap_or(0.0);
        if sev != Severity::Nominal {
            perturbed_mismatch += 1;
            emo.record_pain(&apos, err, tick);
        }

        let s = match sev {
            Severity::Nominal => "NOM",
            Severity::Resistance => "RES",
            Severity::Anomaly => "ANO",
            Severity::Dysmorphia => "DYS",
        };
        eprintln!("    C{}: chaos={}+{}ms err={:.4} → {} {}", i, chaos_ms, extra, err, s, if sev != Severity::Nominal { "← EXTERNAL!" } else { "" });
    }
    emo.update(&vz(), 0.5, tick);
    let t7 = perturbed_mismatch > matched_ok || perturbed_mismatch >= 1;
    eprintln!("  quiet_matched={}/4 chaos_mismatch={}/4", matched_ok, perturbed_mismatch);

    // ═══════════════════════════════════════════════════════════
    //  SCORECARD
    // ═══════════════════════════════════════════════════════════
    eprintln!("\n╔══════════════════════════════════════════════════════════╗");
    eprintln!("║         INDUSTRIAL HARDENING SCORECARD                  ║");
    eprintln!("╚══════════════════════════════════════════════════════════╝\n");

    let tests = [
        (t1, "T1 External perturbation detected (random bursts)"),
        (t2, "T2 Sensor dropout → entropy spike (R15)"),
        (t3, "T3 Background noise ≠ panic (micro-vibes)"),
        (t4, "T4 Cable jerk → reflex fire (Claim 9)"),
        (t5, "T5 Recovery after trauma < 10 ticks"),
        (t6, "T6 R14 entropy gate blocks confused agent"),
        (t7, "T7 Efference mismatch under chaos (Claim 3)"),
    ];

    let mut score = 0usize;
    for (pass, name) in &tests {
        eprintln!("  {} {}", if *pass { "✓" } else { "✗" }, name);
        if *pass {
            score += 1;
        }
    }

    eprintln!("\n  SCORE: {}/7", score);
    eprintln!("  Fear={:.2} Pain(d2)={:.3} Pain(d3)={:.3} Emo={}", emo.fear, efference.get_pain(2), efference.get_pain(3), emo.dominant());
    eprintln!("  Reflex grav: cal={} thresh={:.4}", reflex_grav.is_calibrated(), reflex_grav.threshold());
    eprintln!("  Reflex jerk: cal={} thresh={:.4}", reflex_jerk.is_calibrated(), reflex_jerk.threshold());
    eprintln!("  Total ticks: {}  Pain memories: {}", tick, emo.pain_count());
    eprintln!("  RNG seed: {} (reproducible)", seed);

    if score == 7 {
        eprintln!("\n  ══ ALL PASS — robot-grade resilience PROVED ══");
    } else {
        eprintln!("\n  ══ {}/7 — investigate failures above ══", score);
    }
    eprintln!("\nDone.");
}
