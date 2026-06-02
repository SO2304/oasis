//! OASIS — PC Spore Listener
//!
//! Listens for UDP spore broadcasts from any OASIS device on the network.
//! Merges received digests into local collective memory.

use oasis_rt::federation::FederatedMesh;
use oasis_rt::spore;
use std::net::UdpSocket;
use std::time::Instant;

fn main() {
    eprintln!("╔══════════════════════════════════════════════╗");
    eprintln!("║  OASIS Spore Listener — PC side             ║");
    eprintln!("║  Listening on 0.0.0.0:4200 (multicast)      ║");
    eprintln!("╚══════════════════════════════════════════════╝\n");

    let mut fed = FederatedMesh::new();

    // Try to load existing memory
    match fed.load("/tmp/oasis-pc-memory.bin") {
        Ok(n) => eprintln!("  Loaded {} existing digests", n),
        Err(_) => eprintln!("  No existing memory (fresh start)"),
    }

    // Bind + join multicast
    let sock = UdpSocket::bind("0.0.0.0:4200").expect("Cannot bind :4200");
    sock.join_multicast_v4(&"239.0.42.1".parse().unwrap(), &"0.0.0.0".parse().unwrap()).ok();
    // Also accept unicast on same port
    sock.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();

    eprintln!("  Waiting for spores...\n");

    let start = Instant::now();
    let mut total_received = 0u32;
    let mut buf = [0u8; 16384];

    loop {
        match sock.recv_from(&mut buf) {
            Ok((len, addr)) => {
                let magic = b"SPORE\x01";
                if len < magic.len() + 4 || &buf[..magic.len()] != magic {
                    eprintln!("  [{}] Bad packet from {} ({} bytes)", elapsed(start), addr, len);
                    continue;
                }

                let payload_len = u32::from_le_bytes(buf[magic.len()..magic.len() + 4].try_into().unwrap()) as usize;

                if magic.len() + 4 + payload_len > len {
                    eprintln!("  [{}] Truncated from {} ({}/{})", elapsed(start), addr, len, payload_len);
                    continue;
                }

                let payload = &buf[magic.len() + 4..magic.len() + 4 + payload_len];

                // Write to temp file and merge
                let tmp = "/tmp/oasis-spore-pc-recv.bin";
                if std::fs::write(tmp, payload).is_ok() {
                    match fed.merge_foreign(tmp, 0.5) {
                        Ok(n) => {
                            total_received += n;
                            eprintln!("  [{}] SPORE from {} — {} new digests (total: {})", elapsed(start), addr, n, fed.digest_count());

                            // Save updated memory
                            let _ = fed.save("/tmp/oasis-pc-memory.bin");

                            // Print what we received
                            eprintln!("  Memory: {} digests | {} trust pairs", fed.digest_count(), total_received);
                        }
                        Err(e) => eprintln!("  [{}] Merge error: {}", elapsed(start), e),
                    }
                    std::fs::remove_file(tmp).ok();
                }
            }
            Err(_) => {
                // Timeout — print heartbeat
                eprint!(".");
            }
        }
    }
}

fn elapsed(start: Instant) -> String {
    let s = start.elapsed().as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}
