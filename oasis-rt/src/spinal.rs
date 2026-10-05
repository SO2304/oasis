//! OASIS-RT — Spinal Cord: auto-discovery of the host body.
//! Zero config. Scans sysfs/devfs or uses static board config.
//! Linux, QNX, VxWorks (runtime). FreeRTOS, Zephyr (compile-time).

use crate::vec::*;
use std::fs;
use std::path::{Path, PathBuf};

// Semantic dimension zones — PURPOSE, not device
pub const ZONE_INERTIAL: (usize, usize) = (10, 19); // accel, gyro
pub const ZONE_ORIENT: (usize, usize) = (20, 29); // compass, mag
pub const ZONE_ENV: (usize, usize) = (30, 39); // pressure, light, temp
pub const ZONE_POSITION: (usize, usize) = (40, 49); // GPS, odometry, steps
pub const ZONE_PERCEPT: (usize, usize) = (50, 59); // audio, camera
pub const ZONE_ACTUATOR: (usize, usize) = (60, 69); // motors, LEDs, speakers
pub const ZONE_RADIO: (usize, usize) = (70, 79); // WiFi, BT, NFC
pub const ZONE_THERMAL: (usize, usize) = (80, 89); // CPU temp, battery
pub const ZONE_UNKNOWN: (usize, usize) = (90, 99); // unclassified devices

/// Returned by `assign_dims` when a semantic zone is full and no dim is free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DimAssignError {
    /// All dims in the requested zone are already used by other devices.
    /// Carries the (start, end) range of the saturated zone.
    ZoneFull(usize, usize),
}

#[derive(Clone, Debug)]
pub enum DeviceKind {
    Accelerometer,
    Gyroscope,
    Magnetometer,
    Barometer,
    LightSensor,
    Temperature,
    Humidity,
    Proximity,
    Led,
    Backlight,
    Motor,
    Gpio,
    Battery,
    StepCounter,
    Gps,
    AudioOut,
    AudioIn,
    WifiRadio,
    Bluetooth,
    Nfc,
    Unknown(String),
}

#[derive(Clone, Debug)]
pub struct DiscoveredDevice {
    pub kind: DeviceKind,
    pub name: String,
    pub path: PathBuf,
    pub readable: bool,
    pub writable: bool,
    pub dims: Vec<usize>,
    pub alive: bool,
    pub read_count: u32,
    pub fail_count: u32,
}

pub struct BodyMap {
    pub devices: Vec<DiscoveredDevice>,
    /// One slot per HyperState dimension. Length == `crate::vec::DIM` (currently 128).
    /// Sized via the kernel's canonical DIM constant rather than a magic number so that
    /// changing DIM in `vec.rs` automatically resizes the body's dim-allocation map.
    pub dim_used: [bool; DIM],
    pub scan_count: u32,
}

impl BodyMap {
    pub fn new() -> Self {
        Self { devices: Vec::new(), dim_used: [false; DIM], scan_count: 0 }
    }

    /// PHASE 1+2: Scan the host, discover and probe all hardware.
    /// Works on any Linux: Android, Pi, desktop, embedded.
    pub fn scan(&mut self) {
        self.scan_count += 1;
        self.scan_iio();
        self.scan_leds();
        self.scan_backlight();
        self.scan_power_supply();
        self.scan_hwmon();
        self.scan_input_devices();
        self.scan_termux_fallback();
    }

    fn scan_iio(&mut self) {
        let iio = Path::new("/sys/bus/iio/devices");
        for entry in fs::read_dir(iio).into_iter().flatten().flatten() {
            let p = entry.path();
            let name = read_sysfs(&p.join("name")).unwrap_or_default();
            let kind = classify_iio(&name);
            if !matches!(kind, DeviceKind::Unknown(_)) {
                self.register(kind, name, p, true, false);
            }
        }
    }

    fn scan_leds(&mut self) {
        for entry in fs::read_dir("/sys/class/leds").into_iter().flatten().flatten() {
            let p = entry.path();
            if p.join("brightness").exists() {
                self.register(DeviceKind::Led, entry.file_name().to_string_lossy().into(), p, true, true);
            }
        }
    }

