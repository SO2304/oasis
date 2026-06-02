//! OASIS — X25519 key pair generator for spore v5 identity.
//!
//! Generates a fresh long-term X25519 keypair for a drone/node. The private
//! key goes into `OASIS_SPORE_ID_PRIV_HEX` (or a protected file); the public
//! key + fingerprint is distributed to peers out-of-band during pairing.
//!
//! Usage:
//!   oasis_keygen                        # hex output, human-readable
//!   oasis_keygen --json                 # JSON output for scripting
//!   oasis_keygen --env-file ./node.env  # write sourceable env file
//!
//! Output includes a fingerprint that operators read aloud / compare
//! out-of-band to confirm the pubkey wasn't substituted during pairing.
//! Same fingerprint appears in SPORE\x05 envelopes at offset 6..14.

use oasis_rt::spore_crypto;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut json = false;
    let mut env_file: Option<String> = None;
    let mut label = String::from("node");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json = true;
                i += 1;
            }
            "--env-file" => {
                env_file = Some(args.get(i + 1).map(|s| s.clone()).unwrap_or_else(|| {
                    eprintln!("--env-file needs path");
                    std::process::exit(2)
                }));
                i += 2;
            }
            "--label" => {
                label = args.get(i + 1).cloned().unwrap_or_else(|| {
                    eprintln!("--label needs value");
                    std::process::exit(2)
                });
                i += 2;
            }
            "-h" | "--help" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unknown arg: {}", other);
                print_usage();
                return ExitCode::from(2);
            }
        }
    }

    let (priv_key, pub_key) = spore_crypto::x25519_generate_keypair();
    let fp = spore_crypto::sender_fingerprint(&pub_key);

    let priv_hex = hex_lower(&priv_key);
    let pub_hex = hex_lower(&pub_key);
    let fp_hex = hex_lower(&fp);
    let fp_human = human_fp(&fp);

    if json {
        println!(r#"{{"label":"{}","priv_hex":"{}","pub_hex":"{}","fp_hex":"{}","fp_human":"{}"}}"#, label, priv_hex, pub_hex, fp_hex, fp_human);
    } else if let Some(path) = env_file {
        let body = format!(
            "# OASIS node {} — generated keypair\n\
             # KEEP PRIV SECRET. Share only pub + fingerprint.\n\
             export OASIS_SPORE_ID_PRIV_HEX={}\n\
             export OASIS_SPORE_ID_PUB_HEX={}\n\
             # fingerprint (read aloud during pairing): {}\n",
            label, priv_hex, pub_hex, fp_human
        );
        if let Err(e) = std::fs::write(&path, body) {
            eprintln!("write {} failed: {}", path, e);
            return ExitCode::FAILURE;
        }
        eprintln!("[oasis_keygen] wrote {} (fingerprint {})", path, fp_human);
    } else {
        println!("=== OASIS node keypair: {} ===", label);
        println!("priv (SECRET):   {}", priv_hex);
        println!("pub  (share):    {}", pub_hex);
        println!("fingerprint:     {}", fp_human);
        println!();
        println!("Pair with peers by sharing pub_hex + fingerprint over a trusted channel.");
        println!("Set OASIS_SPORE_ID_PRIV_HEX={}", priv_hex);
        println!("    OASIS_SPORE_ID_PUB_HEX={}", pub_hex);
    }
    ExitCode::SUCCESS
}

fn hex_lower(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for byte in b {
        s.push_str(&format!("{:02x}", byte));
    }
    s
}

/// Format an 8-byte fingerprint as human-readable groups (XX-XX-XX-XX-XX-XX-XX-XX).
fn human_fp(fp: &[u8; 8]) -> String {
    fp.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join("-")
}

fn print_usage() {
    eprintln!("Usage: oasis_keygen [OPTIONS]");
    eprintln!("  --label NAME        Label for the key (default 'node')");
    eprintln!("  --json              JSON output (for scripting)");
    eprintln!("  --env-file PATH     Write a sourceable env file");
    eprintln!("  -h, --help          Show this help");
}
