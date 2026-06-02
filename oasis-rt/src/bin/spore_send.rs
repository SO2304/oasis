//! OASIS — Spore Sender (CLI)
//!
//! Broadcasts FederatedMesh digests via UDP. Supports:
//!   - Multicast (default 239.0.42.1:4200) for local LAN auto-discovery
//!   - Unicast to specific peer
//!   - v2 fragmentation + XOR-parity FEC for lossy long-range links
//!   - Sender-side N-fold repeat (cheap redundancy on top of FEC)
//!   - QR code fallback for offline / out-of-band transfer
//!
//! Usage:
//!   spore_send                                    # multicast, single send
//!   spore_send --target 192.168.1.50:4200         # unicast
//!   spore_send --fragmented                       # v2 fragments (no FEC)
//!   spore_send --fragmented --fec                 # v2 fragments + XOR parity
//!   spore_send --fragmented --fec --repeat 3      # 3x send each chunk
//!   spore_send --qr                               # print base64 QR payload
//!
//! Environment:
//!   OASIS_SPORE_TARGET — overrides --target if --target not provided
//!   OASIS_SPORE_TRUST  — trust value to embed (default 0.5; receiver-side uses)

use oasis_rt::federation::FederatedMesh;
use oasis_rt::spore;
use oasis_rt::vec::*;
use std::process::ExitCode;

const DEFAULT_MULTICAST: &str = "239.0.42.1:4200";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cfg = match parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {}", e);
            print_usage();
            return ExitCode::from(2);
        }
    };
    if cfg.help {
        print_usage();
        return ExitCode::SUCCESS;
    }

    let mut fed = build_demo_mesh();
    eprintln!("[spore_send] {} digests built", fed.digest_count());

    let target = cfg.target.clone()
        .or_else(|| std::env::var("OASIS_SPORE_TARGET").ok())
        .unwrap_or_else(|| DEFAULT_MULTICAST.to_string());

    if cfg.qr {
        match spore::encode_qr(&fed, 10) {
            Ok(qr) => {
                println!("{}", qr);
                eprintln!("[spore_send] QR: {} chars ({} digests)", qr.len(), fed.digest_count());
                return ExitCode::SUCCESS;
            }
            Err(e) => { eprintln!("[spore_send] QR encode failed: {}", e); return ExitCode::FAILURE; }
        }
    }

    let result = if cfg.fragmented {
        spore::broadcast_v2(&fed, &target, cfg.fec, cfg.repeat)
            .map(|n| format!("v2 fragmented: {} bytes (fec={}, repeat={}) → {}", n, cfg.fec, cfg.repeat, target))
    } else if target == DEFAULT_MULTICAST {
        spore::broadcast(&fed)
            .map(|n| format!("v1 multicast: {} bytes → {}", n, DEFAULT_MULTICAST))
    } else {
        spore::unicast(&fed, &target)
            .map(|n| format!("v1 unicast: {} bytes → {}", n, target))
    };

    let _ = &mut fed; // suppress unused-mut warning when no QR path taken
    match result {
        Ok(msg) => {
            eprintln!("[spore_send] sent: {}", msg);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("[spore_send] send failed: {}", e);
            ExitCode::FAILURE
        }
    }
}

#[derive(Default)]
struct Cfg {
    help: bool,
    qr: bool,
    fragmented: bool,
    fec: bool,
    repeat: u8,
    target: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Cfg, String> {
    let mut cfg = Cfg { repeat: 1, ..Default::default() };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => { cfg.help = true; i += 1; }
            "--qr" => { cfg.qr = true; i += 1; }
            "--fragmented" => { cfg.fragmented = true; i += 1; }
            "--fec" => { cfg.fec = true; i += 1; }
            "--repeat" => {
                let v = args.get(i + 1).ok_or("--repeat needs value")?;
                cfg.repeat = v.parse().map_err(|_| format!("--repeat: bad u8 {}", v))?;
                if cfg.repeat == 0 { return Err("--repeat must be ≥ 1".into()); }
                i += 2;
            }
            "--target" => {
                cfg.target = Some(args.get(i + 1).ok_or("--target needs value")?.clone());
                i += 2;
            }
            other => return Err(format!("unknown arg: {}", other)),
        }
    }
    if cfg.fec && !cfg.fragmented {
        return Err("--fec requires --fragmented (XOR parity is per-chunk)".into());
    }
    Ok(cfg)
}

fn print_usage() {
    eprintln!("Usage: spore_send [OPTIONS]");
    eprintln!("  --target HOST:PORT  Send to specific peer (default: multicast 239.0.42.1:4200)");
    eprintln!("  --fragmented        Use v2 fragmented protocol (MTU-safe)");
    eprintln!("  --fec               Add XOR-parity chunk (recovers 1 lost chunk; needs --fragmented)");
    eprintln!("  --repeat N          Send each chunk N times (lossy links)");
    eprintln!("  --qr                Print base64 QR payload to stdout");
    eprintln!("  -h, --help          Show this help");
    eprintln!();
    eprintln!("Env: OASIS_SPORE_TARGET overrides --target if not given.");
}

fn build_demo_mesh() -> FederatedMesh {
    let mut fed = FederatedMesh::new();
    let mut a1 = vz(); a1[10] = 0.1; a1[12] = 0.01; a1[35] = 0.5;
    fed.pool_push_test(a1, 0.3, 1.0, 0.3);
    let mut a2 = vz(); a2[50] = 0.8; a2[52] = 0.6; a2[54] = 0.3;
    fed.pool_push_test(a2, 0.5, 1.0, 0.2);
    let mut a3 = vz(); a3[50] = -0.5; a3[51] = 0.7;
    fed.pool_push_test(a3, 0.4, -1.0, 0.6);
    fed
}