    fn scan_backlight(&mut self) {
        for entry in fs::read_dir("/sys/class/backlight").into_iter().flatten().flatten() {
            let p = entry.path();
            self.register(DeviceKind::Backlight, entry.file_name().to_string_lossy().into(), p, true, true);
        }
    }

    fn scan_power_supply(&mut self) {
        for entry in fs::read_dir("/sys/class/power_supply").into_iter().flatten().flatten() {
            let p = entry.path();
            if read_sysfs(&p.join("type")).unwrap_or_default().trim() == "Battery" {
                self.register(DeviceKind::Battery, entry.file_name().to_string_lossy().into(), p, true, false);
            }
        }
    }

    fn scan_hwmon(&mut self) {
        for entry in fs::read_dir("/sys/class/hwmon").into_iter().flatten().flatten() {
            let p = entry.path();
            if p.join("temp1_input").exists() {
                let name = read_sysfs(&p.join("name")).unwrap_or_default();
                self.register(DeviceKind::Temperature, name.trim().into(), p, true, false);
            }
        }
    }

    /// Scan /proc/bus/input/devices for input peripherals
    fn scan_input_devices(&mut self) {
        let content = match fs::read_to_string("/proc/bus/input/devices") {
            Ok(c) => c,
            Err(_) => return,
        };
        for block in content.split("\n\n") {
            let name = block
                .lines()
                .find(|l| l.starts_with("N: Name="))
                .map(|l| l.trim_start_matches("N: Name=").trim_matches('"').to_string())
                .unwrap_or_default();
            let handlers = block.lines().find(|l| l.starts_with("H: Handlers=")).unwrap_or("");
            let lname = name.to_lowercase();
            if lname.contains("accel") {
                self.register(DeviceKind::Accelerometer, name, PathBuf::from("/dev/input"), true, false);
            } else if lname.contains("gyro") {
                self.register(DeviceKind::Gyroscope, name, PathBuf::from("/dev/input"), true, false);
            } else if lname.contains("magnet") || lname.contains("compass") {
                self.register(DeviceKind::Magnetometer, name, PathBuf::from("/dev/input"), true, false);
            } else if lname.contains("baro") || lname.contains("pressure") {
                self.register(DeviceKind::Barometer, name, PathBuf::from("/dev/input"), true, false);
            } else if lname.contains("light") || lname.contains("als") {
                self.register(DeviceKind::LightSensor, name, PathBuf::from("/dev/input"), true, false);
            } else if lname.contains("proximity") {
                self.register(DeviceKind::Proximity, name, PathBuf::from("/dev/input"), true, false);
            } else if lname.contains("step") {
                self.register(DeviceKind::StepCounter, name, PathBuf::from("/dev/input"), true, false);
            }
        }
    }

    /// Fallback: detect Termux environment (Android phone)
    fn scan_termux_fallback(&mut self) {
        let termux = Path::new("/data/data/com.termux/files/usr/bin");
        if !termux.exists() {
            return;
        }
        // Collect which kinds already exist to avoid borrow conflict
        let existing: Vec<std::mem::Discriminant<DeviceKind>> = self.devices.iter().map(|d| std::mem::discriminant(&d.kind)).collect();
        let has = |k: &DeviceKind| existing.contains(&std::mem::discriminant(k));
        let p = termux.to_path_buf();
        // Sensors
        let sensors: &[(DeviceKind, &str)] = &[
            (DeviceKind::Accelerometer, "termux-imu"),
            (DeviceKind::Gyroscope, "termux-gyro"),
            (DeviceKind::Barometer, "termux-baro"),
            (DeviceKind::LightSensor, "termux-light"),
            (DeviceKind::Magnetometer, "termux-mag"),
            (DeviceKind::Gps, "termux-gps"),
            (DeviceKind::StepCounter, "termux-steps"),
        ];
        for (kind, name) in sensors {
            if !has(kind) {
                self.register(kind.clone(), name.to_string(), p.clone(), true, false);
            }
        }
        // Actuators
        let actuators: &[(DeviceKind, &str)] = &[(DeviceKind::Motor, "termux-vibrate"), (DeviceKind::AudioOut, "termux-tts")];
        for (kind, name) in actuators {
            if !has(kind) {
                self.register(kind.clone(), name.to_string(), p.clone(), false, true);
            }
        }
    }

