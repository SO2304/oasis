//! `px4_order_arm` — B1 on PX4 SITL: a signed OASIS order travels inside a MAVLink
//! `V2_EXTENSION`, the Part F gate decides, and **only** an `Act` becomes
//! `MAV_CMD_COMPONENT_ARM_DISARM` for PX4.
//!
//! Two roles, so the carrier crosses a **real socket** instead of an in-process call:
//!
//! ```text
//! commander ──V2_EXTENSION(v0B envelope)──▶ vehicle ──COMMAND_LONG(400)──▶ PX4 SITL
//! ```
//!
//! ```text
//! px4_order_arm vehicle   [--listen 127.0.0.1:14560] [--px4 127.0.0.1:18570]
//!                         [--bind 0.0.0.0:14550] [--revoked] [--boot-id N] [--seconds N]
//! px4_order_arm commander --case valid|forged|tamper|replay [--to 127.0.0.1:14560]
//!                         [--boot-id N] [--seq N]
//! ```
//!
//! ⚠️ **Test keys.** Both roles derive their Ed25519 identities from the deterministic
//! seeds the unit tests use (`fp(1)`/`ed_seed(1)` for the commander, `fp(2)`/`ed_seed(2)`
//! for the vehicle). This is a demonstration harness, not a deployment.
//!
//! ⚠️ **Shared clock.** The actuator clock here is wall-clock milliseconds, which both
//! processes read independently, and `boot_id` is a flag given to both. That is honest for
//! two processes on one machine and it is **not** how a real commander learns the
//! actuator's clock — the answer to that is part K (`TimeView` over a signed `OTM1`
//! beacon), and it is deliberately not reused here so this binary stays small.

use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use oasis_rt::actuation::{encode_oac1, parse_oac1_any, ActCommand, Decision};
use oasis_rt::mavlink_min::{encode_heartbeat, parse_frame, MavMsg};
use oasis_rt::mavlink_order::{
    encode_v2_extension, extract_envelope, ArmContext, ArmRules, Vehicle, OASIS_MESSAGE_TYPE,
};
use oasis_rt::mesh::{
    inner_slice, mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter,
    FP_LEN, MESH_V0B_NETWORK_LEN,
};

const NET: [u8; MESH_V0B_NETWORK_LEN] = *b"OASISnet";
const VEHICLE_ID: u16 = 1;
/// `MAV_MODE_FLAG_SAFETY_ARMED`, bit 7 of `HEARTBEAT.base_mode`.
const SAFETY_ARMED: u8 = 0x80;

fn fp(i: u8) -> [u8; FP_LEN] {
    [i, 0, 0, 0, 0, 0, 0, 0]
}

fn ed_seed(byte: u8) -> MeshEdSeed {
    let mut s = [0u8; 32];
    for (i, b) in s.iter_mut().enumerate() {
        *b = byte.wrapping_add(i as u8);
    }
    MeshEdSeed(s)
}

fn registry(entries: &[([u8; FP_LEN], &MeshEdSeed)]) -> MeshPubRegistry {
    let mut r = MeshPubRegistry::new();
    for (f, seed) in entries {
        r.insert(*f, mesh_v10_pubkey_from_seed(seed).unwrap());
    }
    r
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let role = args.get(1).map(String::as_str).unwrap_or("");
    let boot_id: u64 = arg(&args, "--boot-id").and_then(|v| v.parse().ok()).unwrap_or(7);
    match role {
        "vehicle" => vehicle(&args, boot_id),
        "commander" => commander(&args, boot_id),
        _ => {
            eprintln!(
                "usage:\n  px4_order_arm vehicle   [--listen A] [--px4 A] [--revoked] \
                 [--boot-id N] [--seconds N]\n  px4_order_arm commander --case \
                 valid|forged|tamper|replay [--to A] [--boot-id N] [--seq N]"
            );
            std::process::exit(2);
        }
    }
}

// ─── the vehicle side: carrier → v0B → gate → PX4 ───

