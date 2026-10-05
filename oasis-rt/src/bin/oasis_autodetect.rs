//! OASIS — Zero-driver hardware auto-detection CLI
//!
//! Scans the host OS at runtime and enumerates every sensor + actuator the
//! OASIS kernel can talk to WITHOUT a vendor driver crate. Output:
//!
//! - 1 line per discovered device: kind, name, dims, path, R/W
//! - summary stats (sensor count, actuator count, body entropy)
//!
//! Scan strategy (Linux / Android-Termux):
//!   - /sys/bus/iio/devices/        (accel, gyro, baro, ALS, etc.)
//!   - /sys/class/leds/             (LEDs)
//!   - /sys/class/backlight/        (display backlight)
//!   - /sys/class/power_supply/     (battery)
//!   - /sys/class/hwmon/            (temp sensors, fans)
//!   - /proc/bus/input/devices      (input peripherals)
//!   - /dev (devfs)                 (gpio, serial, etc.)
//!   - Termux fallback              (/data/data/com.termux — phone sensors)
//!
//! On Windows/macOS: runs but finds nothing since these paths don't exist.
//! That's HONEST: OASIS zero-driver works on Unix-family systems with sysfs.

use oasis_rt::spinal::{BodyMap, DeviceKind};

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║ OASIS — Zero-driver hardware detection                    ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    let mut body = BodyMap::new();
    let t0 = std::time::Instant::now();
    body.scan_all();
    let dt = t0.elapsed();

    println!("Scan time:          {:.2} ms", dt.as_secs_f64() * 1000.0);
    println!("Devices discovered: {}", body.device_count());
    println!("  sensors:          {}", body.sensor_count());
    println!("  actuators:        {}", body.actuator_count());
    println!("  body entropy:     {:.3}", body.body_entropy());
    println!();

    if body.device_count() == 0 {
        println!("No devices discovered. This is expected on:");
        println!("  - Windows / macOS without sysfs");
        println!("  - Sandboxed environments without /sys access");
        println!("  - Embedded MCUs (use BodyMap::register_static at compile time)");
        println!();
        println!("On a real Linux host or Android phone you would see:");
        println!("  - IMU (accel/gyro/magn) via /sys/bus/iio/devices/");
        println!("  - ALS, barometer, thermal sensors");
        println!("  - LEDs, backlight, haptic motors");
        println!("  - Battery state");
        println!("  - Audio in/out, GPS (via Termux-API on Android)");
        std::process::exit(0);
    }

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║ DISCOVERED DEVICES                                        ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    for d in &body.devices {
        let rw = match (d.readable, d.writable) {
            (true, true) => "R/W",
            (true, false) => "R/-",
            (false, true) => "-/W",
            (false, false) => "-/-",
        };
        let kind_s = match &d.kind {
            DeviceKind::Accelerometer => "accelerometer",
            DeviceKind::Gyroscope => "gyroscope",
            DeviceKind::Magnetometer => "magnetometer",
            DeviceKind::Barometer => "barometer",
            DeviceKind::LightSensor => "light",
            DeviceKind::Temperature => "temperature",
            DeviceKind::Humidity => "humidity",
            DeviceKind::Proximity => "proximity",
            DeviceKind::Led => "led",
            DeviceKind::Backlight => "backlight",
            DeviceKind::Motor => "motor",
            DeviceKind::Gpio => "gpio",
            DeviceKind::Battery => "battery",
            DeviceKind::StepCounter => "step_counter",
            DeviceKind::Gps => "gps",
            DeviceKind::AudioOut => "audio_out",
            DeviceKind::AudioIn => "audio_in",
            DeviceKind::WifiRadio => "wifi",
            DeviceKind::Bluetooth => "bluetooth",
            DeviceKind::Nfc => "nfc",
            DeviceKind::Unknown(s) => s.as_str(),
        };
        let dims = d.dims.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(",");
        println!("  {:<15} {:<30} [{:<3}] dims=[{}]", kind_s, truncate(&d.name, 30), rw, dims);
        println!("    path: {}", d.path.display());
    }

    println!();
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║ WHAT THIS PROVES                                          ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!("OASIS has mapped {} hardware endpoints without a single driver crate.", body.device_count());
    println!("Each device is routed to a semantic tensor zone (inertial/env/etc.);");
    println!("no per-vendor IDL, no rclcpp::hardware_interface template.");
    println!();
    println!("ROS 2 equivalent would require:");
    println!("  - individual driver package per vendor (e.g., bno055_driver,");
    println!("    xsens_driver, realsense_ros...)");
    println!("  - URDF + hardware_interface configuration");
    println!("  - launch file gluing them together");
    println!();
    println!("Honest caveat: zero-driver applies to sysfs-exposed hardware.");
    println!("Vendor-proprietary protocols (e.g., Ouster OS1 lidar over UDP");
    println!("with custom packet format) still need a driver. OASIS auto-detects");
    println!("EVERYTHING the kernel surfaces via sysfs/devfs/Termux.");
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n.saturating_sub(1)])
    }
}
