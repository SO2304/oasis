//! OASIS-RT — MAVLink UDP sniffer for integration validation.
//!
//! Listens on UDP 14550, parses incoming MAVLink v2 frames using oasis_rt::mavlink_min,
//! prints parse stats per message type. Used to validate CRC and frame structure against
//! canonical pymavlink-generated frames.
//!
//! Usage:
//!   cargo run --release --bin mavlink_sniff
//!
//! Prints summary every 50 frames to stdout. Exits on SIGINT or after STOP_AFTER frames.

use oasis_rt::mavlink_min::{parse_frame, MavMsg};
use std::collections::BTreeMap;
use std::net::UdpSocket;

fn main() -> std::io::Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:14550")?;
    sock.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
    eprintln!("[mavlink_sniff] listening on UDP 14550");

    let stop_after: u32 = std::env::var("STOP_AFTER")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(300);

    let mut counts: BTreeMap<u32, u32> = BTreeMap::new();
    let mut parsed = 0u32;
    let mut rejected = 0u32;
    let mut buf = [0u8; 2048];

    let t0 = std::time::Instant::now();

    while parsed + rejected < stop_after {
        let (n, _peer) = match sock.recv_from(&mut buf) {
            Ok(x) => x,
            Err(_) => break,
        };
        let mut off = 0;
        while off < n {
            let slice = &buf[off..n];
            match parse_frame(slice) {
                Some((h, msg)) => {
                    parsed += 1;
                    *counts.entry(h.msgid).or_insert(0) += 1;
                    off += 10 + h.len as usize + 2;
                    // Log first occurrence of each msg type
                    if counts[&h.msgid] == 1 {
                        eprintln!("[mavlink_sniff] FIRST parse msgid={} type={}", h.msgid, msg_name(&msg));
                    }
                }
                None => {
                    rejected += 1;
                    // Frame didn't parse — advance past this byte to resync
                    off += 1;
                }
            }
        }
    }

    let elapsed = t0.elapsed().as_secs_f64();
    println!("=== MAVLink sniff report ===");
    println!("Parsed frames:   {}", parsed);
    println!("Rejected frames: {}", rejected);
    println!("Elapsed:         {:.2}s", elapsed);
    println!("Throughput:      {:.1} fps", parsed as f64 / elapsed.max(0.001));
    println!("\nBy msgid:");
    for (mid, n) in &counts {
        println!("  {:>4} ({}) : {}", mid, msgid_name(*mid), n);
    }

    if parsed == 0 {
        eprintln!("FAIL: no frames parsed");
        std::process::exit(1);
    }
    if rejected > parsed / 10 {
        eprintln!("WARN: >10% rejection rate ({} / {})", rejected, parsed + rejected);
    }
    Ok(())
}

fn msg_name(m: &MavMsg) -> &'static str {
    match m {
        MavMsg::Heartbeat { .. } => "Heartbeat",
        MavMsg::Attitude { .. } => "Attitude",
        MavMsg::AttitudeQuaternion { .. } => "AttitudeQuaternion",
        MavMsg::GlobalPositionInt { .. } => "GlobalPositionInt",
        MavMsg::LocalPositionNed { .. } => "LocalPositionNed",
        MavMsg::VfrHud { .. } => "VfrHud",
        MavMsg::DistanceSensor { .. } => "DistanceSensor",
        MavMsg::SysStatus { .. } => "SysStatus",
        MavMsg::CommandAck { .. } => "CommandAck",
        MavMsg::Unknown { .. } => "Unknown",
    }
}

fn msgid_name(id: u32) -> &'static str {
    match id {
        0 => "HEARTBEAT",
        1 => "SYS_STATUS",
        30 => "ATTITUDE",
        31 => "ATTITUDE_QUATERNION",
        32 => "LOCAL_POSITION_NED",
        33 => "GLOBAL_POSITION_INT",
        74 => "VFR_HUD",
        76 => "COMMAND_LONG",
        77 => "COMMAND_ACK",
        11 => "SET_MODE",
        84 => "SET_POSITION_TARGET_LOCAL_NED",
        132 => "DISTANCE_SENSOR",
        _ => "(unknown)",
    }
}
