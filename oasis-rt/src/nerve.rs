//! OASIS-RT — Peripheral Nervous System
//! Tension field IS the middleware. No ROS, no DDS, no pub/sub.
//! Afferent: sensor → dimension. Efferent: dimension → actuator.
//! Proprioception via efference copy (Mechanism 3). Zero dependency.

use crate::efference::{ReflectionEngine, Severity};
use crate::vec::*;
use std::process::Command;
use std::time::Instant;

// ─── Dimension Map (the 128D nervous system) ────────────────
// Sensory (afferent) dims 10-49, Motor (efferent) dims 60-69

// Afferent (input) — sensory nerves, dims 10-55
pub const DIM_ACCEL_X: usize = 10; // LSM6DSVTR Accelerometer
pub const DIM_ACCEL_Y: usize = 11;
pub const DIM_ACCEL_Z: usize = 12;
pub const DIM_GYRO_X: usize = 13; // LSM6DSVTR Gyroscope
pub const DIM_GYRO_Y: usize = 14;
pub const DIM_GYRO_Z: usize = 15;
pub const DIM_COMPASS: usize = 22; // Samsung Orientation (azimuth cos)
pub const DIM_COMPASS_SIN: usize = 23;
pub const DIM_PRESSURE: usize = 30; // BMP580 Barometer (hPa)
pub const DIM_LIGHT: usize = 35; // STK31610 Light (lux)
pub const DIM_MAG_X: usize = 36; // AK09918C Magnetometer (µT)
pub const DIM_MAG_Y: usize = 37;
pub const DIM_MAG_Z: usize = 38;
pub const DIM_GPS_LAT: usize = 40; // GPS latitude
pub const DIM_GPS_LON: usize = 41; // GPS longitude
pub const DIM_GPS_ALT: usize = 42; // GPS altitude (m)
pub const DIM_PROXIMITY: usize = 43; // Hover Proximity (cm)
pub const DIM_STEPS: usize = 44; // Step Counter (cumulative)
pub const DIM_BATTERY: usize = 45; // Battery percentage [0,1]
pub const DIM_GPS_SPEED: usize = 46; // GPS speed (m/s)
pub const DIM_GPS_BEARING: usize = 47; // GPS bearing (rad)
pub const DIM_GPS_ACCURACY: usize = 48; // GPS accuracy (m) → high = entropy
pub const DIM_AUDIO_ENERGY: usize = 50; // Mic RMS energy
pub const DIM_AUDIO_PITCH: usize = 51; // Mic dominant pitch

// Efferent (output) — motor nerves, dims 60-72
pub const DIM_VIBRATE: usize = 60; // LRA vibration motor
pub const DIM_FLASH: usize = 61; // Camera LED torch
pub const DIM_SPEAK: usize = 62; // TTS speech output
pub const DIM_NOTIFY: usize = 63; // System notification
pub const DIM_VOLUME: usize = 64; // Volume control
pub const DIM_BRIGHTNESS: usize = 65; // Screen AMOLED brightness [0-255]
pub const DIM_MEDIA: usize = 66; // Media player (audio file)
pub const DIM_WIFI: usize = 67; // WiFi radio toggle

// ─── Afferent Nerve (sensor → tension) ──────────────────────

pub struct AfferentNerve {
    pub name: &'static str,
    pub alive: bool,
    pub last_read_us: u64,
    pub read_count: u32,
    pub fail_count: u32,
}

impl AfferentNerve {
    pub fn new(name: &'static str) -> Self {
        Self { name, alive: false, last_read_us: 0, read_count: 0, fail_count: 0 }
    }