    /// PHASE 3: Register a device and auto-assign dimensions.
    /// If the semantic zone for this device kind is saturated, the device is still
    /// registered but with an empty `dims` vector — the caller (or a future
    /// scan iteration) sees `dims.is_empty()` and knows the device is dim-orphan.
    fn register(&mut self, kind: DeviceKind, name: String, path: PathBuf, readable: bool, writable: bool) {
        let dims = self.assign_dims(&kind).unwrap_or_default();
        self.devices
            .push(DiscoveredDevice { kind, name, path, readable, writable, dims, alive: true, read_count: 0, fail_count: 0 });
    }

    /// Auto-assign 1-3 dimensions in the device's semantic zone.
    /// Returns `Err(ZoneFull)` if the zone is saturated (no free dim available).
    /// Callers can fall back to `ZONE_UNKNOWN` or refuse the device.
    pub fn assign_dims(&mut self, kind: &DeviceKind) -> Result<Vec<usize>, DimAssignError> {
        let (zone_start, zone_end) = match kind {
            DeviceKind::Accelerometer | DeviceKind::Gyroscope => ZONE_INERTIAL,
            DeviceKind::Magnetometer => ZONE_ORIENT,
            DeviceKind::Barometer | DeviceKind::LightSensor | DeviceKind::Temperature | DeviceKind::Humidity => ZONE_ENV,
            DeviceKind::Gps | DeviceKind::StepCounter | DeviceKind::Proximity => ZONE_POSITION,
            DeviceKind::AudioIn => ZONE_PERCEPT,
            DeviceKind::Led | DeviceKind::Backlight | DeviceKind::Motor | DeviceKind::AudioOut | DeviceKind::Gpio => ZONE_ACTUATOR,
            DeviceKind::WifiRadio | DeviceKind::Bluetooth | DeviceKind::Nfc => ZONE_RADIO,
            DeviceKind::Battery => ZONE_THERMAL,
            DeviceKind::Unknown(_) => ZONE_UNKNOWN,
        };
        let ndims = match kind {
            DeviceKind::Accelerometer | DeviceKind::Gyroscope | DeviceKind::Magnetometer | DeviceKind::Gps => 3,
            _ => 1,
        };
        let mut assigned = Vec::new();
        for d in zone_start..=zone_end {
            if !self.dim_used[d] {
                self.dim_used[d] = true;
                assigned.push(d);
                if assigned.len() >= ndims {
                    return Ok(assigned);
                }
            }
        }
        if assigned.is_empty() {
            Err(DimAssignError::ZoneFull(zone_start, zone_end))
        } else {
            // Partial assignment: returned what was available (zone got saturated mid-loop).
            Ok(assigned)
        }
    }

    /// RTOS mode: static registration (FreeRTOS, Zephyr, bare-metal).
    /// No filesystem needed. Board config registers devices at compile time.
    /// Works on ANY platform — the universal graft entry point.
    pub fn register_static(&mut self, devices: &[(DeviceKind, &str, bool, bool)]) {
        for (kind, name, readable, writable) in devices {
            self.register(kind.clone(), name.to_string(), PathBuf::new(), *readable, *writable);
        }
    }

