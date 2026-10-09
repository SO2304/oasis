//! `oasis_mbtcp_order` — send one signed `OMB1` order straight to a Modbus TCP gateway.
//!
//! The agent turns an HMI's plain Modbus write into a signed order; this sends one
//! directly, under whatever identity its config names. That is what makes it useful for a
//! campaign: it can command **as a chosen principal**, which an HMI cannot, so a case can
//! show that a peer whose key the gateway knows but who holds no `ACTUATE` permission is
//! refused — the key and the permission being two different things, and the TCP path
//! having conflated them until 2026-10-09.
//!
//! ```text
//! oasis_mbtcp_order --config revoke.conf --reg 10 --value 700
//! ```
//!
//! Exit 0 when the gateway acted, 1 when it refused, 2 on a usage or transport error.
//!
//! ⚠️ This is a **test and operations tool**, not a second agent: it does not serve an HMI,
//! it answers nobody, and it holds no clock view. The deadline comes from the gateway's own
//! clock in the same way the agent gets it, because the gate judges freshness in the
//! gateway's `boot_id`.

use std::process::ExitCode;
use std::time::Duration;

use oasis_rt::actuation::MAX_VALIDITY_MS;
use oasis_rt::mbtcp_conf::Config;
use oasis_rt::mbtcp_net::{connect_timeout, read_frame, write_frame};
use oasis_rt::mbtcp_pilot::{decode_reply, SeqStore, TxCounterStore, CLOCK_REPLY_LEN, CLOCK_REQ};
use oasis_rt::mesh::MeshRouter;
use oasis_rt::modbus_gateway::{encode_omb1, MbOrder, MAX_REGS};
use oasis_rt::modbus_tcp::Outcome;

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().collect();
    let get = |f: &str| -> Option<String> { a.iter().position(|x| x == f).and_then(|i| a.get(i + 1)).cloned() };
    let (Some(cfg), Some(reg), Some(val)) = (get("--config"), get("--reg"), get("--value")) else {
        eprintln!("usage: oasis_mbtcp_order --config <file> --reg <n> --value <n>");
        return ExitCode::from(2);
    };
    let (Ok(reg), Ok(val)) = (reg.parse::<u16>(), val.parse::<u16>()) else {
        eprintln!("--reg and --value want numbers");
        return ExitCode::from(2);
    };

    let conf = match Config::load(&cfg) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config: {e}");
            return ExitCode::from(2);
        }
    };
    let to = Duration::from_millis(conf.timeout_ms);

    // The gateway's clock, as the agent does it: the gate judges freshness in the
    // gateway's `boot_id`, so an order stamped against ours would be refused as expired.
    let (boot_id, gw_now) = match ask_clock(&conf, to) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("clock: {e}");
            return ExitCode::from(2);
        }
    };

    // Both monotone stores, leased to files beside the config. A one-shot tool that let
    // either restart would be refused as a replay on its second run — which is how this
    // was learnt, twice.
    let mut seq = match SeqStore::load(&format!("{cfg}.seq")) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sequence: {e}");
            return ExitCode::from(2);
        }
    };
    let mut txc = match TxCounterStore::load(&format!("{cfg}.txc")) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("tx counter: {e}");
            return ExitCode::from(2);
        }
    };

    let mut origin = MeshRouter::new_v0b(conf.our_fp, conf.network_id, conf.our_seed.clone(), conf.registry.clone());
    txc.restore_into(&mut origin);
    if let Err(e) = txc.ensure(origin.tx_counter()) {
        eprintln!("tx counter: {e}");
        return ExitCode::from(2);
    }
    let cmd_seq = match seq.reserve() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("sequence: {e}");
            return ExitCode::from(2);
        }
    };

    let mut values = [0u16; MAX_REGS];
    values[0] = val;
    let order = MbOrder {
        gateway_id: conf.gateway_id,
        cmd_seq,
        boot_id,
        deadline_ms: gw_now + MAX_VALIDITY_MS / 2,
        unit: conf.unit,
        fc: 0x06,
        start: reg,
        count: 1,
        values,
    };
    let Some((buf, n)) = encode_omb1(&order) else {
        eprintln!("order: unsupported write");
        return ExitCode::from(2);
    };
    let Some(env) = origin.origin_wrap_v0b(&buf[..n]) else {
        eprintln!("sign failed");
        return ExitCode::from(2);
    };

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
            println!("ORDER acted reg={reg} value={val} cmd_seq={cmd_seq}");
            ExitCode::SUCCESS
        }
        Some((_, other)) => {
            println!("ORDER refused reg={reg} value={val} cmd_seq={cmd_seq} outcome={other:?}");
            ExitCode::from(1)
        }
        None => {
            println!("ORDER no usable reply ({} bytes)", reply.len());
            ExitCode::from(1)
        }
    }
}

fn ask_clock(conf: &Config, to: Duration) -> Result<(u64, u64), String> {
    let mut s = connect_timeout(&conf.plc, to).map_err(|e| format!("{:?}", e.kind()))?;
    write_frame(&mut s, &CLOCK_REQ).map_err(|e| format!("send {:?}", e.kind()))?;
    let r = read_frame(&mut s).map_err(|e| format!("read {:?}", e.kind()))?;
    if r.len() != CLOCK_REPLY_LEN || r[0..4] != *b"OTM1" {
        return Err("bad answer".into());
    }
    Ok((u64::from_le_bytes(r[4..12].try_into().unwrap()), u64::from_le_bytes(r[12..20].try_into().unwrap())))
}