    /// Read sensor and inject into tension vector. Returns true if successful.
    pub fn read_into(&mut self, out: &mut V, cmd: &str, args: &[&str], parser: fn(&str, &mut V) -> bool) -> bool {
        let t0 = Instant::now();
        let result = Command::new(cmd).args(args).output().ok().and_then(|o| {
            let raw = String::from_utf8_lossy(&o.stdout).to_string();
            if parser(&raw, out) {
                Some(())
            } else {
                None
            }
        });
        self.last_read_us = t0.elapsed().as_micros() as u64;
        match result {
            Some(()) => {
                self.alive = true;
                self.read_count += 1;
                true
            }
            None => {
                self.alive = false;
                self.fail_count += 1;
                false
            }
        }
    }

    /// Entropy contribution: dead sensor = high entropy (R15)
    pub fn entropy_penalty(&self) -> f64 {
        if self.alive {
            0.0
        } else {
            0.3
        }
    }
}

// ─── Efferent Nerve (tension → actuator) ────────────────────

pub struct EfferentNerve {
    pub name: &'static str,
    pub dimension: usize,
    pub threshold: f64,   // minimum tension to fire
    pub cooldown_ms: u32, // min time between firings
    pub last_fired_ms: u64,
    pub fire_count: u32,
    pub total_pain: f64, // accumulated proprioceptive error
}

impl EfferentNerve {
    pub fn new(name: &'static str, dim: usize, threshold: f64, cooldown_ms: u32) -> Self {
        Self { name, dimension: dim, threshold, cooldown_ms, last_fired_ms: 0, fire_count: 0, total_pain: 0.0 }
    }

    /// Check if this nerve should fire based on tension field value
    pub fn should_fire(&self, tension: &V, now_ms: u64) -> bool {
        tension[self.dimension].abs() > self.threshold && now_ms.saturating_sub(self.last_fired_ms) > self.cooldown_ms as u64
    }

    /// Fire the actuator with efference-integrated proprioception.
    /// Returns severity of prediction error (Mechanism 3).
    pub fn fire(&mut self, tension: &V, efference: &mut ReflectionEngine, cmd: &str, args: &[&str], now_ms: u64) -> Severity {
        let intensity = tension[self.dimension].abs().min(1.0);

        // PREDICT: what should happen when we fire this actuator?
        let mut predicted = vz();
        predicted[self.dimension] = intensity;
        efference.predict(self.dimension, &predicted, &vz(), intensity, 0.5);

        // COMMAND: fire-and-forget (NEVER block the kernel)
        let _ = Command::new(cmd).args(args).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn(); // spawn, not output — kernel continues immediately

        // REFLECT: what actually happened? (we estimate from the command result)
        // Real proprioception would read sensors post-command
        let mut actual = vz();
        actual[self.dimension] = intensity * 0.95; // actuator never perfect
        let refl = efference.reflect(self.dimension, &actual, &vz());

        let severity = refl.as_ref().map(|r| r.severity).unwrap_or(Severity::Nominal);
        if let Some(r) = &refl {
            self.total_pain += r.magnitude;
        }

        self.last_fired_ms = now_ms;
        self.fire_count += 1;
        severity
    }
}

// ─── Peripheral Nervous System ──────────────────────────────

pub struct NervousSystem {
    pub afferents: Vec<AfferentNerve>,
    pub efferents: Vec<EfferentNerve>,
    pub efference: ReflectionEngine,
    pub total_afferent_reads: u32,
    pub total_efferent_fires: u32,
}

impl NervousSystem {
    pub fn new() -> Self {
        Self {
            afferents: Vec::new(),
            efferents: Vec::new(),
            efference: ReflectionEngine::new(),
            total_afferent_reads: 0,
            total_efferent_fires: 0,
        }
    }

    pub fn add_afferent(&mut self, name: &'static str) -> usize {
        let idx = self.afferents.len();
        self.afferents.push(AfferentNerve::new(name));
        idx
    }

    pub fn add_efferent(&mut self, name: &'static str, dim: usize, threshold: f64, cooldown_ms: u32) -> usize {
        let idx = self.efferents.len();
        self.efferents.push(EfferentNerve::new(name, dim, threshold, cooldown_ms));
        idx
    }

