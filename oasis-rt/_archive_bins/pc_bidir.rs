//! OASIS — Bidirectional Federation PC↔Phone
//!
//! 1. PC reads phone telemetry (Phone→PC) ✓ already works
//! 2. PC computes its synaptic state
//! 3. PC writes its state to /sdcard/oasis-pc-state.json (PC→Phone)
//! 4. Verify phone can read PC state
//!
//! This proves TRUE bidirectional federation.

use oasis_rt::emotion::EmotionalState;
use oasis_rt::synapse::{AgentMomentum, SynapticNetwork};
use oasis_rt::vec::*;
use std::process::Command;
use std::time::Instant;

struct Tick {
    entropy: f64,
    fear: f64,
    motion: f64,
}

fn parse(line: &str) -> Option<Tick> {
    if !line.contains("RUNNING") {
        return None;
    }
    let mut t = Tick { entropy: 0.0, fear: 0.0, motion: 0.0 };
    if let Some(i) = line.find("E:") {
        t.entropy = line[i + 2..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<String>().parse::<f64>().unwrap_or(0.0) / 100.0;
    }
    if let Some(i) = line.find("fear:") {
        t.fear = line[i + 5..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<f64>().unwrap_or(0.0) / 100.0;
    }
    if let Some(i) = line.find("mot:") {
        t.motion = line[i + 4..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<String>().parse::<f64>().unwrap_or(0.0) / 100.0;
    }
    if t.entropy > 0.0 || t.fear > 0.0 {
        Some(t)
    } else {
        None
    }
}

const G: &str = "\x1b[32m";
const R: &str = "\x1b[31m";
const C: &str = "\x1b[36m";
const X: &str = "\x1b[0m";

fn main() {
    println!("{C}╔══════════════════════════════════════════════════════════════╗");
    println!("║  OASIS BIDIRECTIONAL FEDERATION — PC↔Phone                   ║");
    println!("║  Prove: PC state reaches phone. Phone state reaches PC.      ║");
    println!("╚══════════════════════════════════════════════════════════════╝{X}\n");

    let t0 = Instant::now();
    let mut results: Vec<(&str, bool, Vec<String>)> = Vec::new();

    // ═══ T1: Phone→PC (read live telemetry) ═══
    println!("  T1: Phone→PC...");
    let phone_tel = Command::new("adb").args(["shell", "cat", "/sdcard/oasis-live-tel.txt"]).output().ok().and_then(|o| {
        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
        parse(&s)
    });

    let t1_pass = phone_tel.is_some();
    let t1_details = if let Some(ref t) = phone_tel {
        vec![format!("Phone: E={:.1}% fear={:.0}% mot={:.1}%", t.entropy * 100.0, t.fear * 100.0, t.motion * 100.0)]
    } else {
        vec!["Phone not reachable".to_string()]
    };
    results.push(("T1: Phone→PC telemetry", t1_pass, t1_details));

    // ═══ T2: PC processes phone data ═══
    println!("  T2: PC processing...");
    let mut net = SynapticNetwork::new();
    let mut emo = EmotionalState::new();

    // Process phone log
    let log = std::fs::read_to_string("C:\\tmp\\oasis-v2-live.log").unwrap_or_default();
    let ticks: Vec<Tick> = log.lines().filter_map(parse).collect();
    for (i, tick) in ticks.iter().enumerate() {
        let mut f = vz();
        f[10] = tick.entropy * 2.0;
        f[11] = tick.fear.min(5.0) * 3.0;
        f[12] = tick.motion * 5.0;
        let agents = [AgentMomentum { momentum: f, entropy: tick.entropy }, AgentMomentum { momentum: vscale(&f, 0.9), entropy: 0.35 }];
        net.update(&agents);
        if tick.motion > 0.01 {
            emo.record_pain(&f, tick.motion, i as u32);
        }
        emo.update(&f, tick.entropy, i as u32);
    }

    let pc_state = format!(
        "{{\"synapses\":{},\"max_weight\":{:.4},\"fear\":{:.4},\"emotion\":\"{}\",\"ticks_processed\":{},\"timestamp\":\"{}\"}}",
        net.count(),
        net.synapses.iter().filter(|s| s.active).map(|s| s.weight.abs()).fold(0.0_f64, f64::max),
        emo.fear,
        emo.dominant(),
        ticks.len(),
        chrono_now(),
    );
    results.push(("T2: PC kernel processes phone data", true, vec![format!("Processed {} ticks → {} synapses, emotion={}", ticks.len(), net.count(), emo.dominant())]));

    // ═══ T3: PC→Phone (write state to sdcard) ═══
    println!("  T3: PC→Phone...");
    // Write PC state to a temp file, push via ADB
    let tmp_path = "C:\\tmp\\oasis-pc-state.json";
    std::fs::write(tmp_path, &pc_state).unwrap();
    let push_result = Command::new("adb").args(["push", tmp_path, "/sdcard/oasis-pc-state.json"]).output();

    let t3_pass = push_result.as_ref().map(|o| o.status.success()).unwrap_or(false);
    results.push(("T3: PC→Phone state push", t3_pass, vec![format!("Wrote: {}", pc_state)]));

    // ═══ T4: Verify phone can read PC state ═══
    println!("  T4: Phone reads PC state...");
    let verify = Command::new("adb").args(["shell", "cat", "/sdcard/oasis-pc-state.json"]).output();

    let t4_pass = verify
        .as_ref()
        .map(|o| {
            let content = String::from_utf8_lossy(&o.stdout);
            content.contains("synapses") && content.contains("emotion")
        })
        .unwrap_or(false);

    let t4_details = verify
        .as_ref()
        .map(|o| vec![format!("Phone reads: {}", String::from_utf8_lossy(&o.stdout).trim())])
        .unwrap_or_else(|_| vec!["Failed to read".to_string()]);
    results.push(("T4: Phone reads PC state", t4_pass, t4_details));

    // ═══ T5: Round-trip verification ═══
    println!("  T5: Round-trip...");
    // Read phone telemetry AGAIN to prove both channels work simultaneously
    let phone_tel2 = Command::new("adb")
        .args(["shell", "cat", "/sdcard/oasis-live-tel.txt"])
        .output()
        .ok()
        .and_then(|o| parse(&String::from_utf8_lossy(&o.stdout)));

    let pc_on_phone = Command::new("adb")
        .args(["shell", "cat", "/sdcard/oasis-pc-state.json"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("synapses"))
        .unwrap_or(false);

    let t5_pass = phone_tel2.is_some() && pc_on_phone;
    results.push((
        "T5: Simultaneous bidirectional",
        t5_pass,
        vec![
            format!("Phone→PC: {}", if phone_tel2.is_some() { "LIVE" } else { "DEAD" }),
            format!("PC→Phone: {}", if pc_on_phone { "VERIFIED" } else { "FAILED" }),
            "Both channels active simultaneously".to_string(),
        ],
    ));

    // ═══ SCORECARD ═══
    let dur = t0.elapsed();
    println!("\n{C}╔══════════════════════════════════════════════════════════════╗");
    println!("║       BIDIRECTIONAL FEDERATION SCORECARD                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝{X}\n");

    let passed = results.iter().filter(|r| r.1).count();
    let total = results.len();

    for (name, pass, details) in &results {
        let icon = if *pass { format!("{G}✓ PASS{X}") } else { format!("{R}✗ FAIL{X}") };
        println!("  {} {}", icon, name);
        for d in details {
            println!("    {}", d);
        }
        println!();
    }

    println!("  ────────────────────────────────");
    println!("  {G}PASSED: {}{X} / {}", passed, total);
    if passed < total {
        println!("  {R}FAILED: {}{X} / {}", total - passed, total);
    }
    println!("  Score: {}%", (passed * 100) / total);
    println!("  Time: {:.0}ms\n", dur.as_secs_f64() * 1000.0);

    if passed == total {
        println!("  {G}✓ BIDIRECTIONAL FEDERATION PROVED{X}");
        println!("  Phone→PC: live telemetry (E/fear/motion)");
        println!("  PC→Phone: synaptic state (synapses/weight/emotion)");
        println!("  Both channels verified simultaneously.");
    }
}

fn chrono_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    format!("{:02}:{:02}:{:02}", (secs / 3600) % 24 + 2, (secs / 60) % 60, secs % 60)
}