fn vehicle(args: &[String], boot_id: u64) {
    let listen: SocketAddr = arg(args, "--listen")
        .unwrap_or_else(|| "127.0.0.1:14560".into())
        .parse()
        .expect("--listen");
    let px4: SocketAddr = arg(args, "--px4")
        .unwrap_or_else(|| "127.0.0.1:18570".into())
        .parse()
        .expect("--px4");
    let revoked = flag(args, "--revoked");
    let secs: u64 = arg(args, "--seconds").and_then(|v| v.parse().ok()).unwrap_or(30);
    // PX4 SITL starts mavlink with `-f`, i.e. broadcasting to the remote GCS port rather
    // than replying to whoever contacted it. Binding an ephemeral port therefore receives
    // nothing: the first run reported px4_heartbeats=0 and armed=false while PX4's own log
    // said "Armed by external command". Bind 14550 and the armed bit is observable here.
    let bind_px4: SocketAddr = arg(args, "--bind")
        .unwrap_or_else(|| "0.0.0.0:14550".into())
        .parse()
        .expect("--bind");

    let from_commander = UdpSocket::bind(listen).expect("bind --listen");
    from_commander.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    let to_px4 = UdpSocket::bind(bind_px4).expect("bind px4 socket");
    to_px4.set_read_timeout(Some(Duration::from_millis(100))).unwrap();

    let mut router = MeshRouter::new_v0b(fp(2), NET, ed_seed(2), registry(&[(fp(1), &ed_seed(1))]));
    let mut veh = Vehicle::new(ArmRules { vehicle_id: VEHICLE_ID });

    println!("VEHICLE listen={listen} px4={px4} bind={bind_px4} revoked={revoked} boot_id={boot_id}");
    println!("VEHICLE fp=0201..  trusts commander fp=0101..  network={:?}", NET);

    let start = Instant::now();
    let mut hb_seq: u8 = 0;
    let mut last_hb = Instant::now() - Duration::from_secs(2);
    let mut armed_seen = false;
    let mut px4_heartbeats = 0u32;
    let mut buf = [0u8; 2048];

    while start.elapsed() < Duration::from_secs(secs) {
        // A GCS-style heartbeat, so PX4 accepts commands from us.
        if last_hb.elapsed() >= Duration::from_secs(1) {
            let f = encode_heartbeat(hb_seq, 255, 190);
            let _ = to_px4.send_to(&f, px4);
            hb_seq = hb_seq.wrapping_add(1);
            last_hb = Instant::now();
        }

        // PX4 → us: track the armed bit.
        if let Ok((n, _)) = to_px4.recv_from(&mut buf) {
            let mut off = 0usize;
            while off < n {
                match parse_frame(&buf[off..n]) {
                    Some((h, MavMsg::Heartbeat { base_mode, .. })) => {
                        px4_heartbeats += 1;
                        let armed = base_mode & SAFETY_ARMED != 0;
                        if armed && !armed_seen {
                            armed_seen = true;
                            println!(
                                "PX4 ARMED  base_mode=0x{base_mode:02x} after {} ms",
                                start.elapsed().as_millis()
                            );
                        }
                        off += 12 + h.len as usize;
                    }
                    Some((h, _)) => off += 12 + h.len as usize,
                    None => break,
                }
            }
        }

        // commander → us: a V2_EXTENSION carrying a v0B envelope.
        if let Ok((n, src)) = from_commander.recv_from(&mut buf) {
            let env = match extract_envelope(&buf[..n], OASIS_MESSAGE_TYPE) {
                Some(e) => e,
                None => {
                    println!("RX {src} NOT_OURS bytes={n}");
                    continue;
                }
            };
            let arrived = match router.process(&env) {
                MeshDecision::Drop(why) => {
                    println!("RX {src} MESH_DROP why={why:?} -> no ARM frame");
                    continue;
                }
                MeshDecision::Arrived { envelope, .. } => envelope,
            };
            let (cmd, class) = match parse_oac1_any(inner_slice(&arrived)) {
                Some(x) => x,
                None => {
                    println!("RX {src} NOT_AN_ORDER -> no ARM frame");
                    continue;
                }
            };
            let ctx = ArmContext {
                v0b_ok: true,
                authorized: true,
                revoked,
                actuator_boot_id: boot_id,
                now_ms: now_ms(),
                r14_safe: true,
            };
            let (d, action, frame) = veh.decide(&ctx, &cmd, class);
            match (&d, frame) {
                (Decision::Act, Some(f)) => {
                    let sent = to_px4.send_to(&f, px4).map(|n| n).unwrap_or(0);
                    println!(
                        "GATE Act seq={} action={:?} -> COMMAND_LONG(400) sent, {sent} bytes",
                        cmd.cmd_seq, action
                    );
                }
                (Decision::Reject(r), None) => {
                    println!("GATE Reject({r:?}) seq={} -> no ARM frame", cmd.cmd_seq)
                }
                other => println!("GATE UNEXPECTED {other:?}"),
            }
        }
    }

    println!(
        "VEHICLE done: executed={} rejects={:?} px4_heartbeats={} armed={}",
        veh.act.executed, veh.act.rejects, px4_heartbeats, armed_seen
    );
}

// ─── the commander side: build a case, send one frame ───

fn commander(args: &[String], boot_id: u64) {
    let to: SocketAddr = arg(args, "--to")
        .unwrap_or_else(|| "127.0.0.1:14560".into())
        .parse()
        .expect("--to");
    let case = arg(args, "--case").unwrap_or_else(|| "valid".into());
    let seq: u32 = arg(args, "--seq").and_then(|v| v.parse().ok()).unwrap_or(1);

    // An ARM order: force = 1.0 is "arm", and it must name this vehicle.
    let cmd = ActCommand {
        actuator_id: VEHICLE_ID,
        cmd_seq: seq,
        boot_id,
        deadline_ms: now_ms() + 3_000,
        force: 1.0,
        torque: 0.0,
        velocity: 0.0,
        pos: [0.0, 0.0, 0.0],
    };

    // "forged" signs with a key the vehicle's registry does not hold.
    let (my_fp, my_seed) = if case == "forged" { (fp(3), ed_seed(3)) } else { (fp(1), ed_seed(1)) };
    let mut origin = MeshRouter::new_v0b(my_fp, NET, my_seed, registry(&[]));
    let mut env = origin.origin_wrap_v0b(&encode_oac1(&cmd)).expect("origin_wrap_v0b");

    if case == "tamper" {
        // Flip one bit of the order AFTER signing: the v0A attack v0B exists to stop.
        let n = env.len();
        env[n - 1] ^= 0x01;
    }

    let frame = encode_v2_extension(0, 1, 191, 0, 1, 1, OASIS_MESSAGE_TYPE, &env)
        .expect("envelope fits in V2_EXTENSION");

    let sock = UdpSocket::bind("0.0.0.0:0").expect("bind");
    let times = if case == "replay" { 2 } else { 1 };
    for i in 0..times {
        let n = sock.send_to(&frame, to).expect("send");
        println!(
            "COMMANDER case={case} seq={seq} boot_id={boot_id} send#{} {n} bytes -> {to}",
            i + 1
        );
        std::thread::sleep(Duration::from_millis(300));
    }
    println!("COMMANDER done (envelope {} B, frame {} B)", env.len(), frame.len());
}
