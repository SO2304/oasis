//! OASIS — Operator revocation broadcaster.
//!
//! Signs a revocation list with the operator's Ed25519 key and either writes
//! it to a file (for local load via `OASIS_REVOCATION_FILE`) or broadcasts
//! it over UDP as a SPORE\x06 envelope.
//!
//! Usage:
//!   # Revoke two drones and broadcast via multicast
//!   spore_revoke \
//!     --op-seed-hex $OP_KEY \
//!     --revoke 4A-3F-..,19-8E-.. \
//!     --broadcast 239.0.42.1:4200
//!
//!   # Write signed blob to file for out-of-band distribution
//!   spore_revoke \
//!     --op-seed-hex $OP_KEY \
//!     --revoke 4A-3F-..,19-8E-.. \
//!     --write-file /etc/oasis/revoke.bin
//!
//!   # Merge with existing list on disk then re-sign + write
//!   spore_revoke \
//!     --op-seed-hex $OP_KEY \
//!     --op-pub-hex $OP_PUB \
//!     --load-file /etc/oasis/revoke.bin \
//!     --revoke 99-00-.. \
//!     --write-file /etc/oasis/revoke.bin

use oasis_rt::spore_crypto;
use std::net::UdpSocket;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut op_seed_hex: Option<String> = None;
    let mut op_pub_hex: Option<String> = None;
    let mut revoke_fps: Vec<String> = Vec::new();
    let mut broadcast_target: Option<String> = None;
    let mut write_file: Option<String> = None;
    let mut load_file: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--op-seed-hex" => { op_seed_hex = Some(args.get(i+1).cloned().unwrap_or_default()); i += 2; }
            "--op-pub-hex" => { op_pub_hex = Some(args.get(i+1).cloned().unwrap_or_default()); i += 2; }
            "--revoke" => {
                for fp in args.get(i+1).map(|s| s.clone()).unwrap_or_default().split(',') {
                    revoke_fps.push(fp.trim().to_string());
                }
                i += 2;
            }
            "--broadcast" => { broadcast_target = Some(args.get(i+1).cloned().unwrap_or_default()); i += 2; }
            "--write-file" => { write_file = Some(args.get(i+1).cloned().unwrap_or_default()); i += 2; }
            "--load-file" => { load_file = Some(args.get(i+1).cloned().unwrap_or_default()); i += 2; }
            "-h" | "--help" => { print_usage(); return ExitCode::SUCCESS; }
            other => { eprintln!("unknown arg: {}", other); print_usage(); return ExitCode::from(2); }
        }
    }

    let op_seed = match op_seed_hex.as_deref().and_then(parse_hex32) {
        Some(s) => s,
        None => { eprintln!("error: --op-seed-hex (64 hex chars) required"); return ExitCode::from(2); }
    };

    // Start from existing list if --load-file given
    let mut list = spore_crypto::RevocationList::new();
    if let Some(path) = &load_file {
        let op_pub = op_pub_hex.as_deref().and_then(parse_hex32).unwrap_or_else(|| {
            eprintln!("error: --op-pub-hex required when --load-file given");
            std::process::exit(2);
        });
        match spore_crypto::load_revocation_file(path, &op_pub) {
            Ok(l) => { eprintln!("[spore_revoke] loaded {} existing revocations from {}", l.len(), path); list = l; }
            Err(e) => { eprintln!("error: load_revocation_file failed: {}", e); return ExitCode::FAILURE; }
        }
    }

    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut added = 0usize;
    for fp_str in &revoke_fps {
        match parse_fp8(fp_str) {
            Some(fp) => {
                if list.revoke(fp, now) { added += 1; }
            }
            None => { eprintln!("error: bad fingerprint '{}'", fp_str); return ExitCode::from(2); }
        }
    }
    eprintln!("[spore_revoke] {} added, {} total revocations", added, list.len());

    let signed = match list.serialize_signed(&op_seed) {
        Ok(b) => b,
        Err(e) => { eprintln!("error: sign failed: {}", e); return ExitCode::FAILURE; }
    };

    if let Some(path) = &write_file {
        if let Err(e) = std::fs::write(path, &signed) {
            eprintln!("error: write {} failed: {}", path, e);
            return ExitCode::FAILURE;
        }
        eprintln!("[spore_revoke] wrote {} bytes to {}", signed.len(), path);
    }

    if let Some(target) = &broadcast_target {
        let envelope = spore_crypto::wrap_revocation_envelope(&signed);
        let sock = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => { eprintln!("error: bind failed: {}", e); return ExitCode::FAILURE; }
        };
        sock.set_multicast_ttl_v4(2).ok();
        match sock.send_to(&envelope, target) {
            Ok(n) => eprintln!("[spore_revoke] broadcast {} bytes (envelope) → {}", n, target),
            Err(e) => { eprintln!("error: send_to({}) failed: {}", target, e); return ExitCode::FAILURE; }
        }
    }

    if write_file.is_none() && broadcast_target.is_none() {
        eprintln!("warning: neither --write-file nor --broadcast given; output discarded");
    }

    ExitCode::SUCCESS
}

fn parse_hex32(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 { return None; }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let hi = hex_digit(hex.as_bytes()[i*2])?;
        let lo = hex_digit(hex.as_bytes()[i*2+1])?;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

fn parse_fp8(s: &str) -> Option<[u8; 8]> {
    // Accept forms: "AA-BB-CC-DD-EE-FF-00-11" or "AABBCCDDEEFF0011"
    let clean: String = s.chars().filter(|c| *c != '-' && *c != ':' && *c != ' ').collect();
    if clean.len() != 16 { return None; }
    let mut out = [0u8; 8];
    for i in 0..8 {
        let hi = hex_digit(clean.as_bytes()[i*2])?;
        let lo = hex_digit(clean.as_bytes()[i*2+1])?;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn print_usage() {
    eprintln!("Usage: spore_revoke [OPTIONS]");
    eprintln!("  --op-seed-hex HEX     Operator Ed25519 private seed (64 hex)     [REQUIRED]");
    eprintln!("  --op-pub-hex HEX      Operator Ed25519 public key (64 hex)        [w/ --load-file]");
    eprintln!("  --revoke FP1,FP2,...  Comma-separated fingerprints to revoke");
    eprintln!("  --load-file PATH      Start from existing signed revocation file");
    eprintln!("  --write-file PATH     Write signed blob to file");
    eprintln!("  --broadcast HOST:PORT Broadcast SPORE\\x06 envelope via UDP");
    eprintln!("  -h, --help            Show this help");
}
