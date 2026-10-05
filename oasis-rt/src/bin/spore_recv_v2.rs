//! OASIS — Spore v2 receiver
//!
//! Listens on UDP for SPORE\x02 fragmented packets, reassembles them, and
//! reports per-message success/failure counts. Companion to spore_send and
//! udp_loss_proxy for end-to-end loss tolerance validation.
//!
//! Usage:
//!   spore_recv_v2 --bind 127.0.0.1:5002 --duration 10
//!
//! On exit (Ctrl-C OR --duration timeout), prints summary:
//!   messages: complete=N, partial=M, total=N+M
//!   delivery_rate: N/(N+M)
//!   bytes_received, packets_received
//!
//! Exit code 0 always; failure to bind exits 1.

use oasis_rt::spore::{parse_chunk_v2, Reassembler};
use oasis_rt::spore_crypto;
use std::collections::HashMap;
use std::net::UdpSocket;
use std::process::ExitCode;
use std::time::{Duration, Instant};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut bind = "127.0.0.1:5002".to_string();
    let mut duration_s: u64 = 10;
    let mut expected_msgs: u32 = 0;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--bind" => {
                bind = args[i + 1].clone();
                i += 2;
            }
            "--duration" => {
                duration_s = args[i + 1].parse().unwrap_or(10);
                i += 2;
            }
            "--expected" => {
                expected_msgs = args[i + 1].parse().unwrap_or(0);
                i += 2;
            }
            "-h" | "--help" => {
                eprintln!("see source");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unknown arg: {}", other);
                return ExitCode::from(2);
            }
        }
    }
    let sock = match UdpSocket::bind(&bind) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bind {} failed: {}", bind, e);
            return ExitCode::FAILURE;
        }
    };
    sock.set_read_timeout(Some(Duration::from_millis(200))).ok();
    eprintln!("[spore_recv_v2] listening on {}, will run {}s", bind, duration_s);

    // Optional decryption key
    let key: Option<[u8; spore_crypto::KEY_LEN]> = std::env::var("OASIS_SPORE_KEY_HEX").ok().and_then(|h| spore_crypto::parse_key_hex(&h));
    if key.is_some() {
        eprintln!("[spore_recv_v2] decryption ENABLED (OASIS_SPORE_KEY_HEX set)");
    }

    let start = Instant::now();
    let mut buf = [0u8; 65535];
    let mut reass: HashMap<u32, Reassembler> = HashMap::new();
    let mut completed: u32 = 0;
    let mut decrypted_ok: u32 = 0;
    let mut decrypt_failed: u32 = 0;
    let mut packets_in: u64 = 0;
    let mut bytes_in: u64 = 0;
    let mut completed_ids: Vec<u32> = Vec::new();

    while start.elapsed() < Duration::from_secs(duration_s) {
        match sock.recv_from(&mut buf) {
            Ok((len, _src)) => {
                packets_in += 1;
                bytes_in += len as u64;
                match parse_chunk_v2(&buf[..len]) {
                    Ok((msg_id, total, idx, flags, payload)) => {
                        if completed_ids.contains(&msg_id) {
                            continue;
                        }
                        let entry = reass.entry(msg_id).or_insert_with(|| Reassembler::new(msg_id, total, flags));
                        if let Some(complete) = entry.feed(idx, payload) {
                            completed += 1;
                            completed_ids.push(msg_id);
                            reass.remove(&msg_id);
                            // If a key is configured, attempt decrypt and report
                            let enc_flag = if let Some(k) = &key {
                                match spore_crypto::decrypt_envelope(k, &complete, b"") {
                                    Ok(_) => {
                                        decrypted_ok += 1;
                                        " [decrypted]"
                                    }
                                    Err(e) => {
                                        decrypt_failed += 1;
                                        eprintln!("[spore_recv_v2] decrypt fail on msg_id={}: {}", msg_id, e);
                                        " [DECRYPT FAIL]"
                                    }
                                }
                            } else if complete.starts_with(spore_crypto::SPORE_V3_MAGIC) {
                                " [encrypted — no key]"
                            } else {
                                ""
                            };
                            eprintln!("[spore_recv_v2] msg_id={} complete (#{} done){}", msg_id, completed, enc_flag);
                        }
                    }
                    Err(_) => { /* not a v2 packet — ignore */ }
                }
            }
            Err(_) => { /* timeout, loop */ }
        }
    }

    let partial = reass.len() as u32;
    let total = completed + partial;
    let rate = if total == 0 { 0.0 } else { completed as f64 / total as f64 };
    println!("=== SPORE_RECV_V2 SUMMARY ===");
    println!("packets_in:    {}", packets_in);
    println!("bytes_in:      {}", bytes_in);
    println!("msgs_complete: {}", completed);
    println!("msgs_partial:  {}", partial);
    if key.is_some() {
        println!("decrypted_ok:  {}", decrypted_ok);
        println!("decrypt_fail:  {}", decrypt_failed);
    }
    if expected_msgs > 0 {
        println!("expected:      {}", expected_msgs);
        println!("delivery_rate: {:.1}% ({}/{})", completed as f64 * 100.0 / expected_msgs as f64, completed, expected_msgs);
    } else {
        println!("delivery_rate: {:.1}% ({}/{} attempted)", rate * 100.0, completed, total);
    }
    ExitCode::SUCCESS
}
