//! OASIS Drone Bridge — real OASIS kernel for Webots drones. JSON stdin→stdout.

use oasis_rt::emotion::*;
use oasis_rt::federation::*;
use oasis_rt::hyper_state::*;
use oasis_rt::reflex::AdaptiveReflex;
use oasis_rt::synapse::{AgentMomentum, SynapticNetwork};
use oasis_rt::vec::*;
use oasis_rt::vitality::{Vitality, VitalityLevel, VitalityState};
use oasis_rt::world_model::*;
use std::collections::HashSet;
use std::io::{self, BufRead, Write};
fn shared() -> String {
    std::env::var("OASIS_SHARED").unwrap_or_else(|_| "C:/dev/oasis/webots/factory_shared/".into())
}
fn enc(x: f64, y: f64, alt: f64) -> V {
    let mut v = vz();
    v[0] = x;
    v[1] = y;
    v[2] = alt;
    v
}
fn jf(s: &str, k: &str) -> f64 {
    let p = format!("\"{}\":", k);
    let i = match s.find(&p) {
        Some(i) => i,
        None => return 0.0,
    };
    let r = s[i + p.len()..].trim_start();
    let e = r.find(|c: char| c == ',' || c == '}' || c == ' ').unwrap_or(r.len());
    r[..e].trim().parse().unwrap_or(0.0)
}
fn jb(s: &str, k: &str, default: bool) -> bool {
    let p = format!("\"{}\":", k);
    let i = match s.find(&p) {
        Some(i) => i,
        None => return default,
    };
    let r = s[i + p.len()..].trim_start();
    if r.starts_with("true") {
        true
    } else if r.starts_with("false") {
        false
    } else {
        default
    }
}

struct Drone {
    name: String,
    cruise: f64,
    targets: Vec<(String, f64, f64)>,
    inspected: HashSet<String>,
    peer_inspected: HashSet<String>,
    cur: Option<(String, f64, f64)>,
    dwell: u32,
    loops: u32,
    agent: Agent,
    world: WorldModel,
    emo: EmotionalState,
    fed: FederatedMesh,
    syn: SynapticNetwork,
    reflex_obs: AdaptiveReflex,
    tick: u32,
    aborted: bool,
    takeoff_done: bool,
    last_dist: f64,
    stuck_pos: (f64, f64),
    stuck_count: u32,
    skip_target: Option<String>, // temporarily skip a target after deadlock
    r14_blocks: u32,
    digests_emitted: u32,
    vitality: VitalityState,
    prev_level: VitalityLevel,
    // Adaptive R14 threshold bump: grows when sustained baseline entropy exceeds threshold,
    // decays during low-entropy periods. Opt-in via OASIS_ADAPTIVE_R14=1.
    r14_adaptive_bump: f64,
    // Persistent spatial memory of dangerous spots (nervous system learning).
    // Each entry: (x, y, tick_created, hits). Decays slowly — 20000 ticks half-life.
    // Injected as Repulsive zones into the WorldModel, influences navigation gradient.
    bad_spots: Vec<(f64, f64, u32, u32)>,
}

// Factory CAD: (cx, cy, intensity, falloff, top_z)
// Height gating: when drone alt > top_z + 0.30, zone is skipped (drone overflies it).
const OBSTACLES: [(f64, f64, f64, f64, f64); 9] = [
    (-1.5, 1.0, 1.00, 1.5, 1.50),  // cnc
    (0.5, 0.0, 0.80, 1.5, 1.00),   // press
    (1.8, -1.0, 0.90, 1.5, 1.20),  // welder
    (1.5, 1.5, 0.80, 1.5, 1.00),   // assembly
    (0.0, -1.5, 0.90, 1.0, 0.50),  // conveyor
    (-2.0, -0.5, 1.00, 1.4, 2.00), // rack1
    (2.2, 0.8, 1.00, 1.4, 2.00),   // rack2
    (-1.0, -1.0, 1.20, 1.0, 2.00), // pillar1 — stronger for tall obstacle
    (1.0, 1.0, 1.20, 1.0, 2.00),   // pillar2 — stronger for tall obstacle
];

