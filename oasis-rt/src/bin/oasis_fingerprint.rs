//! OASIS — Display or verify the fingerprint of an X25519 public key.
//!
//! Use this during pairing to:
//!   1. Compute the fingerprint of a received pubkey and compare it to what
//!      the peer read aloud over an out-of-band channel
//!   2. Verify that a stored pub_hex matches its expected fingerprint
//!
//! Usage:
//!   oasis_fingerprint --pub-hex <64 chars>
//!   oasis_fingerprint --pub-hex <64 chars> --expect AA-BB-CC-DD-EE-FF-00-11
//!   # or read from env:
//!   OASIS_SPORE_ID_PUB_HEX=... oasis_fingerprint
//!
//! Exit codes:
//!   0 — fingerprint displayed (or matches --expect)
//!   1 — --expect provided and DOES NOT match
//!   2 — bad input

use oasis_rt::spore_crypto;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut pub_hex: Option<String> = None;
    let mut expect: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--pub-hex" => {
                pub_hex = Some(args.get(i+1).cloned()
                    .unwrap_or_else(|| { eprintln!("--pub-hex needs value"); std::process::exit(2) }));
                i += 2;
            }
            "--expect" => {
                expect = Some(args.get(i+1).cloned()
                    .unwrap_or_else(|| { eprintln!("--expect needs value"); std::process::exit(2) }));
                i += 2;
            }
            "-h" | "--help" => { print_usage(); return ExitCode::SUCCESS; }
            other => { eprintln!("unknown arg: {}", other); print_usage(); return ExitCode::from(2); }
        }
    }

    let pub_hex = pub_hex
        .or_else(|| std::env::var("OASIS_SPORE_ID_PUB_HEX").ok())
        .unwrap_or_else(|| { eprintln!("error: no pub-hex (provide --pub-hex or OASIS_SPORE_ID_PUB_HEX)"); std::process::exit(2) });

    let pub_bytes = match parse_hex32(&pub_hex) {
        Some(b) => b,
        None => { eprintln!("error: pub-hex must be 64 hex chars"); return ExitCode::from(2); }
    };

    let fp = spore_crypto::sender_fingerprint(&pub_bytes);
    let human = fp.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join("-");

    if let Some(exp) = expect {
        let normalized_exp = exp.replace(['-', ':', ' '], "").to_uppercase();
        let normalized_got = human.replace('-', "").to_uppercase();
        if normalized_exp == normalized_got {
            eprintln!("[oasis_fingerprint] MATCH: {}", human);
            return ExitCode::SUCCESS;
        } else {
            eprintln!("[oasis_fingerprint] MISMATCH: expected {} got {}", exp, human);
            return ExitCode::FAILURE;
        }
    }

    println!("{}", human);
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

fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn print_usage() {
    eprintln!("Usage: oasis_fingerprint [--pub-hex HEX] [--expect FP]");
    eprintln!("  --pub-hex HEX       64-char X25519 public key (default: $OASIS_SPORE_ID_PUB_HEX)");
    eprintln!("  --expect FP         Verify fingerprint matches (exit 1 if mismatch)");
    eprintln!("  -h, --help          Show this help");
}