    /// QNX / VxWorks: scan /dev/ namespace (POSIX resource managers)
    pub fn scan_devfs(&mut self) {
        for entry in fs::read_dir("/dev").into_iter().flatten().flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(true) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let n = name.to_lowercase();
            let p = entry.path();
            let (kind, r, w) = if n.starts_with("imu") || n.starts_with("accel") {
                (DeviceKind::Accelerometer, true, false)
            } else if n.starts_with("gyro") {
                (DeviceKind::Gyroscope, true, false)
            } else if n.starts_with("baro") || n.starts_with("press") {
                (DeviceKind::Barometer, true, false)
            } else if n.starts_with("mag") {
                (DeviceKind::Magnetometer, true, false)
            } else if n.starts_with("gps") || n.starts_with("gnss") {
                (DeviceKind::Gps, true, false)
            } else if n.starts_with("pwm") || n.starts_with("motor") || n.starts_with("servo") {
                (DeviceKind::Motor, true, true)
            } else if n.starts_with("gpio") {
                (DeviceKind::Gpio, true, true)
            } else if n.starts_with("temp") {
                (DeviceKind::Temperature, true, false)
            } else if n.starts_with("led") {
                (DeviceKind::Led, true, true)
            } else {
                continue;
            };
            self.register(kind, name, p, r, w);
        }
    }

    /// Full scan: tries ALL discovery methods for the current platform
    pub fn scan_all(&mut self) {
        self.scan(); // Linux sysfs + Termux fallback
        self.scan_devfs(); // QNX / VxWorks /dev/ namespace
                           // FreeRTOS/Zephyr: use register_static() from board config
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
    pub fn sensor_count(&self) -> usize {
        self.devices.iter().filter(|d| d.readable).count()
    }
    pub fn actuator_count(&self) -> usize {
        self.devices.iter().filter(|d| d.writable).count()
    }

    /// R15 — entropy contribution from dead devices, in `[0.0, 1.0]`.
    ///
    /// Returns `dead / total` (saturated at 1.0). An EMPTY BodyMap returns `0.0`
    /// by design — not 1.0 — because:
    ///  - "no devices registered yet" ≠ "all devices dead"
    ///  - At boot, before `scan()` runs, the body is *unprobed*, not *broken*.
    ///    Reporting maximum entropy here would falsely trip R14/R15 gates and
    ///    refuse actuation on a kernel that simply hasn't enumerated its
    ///    sensors yet.
    ///  - Vitality (vitality.rs) handles "no vital sensors alive" via its own
    ///    tier FSM; this function is a low-level proxy used only when the
    ///    scan has populated the body.
    pub fn body_entropy(&self) -> f64 {
        if self.devices.is_empty() {
            return 0.0; // unprobed body, not broken — see doc above
        }
        let dead = self.devices.iter().filter(|d| !d.alive).count() as f64;
        let total = self.devices.len() as f64;
        (dead / total).min(1.0)
    }

    /// Human-readable body report
    pub fn report(&self) -> String {
        let mut s = format!("BODY: {} devices ({} sensors, {} actuators)\n", self.device_count(), self.sensor_count(), self.actuator_count());
        for d in &self.devices {
            let dims: Vec<String> = d.dims.iter().map(|d| d.to_string()).collect();
            s += &format!("  {:?} '{}' [{}] {}{} {}\n", d.kind, d.name, dims.join(","), if d.readable { "R" } else { "-" }, if d.writable { "W" } else { "-" }, if d.alive { "OK" } else { "DEAD" });
        }
        s
    }
}

// ─── Helpers ────────────────────────────────────────────────

