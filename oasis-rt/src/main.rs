//! OASIS-RT Daemon v0.6 — sensory entropy + homeostatic plasticity.

use oasis_rt::audio::{analyze_pcm, project_audio};
use oasis_rt::branching::TemporalBrancher;
use oasis_rt::dreams::DreamEngine;
use oasis_rt::efference::{ReflectionEngine, Severity};
use oasis_rt::emotion::EmotionalState;
use oasis_rt::federation::FederatedMesh;
use oasis_rt::hal::KillSwitch;
use oasis_rt::hyper_state::*;
use oasis_rt::morpho::MorphoEngine;
use oasis_rt::nerve::{NervousSystem, DIM_BRIGHTNESS, DIM_FLASH, DIM_VIBRATE};
use oasis_rt::reflex::AdaptiveReflex;
use oasis_rt::spinal::BodyMap;
use oasis_rt::spore;
use oasis_rt::synapse::{AgentMomentum, SynapticNetwork};
use oasis_rt::tension::TensionField;
use oasis_rt::vec::*;
use oasis_rt::vitality::{Vitality, VitalityLevel, VitalityState};
use oasis_rt::world_model::{WorldModel, ZoneType};
use serde::Deserialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const POLL_SECS: u64 = 3;
const CALIBRATION_TICKS: u32 = 50; // 5s of sensor data for robust baseline
const MAX_TICKS: u32 = u32::MAX; // infinite — OASIS lives until killed

// ─── Sensor (Android/Termux specific) ──────────────────────

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
    #[serde(rename = "BMP580 Barometer")]
    b: Option<SV>,
    #[serde(rename = "STK31610 Light")]
    l: Option<SV>,
    #[serde(rename = "Samsung Orientation Sensor")]
    o: Option<SV>,
}

#[derive(Clone, Copy)]
struct Sensor {
    ax: f64,
    ay: f64,
    az: f64,
    gx: f64,
    gy: f64,
    gz: f64,
    pressure: f64,
    light: f64,
    azimuth: f64,
    grav_dev: f64,
    motion: f64,
}