    /// Total entropy penalty from dead sensors (R15)
    pub fn sensory_entropy(&self) -> f64 {
        self.afferents.iter().map(|a| a.entropy_penalty()).sum::<f64>().min(1.0)
    }

    /// Count alive sensors
    pub fn alive_count(&self) -> usize {
        self.afferents.iter().filter(|a| a.alive).count()
    }

    /// Summary string
    pub fn status(&self) -> String {
        let alive = self.alive_count();
        let total = self.afferents.len();
        let fires = self.efferents.iter().map(|e| e.fire_count).sum::<u32>();
        let pain: f64 = self.efferents.iter().map(|e| e.total_pain).sum();
        format!("nerves: {}/{} alive | fires: {} | pain: {:.3}", alive, total, fires, pain)
    }
}

// ─── Standard Parsers (phone sensors) ───────────────────────

/// Parse termux-sensor JSON for IMU
pub fn parse_imu(raw: &str, out: &mut V) -> bool {
    let v: serde_json::Value = match serde_json::from_str(raw.trim()) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let k = 0.03;
    if let Some(a) = v.get("LSM6DSVTR Accelerometer").and_then(|a| a.get("values")).and_then(|v| v.as_array()) {
        out[DIM_ACCEL_X] = a.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0) * k;
        out[DIM_ACCEL_Y] = a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) * k;
        out[DIM_ACCEL_Z] = (a.get(2).and_then(|v| v.as_f64()).unwrap_or(9.81) - 9.81) * k;
    }
    if let Some(g) = v.get("LSM6DSVTR Gyroscope").and_then(|g| g.get("values")).and_then(|v| v.as_array()) {
        out[DIM_GYRO_X] = g.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0) * k * 5.0;
        out[DIM_GYRO_Y] = g.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) * k * 5.0;
        out[DIM_GYRO_Z] = g.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) * k * 5.0;
    }
    true
}

/// Parse termux-sensor JSON for magnetometer (AK09918C)
pub fn parse_mag(raw: &str, out: &mut V) -> bool {
    let v: serde_json::Value = match serde_json::from_str(raw.trim()) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let k = 0.01; // µT to latent scale
    if let Some(m) = v.get("AK09918C Magnetometer").and_then(|m| m.get("values")).and_then(|v| v.as_array()) {
        out[DIM_MAG_X] = m.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0) * k;
        out[DIM_MAG_Y] = m.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) * k;
        out[DIM_MAG_Z] = m.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) * k;
        return true;
    }
    false
}

/// Parse termux-sensor JSON for step counter
pub fn parse_steps(raw: &str, out: &mut V) -> bool {
    let v: serde_json::Value = match serde_json::from_str(raw.trim()) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if let Some(s) = v.get("Step Counter").and_then(|s| s.get("values")).and_then(|v| v.as_array()) {
        out[DIM_STEPS] = s.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0) * 0.001;
        return true;
    }
    false
}

/// Parse termux-location JSON for GPS (full: lat, lon, alt, speed, bearing, accuracy)
pub fn parse_gps(raw: &str, out: &mut V) -> bool {
    let v: serde_json::Value = match serde_json::from_str(raw.trim()) {
        Ok(v) => v,
        Err(_) => return false,
    };
    out[DIM_GPS_LAT] = v.get("latitude").and_then(|v| v.as_f64()).unwrap_or(0.0) * 0.001;
    out[DIM_GPS_LON] = v.get("longitude").and_then(|v| v.as_f64()).unwrap_or(0.0) * 0.001;
    out[DIM_GPS_ALT] = v.get("altitude").and_then(|v| v.as_f64()).unwrap_or(0.0) * 0.01;
    out[DIM_GPS_SPEED] = v.get("speed").and_then(|v| v.as_f64()).unwrap_or(0.0) * 0.1;
    let bearing_deg = v.get("bearing").and_then(|v| v.as_f64()).unwrap_or(0.0);
    out[DIM_GPS_BEARING] = bearing_deg.to_radians() * 0.1;
    let accuracy = v.get("accuracy").and_then(|v| v.as_f64()).unwrap_or(100.0);
    out[DIM_GPS_ACCURACY] = (accuracy / 100.0).min(1.0); // 100m+ = max entropy
    true
}