fn read_sysfs(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn classify_iio(name: &str) -> DeviceKind {
    let n = name.to_lowercase();
    if n.contains("accel") {
        DeviceKind::Accelerometer
    } else if n.contains("gyro") {
        DeviceKind::Gyroscope
    } else if n.contains("magn") || n.contains("compass") {
        DeviceKind::Magnetometer
    } else if n.contains("press") || n.contains("baro") {
        DeviceKind::Barometer
    } else if n.contains("light") || n.contains("als") || n.contains("illum") {
        DeviceKind::LightSensor
    } else if n.contains("temp") {
        DeviceKind::Temperature
    } else if n.contains("humid") {
        DeviceKind::Humidity
    } else if n.contains("prox") {
        DeviceKind::Proximity
    } else {
        DeviceKind::Unknown(name.to_string())
    }
}

/// Pure function: returns the semantic zone for a `DeviceKind`. Extracted from
/// `assign_dims` so Kani can reason about zone boundaries without touching
/// BodyMap state.
#[inline]
pub fn zone_for(kind: &DeviceKind) -> (usize, usize) {
    match kind {
        DeviceKind::Accelerometer | DeviceKind::Gyroscope => ZONE_INERTIAL,
        DeviceKind::Magnetometer => ZONE_ORIENT,
        DeviceKind::Barometer | DeviceKind::LightSensor | DeviceKind::Temperature | DeviceKind::Humidity => ZONE_ENV,
        DeviceKind::Gps | DeviceKind::StepCounter | DeviceKind::Proximity => ZONE_POSITION,
        DeviceKind::AudioIn => ZONE_PERCEPT,
        DeviceKind::Led | DeviceKind::Backlight | DeviceKind::Motor | DeviceKind::AudioOut | DeviceKind::Gpio => ZONE_ACTUATOR,
        DeviceKind::WifiRadio | DeviceKind::Bluetooth | DeviceKind::Nfc => ZONE_RADIO,
        DeviceKind::Battery => ZONE_THERMAL,
        DeviceKind::Unknown(_) => ZONE_UNKNOWN,
    }
}

/// All 9 zones, const-time array for proof/audit.
pub const ALL_ZONES: [(usize, usize); 9] = [ZONE_INERTIAL, ZONE_ORIENT, ZONE_ENV, ZONE_POSITION, ZONE_PERCEPT, ZONE_ACTUATOR, ZONE_RADIO, ZONE_THERMAL, ZONE_UNKNOWN];

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: every semantic zone is well-formed (start ≤ end, end < DIM).
    #[kani::proof]
    fn proof_spinal_zones_well_formed() {
        let i: usize = kani::any();
        kani::assume(i < ALL_ZONES.len());
        let (start, end) = ALL_ZONES[i];
        assert!(start <= end);
        assert!(end < DIM);
    }

    /// PROVE: the 9 semantic zones are PAIRWISE DISJOINT.
    #[kani::proof]
    fn proof_spinal_zones_pairwise_disjoint() {
        let i: usize = kani::any();
        let j: usize = kani::any();
        kani::assume(i < ALL_ZONES.len());
        kani::assume(j < ALL_ZONES.len());
        kani::assume(i != j);
        let (s1, e1) = ALL_ZONES[i];
        let (s2, e2) = ALL_ZONES[j];
        assert!(e1 < s2 || e2 < s1, "zones {} and {} overlap: [{},{}] vs [{},{}]", i, j, s1, e1, s2, e2);
    }

    /// PROVE: `assign_dims` returns dims within the target zone for a few kinds.
    #[kani::proof]
    fn proof_spinal_assign_dims_stay_in_zone() {
        let kind_idx: u8 = kani::any();
        kani::assume(kind_idx < 9);
        let kind = match kind_idx {
            0 => DeviceKind::Accelerometer,
            1 => DeviceKind::Gyroscope,
            2 => DeviceKind::Magnetometer,
            3 => DeviceKind::Barometer,
            4 => DeviceKind::LightSensor,
            5 => DeviceKind::Gps,
            6 => DeviceKind::Led,
            7 => DeviceKind::Motor,
            _ => DeviceKind::Battery,
        };
        let (start, end) = zone_for(&kind);
        assert!(start <= end);
        let mut body = BodyMap::new();
        if let Ok(dims) = body.assign_dims(&kind) {
            for &d in &dims {
                assert!(d >= start && d <= end);
            }
        }
    }

    /// PROVE: classify_iio returns Unknown on empty input.
    #[kani::proof]
    fn proof_spinal_classify_empty_is_unknown() {
        let out = classify_iio("");
        assert!(matches!(out, DeviceKind::Unknown(_)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_body() {
        let bm = BodyMap::new();
        assert_eq!(bm.device_count(), 0);
        assert!((bm.body_entropy() - 0.0).abs() < 1e-10); // no devices = no entropy
    }

    #[test]
    fn classify_iio_sensors() {
        assert!(matches!(classify_iio("lsm6dsvtr-accel"), DeviceKind::Accelerometer));
        assert!(matches!(classify_iio("bmp580-press"), DeviceKind::Barometer));
        assert!(matches!(classify_iio("stk31610-light"), DeviceKind::LightSensor));
        assert!(matches!(classify_iio("ak09918c-magn"), DeviceKind::Magnetometer));
        assert!(matches!(classify_iio("random_chip"), DeviceKind::Unknown(_)));
    }

    #[test]
    fn auto_assign_dims_no_collision() {
        let mut bm = BodyMap::new();
        let d1 = bm.assign_dims(&DeviceKind::Accelerometer).unwrap(); // 3 dims in INERTIAL
        let d2 = bm.assign_dims(&DeviceKind::Gyroscope).unwrap(); // 3 more in INERTIAL
        assert_eq!(d1.len(), 3);
        assert_eq!(d2.len(), 3);
        for a in &d1 {
            assert!(!d2.contains(a));
        }
    }

    #[test]
    fn zone_assignment_semantic() {
        let mut bm = BodyMap::new();
        let accel = bm.assign_dims(&DeviceKind::Accelerometer).unwrap();
        let baro = bm.assign_dims(&DeviceKind::Barometer).unwrap();
        let led = bm.assign_dims(&DeviceKind::Led).unwrap();
        assert!(accel.iter().all(|&d| d >= ZONE_INERTIAL.0 && d <= ZONE_INERTIAL.1));
        assert!(baro.iter().all(|&d| d >= ZONE_ENV.0 && d <= ZONE_ENV.1));
        assert!(led.iter().all(|&d| d >= ZONE_ACTUATOR.0 && d <= ZONE_ACTUATOR.1));
    }

    #[test]
    fn assign_dims_returns_err_when_zone_full() {
        let mut bm = BodyMap::new();
        // Force-saturate ZONE_INERTIAL (10..=19, 10 dims). Each accel/gyro takes 3 → 4 calls fill 12 slots, but cap at 10 → 4th call partial then ZoneFull.
        for d in ZONE_INERTIAL.0..=ZONE_INERTIAL.1 {
            bm.dim_used[d] = true;
        }
        let err = bm.assign_dims(&DeviceKind::Accelerometer).unwrap_err();
        match err {
            DimAssignError::ZoneFull(s, e) => {
                assert_eq!((s, e), ZONE_INERTIAL);
            }
        }
    }

    #[test]
    fn unknown_device_lands_in_zone_unknown() {
        let mut bm = BodyMap::new();
        let dims = bm.assign_dims(&DeviceKind::Unknown("foo".into())).unwrap();
        assert!(dims.iter().all(|&d| d >= ZONE_UNKNOWN.0 && d <= ZONE_UNKNOWN.1));
    }

    #[test]
    fn body_entropy_scales_with_dead() {
        let mut bm = BodyMap::new();
        bm.register(DeviceKind::Accelerometer, "a".into(), PathBuf::new(), true, false);
        bm.register(DeviceKind::Gyroscope, "g".into(), PathBuf::new(), true, false);
        assert!(bm.body_entropy() < 0.01); // both alive
        bm.devices[0].alive = false;
        assert!((bm.body_entropy() - 0.5).abs() < 0.01); // 1/2 dead
        bm.devices[1].alive = false;
        assert!((bm.body_entropy() - 1.0).abs() < 0.01); // all dead
    }

    #[test]
    fn scan_all_no_crash() {
        let mut bm = BodyMap::new();
        bm.scan_all(); // safe on any platform
        assert!(bm.scan_count >= 1);
    }

    #[test]
    fn register_static_rtos() {
        let mut bm = BodyMap::new();
        // FreeRTOS: STM32 + MPU6050 + 2 servos + BME280 + LED
        bm.register_static(&[
            (DeviceKind::Accelerometer, "mpu6050", true, false),
            (DeviceKind::Motor, "servo_pan", false, true),
            (DeviceKind::Motor, "servo_tilt", false, true),
            (DeviceKind::Temperature, "bme280", true, false),
            (DeviceKind::Led, "status_led", false, true),
        ]);
        assert_eq!(bm.device_count(), 5);
        assert_eq!(bm.sensor_count(), 2);
        assert_eq!(bm.actuator_count(), 3);
        let all_dims: Vec<usize> = bm.devices.iter().flat_map(|d| d.dims.clone()).collect();
        for i in 0..all_dims.len() {
            for j in i + 1..all_dims.len() {
                assert_ne!(all_dims[i], all_dims[j]);
            }
        }
        // Zephyr: nRF52840 + LIS2DH + LSM6DS3 + 2 LEDs + BLE
        let mut z = BodyMap::new();
        z.register_static(&[
            (DeviceKind::Accelerometer, "lis2dh@18", true, false),
            (DeviceKind::Gyroscope, "lsm6ds3tr-c@6a", true, false),
            (DeviceKind::Led, "led0", false, true),
            (DeviceKind::Bluetooth, "bt0", true, true),
        ]);
        assert_eq!(z.device_count(), 4);
        assert!(z.body_entropy() < 0.01);
    }

    #[test]
    fn mixed_scan_and_static() {
        let mut bm = BodyMap::new();
        bm.scan_all();
        let before = bm.device_count();
        bm.register_static(&[(DeviceKind::Motor, "canbus_motor", false, true)]);
        assert_eq!(bm.device_count(), before + 1);
        assert!(bm.report().contains("Motor"));
    }
}
