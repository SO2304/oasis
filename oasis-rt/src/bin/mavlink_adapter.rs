//! OASIS-RT — MAVLink ↔ drone_bridge.exe adapter (200 LOC).
//!
//! Listens for MAVLink v2 frames on UDP 14550 (PX4 SITL default), translates to OASIS
//! JSON, pipes to `drone_bridge.exe` stdin. Reads OASIS JSON from bridge stdout,
//! converts to SET_POSITION_TARGET_LOCAL_NED, sends back via UDP.
//!
//! Usage:
//!   cargo run --release --bin mavlink_adapter -- d00 0 cnc,rack1
//!
//! With PX4 SITL running (px4-gazebo):
//!   $ cd PX4-Autopilot && make px4_sitl gazebo
//!   $ cargo run --release --bin mavlink_adapter -- d00 0   # (in another terminal)
//!
//! Without PX4, this binary just waits on UDP. Fake frames can be sent via `nc -u`
//! for smoke tests (see tests/mavlink_adapter_smoke.sh if added).

use oasis_rt::mavlink_min::{
    parse_frame_checked, encode_heartbeat, encode_set_position_target,
    encode_position_setpoint, encode_command_long, encode_set_mode, encode_param_set,
    MavToOasisAccumulator, ReplayState, MavMsg,
};
use std::io::{BufRead, BufReader, Write};
use std::net::UdpSocket;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// Path to drone_bridge binary. On Linux use env var OASIS_BRIDGE_PATH to override.
fn bridge_path() -> String {
    std::env::var("OASIS_BRIDGE_PATH")
        .unwrap_or_else(|_| "target/release/drone_bridge.exe".into())
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let name = args.get(1).cloned().unwrap_or_else(|| "d00".into());
    let did = args.get(2).cloned().unwrap_or_else(|| "0".into());
    let targets = args.get(3).cloned().unwrap_or_default();

    eprintln!("[mavlink_adapter] {} id={} targets={:?}", name, did, targets);

    let sock = UdpSocket::bind("0.0.0.0:14550")?;
    sock.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
    eprintln!("[mavlink_adapter] listening on UDP 14550 for PX4 / QGC");

    let mut bridge_cmd = Command::new(bridge_path());
    bridge_cmd.arg(&name).arg(&did);
    if !targets.is_empty() { bridge_cmd.arg(&targets); }
    let mut bridge = bridge_cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let bridge_stdin = Arc::new(Mutex::new(bridge.stdin.take().unwrap()));
    let bridge_stdout = bridge.stdout.take().unwrap();

    let acc = Arc::new(Mutex::new(MavToOasisAccumulator::default()));

    // Replay state: load from disk if OASIS_REPLAY_STATE_PATH set and file exists.
    let state_path = std::env::var("OASIS_REPLAY_STATE_PATH").ok();
    let initial_state = match &state_path {
        Some(p) => match ReplayState::load(p) {
            Ok(st) => {
                eprintln!("[mavlink_adapter] replay state LOADED from {}", p);
                st
            }
            Err(e) => {
                eprintln!("[mavlink_adapter] replay state load skipped: {} (starting fresh)", e);
                ReplayState::new()
            }
        },
        None => ReplayState::new(),
    };
    let mut initial_state = initial_state;
    // Apply OASIS_MAVLINK_ALLOWED_LINKS="1,2,3" if set (closes attack surface on unknown link_ids)
    if let Ok(s) = std::env::var("OASIS_MAVLINK_ALLOWED_LINKS") {
        let ids: Vec<u8> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        if !ids.is_empty() {
            eprintln!("[mavlink_adapter] allowlist: link_ids={:?}", ids);
            initial_state.set_allowlist(&ids);
        }
    }
    let replay = Arc::new(Mutex::new(initial_state));
    let mut replay_rejects: u32 = 0;

    // Periodic replay-state save (every 10s). Protects against signal-triggered exits
    // (SIGTERM etc.) where main()'s shutdown save wouldn't run.
    if let Some(p) = state_path.clone() {
        let replay_save = replay.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(10));
            let st = replay_save.lock().unwrap();
            let _ = st.save(&p);
        });
    }

    // Closed-loop latency tracking: record sensor-in timestamp, measure time to cmd-out.
    let last_sensor_in = Arc::new(Mutex::new(None::<Instant>));
    let last_cmd_sent = Arc::new(Mutex::new(None::<Instant>));
    // Last SET_POSITION_TARGET values — needed so the fallback thread can
    // retransmit the most recent OASIS command when drone_bridge is quiet.
    // PX4 OFFBOARD requires ≥2 Hz setpoint stream or auto-disarms.
    let last_setpoint: Arc<Mutex<(f32, f32, f32)>> = Arc::new(Mutex::new((0.0, 0.0, 0.0)));
    // Current position (from PX4's LOCAL_POSITION_NED → Accumulator.{x,y,alt}).
    // Updated every incoming frame; read by keepalive thread for navigation control.
    let current_pos: Arc<Mutex<(f32, f32, f32)>> = Arc::new(Mutex::new((0.0, 0.0, 0.0)));
    let last_pos_update: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));

    // Target altitude — read from drone_bridge's `s_alt` output (bridge already emits it).
    // Env var OASIS_TARGET_ALT overrides bridge default. Capped by OASIS_ALT_LIMIT.
    let target_alt: Arc<Mutex<f32>> = Arc::new(Mutex::new(
        std::env::var("OASIS_TARGET_ALT").ok().and_then(|s| s.parse().ok()).unwrap_or(1.3)
    ));

    // Waypoint mission (optional). Format: "x1,y1,z1;x2,y2,z2;..." (NED local frame, z positive = altitude up).
    // On reaching within OASIS_WP_RADIUS (default 0.5m) of current waypoint, advance to next.
    // On last waypoint: hover there. Empty/unset = no waypoint mission, fall back to hover at target_alt.
    let waypoints: Vec<(f32, f32, f32)> = std::env::var("OASIS_WAYPOINTS")
        .ok()
        .map(|s| s.split(';').filter_map(|wp| {
            let parts: Vec<f32> = wp.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            if parts.len() == 3 { Some((parts[0], parts[1], parts[2])) } else { None }
        }).collect())
        .unwrap_or_default();
    let wp_radius: f32 = std::env::var("OASIS_WP_RADIUS").ok().and_then(|s| s.parse().ok()).unwrap_or(0.5);
    let wp_index: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let waypoints_arc = Arc::new(waypoints);

    // Thread B: HEARTBEAT reply every 1s — prevents PX4/GCS from flagging us offline.
    // Optionally, if OASIS_AUTO_OFFBOARD=1, send SET_MODE(OFFBOARD) + COMPONENT_ARM_DISARM
    // after 5s of heartbeat exchange. Used to validate full bidirectional command path.
    let sock_hb = sock.try_clone()?;
    let initial_peer = std::env::var("OASIS_MAV_PEER").ok()
        .and_then(|s| s.parse::<std::net::SocketAddr>().ok());
    let last_peer_hb: Arc<Mutex<Option<std::net::SocketAddr>>> = Arc::new(Mutex::new(initial_peer));
    let last_peer_hb_clone = last_peer_hb.clone();
    let auto_offboard: bool = std::env::var("OASIS_AUTO_OFFBOARD")
        .ok().map(|s| s == "1").unwrap_or(false);
    let _hb_handle = std::thread::spawn(move || {
        let mut hb_seq: u8 = 0;
        let mut cmd_sent = false;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            // HEARTBEAT every tick
            let frame = encode_heartbeat(hb_seq, 1, 1);
            hb_seq = hb_seq.wrapping_add(1);
            let peer_opt = *last_peer_hb_clone.lock().unwrap();
            if let Some(peer) = peer_opt {
                let _ = sock_hb.send_to(&frame, peer);
                // After 5 heartbeats, optionally send OFFBOARD + ARM sequence
                if auto_offboard && !cmd_sent && hb_seq >= 5 {
                    eprintln!("[mavlink_adapter] AUTO_OFFBOARD: sending SET_MODE(OFFBOARD) + ARM");
                    let set_mode = encode_set_mode(hb_seq, 255, 190, 1, 0x01, 0x00060000);
                    let _ = sock_hb.send_to(&set_mode, peer);
                    std::thread::sleep(Duration::from_millis(200));
                    let arm = encode_command_long(hb_seq.wrapping_add(1), 255, 190, 1, 1, 400, 0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
                    let _ = sock_hb.send_to(&arm, peer);

                    // Boost PX4 velocity/acceleration limits for faster waypoint tracking.
                    // PX4's default MPC_XY_VEL_MAX ~12 m/s but jmavsim iris model may override with
                    // smaller values in the airframe. We push 4 m/s which gives ~1-2 m/s real speed.
                    // Opt-in via OASIS_BOOST_PARAMS=1 so non-autonomous flights don't touch params.
                    if std::env::var("OASIS_BOOST_PARAMS").ok().as_deref() == Some("1") {
                        std::thread::sleep(Duration::from_millis(200));
                        let params: &[(&str, f32)] = &[
                            ("MPC_XY_VEL_MAX", 4.0),
                            ("MPC_XY_CRUISE", 3.0),
                            ("MPC_ACC_HOR_MAX", 5.0),
                            ("MPC_Z_VEL_MAX_UP", 2.0),
                            ("MPC_Z_VEL_MAX_DN", 1.5),
                        ];
                        for (i, (name, val)) in params.iter().enumerate() {
                            let s = hb_seq.wrapping_add(2 + i as u8);
                            let frame = encode_param_set(s, 255, 190, 1, 1, name, *val);
                            let _ = sock_hb.send_to(&frame, peer);
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        eprintln!("[mavlink_adapter] BOOST_PARAMS: sent {} PARAM_SET frames (MPC_XY_VEL_MAX=4, etc)", params.len());
                    }

                    cmd_sent = true;
                }
            }
        }
    });

    // Thread C: OFFBOARD setpoint keepalive (10 Hz). PX4 auto-disarms if SET_POSITION_TARGET
    // stream drops below ~2 Hz in OFFBOARD mode. This thread retransmits the last known
    // OASIS command every 100 ms so the stream stays dense even when drone_bridge is idle.
    // Opt-in via OASIS_OFFBOARD_STREAM=1 (default off; avoids flooding for non-OFFBOARD tests).
    let offboard_stream: bool = std::env::var("OASIS_OFFBOARD_STREAM")
        .ok().map(|s| s == "1").unwrap_or(false);
    if offboard_stream {
        let sock_stream = sock.try_clone()?;
        let last_peer_stream = last_peer_hb.clone();
        let last_sp_stream = last_setpoint.clone();
        let current_pos_stream = current_pos.clone();
        let target_alt_stream = target_alt.clone();
        let last_pos_update_stream = last_pos_update.clone();
        let waypoints_stream = waypoints_arc.clone();
        let wp_index_stream = wp_index.clone();
        let alt_kp: f32 = std::env::var("OASIS_ALT_KP").ok().and_then(|s| s.parse().ok()).unwrap_or(0.8);
        let pos_kp: f32 = std::env::var("OASIS_POS_KP").ok().and_then(|s| s.parse().ok()).unwrap_or(0.6);
        let alt_limit: f32 = std::env::var("OASIS_ALT_LIMIT").ok().and_then(|s| s.parse().ok()).unwrap_or(5.0);
        let vz_max: f32 = 1.5;
        let vxy_max: f32 = 2.0;
        let _stream_handle = std::thread::spawn(move || {
            let mut seq: u8 = 128;
            loop {
                std::thread::sleep(Duration::from_millis(100));
                let (bridge_vx, bridge_vy, _) = *last_sp_stream.lock().unwrap();
                let (cx, cy, cz) = *current_pos_stream.lock().unwrap();
                let tgt_alt = *target_alt_stream.lock().unwrap();

                // Safety: if no position update for >500ms, hover in place (zero horiz velocity + alt hold).
                let stale = last_pos_update_stream.lock().unwrap().elapsed() > Duration::from_millis(500);
                if stale {
                    let frame = encode_set_position_target(seq, 1, 1, 1, 1, 0.0, 0.0, 0.0);
                    seq = seq.wrapping_add(1);
                    if let Some(peer) = *last_peer_stream.lock().unwrap() {
                        let _ = sock_stream.send_to(&frame, peer);
                    }
                    continue;
                }

                // Waypoint mission: use POSITION setpoint (type_mask 0x0DF8).
                // PX4's internal position controller handles velocity + acceleration damping.
                // Avoids the velocity-setpoint damping problem observed with type_mask 0x01C7.
                if !waypoints_stream.is_empty() {
                    let mut idx = wp_index_stream.lock().unwrap();
                    let last_idx = waypoints_stream.len() - 1;
                    let (wx, wy, _wz) = waypoints_stream[*idx];
                    let dist = ((wx - cx).powi(2) + (wy - cy).powi(2)).sqrt();
                    if dist < wp_radius && *idx < last_idx {
                        *idx += 1;
                        eprintln!("[mavlink_adapter] WAYPOINT_REACHED idx={}/{} pos=({:.2},{:.2},{:.2})",
                            *idx, last_idx, cx, cy, cz);
                    }
                    let (tx, ty, tz) = waypoints_stream[*idx];
                    let tz_capped = tz.min(alt_limit);
                    // Position setpoint: NED z = -altitude (drone altitude tz → frame z = -tz)
                    let frame = encode_position_setpoint(seq, 1, 1, 1, 1, tx, ty, -tz_capped);
                    if seq % 10 == 0 {
                        eprintln!("[mavlink_adapter] WP_POS idx={} target=({:.1},{:.1},{:.1}) current=({:.2},{:.2},{:.2}) dist={:.2}m",
                            *idx, tx, ty, tz, cx, cy, cz, dist);
                    }
                    seq = seq.wrapping_add(1);
                    if let Some(peer) = *last_peer_stream.lock().unwrap() {
                        let _ = sock_stream.send_to(&frame, peer);
                    }
                    // Silence unused-var warnings on the velocity-path locals
                    let _ = (bridge_vx, bridge_vy, tgt_alt, pos_kp, vxy_max);
                } else {
                    // Fallback: velocity setpoint using bridge dvx/dvy + altitude P-controller
                    let (vx, vy) = (bridge_vx, bridge_vy);
                    let target_z = tgt_alt;
                    let alt_error = target_z - cz;
                    let mut vz_ned = (-alt_kp * alt_error).clamp(-vz_max, vz_max);
                    if cz > alt_limit { vz_ned = 0.5; }
                    let frame = encode_set_position_target(seq, 1, 1, 1, 1, vx, vy, vz_ned);
                    seq = seq.wrapping_add(1);
                    if let Some(peer) = *last_peer_stream.lock().unwrap() {
                        let _ = sock_stream.send_to(&frame, peer);
                    }
                }
            }
        });
    }

    // Thread A: read bridge stdout → convert JSON to SET_POSITION_TARGET → send over UDP
    let sock_tx = sock.try_clone()?;
    let last_sensor_in_tx = last_sensor_in.clone();
    let last_cmd_sent_tx = last_cmd_sent.clone();
    let last_sp_tx = last_setpoint.clone();
    let target_alt_tx = target_alt.clone();
    let tx_handle = std::thread::spawn(move || {
        let reader = BufReader::new(bridge_stdout);
        let mut seq: u8 = 0;
        let mut last_peer: Option<std::net::SocketAddr> = None;
        for line in reader.lines().flatten() {
            let dvx = json_f(&line, "dvx").unwrap_or(0.0) as f32;
            let dvy = json_f(&line, "dvy").unwrap_or(0.0) as f32;
            let abort = line.contains("\"abort\":true");
            let (vx, vy) = if abort { (0.0, 0.0) } else { (dvx, dvy) };
            // Update last known setpoint for keepalive thread
            *last_sp_tx.lock().unwrap() = (vx, vy, 0.0);
            // Read s_alt (target altitude) from drone_bridge output and forward to
            // altitude controller — unless OASIS_TARGET_ALT env var has pinned it.
            if std::env::var("OASIS_TARGET_ALT").is_err() {
                if let Some(s_alt) = json_f(&line, "s_alt") {
                    *target_alt_tx.lock().unwrap() = s_alt as f32;
                }
            }
            let frame = encode_set_position_target(seq, 1, 1, 1, 1, vx, vy, 0.0);
            seq = seq.wrapping_add(1);
            if let Ok(env_peer) = std::env::var("OASIS_MAV_PEER") {
                if let Ok(addr) = env_peer.parse() { last_peer = Some(addr); }
            }
            if let Some(peer) = last_peer {
                let _ = sock_tx.send_to(&frame, peer);
            }
            // Record cmd-out timestamp + optional latency log
            let now = Instant::now();
            *last_cmd_sent_tx.lock().unwrap() = Some(now);
            if let Some(in_t) = *last_sensor_in_tx.lock().unwrap() {
                let latency_us = now.duration_since(in_t).as_micros();
                if std::env::var("OASIS_LOG_LATENCY").is_ok() {
                    eprintln!("[mavlink_adapter] closed_loop_latency_us={}", latency_us);
                }
            }
        }
    });

    // Main thread: read UDP frames → parse → accumulate → emit OASIS JSON
    let mut buf = [0u8; 2048];
    loop {
        let (n, peer) = match sock.recv_from(&mut buf) {
            Ok(r) => r,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                       || e.kind() == std::io::ErrorKind::TimedOut => {
                // No packets; if bridge exited, break
                if let Ok(Some(_)) = bridge.try_wait() { break; }
                continue;
            }
            Err(e) => return Err(e),
        };
        std::env::set_var("OASIS_MAV_PEER", peer.to_string());
        *last_peer_hb.lock().unwrap() = Some(peer);
        // Bytes may contain multiple frames; parse iteratively.
        let mut offset = 0;
        while offset < n {
            let slice = &buf[offset..n];
            // parse_frame_checked rejects replays of signed frames per MAVLink spec.
            // Unsigned frames pass through unchanged.
            let mut state = replay.lock().unwrap();
            let parse_result = parse_frame_checked(slice, &mut state);
            drop(state);
            match parse_result {
                Some((header, msg)) => {
                    *last_sensor_in.lock().unwrap() = Some(Instant::now());
                    // Update current position (x, y, alt) for controllers.
                    // Refresh last_pos_update on every LocalPositionNed so staleness detection works.
                    {
                        let a = acc.lock().unwrap();
                        if matches!(msg, MavMsg::LocalPositionNed { .. } | MavMsg::GlobalPositionInt { .. }) {
                            *current_pos.lock().unwrap() = (a.x as f32, a.y as f32, a.alt as f32);
                            *last_pos_update.lock().unwrap() = Instant::now();
                        }
                    }
                    // Log COMMAND_ACK responses — proves PX4 receives + processes our commands
                    if let MavMsg::CommandAck { command, result } = &msg {
                        let result_name = match result {
                            0 => "ACCEPTED", 1 => "TEMPORARILY_REJECTED", 2 => "DENIED",
                            3 => "UNSUPPORTED", 4 => "FAILED", 5 => "IN_PROGRESS",
                            6 => "CANCELLED", _ => "UNKNOWN"
                        };
                        eprintln!("[mavlink_adapter] COMMAND_ACK cmd={} result={} ({})", command, result, result_name);
                    }
                    // Log PARAM_VALUE responses (confirms PX4 applied our PARAM_SET requests)
                    if let MavMsg::ParamValue { param_id, param_value, .. } = &msg {
                        let end = param_id.iter().position(|&b| b == 0).unwrap_or(16);
                        if let Ok(name) = std::str::from_utf8(&param_id[..end]) {
                            // Log only MPC_* params (the ones we care about for flight tuning)
                            if name.starts_with("MPC_") {
                                eprintln!("[mavlink_adapter] PARAM_VALUE {}={:.3}", name, param_value);
                            }
                        }
                    }
                    {
                        let mut a = acc.lock().unwrap();
                        a.apply(&msg);
                        if a.ready_to_emit() {
                            let json = a.to_oasis_json();
                            let mut stdin = bridge_stdin.lock().unwrap();
                            let _ = writeln!(stdin, "{}", json);
                            let _ = stdin.flush();
                        }
                    }
                    let has_sig = (slice[2] & 0x01) != 0;
                    let adv = 10 + header.len as usize + 2 + if has_sig { 13 } else { 0 };
                    offset += adv;
                }
                None => {
                    // Could be malformed OR replay — if slice looks like a signed frame, count reject.
                    if slice.len() > 2 && slice[0] == 0xFD && (slice[2] & 0x01) != 0 {
                        replay_rejects += 1;
                        if replay_rejects == 1 || replay_rejects % 50 == 0 {
                            eprintln!("[mavlink_adapter] REPLAY_REJECT count={}", replay_rejects);
                        }
                        // Advance past this frame so we don't loop on it
                        if slice.len() > 1 {
                            let len = slice[1] as usize;
                            offset += 10 + len + 2 + 13;
                            continue;
                        }
                    }
                    break;
                }
            }
        }
    }

    let _ = tx_handle.join();
    let _ = bridge.wait();

    // On shutdown, persist replay state so a restart doesn't re-accept stale signed frames.
    if let Some(p) = state_path {
        let st = replay.lock().unwrap();
        if let Err(e) = st.save(&p) {
            eprintln!("[mavlink_adapter] replay state save failed: {}", e);
        } else {
            eprintln!("[mavlink_adapter] replay state saved to {}", p);
        }
    }
    Ok(())
}

fn json_f(line: &str, key: &str) -> Option<f64> {
    let pat = format!("\"{}\":", key);
    let start = line.find(&pat)? + pat.len();
    let rest = &line[start..];
    let end = rest.find(|c: char| c == ',' || c == '}' || c == ' ')?;
    rest[..end].trim().parse().ok()
}
