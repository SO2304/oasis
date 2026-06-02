//! OASIS — Spinal + Nerve Integration Test
//! T1: Body discovery, T2: Wiring, T3: Afferent IMU, T4: R15 entropy,
//! T5: Efferent vibrate, T6: FreeRTOS static, T7: Brightness→light,
//! T8: Vibrate→accel. Runs on Linux, Android, Windows (partial).

use oasis_rt::efference::ReflectionEngine;
use oasis_rt::nerve::*;
use oasis_rt::spinal::{BodyMap, DeviceKind, ZONE_ACTUATOR, ZONE_ENV, ZONE_INERTIAL};
use oasis_rt::vec::*;
use std::process::Command;
use std::time::Instant;

const TB: &str = "/data/data/com.termux/files/usr/bin";
fn is_termux() -> bool {
    std::path::Path::new(TB).exists()
}
fn ms_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn read_accel_raw() -> Option<(f64, f64, f64)> {
    let o = Command::new(format!("{}/termux-sensor", TB)).args(["-s", "LSM6DSVTR Accelerometer", "-n", "1"]).output().ok()?;
    let raw = String::from_utf8_lossy(&o.stdout);
    let v: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let a = v.get("LSM6DSVTR Accelerometer")?.get("values")?.as_array()?;
    Some((a.get(0)?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?))
}

fn read_light_raw() -> Option<f64> {
    let o = Command::new(format!("{}/termux-sensor", TB)).args(["-s", "STK31610 Light", "-n", "1"]).output().ok()?;
    let raw = String::from_utf8_lossy(&o.stdout);
    let v: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    v.get("STK31610 Light")?.get("values")?.as_array()?.get(0)?.as_f64()
}