/// Parse termux-battery-status JSON
pub fn parse_battery(raw: &str, out: &mut V) -> bool {
    let v: serde_json::Value = match serde_json::from_str(raw.trim()) {
        Ok(v) => v,
        Err(_) => return false,
    };
    out[DIM_BATTERY] = v.get("percentage").and_then(|v| v.as_f64()).unwrap_or(100.0) / 100.0;
    true
}

// ─── Actuator Commands (efferent output) ────────────────────

/// Vibrate phone for duration_ms (maps from DIM_VIBRATE intensity)
pub fn cmd_vibrate(intensity: f64) -> (&'static str, Vec<String>) {
    let ms = (intensity * 500.0).max(50.0).min(1000.0) as u32;
    ("termux-vibrate", vec!["-d".into(), ms.to_string(), "-f".into()])
}

/// Toggle flashlight (maps from DIM_FLASH)
pub fn cmd_flash(on: bool) -> (&'static str, Vec<String>) {
    ("termux-torch", vec![if on { "on" } else { "off" }.into()])
}

/// Speak text via TTS (maps from DIM_SPEAK)
pub fn cmd_speak(text: &str) -> (&'static str, Vec<String>) {
    ("termux-tts-speak", vec![text.into()])
}

/// Send notification (maps from DIM_NOTIFY)
pub fn cmd_notify(title: &str, text: &str) -> (&'static str, Vec<String>) {
    ("termux-notification", vec!["-t".into(), title.into(), "-c".into(), text.into()])
}

/// Set screen brightness 0-255 (maps from DIM_BRIGHTNESS)
/// Proprioceptive loop: brightness → light sensor reads the change
pub fn cmd_brightness(intensity: f64) -> (&'static str, Vec<String>) {
    let level = (intensity * 255.0).max(0.0).min(255.0) as u32;
    ("termux-brightness", vec![level.to_string()])
}

/// Play audio file (maps from DIM_MEDIA)
/// Proprioceptive loop: speaker → microphone picks up the sound
pub fn cmd_media_play(path: &str) -> (&'static str, Vec<String>) {
    ("termux-media-player", vec!["play".into(), path.into()])
}