fn read_sensors_blocking() -> Option<Sensor> {
    let mut ch = Command::new("termux-sensor")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .args(["-s", "LSM6DSVTR Accelerometer,LSM6DSVTR Gyroscope,BMP580 Barometer,STK31610 Light,Samsung Orientation Sensor", "-n", "1"])
        .spawn()
        .ok()?;
    let t0 = Instant::now();
    loop {
        match ch.try_wait() {
            Ok(Some(_)) => break,
            Err(_) => return None,
            Ok(None) => {
                if t0.elapsed().as_secs() > 5 {
                    let _ = ch.kill();
                    let _ = ch.wait();
                    return None;
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }
    let o = ch.wait_with_output().ok()?;
    let d: SD = serde_json::from_slice(&o.stdout).ok()?;
    let g = |s: &Option<SV>, i: usize, d: f64| s.as_ref().and_then(|v| v.values.get(i).copied()).unwrap_or(d);
    let (ax, ay, az) = (g(&d.a, 0, 0.0), g(&d.a, 1, 0.0), g(&d.a, 2, 9.81));
    let (gx, gy, gz) = (g(&d.g, 0, 0.0), g(&d.g, 1, 0.0), g(&d.g, 2, 0.0));
    let pressure = g(&d.b, 0, 1013.25);
    let l = g(&d.l, 0, 0.0);
    let light = if l < 0.0 || l > 100000.0 { 0.0 } else { l };
    let azimuth = g(&d.o, 0, 0.0);
    let am = (ax * ax + ay * ay + az * az).sqrt();
    let gd = (am - 9.80665).abs();
    let gm = (gx * gx + gy * gy + gz * gz).sqrt();
    Some(Sensor {
        ax,
        ay,
        az,
        gx,
        gy,
        gz,
        pressure,
        light,
        azimuth,
        grav_dev: gd,
        motion: ((gd / 5.0).min(1.0) * 0.6 + (gm / 3.0).min(1.0) * 0.4).min(1.0),
    })
}

fn sensor_to_force(s: &Sensor) -> V {
    let mut f = vz();
    let k = 0.03;
    f[10] = s.ax * k;
    f[11] = s.ay * k;
    f[12] = (s.az - 9.81) * k;
    f[13] = s.gx * k * 5.0;
    f[14] = s.gy * k * 5.0;
    f[15] = s.gz * k * 5.0;
    f[22] = s.azimuth.to_radians().cos() * k * 3.0;
    f[23] = s.azimuth.to_radians().sin() * k * 3.0;
    f[0] = s.motion * 0.1;
    f[30] = (s.pressure - 1013.25) * 0.002;
    f[35] = (s.light / 1000.0).min(1.0) * k;
    f
}

fn now_str() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    format!("{:02}:{:02}:{:02}", (secs / 3600) % 24 + 2, (secs / 60) % 60, secs % 60)
}

fn epoch_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn pin_cpu() {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    unsafe {
        let mask: u64 = 1 << 4;
        let ret: i64;
        std::arch::asm!("svc 0", in("x8") 122u64, in("x0") 0u64, in("x1") 8u64,
            in("x2") &mask as *const _ as u64, lateout("x0") ret);
        eprintln!("  CPU pin: {}", if ret == 0 { "OK core 4" } else { "failed" });
    }
}

fn main() {
    eprintln!("OASIS-RT v0.6");
    pin_cpu();

    let mut body = BodyMap::new();
    body.scan_all();
    let mut nerves = NervousSystem::new();
    nerves.add_efferent("vib", DIM_VIBRATE, 0.3, 3000);
    nerves.add_efferent("flash", DIM_FLASH, 0.5, 5000);
    nerves.add_efferent("bright", DIM_BRIGHTNESS, 0.2, 2000);

    let mut csv = BufWriter::new(File::create("/sdcard/oasis-8h.csv").unwrap_or_else(|_| File::create("/tmp/oasis-8h.csv").expect("No writable path")));
    let _ = writeln!(csv, "tick,epoch_ms,time,ax,ay,az,gx,gy,gz,pressure,light,azimuth,grav_dev,motion,entropy,collapsed,fear,curiosity,satisfaction,frustration,urgency,dominant,synapses,r14_safe,reflex_fired,reflex_which,kernel_us");

    let mut ag = [agent_new(3), agent_new(3), agent_new(3)];
    let mut emo = EmotionalState::new();
    let mut syn = SynapticNetwork::new();
    syn.formation_threshold = 0.3;
    syn.prune_threshold = 0.01;
    let mut fed = FederatedMesh::new();
    let mut tf = TensionField::new();
    let mut brancher = TemporalBrancher::new(5); // Mechanism 4
    let mut morpho = MorphoEngine::new(); // Mechanism 6
    for _ in 0..3 {
        morpho.register();
    }
    let mut dreams = DreamEngine::new(); // Mechanism 8
    let mut world = WorldModel::new(); // Mechanism 10
    let mut efference = ReflectionEngine::new();
    let kill = KillSwitch::new();
    let mut vitality = VitalityState::new();
    let sv = [(Vitality::Vital, "accel"), (Vitality::Vital, "gyro"), (Vitality::Important, "baro"), (Vitality::Important, "compass"), (Vitality::Optional, "light"), (Vitality::Optional, "gps")];
    let (mut rg, mut rm, mut ry, mut ra) = (AdaptiveReflex::new(3.0), AdaptiveReflex::new(3.0), AdaptiveReflex::new(3.0), AdaptiveReflex::new(3.0)); // sigma 3.0: >3σ = real shock
    let goal = {
        let mut g = vz();
        g[27] = 1.0;
        g[10] = 0.5;
        g
    };
    brancher.set_goal(goal);
    let mut prev_audio_energy = 0.0_f64;
    let mut se = SensoryEntropy::new();
    let (audio_ready, audio_recording) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
    let audio_result: Arc<Mutex<(V, f64)>> = Arc::new(Mutex::new((vz(), 0.0)));
    let (mut total_reflexes, mut total_r14, mut sensor_fails) = (0u32, 0u32, 0u32);
    let (mut max_fear, mut max_entropy) = (0.0_f64, 0.0_f64);
    let (mut kernel_sum, mut kernel_max, mut prev_fear) = (0u64, 0u64, 0.0_f64);
    let start_time = Instant::now();
    let pl = emo.load_pain("/sdcard/oasis-pain.bin").unwrap_or(0);
    let dl = fed.load("/sdcard/oasis-memory.bin").unwrap_or(0);
    let fl = fed.merge_foreign("/sdcard/oasis-foreign.bin", 0.5).unwrap_or(0);
    let _ = spore::start_listener("/sdcard/oasis-spore-inbox.bin");
    eprintln!("  Mem: {}p {}d {}f | body: {} devices", pl, dl, fl, body.device_count());

    let sensor_data: Arc<Mutex<Option<Sensor>>> = Arc::new(Mutex::new(None));
    let sensor_fresh = Arc::new(AtomicBool::new(false));
    {
        let (sd, sf) = (Arc::clone(&sensor_data), Arc::clone(&sensor_fresh));
        std::thread::spawn(move || loop {
            if let Some(s) = read_sensors_blocking() {
                if let Ok(mut l) = sd.lock() {
                    *l = Some(s);
                }
                sf.store(true, Ordering::Relaxed);
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        });
    }
    let mut last_sensor = Sensor {
        ax: 0.0,
        ay: 0.0,
        az: 9.81,
        gx: 0.0,
        gy: 0.0,
        gz: 0.0,
        pressure: 1013.25,
        light: 0.0,
        azimuth: 0.0,
        grav_dev: 0.0,
        motion: 0.0,
    };

    for t in 1..=MAX_TICKS {
        if kill.is_triggered() {
            eprintln!("KILL SWITCH — {:?}", kill.get_event());
            break;
        }
        let sa: Vec<(bool, Vitality)> = sv.iter().map(|(v, _)| (sensor_fails < 3, *v)).collect();
        vitality.update(&sa);
        if vitality.should_shutdown() {
            kill.panic(oasis_rt::hal::PanicReason::VitalityDead, "VITALITY", vitality.diagnostic());
            break;
        }
        if sensor_fresh.load(Ordering::Relaxed) {
            sensor_fresh.store(false, Ordering::Relaxed);
            if let Ok(l) = sensor_data.lock() {
                if let Some(ref s) = *l {
                    last_sensor = *s;
                    sensor_fails = 0;
                }
            }
        }
        let s = &last_sensor;

        let t0 = Instant::now();

        // Audio perception — grab pre-computed from background thread
        let mut audio_force = vz();
        if audio_ready.load(Ordering::Relaxed) {
            audio_ready.store(false, Ordering::Relaxed);
            if let Ok(lock) = audio_result.lock() {
                audio_force = lock.0;
                prev_audio_energy = lock.1;
                if t <= CALIBRATION_TICKS * 5 {
                    ra.feed(lock.1);
                }
            }
        }
        // Launch audio capture in background (every 10 ticks)
        if t % 10 == 0 && !audio_recording.load(Ordering::Relaxed) {
            let (rec, rdy, res) = (Arc::clone(&audio_recording), Arc::clone(&audio_ready), Arc::clone(&audio_result));
            let prev_e = prev_audio_energy;
            rec.store(true, Ordering::Relaxed);
            std::thread::spawn(move || {
                #[cfg(any(target_os = "linux", target_os = "android"))]
                unsafe {
                    let mask: u64 = 0x0F;
                    let _: i64;
                    std::arch::asm!("svc 0", in("x8") 122u64, in("x0") 0u64,
                        in("x1") 8u64, in("x2") &mask as *const _ as u64, lateout("x0") _);
                }
                let _ = Command::new("termux-microphone-record")
                    .args(["-l", "300", "-r", "16000", "-c", "1", "-f", "/sdcard/oasis-audio.wav"])
                    .output();
                if let Ok(wav) = std::fs::read("/sdcard/oasis-audio.wav") {
                    if wav.len() > 44 {
                        let pcm: Vec<i16> = wav[44..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
                        let percept = analyze_pcm(&pcm, prev_e);
                        let mut force = vz();
                        project_audio(&percept, &mut force);
                        if let Ok(mut lock) = res.lock() {
                            *lock = (force, percept.energy);
                        }
                    }
                }
                rdy.store(true, Ordering::Relaxed);
                rec.store(false, Ordering::Relaxed);
            });
        }

        // P0: Reflex (from lib)
        let r0 = Instant::now();
        if t <= CALIBRATION_TICKS {
            rg.feed(s.grav_dev);
            rm.feed(s.motion);
            let gm = (s.gx * s.gx + s.gy * s.gy + s.gz * s.gz).sqrt();
            ry.feed(gm);
            if t == CALIBRATION_TICKS {
                rg.calibrate();
                rm.calibrate();
                ry.calibrate();
                ra.calibrate();
            }
        }
        let gm = (s.gx * s.gx + s.gy * s.gy + s.gz * s.gz).sqrt();
        let (fg, fm, fy, fa) = (rg.check(s.grav_dev), rm.check(s.motion), ry.check(gm), ra.check(audio_force[50]));
        let fired = fg || fm || fy || fa;
        let rc = (fg as u8) + (fm as u8) + (fy as u8) + (fa as u8); // multi-channel reflex count
        let reflex_which = if rc > 1 {
            "MULTI"
        } else if fg {
            "GRAV"
        } else if fm {
            "MOTION"
        } else if fy {
            "GYRO"
        } else if fa {
            "AUDIO"
        } else {
            "NONE"
        };
        if fired {
            total_reflexes += 1;
        }
        let rns = r0.elapsed().as_nanos() as u64;

        let audio_pain = if audio_force[51] > 0.3 { audio_force[51] } else { 0.0 };
        let pain = s.motion.max(s.grav_dev * 0.5).max(audio_pain);
        if pain > 0.03 && t > CALIBRATION_TICKS {
            emo.record_pain(&ag[0].pos, pain, t);
        }

        // Mechanism 10: World Model — create pressure zones from sensors
        if t % 10 == 0 {
            world = WorldModel::new(); // Refresh zones
            let mut baro_center = vz();
            baro_center[30] = (s.pressure - 1013.25) * 0.1;
            if let Err(e) = world.try_add_zone(ZoneType::Entropy, baro_center, 0.5, 1.0) {
                eprintln!("[oasis-daemon] zone cap hit: {:?}", e);
            }
            if s.motion > 0.1 {
                let mut motion_center = ag[0].pos;
                motion_center[0] += s.motion * 0.5;
                if let Err(e) = world.try_add_zone(ZoneType::Repulsive, motion_center, s.motion * 2.0, 2.0) {
                    eprintln!("[oasis-daemon] zone cap hit: {:?}", e);
                }
            }
            if let Err(e) = world.try_add_zone(ZoneType::Attractive, goal, 1.0, 0.5) {
                eprintln!("[oasis-daemon] zone cap hit: {:?}", e);
            }
        }

        let fb = sensor_to_force(&s);
        tf.emit(&fb, 0.3 + s.motion * 0.7, 8); // ag[0] base: raw sensor force
        if vn(&audio_force) > 0.01 {
            tf.emit(&audio_force, 0.3 + audio_force[50] * 0.5, 6);
        }
        let mut eb = vz();
        eb[30] = (s.pressure - 1013.25) * 0.005;
        eb[35] = (s.light / 500.0).min(1.0) * 0.03 - 0.015;
        tf.emit(&eb, 0.3, 8);
        let mut nb = vz();
        nb[10] = 0.02;
        nb[27] = 0.05;
        tf.emit(&nb, 0.5, 8);
        let (wm_grad, _, _, _) = world.sample(&ag[0].pos);
        if vn(&wm_grad) > 0.001 {
            tf.emit(&wm_grad, 0.5, 6);
        }
        let mut ef = vz();
        ef[60] = s.motion;
        ef[61] = emo.fear.min(1.0);
        ef[65] = s.light / 500.0;
        tf.emit(&ef, 0.5, 4); // efferent dims: motion→vibrate, fear→flash, light→brightness

        // Emotions
        emo.record_goal_distance(vd(&ag[0].pos, &goal));
        emo.update(&ag[0].pos, ag[0].entropy, t);

        // Mechanism 4: Temporal Branching — every 5 ticks when safe
        let (net, _, _) = tf.sample();
        let gain = 1.0 + emo.satisfaction * 0.5;
        let safe = is_action_safe(&ag[0], vitality.r14_threshold());
        let steering = if safe && t % 5 == 0 {
            let br = brancher.branch(&ag[0], &net, &wm_grad);
            vscale(&br.best_direction, gain)
        } else if safe {
            vscale(&net, gain)
        } else {
            total_r14 += 1;
            vscale(&net, 0.1 * gain)
        };
        // Mechanism 3: Efference — proprioception (not nociception)
        efference.predict(0, &ag[0].pos, &vz(), vn(&steering), 0.2);
        evolve(&mut ag[0], &steering, 0.1, 0.05);
        let sev = se.feed(&[s.ax, s.ay, s.az, s.gx, s.gy, s.gz]);
        inject_sensory(&mut ag[0], sev);
        if let Some(dev) = efference.reflect(0, &ag[0].pos, &vz()) {
            if dev.severity == Severity::Dysmorphia {
                total_r14 += 1;
            }
        }
        // Efferent actuators: fire if tension demands it
        let now_ms = epoch_ms();
        let (tension_snap, _, _) = tf.sample();
        let eff_cmds: [(&str, &[&str]); 3] = [("termux-vibrate", &["-d", "100", "-f"]), ("termux-torch", &["on"]), ("termux-brightness", &["128"])];
        for (i, eff_nerve) in nerves.efferents.iter_mut().enumerate() {
            if eff_nerve.should_fire(&tension_snap, now_ms) {
                let (cmd, args) = eff_cmds[i.min(2)];
                eff_nerve.fire(&tension_snap, &mut efference, cmd, args, now_ms);
            }
        }
        let (cur, fea, ph) = (emo.curiosity.max(0.1), emo.fear.min(2.0), (t as f64 * 0.1).sin() * 0.3);
        let mut ef1 = vz();
        ef1[10] = s.ax * 0.03 * cur;
        ef1[11] = s.ay * 0.03 * (1.0 + ph);
        ef1[30] = (s.pressure - 1013.25) * 0.05 * cur;
        evolve(&mut ag[1], &ef1, 0.1, 0.05);
        let mut ef2 = vz();
        ef2[10] = s.ax * 0.03 * (1.0 - fea * 0.3);
        ef2[13] = s.gx * 0.15;
        ef2[22] = wm_grad[22] + 0.1;
        ef2[27] = 0.05 * (1.0 + fea);
        evolve(&mut ag[2], &ef2, 0.1, 0.05);
        inject_sensory(&mut ag[1], sev);
        inject_sensory(&mut ag[2], sev);

        if t % 3 == 0 {
            // Synapses
            let ags = [
                AgentMomentum { momentum: ag[0].momentum, entropy: ag[0].entropy },
                AgentMomentum { momentum: ag[1].momentum, entropy: ag[1].entropy },
                AgentMomentum { momentum: ag[2].momentum, entropy: ag[2].entropy },
            ];
            let ev = syn.update(&ags);
            if prev_fear - emo.fear > 0.05 {
                syn.reinforce(0, (prev_fear - emo.fear).min(1.0) * 0.5);
            }
            if pain > 0.05 {
                syn.reinforce(0, -pain.min(1.0) * 0.3);
            }
            if ev > 0 && t % 30 == 0 {
                let w: Vec<f64> = syn.synapses[..syn.len].iter().filter(|s| s.active).map(|s| s.weight).collect();
                eprintln!("  SYN T{}: n={} ev={} w={:?}", t, syn.count(), ev, w);
            }
        }

        if t % 20 == 0 {
            // Mechanism 6: Morphogenesis
            let ents = [ag[0].entropy, ag[1].entropy, ag[2].entropy];
            let moms = [vn(&ag[0].momentum), vn(&ag[1].momentum), vn(&ag[2].momentum)];
            let threat = if emo.fear > 0.3 { emo.fear.min(0.8) } else { 0.0 };
            morpho.differentiate(&ents, &moms, threat, (ag[0].entropy + s.motion).min(1.0), pain.min(0.5), true);
        }

        // Mechanism 8: Dreams — adaptive trigger (force every ≥ 500 ticks OR
        // opportunistic on entropy drop below EMA). Replaces the old absolute
        // threshold that blocked dreams during sustained high-entropy sessions.
        if let Some(dr) = dreams.maybe_dream(t, ag[0].entropy, &mut syn) {
            eprintln!("  DREAM T{}: r={} s={} w={} i={}", t, dr.replayed, dr.strengthened, dr.weakened, dr.imagined);
        }
        if t % 10 == 0 {
            let outcome = if emo.fear < 0.1 { 0.5 } else { -emo.fear.min(1.0) };
            dreams.record(&[ag[0].pos, ag[1].pos], &[ag[0].entropy, ag[1].entropy], outcome, t);
        }
        // Mechanism 11: Federation — harvest + propagate every 50 ticks
        if t % 50 == 0 {
            let ents = [ag[0].entropy, ag[1].entropy, ag[2].entropy];
            let evts: Vec<_> = syn.synapses[..syn.len]
                .iter()
                .filter(|s| s.active && s.weight.abs() > 0.03)
                .map(|s| (s.pre, s.post, 0.0, s.weight, "POTENTIATED"))
                .collect();
            let (h, p) = (fed.harvest(&evts, &syn.synapses[..syn.len], &ents), fed.propagate(&mut syn.synapses[..syn.len], 3, &ents));
            if h > 0 || p > 0 {
                eprintln!("  FED T{}: h={} p={}", t, h, p);
            }
        }

        // Field tick + stabilize: pull toward RUNNING anchor (dims 1,2) when uncertain
        tf.tick();
        if ag[0].entropy > 0.5 {
            let mut st = vz();
            st[1] = 0.5 - ag[0].pos[1]; // pull toward RUNNING anchor [0.3, 1.0]
            st[2] = 1.0 - ag[0].pos[2];
            tf.emit(&st, (ag[0].entropy - 0.4) * 1.5, 5);
        }

        let us = t0.elapsed().as_micros() as u64;
        if emo.fear > max_fear {
            max_fear = emo.fear;
        }
        prev_fear = emo.fear;
        if ag[0].entropy > max_entropy {
            max_entropy = ag[0].entropy;
        }
        kernel_sum += us;
        if us > kernel_max {
            kernel_max = us;
        }

        // CSV
        let _ = writeln!(
            csv,
            "{},{},{},{:.3},{:.3},{:.3},{:.4},{:.4},{:.4},{:.1},{:.0},{:.0},{:.3},{:.3},{:.3},{},{:.3},{:.3},{:.3},{:.3},{:.1},{},{},{},{},{},{}",
            t,
            epoch_ms(),
            now_str(),
            s.ax,
            s.ay,
            s.az,
            s.gx,
            s.gy,
            s.gz,
            s.pressure,
            s.light,
            s.azimuth,
            s.grav_dev,
            s.motion,
            ag[0].entropy,
            STATE_NAMES[ag[0].collapsed],
            emo.fear,
            emo.curiosity,
            emo.satisfaction,
            emo.frustration,
            emo.urgency,
            emo.dominant(),
            syn.count(),
            if safe { 1 } else { 0 },
            if fired { 1 } else { 0 },
            reflex_which,
            us
        );

        // Heartbeat + Console (merged)
        if t % 10 == 0 {
            let elapsed = start_time.elapsed().as_secs();
            let line = format!(
                "[{}] T{:5} | +{}h{:02}m | E:{:.1}% {} | fear:{:.0}% {} | syn:{} | mot:{:.1}% | {:.0}hPa {:.0}lux | {}µs",
                now_str(),
                t,
                elapsed / 3600,
                (elapsed % 3600) / 60,
                ag[0].entropy * 100.0,
                STATE_NAMES[ag[0].collapsed],
                emo.fear * 100.0,
                emo.dominant(),
                syn.count(),
                s.motion * 100.0,
                s.pressure,
                s.light,
                us
            );
            eprintln!("{}", line);
            if let Ok(mut hb) = File::create("/sdcard/oasis-live-tel.txt") {
                let _ = writeln!(hb, "{}", line);
            }
            csv.flush().unwrap();
        }

        // Summary + Memory persistence
        if t % 100 == 0 {
            let elapsed = start_time.elapsed().as_secs();
            if let Ok(mut sf) = File::create("/sdcard/oasis-8h-summary.txt") {
                let _ = writeln!(sf, "OASIS-RT v0.6 — tick {} — +{}h{}m", t, elapsed / 3600, (elapsed % 3600) / 60);
                let _ = writeln!(sf, "Reflexes: {} | R14: {} | MaxFear: {:.0}% | MaxE: {:.1}%", total_reflexes, total_r14, max_fear * 100.0, max_entropy * 100.0);
                let _ = writeln!(sf, "Synapses: {} | Fails: {} | Kernel avg: {}µs max: {}µs", syn.count(), sensor_fails, kernel_sum / t as u64, kernel_max);
                let _ = writeln!(sf, "Memory: {} digests | {} pain memories", fed.digest_count(), emo.pain_count());
            }
            // Persist + broadcast collective memory
            let _ = emo.save_pain("/sdcard/oasis-pain.bin");
            let _ = fed.save("/sdcard/oasis-memory.bin");
            if let Ok(sent) = spore::broadcast(&fed) {
                eprintln!("  SPORE broadcast: {} bytes to 239.0.42.1:4200", sent);
            }
            csv.flush().unwrap();
        }
        // Check spore inbox (file-based: listener thread or ADB push)
        if t % 10 == 0 {
            if let Ok(n) = fed.merge_foreign("/sdcard/oasis-spore-inbox.bin", 0.5) {
                if n > 0 {
                    eprintln!("  SPORE received: {} foreign digests!", n);
                }
                std::fs::remove_file("/sdcard/oasis-spore-inbox.bin").ok();
            }
        }

        let tick_elapsed = t0.elapsed();
        let target = std::time::Duration::from_millis(100); // 10 Hz kernel tick
        if tick_elapsed < target {
            std::thread::sleep(target - tick_elapsed);
        }
    }

    csv.flush().unwrap();
    eprintln!("\n✅ Complete. {} ticks.", MAX_TICKS);
}