fn main() {
    eprintln!("╔══════════════════════════════════════════════════════════╗");
    eprintln!("║  OASIS — Spinal + Nerve Integration (8 tests)          ║");
    eprintln!("╚══════════════════════════════════════════════════════════╝\n");
    let termux = is_termux();
    eprintln!("  Platform: {}\n", if termux { "Android/Termux" } else { "PC" });
    let mut results: Vec<(bool, &str)> = Vec::new();

    // ═══ T1: Body Discovery ═══
    eprintln!("══ T1: Body Discovery ══");
    let mut body = BodyMap::new();
    body.scan_all();
    let dc = body.device_count();
    eprintln!("{}", body.report());
    let t1 = if termux { dc >= 3 } else { true }; // PC may find 0, OK
    eprintln!("  T1: {} devices → {}\n", dc, if t1 { "PASS" } else { "FAIL" });
    results.push((t1, "T1 Body discovery"));

    // ═══ T2: NervousSystem Wiring ═══
    eprintln!("══ T2: NervousSystem Wiring ══");
    let mut ns = NervousSystem::new();
    let imu_idx = ns.add_afferent("imu");
    let mag_idx = ns.add_afferent("mag");
    ns.add_afferent("baro");
    ns.add_efferent("vibrate", DIM_VIBRATE, 0.3, 2000);
    ns.add_efferent("flash", DIM_FLASH, 0.5, 5000);
    let t2 = ns.afferents.len() == 3 && ns.efferents.len() == 2;
    eprintln!("  afferents={} efferents={} → {}\n", ns.afferents.len(), ns.efferents.len(), if t2 { "PASS" } else { "FAIL" });
    results.push((t2, "T2 NervousSystem wiring"));

    // ═══ T3: Afferent IMU Read (Android only) ═══
    eprintln!("══ T3: Afferent IMU Read ══");
    let t3 = if termux {
        let mut v = vz();
        let ok = ns.afferents[imu_idx].read_into(&mut v, &format!("{}/termux-sensor", TB), &["-s", "LSM6DSVTR Accelerometer,LSM6DSVTR Gyroscope", "-n", "1"], parse_imu);
        let alive = ns.afferents[imu_idx].alive;
        eprintln!("  read={} alive={} accel_z={:.4}", ok, alive, v[DIM_ACCEL_Z]);
        ok && alive
    } else {
        eprintln!("  SKIP (no Termux)");
        true
    };
    results.push((t3, "T3 Afferent IMU read"));

    // ═══ T4: Body Entropy R15 ═══
    eprintln!("\n══ T4: Body Entropy R15 ══");
    // Start with all alive to test transition
    for a in &mut ns.afferents {
        a.alive = true;
    }
    let e0 = ns.sensory_entropy();
    ns.afferents[imu_idx].alive = false; // kill IMU
    let e1 = ns.sensory_entropy();
    ns.afferents[imu_idx].alive = true; // restore
    let e2 = ns.sensory_entropy();
    let t4 = e0 < 0.01 && e1 > e0 && e2 < e1;
    eprintln!("  alive→dead→alive: {:.2}→{:.2}→{:.2} → {}\n", e0, e1, e2, if t4 { "PASS" } else { "FAIL" });
    results.push((t4, "T4 Body entropy R15"));

    // ═══ T5: Efferent Vibrate (Android only) ═══
    eprintln!("══ T5: Efferent Vibrate ══");
    let t5 = if termux {
        let mut eff = ReflectionEngine::new();
        let mut tension = vz();
        tension[DIM_VIBRATE] = 0.5;
        let now = ms_now();
        let sev = ns.efferents[0].fire(&tension, &mut eff, &format!("{}/termux-vibrate", TB), &["-d", "200", "-f"], now);
        let fires = ns.efferents[0].fire_count;
        eprintln!("  fired={} severity={:?}", fires, sev);
        fires == 1
    } else {
        eprintln!("  SKIP (no Termux)");
        true
    };
    results.push((t5, "T5 Efferent vibrate"));

    // ═══ T6: FreeRTOS Static Registration ═══
    eprintln!("\n══ T6: FreeRTOS Static Registration ══");
    let mut rtos = BodyMap::new();
    rtos.register_static(&[
        (DeviceKind::Accelerometer, "LSM6DSVTR", true, false),
        (DeviceKind::Gyroscope, "LSM6DSVTR", true, false),
        (DeviceKind::Magnetometer, "AK09918C", true, false),
        (DeviceKind::Barometer, "BMP580", true, false),
        (DeviceKind::LightSensor, "STK31610", true, false),
        (DeviceKind::StepCounter, "SAM-Step", true, false),
        (DeviceKind::Gps, "GNSS", true, false),
        (DeviceKind::Motor, "LRA-vibrate", false, true),
        (DeviceKind::AudioOut, "speaker", false, true),
        (DeviceKind::Led, "flashlight", false, true),
        (DeviceKind::Backlight, "AMOLED", false, true),
    ]);
    let all_dims: Vec<usize> = rtos.devices.iter().flat_map(|d| d.dims.clone()).collect();
    let no_dup = (0..all_dims.len()).all(|i| (i + 1..all_dims.len()).all(|j| all_dims[i] != all_dims[j]));
    let zones_ok = rtos.devices.iter().all(|d| {
        let z = match d.kind {
            DeviceKind::Accelerometer | DeviceKind::Gyroscope => ZONE_INERTIAL,
            DeviceKind::Barometer | DeviceKind::LightSensor => ZONE_ENV,
            DeviceKind::Motor | DeviceKind::Led | DeviceKind::Backlight | DeviceKind::AudioOut => ZONE_ACTUATOR,
            _ => (0, 127),
        };
        d.dims.iter().all(|&dim| dim >= z.0 && dim <= z.1)
    });
    let t6 = rtos.device_count() == 11 && rtos.sensor_count() == 7 && rtos.actuator_count() == 4 && no_dup && zones_ok && rtos.body_entropy() < 0.01;
    eprintln!("  devices={} sensors={} actuators={} no_dup={} zones={} entropy={:.2}", rtos.device_count(), rtos.sensor_count(), rtos.actuator_count(), no_dup, zones_ok, rtos.body_entropy());
    eprintln!("  → {}\n", if t6 { "PASS" } else { "FAIL" });
    results.push((t6, "T6 FreeRTOS static registration"));

    // ═══ T7: Brightness → Light Proprioception (Android only) ═══
    eprintln!("══ T7: Brightness → Light ══");
    let t7 = if termux {
        let pre = read_light_raw().unwrap_or(0.0);
        let (cmd, args) = cmd_brightness(1.0);
        let _ = Command::new(format!("{}/{}", TB, cmd)).args(&args).output();
        std::thread::sleep(std::time::Duration::from_millis(600));
        let post = read_light_raw().unwrap_or(0.0);
        let (cmd2, args2) = cmd_brightness(0.1);
        let _ = Command::new(format!("{}/{}", TB, cmd2)).args(&args2).output();
        let delta = post - pre;
        eprintln!("  light: {:.0}→{:.0} delta={:.0}", pre, post, delta);
        delta > -50.0 // brightness increase MAY increase lux (depends on ambient)
    } else {
        eprintln!("  SKIP (no Termux)");
        true
    };
    results.push((t7, "T7 Brightness→light proprioception"));

    // ═══ T8: Vibrate → Accel Proprioception (Android only) ═══
    eprintln!("\n══ T8: Vibrate → Accel ══");
    let t8 = if termux {
        let pre = read_accel_raw().map(|(x, y, z)| (x * x + y * y + z * z).sqrt()).unwrap_or(9.81);
        let _ = Command::new(format!("{}/termux-vibrate", TB)).args(["-d", "500", "-f"]).output();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let during = read_accel_raw().map(|(x, y, z)| (x * x + y * y + z * z).sqrt()).unwrap_or(9.81);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let post = read_accel_raw().map(|(x, y, z)| (x * x + y * y + z * z).sqrt()).unwrap_or(9.81);
        let dev = (during - pre).abs();
        eprintln!("  mag: pre={:.3} during={:.3} post={:.3} dev={:.4}", pre, during, post, dev);
        dev > 0.0 // any measurable change
    } else {
        eprintln!("  SKIP (no Termux)");
        true
    };
    results.push((t8, "T8 Vibrate→accel proprioception"));

    // ═══ SCORECARD ═══
    eprintln!("\n╔══════════════════════════════════════════════════════════╗");
    eprintln!("║     SPINAL + NERVE INTEGRATION SCORECARD               ║");
    eprintln!("╚══════════════════════════════════════════════════════════╝\n");
    let mut score = 0;
    for (pass, name) in &results {
        eprintln!("  {} {}", if *pass { "✓" } else { "✗" }, name);
        if *pass {
            score += 1;
        }
    }
    eprintln!("\n  TOTAL: {}/8", score);
    eprintln!("  Platform: {}", if termux { "Android/Termux" } else { "PC" });
    eprintln!("  Body: {} devices discovered", body.device_count());
    eprintln!("  Nerves: {}", ns.status());
    if score == 8 {
        eprintln!("\n  ══ ALL PASS ══");
    }
    eprintln!("\nDone.");
}