/// Toggle WiFi radio (maps from DIM_WIFI)
pub fn cmd_wifi(on: bool) -> (&'static str, Vec<String>) {
    ("termux-wifi-enable", vec![if on { "true" } else { "false" }.into()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn afferent_lifecycle() {
        let nerve = AfferentNerve::new("test");
        assert!(!nerve.alive);
        assert!((nerve.entropy_penalty() - 0.3).abs() < 1e-10);
    }

    #[test]
    fn efferent_cooldown_and_threshold() {
        let mut n = EfferentNerve::new("vib", DIM_VIBRATE, 0.1, 1000);
        let mut t = vz();
        t[DIM_VIBRATE] = 0.5;
        assert!(n.should_fire(&t, 2000));
        n.last_fired_ms = 2000;
        assert!(!n.should_fire(&t, 2500)); // cooldown
        assert!(n.should_fire(&t, 3001)); // cooldown expired
        t[DIM_VIBRATE] = 0.01; // below threshold
        assert!(!n.should_fire(&t, 5000));
    }

    #[test]
    fn nervous_system_entropy() {
        let mut ns = NervousSystem::new();
        ns.add_afferent("imu");
        ns.add_afferent("gps");
        ns.add_afferent("baro");
        assert!(ns.sensory_entropy() > 0.8); // all dead
        ns.afferents[0].alive = true;
        ns.afferents[1].alive = true;
        ns.afferents[2].alive = true;
        assert!(ns.sensory_entropy() < 0.01); // all alive
    }

    #[test]
    fn dimension_map_no_overlap() {
        let af = [
            DIM_ACCEL_X,
            DIM_ACCEL_Y,
            DIM_ACCEL_Z,
            DIM_GYRO_X,
            DIM_GYRO_Y,
            DIM_GYRO_Z,
            DIM_COMPASS,
            DIM_COMPASS_SIN,
            DIM_PRESSURE,
            DIM_LIGHT,
            DIM_MAG_X,
            DIM_MAG_Y,
            DIM_MAG_Z,
            DIM_GPS_LAT,
            DIM_GPS_LON,
            DIM_GPS_ALT,
            DIM_PROXIMITY,
            DIM_STEPS,
            DIM_BATTERY,
            DIM_GPS_SPEED,
            DIM_GPS_BEARING,
            DIM_GPS_ACCURACY,
            DIM_AUDIO_ENERGY,
            DIM_AUDIO_PITCH,
        ];
        let ef = [DIM_VIBRATE, DIM_FLASH, DIM_SPEAK, DIM_NOTIFY, DIM_VOLUME, DIM_BRIGHTNESS, DIM_MEDIA, DIM_WIFI];
        for a in &af {
            for e in &ef {
                assert_ne!(a, e);
            }
        }
        for i in 0..af.len() {
            for j in i + 1..af.len() {
                assert_ne!(af[i], af[j]);
            }
        }
        for i in 0..ef.len() {
            for j in i + 1..ef.len() {
                assert_ne!(ef[i], ef[j]);
            }
        }
    }

    #[test]
    fn parse_imu_roundtrip() {
        let json = r#"{"LSM6DSVTR Accelerometer":{"values":[1.0,2.0,11.81]},"LSM6DSVTR Gyroscope":{"values":[0.1,0.2,0.3]}}"#;
        let mut v = vz();
        assert!(parse_imu(json, &mut v));
        assert!((v[DIM_ACCEL_X] - 0.03).abs() < 1e-6);
        assert!((v[DIM_ACCEL_Z] - 0.06).abs() < 1e-6);
        assert!(!parse_imu("NOT JSON", &mut v));
    }

    #[test]
    fn parse_gps_full() {
        let json = r#"{"latitude":50.85,"longitude":4.35,"altitude":52.0,"speed":1.4,"bearing":180.0,"accuracy":8.0}"#;
        let mut v = vz();
        assert!(parse_gps(json, &mut v));
        assert!((v[DIM_GPS_LAT] - 0.05085).abs() < 1e-4);
        assert!((v[DIM_GPS_SPEED] - 0.14).abs() < 1e-4);
        assert!(v[DIM_GPS_ACCURACY] < 0.1);
        // High accuracy = entropy
        let json2 = r#"{"latitude":0,"longitude":0,"altitude":0,"speed":0,"bearing":0,"accuracy":200.0}"#;
        parse_gps(json2, &mut v);
        assert!((v[DIM_GPS_ACCURACY] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parse_mag_and_steps() {
        let mut v = vz();
        assert!(parse_mag(r#"{"AK09918C Magnetometer":{"values":[15.0,-30.0,45.0]}}"#, &mut v));
        assert!((v[DIM_MAG_X] - 0.15).abs() < 1e-6);
        assert!(parse_steps(r#"{"Step Counter":{"values":[4200.0]}}"#, &mut v));
        assert!((v[DIM_STEPS] - 4.2).abs() < 1e-4);
    }

    #[test]
    fn cmd_clamps() {
        let (_, a) = cmd_vibrate(2.0);
        assert!(a[1].parse::<u32>().unwrap() <= 1000);
        let (_, a) = cmd_brightness(1.5);
        assert_eq!(a[0].parse::<u32>().unwrap(), 255);
        let (_, a) = cmd_brightness(-0.5);
        assert_eq!(a[0].parse::<u32>().unwrap(), 0);
    }
}