fn build_world_for_alt(alt: f64) -> WorldModel {
    let mut w = WorldModel::new();
    for (cx, cy, intensity, falloff, top_z) in OBSTACLES.iter() {
        if alt > top_z + 0.30 {
            continue;
        } // drone clears this obstacle
        let mut c = vz();
        c[0] = *cx;
        c[1] = *cy;
        if let Err(e) = w.try_add_zone(ZoneType::Repulsive, c, *intensity, *falloff) {
            eprintln!("[drone_bridge] zone cap hit: {:?}", e);
        }
    }
    for &(cx, cy) in &[(-2.5, 2.0_f64), (2.5, 2.0), (-2.5, -2.0), (2.5, -2.0)] {
        let mut c = vz();
        c[0] = cx;
        c[1] = cy;
        if let Err(e) = w.try_add_zone(ZoneType::Entropy, c, 0.3, 1.0) {
            eprintln!("[drone_bridge] zone cap hit: {:?}", e);
        }
    }
    w
}

// Pick the closest target NOT YET inspected by THIS drone.
// peer_inspected is informational. Each drone fully covers its own list, so
// loop count = full coverage. skip_target temporarily excludes a deadlocked target.
fn pick_next(d: &Drone, x: f64, y: f64) -> Option<(String, f64, f64)> {
    d.targets
        .iter()
        .filter(|t| !d.inspected.contains(&t.0) && d.skip_target.as_ref().map_or(true, |s| s != &t.0))
        .min_by(|a, b| {
            let da = (a.1 - x).powi(2) + (a.2 - y).powi(2);
            let db = (b.1 - x).powi(2) + (b.2 - y).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

fn parse_peer_inspected(content: &str) -> HashSet<String> {
    let mut s = HashSet::new();
    if let Some(start) = content.find("\"inspected\":[") {
        let arr = &content[start + 13..];
        if let Some(end) = arr.find(']') {
            for tok in arr[..end].split(',') {
                let t = tok.trim().trim_matches('"');
                if !t.is_empty() {
                    s.insert(t.to_string());
                }
            }
        }
    }
    s
}
fn write_state(d: &Drone) {
    let items: Vec<String> = d.inspected.iter().map(|l| format!("\"{}\"", l)).collect();
    let s = format!("{{\"inspected\":[{}],\"loops\":{}}}", items.join(","), d.loops);
    let _ = std::fs::write(format!("{}{}_state.json", shared(), d.name), s);
}

fn write_cmd(out: &mut impl Write, dvx: f64, dvy: f64, s_alt: f64, abort: bool, d: &Drone) {
    let _ = writeln!(
        out,
        "{{\"dvx\":{:.4},\"dvy\":{:.4},\"s_alt\":{:.2},\"abort\":{},\"inspected\":{},\"loops\":{},\"fear\":{:.3},\"entropy\":{:.3}}}",
        dvx,
        dvy,
        s_alt,
        abort,
        d.inspected.len(),
        d.loops,
        d.emo.fear,
        d.agent.entropy
    );
    let _ = out.flush();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let name = args.get(1).cloned().unwrap_or_else(|| "patrol1".into());
    let _did: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

    // Cruise altitude: OASIS_CRUISE env var overrides; else supervisor=1.70, others=1.30
    let cruise: f64 = std::env::var("OASIS_CRUISE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| if name == "supervisor" { 1.70 } else { 1.30 });

    // Canonical target catalog keyed by name — positions match OBSTACLES array.
    let catalog: &[(&str, f64, f64)] =
        &[("cnc", -1.5, 1.0), ("press", 0.5, 0.0), ("welder", 1.8, -1.0), ("assembly", 1.5, 1.5), ("rack1", -2.0, -0.5), ("rack2", 2.2, 0.8), ("conv_w", -1.0, -1.5), ("conv_e", 1.0, -1.5)];

    // Extra CLI args = comma-separated target names to filter catalog (per-drone subset).
    // e.g. `drone_bridge d00 0 cnc,rack1,conv_w` — this drone only hunts those 3.
    // Falls back to name-based default (patrol1/patrol2/supervisor) if no CLI arg.
    let raw_targets: Vec<(&str, f64, f64)> = if let Some(spec) = args.get(3) {
        let wanted: Vec<&str> = spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        catalog.iter().filter(|(n, _, _)| wanted.contains(n)).copied().collect()
    } else {
        match name.as_str() {
            "patrol1" => vec![("cnc", -1.5, 1.0), ("rack1", -2.0, -0.5), ("conv_w", -1.0, -1.5), ("press", 0.5, 0.0)],
            "patrol2" => vec![("welder", 1.8, -1.0), ("rack2", 2.2, 0.8), ("assembly", 1.5, 1.5), ("conv_e", 1.0, -1.5)],
            _ => catalog.to_vec(),
        }
    };
    let targets: Vec<(String, f64, f64)> = raw_targets.iter().map(|t| (t.0.to_string(), t.1, t.2)).collect();

    eprintln!(
        "[{}] === OASIS DRONE BRIDGE === cruise={:.1}m targets={} | kernel: HyperState M2-R14 + WorldModel M10 + Emotion M5 + Synapse M7 + Reflex M9 + Federation M11",
        name,
        cruise,
        targets.len()
    );

    let mut d = Drone {
        name: name.clone(),
        cruise,
        targets,
        inspected: HashSet::new(),
        peer_inspected: HashSet::new(),
        cur: None,
        dwell: 0,
        loops: 0,
        agent: agent_new(0),
        world: build_world_for_alt(0.0),
        emo: EmotionalState::new(),
        fed: FederatedMesh::new(),
        syn: SynapticNetwork::new(),
        reflex_obs: AdaptiveReflex::new(2.0), // 2-sigma threshold on sonar
        tick: 0,
        aborted: false,
        takeoff_done: false,
        last_dist: 99.0,
        stuck_pos: (0.0, 0.0),
        stuck_count: 0,
        skip_target: None,
        r14_blocks: 0,
        digests_emitted: 0,
        r14_adaptive_bump: 0.0,
        vitality: VitalityState::new(),
        prev_level: VitalityLevel::Healthy,
        bad_spots: Vec::new(),
    };

    // Phone brain transfer (Mechanism 11 cross-morphology): if OASIS_PHONE_BRAIN is set,
    // load persistent state from phone's 3h23 session (128 pain memories, federated
    // digests). The drone inherits the nervous system's accumulated experience even
    // though it has a different body (quadrotor vs pocket phone with accelerometer).
    if let Ok(brain_dir) = std::env::var("OASIS_PHONE_BRAIN") {
        let pain_path = format!("{}/oasis-pain.bin", brain_dir);
        match d.emo.load_pain(&pain_path) {
            Ok(n) => eprintln!("[{}] >>> PHONE-BRAIN loaded {} pain memories from {}", name, n, pain_path),
            Err(e) => eprintln!("[{}] !!! PHONE-BRAIN pain load failed: {}", name, e),
        }
        let fed_path = format!("{}/oasis-memory.bin", brain_dir);
        match d.fed.load(&fed_path) {
            Ok(n) => eprintln!("[{}] >>> PHONE-BRAIN loaded {} federated digests from {}", name, n, fed_path),
            Err(e) => eprintln!("[{}] !!! PHONE-BRAIN fed load failed: {}", name, e),
        }
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines().flatten() {
        if line.is_empty() {
            continue;
        }
        run_tick(&mut d, &line, &mut out);
    }

    eprintln!("[{}] === FINAL: ins={} loops={} ===", d.name, d.inspected.len(), d.loops);
}

fn stack_is_full() -> bool {
    std::env::var("OASIS_STACK").map(|s| s == "full").unwrap_or(true)
}

fn run_tick(d: &mut Drone, line: &str, out: &mut impl Write) {
    d.tick += 1;
    let x = jf(line, "x");
    let y = jf(line, "y");
    let alt = jf(line, "alt");
    let roll = jf(line, "roll");
    let pitch = jf(line, "pitch");
    let rf = jf(line, "rf");
    let rl = jf(line, "rl");
    let rr = jf(line, "rr");
    let rb = jf(line, "rb");
    let obs = rf.min(rl).min(rr).min(rb);

    // Vitality tracking — ALWAYS updated each tick, even during reflex pipeline,
    // so fault-injection transitions are visible regardless of flight phase.
    let imu_alive = jb(line, "imu_alive", true);
    let gps_alive = jb(line, "gps_alive", true);
    let sonar_alive = jb(line, "sonar_alive", true);
    d.vitality.update(&[(imu_alive, Vitality::Vital), (gps_alive, Vitality::Vital), (sonar_alive, Vitality::Important)]);
    if d.vitality.level != d.prev_level {
        eprintln!(
            "[{}] ~~~ VITALITY {:?} → {:?} (imu={} gps={} sonar={} thr={:.2} ent+={:.2})",
            d.name,
            d.prev_level,
            d.vitality.level,
            imu_alive,
            gps_alive,
            sonar_alive,
            d.vitality.r14_threshold(),
            d.vitality.entropy_contribution
        );
        d.prev_level = d.vitality.level;
    }

    // Reflex pipeline (Mechanism 9): divergence kill, flipped cut, pre-flip level, ground recovery, takeoff.
    // Divergence envelope expanded to ±5.5/±4.5 for 12×10m recon arena (legacy factory ±5/±4 still within bounds).
    if x.abs() > 5.5 || y.abs() > 4.5 || alt > 5.0 || alt < -0.5 {
        if !d.aborted {
            eprintln!("[{}] !!! DIVERGENCE x={:.2} y={:.2} alt={:.2}", d.name, x, y, alt);
            d.aborted = true;
        }
        write_cmd(out, 0.0, 0.0, d.cruise, true, d);
        return;
    }
    if alt < 0.5 && (roll.abs() > 0.8 || pitch.abs() > 0.8) {
        if !d.aborted {
            eprintln!("[{}] !!! FLIPPED roll={:.2} pitch={:.2}", d.name, roll, pitch);
            d.aborted = true;
        }
        write_cmd(out, 0.0, 0.0, d.cruise, true, d);
        return;
    }
    if roll.abs() > 0.45 || pitch.abs() > 0.45 {
        if d.tick % 30 == 0 {
            eprintln!("[{}] ~~~ AUTO-LEVEL roll={:+.2} pitch={:+.2}", d.name, roll, pitch);
        }
        write_cmd(out, 0.0, 0.0, d.cruise, false, d);
        return;
    }
    if alt < 0.5 {
        if d.tick % 50 == 0 {
            eprintln!("[{}] ~~~ GROUND RECOVERY alt={:.2}", d.name, alt);
        }
        write_cmd(out, 0.0, 0.0, d.cruise, false, d);
        return;
    }
    if !d.takeoff_done {
        if d.tick > 250 && (alt - d.cruise).abs() < 0.15 {
            d.takeoff_done = true;
            eprintln!("[{}] >>> TAKEOFF complete tick={} alt={:.2}", d.name, d.tick, alt);
        }
        write_cmd(out, 0.0, 0.0, d.cruise, false, d);
        return;
    }

    // Mechanism 2 — HyperState pos + Shannon entropy.
    let pos_v = enc(x, y, alt);
    d.agent.pos = pos_v;
    d.agent.entropy = entropy(&pos_v);

    // Stuck detector: deadlocked >600t → fear digest + skip target temporarily.
    let dpos = ((x - d.stuck_pos.0).powi(2) + (y - d.stuck_pos.1).powi(2)).sqrt();
    if dpos < 0.10 {
        d.stuck_count += 1;
    } else {
        d.stuck_pos = (x, y);
        d.stuck_count = 0;
    }
    if d.stuck_count > 600 {
        if let Some((label, _, _)) = d.cur.clone() {
            // Merge with existing bad spot if within 0.30m, else add new.
            let merged = d.bad_spots.iter_mut().find(|(bx, by, _, _)| ((x - *bx).powi(2) + (y - *by).powi(2)).sqrt() < 0.30);
            if let Some(spot) = merged {
                spot.2 = d.tick;
                spot.3 += 1; // refresh timestamp, increment hit count
            } else {
                d.bad_spots.push((x, y, d.tick, 1));
            }
            eprintln!("[{}] !!! STUCK at ({:+.2},{:+.2}) tgt={} → bad_spot learned (total={})", d.name, x, y, label, d.bad_spots.len());
            // Share with peers via federation (digest)
            let mut axis = vz();
            axis[0] = x;
            axis[1] = y;
            axis[2] = alt;
            d.fed.pool_push_test(axis, 0.6, -1.0, d.agent.entropy);
            d.digests_emitted += 1;
            d.skip_target = Some(label);
            d.cur = None;
        }
        d.stuck_count = 0;
    }
    // Decay bad_spots: remove spots older than 20000 ticks (spatial memory half-life)
    d.bad_spots.retain(|(_, _, t, _)| d.tick.saturating_sub(*t) < 20000);
    if let Some(s) = d.skip_target.clone() {
        if d.inspected.iter().any(|i| i != &s) {
            d.skip_target = None;
        }
    }

    // Pick / verify current target
    if d.cur.as_ref().map_or(true, |t| d.inspected.contains(&t.0) || d.peer_inspected.contains(&t.0)) {
        d.cur = pick_next(d, x, y);
        if d.cur.is_none() {
            // Honest rule: loop counts ONLY on full coverage.
            if d.inspected.len() == d.targets.len() {
                d.loops += 1;
                eprintln!("[{}] ### LOOP {} FULL ({}/{}) peers={:?}", d.name, d.loops, d.inspected.len(), d.targets.len(), d.peer_inspected);
                d.inspected.clear();
            } else {
                d.skip_target = None;
            }
            d.cur = pick_next(d, x, y);
        }
        d.dwell = 0;
    }

    // Mechanism 10 — altitude-gated WorldModel sample + learned bad spots injected.
    // Spatial memory (bad_spots) influences navigation gradient: drones naturally
    // avoid places they previously got stuck. Intensity scales with hit count.
    d.world = build_world_for_alt(alt);
    for (bx, by, _t, hits) in &d.bad_spots {
        let mut c = vz();
        c[0] = *bx;
        c[1] = *by;
        let intensity = (0.8 + 0.3 * (*hits as f64).min(5.0)).min(2.0);
        if let Err(e) = d.world.try_add_zone(ZoneType::Repulsive, c, intensity, 1.2) {
            eprintln!("[drone_bridge] cap hit at sensor reading: {:?}", e);
        }
    }
    let (grad, _rep, _att, _wm_ent) = d.world.sample(&pos_v);

    // Goal attraction (added on top — target is dynamic, not in static world)
    let mut dvx;
    let mut dvy;
    let mut tgt = String::new();
    let mut dist_g = 99.0_f64;
    if let Some((label, tx, ty)) = &d.cur {
        tgt = label.clone();
        let dx = tx - x;
        let dy = ty - y;
        dist_g = (dx * dx + dy * dy).sqrt();
        let mag = (dist_g * 0.35_f64).min(0.22);
        if dist_g > 0.001 {
            dvx = dx / dist_g * mag;
            dvy = dy / dist_g * mag;
        } else {
            dvx = 0.0;
            dvy = 0.0;
        }
        d.emo.record_goal_distance(dist_g);
        d.last_dist = dist_g;

        // Inspection: adaptive radius (bio-inspired). Nominal 1.30m, but if drone
        // has been stuck pursuing this target, allow "inspection from afar" up to
        // 1.75m — reflects real camera/sensor inspection from a safe standoff.
        let inspect_r = if d.stuck_count > 300 { 1.75 } else { 1.30 };
        if dist_g < inspect_r && (alt - d.cruise).abs() < 0.30 {
            d.dwell += 1;
            if d.dwell >= 40 {
                d.inspected.insert(label.clone());
                // Mechanism 7: reward Hebbian synapse on success (positive reinforcement)
                d.syn.reinforce(0, 1.0);
                // Mechanism 11: emit a REAL federation digest on inspection success.
                // axis = direction to the machine (vector of where success came from)
                let mut axis = vz();
                axis[0] = (tx - x).abs();
                axis[1] = (ty - y).abs();
                axis[2] = alt;
                d.fed.pool_push_test(axis, 0.8, 1.0, d.agent.entropy);
                d.digests_emitted += 1;
                eprintln!(
                    "[{}] *** INSPECTED {} dwell={} ({:+.2},{:+.2}) alt={:.2} | own={}/{} peer={} | dig#{} syn={}",
                    d.name,
                    label,
                    d.dwell,
                    x,
                    y,
                    alt,
                    d.inspected.len(),
                    d.targets.len(),
                    d.peer_inspected.len(),
                    d.digests_emitted,
                    d.syn.count()
                );
                d.cur = None;
                d.dwell = 0;
            }
        } else if d.dwell > 0 {
            d.dwell -= 1;
        }
    } else {
        dvx = 0.0;
        dvy = 0.0;
    }

    // Add WorldModel gradient force (Mechanism 10 real pressure-field navigation)
    dvx += grad[0] * 0.45;
    dvy += grad[1] * 0.45;

    // Walls
    let wx = 2.85;
    let wy = 2.35;
    if x > wx {
        dvx -= (x - wx) * 2.0;
    }
    if x < -wx {
        dvx += (-wx - x) * 2.0;
    }
    if y > wy {
        dvy -= (y - wy) * 2.0;
    }
    if y < -wy {
        dvy += (-wy - y) * 2.0;
    }

    // Sonar reactive (last-mm safety) — stronger when close (reflexive Mechanism 9)
    if rf < 0.35 {
        dvx -= (0.35 - rf) * 0.8;
    }
    if rb < 0.35 {
        dvx += (0.35 - rb) * 0.8;
    }
    if rl < 0.35 {
        dvy -= (0.35 - rl) * 0.8;
    }
    if rr < 0.35 {
        dvy += (0.35 - rr) * 0.8;
    }

    // Mechanism 9 — AdaptiveReflex: feed sonar history to learn baseline,
    // trigger reflex pain when sonar drops below (mean - 2*sigma).
    d.reflex_obs.feed(obs);
    if d.tick == 200 {
        d.reflex_obs.calibrate();
    }
    let reflex_fire = d.reflex_obs.is_calibrated() && d.reflex_obs.check(obs);
    if reflex_fire {
        d.emo.record_pain(&pos_v, 1.0, d.tick);
    }

    // Mechanism 5 — Emotion: record proximity pain (always), run full update
    if obs < 0.30 {
        d.emo.record_pain(&pos_v, (0.30 - obs) * 3.0, d.tick);
    }
    // Fear-modulated speed (OASIS nervous layer — skipped in simple_nav stack)
    if stack_is_full() {
        d.emo.update(&pos_v, d.agent.entropy, d.tick);
        if d.emo.fear > 0.5 {
            let m = (1.0 - d.emo.fear * 0.10_f64).max(0.4);
            dvx *= m;
            dvy *= m;
        }
    }

    // Cap commanded velocity
    dvx = dvx.clamp(-0.30, 0.30);
    dvy = dvy.clamp(-0.30, 0.30);

    // Shutdown on Dead vitality
    if d.vitality.should_shutdown() {
        if !d.aborted {
            eprintln!("[{}] !!! VITALITY DEAD — shutdown", d.name);
            d.aborted = true;
        }
        write_cmd(out, 0.0, 0.0, d.cruise, true, d);
        return;
    }

    // Mechanism 2 R14 — OASIS nervous layer gate. Skipped entirely in simple_nav stack.
    if stack_is_full() && obs < 0.05 {
        inject_sensory(&mut d.agent, 0.8);
    }
    let mode = std::env::var("OASIS_GATE_MODE").unwrap_or_else(|_| "full".into());
    let signal = match mode.as_str() {
        "adaptive_nocouple" => d.agent.entropy,
        _ => (d.agent.entropy + d.vitality.entropy_contribution).min(1.0),
    };
    // Adaptive R14: grow bump when sustained high entropy, decay when low.
    // Opt-in via OASIS_ADAPTIVE_R14=1. When enabled, threshold rises if the kernel has
    // been running hot for a long time — biological analogue of habituation at the gate.
    let adaptive_r14: bool = std::env::var("OASIS_ADAPTIVE_R14").ok().map(|s| s == "1").unwrap_or(false);
    let base_threshold = match mode.as_str() {
        "none" => 2.0,
        "static" => 0.95,
        _ => d.vitality.r14_threshold() + 0.10,
    };
    if adaptive_r14 && mode != "none" && mode != "static" {
        let base_healthy = d.vitality.r14_threshold() + 0.10;
        let over = signal - base_healthy;
        // Grow bump 0.000005/tick above threshold (10000t saturated → +0.05), decay 0.000002/tick below
        if over > 0.0 {
            d.r14_adaptive_bump = (d.r14_adaptive_bump + 0.000005).min(0.10);
        } else {
            d.r14_adaptive_bump = (d.r14_adaptive_bump - 0.000002).max(0.0);
        }
    }
    let threshold = (base_threshold + d.r14_adaptive_bump).min(1.5);
    if stack_is_full() && signal > threshold && threshold < 1.5 {
        d.r14_blocks += 1;
        dvx = 0.0;
        dvy = 0.0;
        if d.r14_blocks == 1 || d.r14_blocks % 50 == 0 {
            // OASIS_LOG_JSON=1 emits structured audit-friendly JSON line instead of prose.
            // Same stderr stream; downstream parsing / certification ingestion trivial.
            let json_mode: bool = std::env::var("OASIS_LOG_JSON").ok().map(|s| s == "1").unwrap_or(false);
            if json_mode {
                eprintln!(
                    "{{\"evt\":\"r14_block\",\"drone\":\"{}\",\"tick\":{},\"mode\":\"{}\",\"signal\":{:.4},\"threshold\":{:.4},\"vitality\":\"{:?}\",\"blocks\":{}}}",
                    d.name, d.tick, mode, signal, threshold, d.vitality.level, d.r14_blocks
                );
            } else {
                eprintln!("[{}] ~~~ R14 BLOCK mode={} signal={:.3} thr={:.2} level={:?} (blocks={})", d.name, mode, signal, threshold, d.vitality.level, d.r14_blocks);
            }
        }
    }

    // Mechanism 7 — 4-agent Hebbian (motor, goal, obstacle, fear) → 6 possible synapses.
    let mut motor_m = vz();
    motor_m[0] = dvx;
    motor_m[1] = dvy;
    let mut goal_m = vz();
    if let Some((_, tx, ty)) = &d.cur {
        let (dx, dy) = (tx - x, ty - y);
        let dn = (dx * dx + dy * dy).sqrt().max(0.001);
        goal_m[0] = dx / dn * 0.3;
        goal_m[1] = dy / dn * 0.3;
    }
    let mut obs_m = vz();
    obs_m[0] = grad[0];
    obs_m[1] = grad[1];
    let mut fear_m = vz();
    if d.emo.fear > 0.3 {
        fear_m[0] = dvx * d.emo.fear;
        fear_m[1] = dvy * d.emo.fear;
    }
    let e = d.agent.entropy;
    let am = [
        AgentMomentum { momentum: motor_m, entropy: e },
        AgentMomentum { momentum: goal_m, entropy: e },
        AgentMomentum { momentum: obs_m, entropy: e },
        AgentMomentum { momentum: fear_m, entropy: e },
    ];
    if stack_is_full() {
        d.syn.update(&am);
    }

    // Federation (Mechanism 11) — Transport-abstracted.
    // Uses FileTransport by default; can be swapped to LoRa / in-memory without changing this code.
    // `OASIS_TRANSPORT` reserved for future use (e.g. `OASIS_TRANSPORT=lora`).
    use oasis_rt::transport::Transport as _; // bring trait into scope for list_peers
    let transport = oasis_rt::transport::FileTransport::new(shared());

    if d.tick % 100 == 0 {
        if let Err(e) = d.fed.save_via(&transport, &d.name) {
            eprintln!("[{}] !!! save_via failed: {}", d.name, e);
        }
    }
    if d.tick % 200 == 0 {
        let trust = 0.7;
        if let Ok(peers) = transport.list_peers(&d.name) {
            for peer in peers {
                if let Ok(n) = d.fed.merge_foreign_via(&transport, &peer, trust) {
                    if n > 0 {
                        eprintln!("[{}] <<< MESH merged {} digests from {}", d.name, n, peer);
                    }
                }
            }
        }
        // Explicit OASIS_MESH_PEERS still honored for cross-filesystem peers
        // (e.g. phone_brain/ loaded paths, not in SHARED dir) — still goes via Transport,
        // so they must be in the same SHARED dir under <name>_fed.bin.
        if let Ok(peers) = std::env::var("OASIS_MESH_PEERS") {
            for peer in peers.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                if let Ok(n) = d.fed.merge_foreign_via(&transport, peer, 0.5) {
                    if n > 0 {
                        eprintln!("[{}] <<< MESH merged {} digests from {} (explicit)", d.name, n, peer);
                    }
                }
            }
        }
    }

    // High-level state share (which targets we've inspected) — for peer skip
    if d.tick % 50 == 0 {
        write_state(d);
    }
    if d.tick % 60 == 0 {
        // Auto-discover state-sharing peers (replaces hardcoded list).
        let mut acc = HashSet::new();
        if let Ok(entries) = std::fs::read_dir(shared()) {
            for entry in entries.flatten() {
                let fname = entry.file_name();
                let fname_str = fname.to_string_lossy();
                if !fname_str.ends_with("_state.json") {
                    continue;
                }
                let peer = fname_str.trim_end_matches("_state.json");
                if peer == d.name {
                    continue;
                }
                let p = format!("{}{}_state.json", shared(), peer);
                if let Ok(c) = std::fs::read_to_string(&p) {
                    for lbl in parse_peer_inspected(&c) {
                        acc.insert(lbl);
                    }
                }
            }
        }
        d.peer_inspected = acc;
    }

    // Motor dampening — two modes, selected by OASIS_STDP_DAMPENING env var:
    //   OASIS_STDP_DAMPENING=1  → real STDP weight from fear(3)→motor(0) synapse
    //   OASIS_MOTOR_DAMPENING=1 → legacy pain-count gain scalar (simpler)
    //   neither set              → no dampening (factor = 1.0)
    //
    // STDP mode: if SynapticNetwork has a fear→motor synapse, its weight modulates
    // the motor output. Negative weight (LTD via post-before-pre STDP) attenuates
    // motor commands when fear pattern is predictive. Positive weight amplifies.
    // Weight is clamped [-1.0, +1.0] by SynapticNetwork; we map to [0.3, 1.0] for safety.
    let stdp_mode: bool = std::env::var("OASIS_STDP_DAMPENING").ok().map(|s| s == "1").unwrap_or(false);
    let dampen = if stdp_mode {
        match d.syn.weight_between(3, 0) {
            Some(w) => ((1.0 + w) * 0.5).clamp(0.3, 1.0), // w ∈ [-1,1] → factor ∈ [0.3, 1.0]
            None => 1.0,                                  // no synapse formed yet → no dampening
        }
    } else {
        d.emo.motor_dampening()
    };
    let dvx_out = dvx * dampen;
    let dvy_out = dvy * dampen;
    write_cmd(out, dvx_out, dvy_out, d.cruise, false, d);

    if d.tick % 10 == 0 {
        eprintln!(
            "[{}] T{:5} alt:{:.2} x:{:+.2} y:{:+.2} f:{:3.0}% obs:{:.2} ent:{:.2} →{}({:.2}) dwell:{} ins:{}/{} loops:{} pi:{} syn:{} dig:{} r14:{}",
            d.name,
            d.tick,
            alt,
            x,
            y,
            d.emo.fear * 100.0,
            obs,
            d.agent.entropy,
            tgt,
            dist_g,
            d.dwell,
            d.inspected.len(),
            d.targets.len(),
            d.loops,
            d.peer_inspected.len(),
            d.syn.count(),
            d.fed.digest_count(),
            d.r14_blocks
        );
    }
}
