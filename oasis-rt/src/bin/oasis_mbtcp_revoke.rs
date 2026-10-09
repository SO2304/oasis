//! `oasis_mbtcp_revoke` — publish a signed `ORV1` revocation list to a Modbus TCP gateway.
//!
//! The operator side of revocation on this path. Until now the gateway's revocation list
//! was config-time only — edit the file and restart — while the mesh carries a signed list
//! with a monotone epoch, persisted before apply. This is the tool that sends one.
//!
//! ```text
//! oasis_mbtcp_revoke --config agent.conf --op-seed op1.seed [--op-seed op2.seed …] --epoch 1 --revoke <fp hex>[,<fp hex>…]
//! ```
//!
//! The list is signed with the operator's Ed25519 key, wrapped in a v0B envelope signed by
//! **this** identity (so the gateway authenticates the carrier as well as the author), and
//! sent over the configured link. The gateway's reply says whether it applied it.
//!
//! Exit 0 when the gateway applied the list, 1 when it refused it, 2 on a usage or
//! transport error — a campaign reads the exit code.
//!
//! **`--op-seed` is repeatable**: each occurrence adds one signature over the same
//! canonical message, which is what a k-of-n authority counts. It counts **distinct keys**,
//! so giving the same seed twice produces a list the gateway refuses — and the tool does
//! **not** deduplicate, because being able to produce that refusal is the point of testing
//! it.
//! The send counter is persisted beside the config (`<config>.txc`), because a one-shot
//! tool otherwise restarts it at 1 and its second run is refused as a replay. The counter
//! belongs to the identity, not to the process.
//!
//! ⚠️ The epoch must be **strictly greater** than the gateway's current one. A lower or
//! equal epoch is refused as a rollback, which is the point of the epoch.

use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use oasis_rt::mbtcp_conf::{read_seed, Config};
use oasis_rt::mbtcp_net::{connect_timeout, read_frame, write_frame};
use oasis_rt::mbtcp_pilot::{decode_reply, TxCounterStore};
use oasis_rt::mesh::MeshRouter;
use oasis_rt::mesh_revocation::{encode_orv1, signed_message, ParsedRevocation};
use oasis_rt::modbus_tcp::Outcome;

fn usage() -> ExitCode {
    eprintln!("usage: oasis_mbtcp_revoke --config <file> --op-seed <file> [--op-seed <file> …] --epoch <n> --revoke <fp>[,<fp>…]");
    ExitCode::from(2)
}

fn parse_fp(s: &str) -> Option<[u8; 8]> {
    let s = s.trim().replace('-', "");
    if s.len() != 16 {
        return None;
    }
    let mut fp = [0u8; 8];
    for (i, b) in fp.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(fp)
}

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().collect();
    let get = |f: &str| -> Option<String> { a.iter().position(|x| x == f).and_then(|i| a.get(i + 1)).cloned() };

    // `--print-pub <seedfile>`: the operator's public key, for the gateway's `operator =`
    // line. Derived here with the same crate the gateway verifies with, so a campaign
    // cannot pass by agreeing with its own arithmetic.
    if let Some(p) = get("--print-pub") {
        let seed = match read_seed(&p) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("seed: {e}");
                return ExitCode::from(2);
            }
        };
        let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(seed.0));
        println!("{}", kp.pk.as_ref().iter().map(|b| format!("{b:02x}")).collect::<String>());
        return ExitCode::SUCCESS;
    }
    // Every --op-seed occurrence, in order.
    let seed_paths: Vec<String> = a.iter().enumerate().filter(|(_, x)| x.as_str() == "--op-seed").filter_map(|(i, _)| a.get(i + 1).cloned()).collect();
    let (Some(cfg), Some(epoch), Some(list)) = (get("--config"), get("--epoch"), get("--revoke")) else {
        return usage();
    };
    if seed_paths.is_empty() {
        return usage();
    }
    let Ok(epoch) = epoch.parse::<u64>() else { return usage() };

    let conf = match Config::load(&cfg) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config: {e}");
            return ExitCode::from(2);
        }
    };
    // The operator seeds are read from files, never compiled — phase 1.2's lesson, and the
    // same reason the node seeds are files.
    let mut op_seeds = Vec::with_capacity(seed_paths.len());
    for p in &seed_paths {
        match read_seed(p) {
            Ok(s) => op_seeds.push(s),
            Err(e) => {
                eprintln!("operator seed {p}: {e}");
                return ExitCode::from(2);
            }
        }
    }
    let mut fps: Vec<[u8; 8]> = Vec::new();
    for part in list.split(',') {
        match parse_fp(part) {
            Some(fp) => fps.push(fp),
            None => {
                eprintln!("not a fingerprint: {part:?} (want 16 hex chars)");
                return ExitCode::from(2);
            }
        }
    }
    // The format requires them strictly ascending with no duplicates, so sort here rather
    // than make the caller do it and be refused for a reason that is not theirs.
    fps.sort_unstable();
    fps.dedup();

    let issued_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    // Sign the canonical message, which is what the gateway recomputes — not the encoded
    // blob, so a change in framing cannot silently invalidate a signature.
    let unsigned = ParsedRevocation { network_id: conf.network_id, epoch, issued_at, fps: fps.clone(), sigs: Vec::new() };
    let msg = signed_message(&unsigned);
    let mut sigs: Vec<([u8; 32], [u8; 64])> = Vec::with_capacity(op_seeds.len());
    for s in &op_seeds {
        let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(s.0));
        let pk: [u8; 32] = kp.pk.as_ref().try_into().expect("ed25519 public key is 32 bytes");
        let sig: [u8; 64] = kp.sk.sign(&msg, None).as_ref().try_into().expect("ed25519 signature is 64 bytes");
        sigs.push((pk, sig));
    }
    let blob = encode_orv1(&conf.network_id, epoch, issued_at, &fps, &sigs);

    let mut origin = MeshRouter::new_v0b(conf.our_fp, conf.network_id, conf.our_seed.clone(), conf.registry.clone());

    // The v0B send counter, persisted like the agent's. A one-shot tool restarts its
    // counter at 1 every run, so the second invocation would be refused as a replay by a
    // gateway that has already seen 1 — which is exactly what happened the first time this
    // tool ran three cases in a row. The counter belongs to the identity, not to the
    // process, so it is leased to a file beside the config.
    let txc_path = format!("{cfg}.txc");
    let mut txc = match TxCounterStore::load(&txc_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("tx counter: {e}");
            return ExitCode::from(2);
        }
    };
    txc.restore_into(&mut origin);
    if let Err(e) = txc.ensure(origin.tx_counter()) {
        eprintln!("tx counter: {e}");
        return ExitCode::from(2);
    }
    let Some(env) = origin.origin_wrap_v0b(&blob) else {
        eprintln!("sign: origin_wrap_v0b failed");
        return ExitCode::from(2);
    };

    let to = Duration::from_millis(conf.timeout_ms);
    let mut sock = match connect_timeout(&conf.plc, to) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("connect {}: {:?}", conf.plc, e.kind());
            return ExitCode::from(2);
        }
    };
    if write_frame(&mut sock, &env).is_err() {
        eprintln!("send failed");
        return ExitCode::from(2);
    }
    let reply = match read_frame(&mut sock) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("no reply: {:?}", e.kind());
            return ExitCode::from(2);
        }
    };
    match decode_reply(&reply) {
        Some((_, Outcome::Done)) => {
            println!("REVOKE applied epoch={epoch} count={} signatures={}", fps.len(), sigs.len());
            ExitCode::SUCCESS
        }
        Some((_, other)) => {
            println!("REVOKE refused epoch={epoch} signatures={} outcome={other:?}", sigs.len());
            ExitCode::from(1)
        }
        None => {
            println!("REVOKE no usable reply ({} bytes)", reply.len());
            ExitCode::from(1)
        }
    }
}
