//! Shared harness for the Modbus TCP pilot benches — one copy, on purpose.
//!
//! `bench_mbtcp_pilot` and `bench_mbtcp_concurrency` both need a PLC stand-in, a gateway,
//! an agent and an HMI write. They used to be one file; a second copy is exactly the
//! defect this repo already paid for once — the two shipped pilot binaries each held
//! their own copy of the HMI exchange, and the agent's copy still had a bug the library
//! had fixed, while the test stayed green because it tested the library. So the scaffolding
//! lives here and both binaries include it with `#[path]`.
//!
//! Not part of the library: this is bench scaffolding, and `src/bin/common/` carries no
//! `main.rs`, so Cargo does not treat it as a target.
//!
//! ⚠️ **The PLC here is a 30-line local responder, not `rmodbus` and not a PLC.** `rmodbus`
//! is a dev-dependency, unavailable to a `[[bin]]`, and the correctness of the frame is
//! already proven against it by `tests/mbtcp_pilot_sockets.rs` and by phase 1.4 on three
//! RP2040. What a bench measures is the **difference between arms through the same peer**.

// Each binary uses a subset; an unused helper here is not dead code in the crate.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use oasis_rt::mbtcp_conf::Config;
use oasis_rt::mbtcp_net::{connect_timeout, modbus_read_request};
use oasis_rt::mbtcp_pilot::{serve_gateway_conn, serve_hmi, AgentState, GatewayState, SeqStore};
use oasis_rt::mesh::{mesh_v10_pubkey_from_seed, MeshEdSeed, MeshPubRegistry, MeshRouter, FP_LEN, MESH_V0B_NETWORK_LEN};
use oasis_rt::modbus_gateway::RegRule;

pub const NET: [u8; MESH_V0B_NETWORK_LEN] = *b"OASISnet";
pub const UNIT: u8 = 0x11;
pub const REG: u16 = 10;

pub fn fp(i: u8) -> [u8; FP_LEN] {
    [i, 0, 0, 0, 0, 0, 0, 0]
}

pub fn seed(byte: u8) -> MeshEdSeed {
    let mut s = [0u8; 32];
    for (i, b) in s.iter_mut().enumerate() {
        *b = byte.wrapping_add(i as u8);
    }
    MeshEdSeed(s)
}

pub fn registry(entries: &[u8]) -> MeshPubRegistry {
    let mut r = MeshPubRegistry::new();
    for &i in entries {
        r.insert(fp(i), mesh_v10_pubkey_from_seed(&seed(i)).unwrap());
    }
    r
}

pub fn conf(listen: &str, peer: &str, our: u8) -> Config {
    Config {
        listen: listen.to_string(),
        plc: peer.to_string(),
        unit: UNIT,
        timeout_ms: 2000,
        network_id: NET,
        our_fp: fp(our),
        our_seed: seed(our),
        registry: registry(&[0xAA, 0xCC]),
        peer_fp: fp(0xAA),
        gateway_id: 1,
        map: vec![RegRule { addr: REG, min: 0, max: 60000 }],
        revoked: Vec::new(),
        // No per-origin rules here: every origin falls back to the shared map, which
        // is the pre-2026-10-09 behaviour and what these cases were written against.
        origin_map: Vec::new(),
        authority: None,
        // The gate requires `ACTUATE` now, not merely a key in the registry.
        perms: {
            let mut r = oasis_rt::enrollment::Registry::default();
            r.entries.push(oasis_rt::enrollment::Entry {
                fp: fp(0xAA),
                pk: mesh_v10_pubkey_from_seed(&seed(0xAA)).unwrap().0,
                role: 0,
                permissions: oasis_rt::enrollment::perm::ACTUATE,
                seq: 0,
            });
            r
        },
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

/// K=10 median and half-spread as a percentage of it — the repo's banding convention.
/// A single number would be a claim nobody can check; this one carries its own spread.
pub fn band(v: &mut [u64]) -> (u64, f64) {
    v.sort_unstable();
    let med = v[v.len() / 2];
    let half = (v[v.len() - 1].saturating_sub(v[0])) as f64 / 2.0;
    (med, if med == 0 { 0.0 } else { 100.0 * half / med as f64 })
}

/// A relay with **no OASIS in it at all**: forwards the HMI's frame to the PLC on a kept
/// connection and returns the answer verbatim.
///
/// This is the arm that makes the accounting close instead of being asserted. An earlier
/// version of the pilot bench blamed its own overhead on "three TCP connects per write",
/// which was true until connections were reused and then simply stale text. Two hops cost
/// what two hops cost whoever is in the middle; this measures that, so the authority layer
/// is charged only for what it actually adds.
pub fn spawn_null_relay(plc: String) -> String {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    thread::spawn(move || {
        for s in srv.incoming() {
            let Ok(mut s) = s else { continue };
            let plc = plc.clone();
            thread::spawn(move || {
                s.set_read_timeout(Some(Duration::from_millis(2000))).ok();
                s.set_nodelay(true).ok();
                let Ok(mut up) = connect_timeout(&plc, Duration::from_millis(2000)) else { return };
                up.set_nodelay(true).ok();
                up.set_read_timeout(Some(Duration::from_millis(2000))).ok();
                while let Ok(req) = modbus_read_request(&mut s) {
                    if up.write_all(&req).is_err() || up.flush().is_err() {
                        return;
                    }
                    let Ok(resp) = modbus_read_request(&mut up) else { return };
                    if s.write_all(&resp).is_err() {
                        return;
                    }
                    let _ = s.flush();
                }
            });
        }
    });
    addr
}

/// The PLC stand-in: answers FC06/FC16 with the Modbus TCP ack. See the header's warning.
///
/// `delay_us` lets a bench give the device a response time, because a real PLC has one and
/// loopback does not: it is what turns "the lock is held across the round trip" from an
/// architectural remark into a measurable effect.
pub fn spawn_plc_delayed(delay_us: u64) -> String {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    thread::spawn(move || {
        for s in srv.incoming() {
            let Ok(mut s) = s else { continue };
            thread::spawn(move || {
                s.set_read_timeout(Some(Duration::from_millis(2000))).ok();
                s.set_nodelay(true).ok();
                while let Ok(req) = modbus_read_request(&mut s) {
                    if req.len() < 12 || (req[7] != 0x06 && req[7] != 0x10) {
                        return;
                    }
                    if delay_us > 0 {
                        thread::sleep(Duration::from_micros(delay_us));
                    }
                    let mut resp = Vec::with_capacity(12);
                    resp.extend_from_slice(&req[0..4]);
                    resp.extend_from_slice(&6u16.to_be_bytes());
                    resp.extend_from_slice(&req[6..12]);
                    if s.write_all(&resp).is_err() {
                        return;
                    }
                    let _ = s.flush();
                }
            });
        }
    });
    addr
}

pub fn spawn_plc() -> String {
    spawn_plc_delayed(0)
}

/// Like `spawn_gateway`, but hands back the gateway's own record of what it decided.
///
/// Without this a bench can only see the Modbus exception the HMI receives — 0x0A for
/// "the gate refused" — and would have to *infer* which of nine conditions fired. That
/// inference is exactly where a plausible wrong story gets written down. The gateway
/// already reports `Served::Decided { cmd_seq, decision, .. }`; this collects it.
pub fn spawn_gateway_counting(conf: Config) -> (String, Arc<Mutex<Vec<(u32, oasis_rt::actuation::Decision)>>>) {
    let srv = TcpListener::bind(&conf.listen).unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    let st = Arc::new(Mutex::new(GatewayState::new(MeshRouter::new_v0b(conf.our_fp, conf.network_id, conf.our_seed.clone(), conf.registry.clone()), now_ms())));
    let log: Arc<Mutex<Vec<(u32, oasis_rt::actuation::Decision)>>> = Arc::new(Mutex::new(Vec::new()));
    let out = Arc::clone(&log);
    thread::spawn(move || {
        for s in srv.incoming() {
            let Ok(mut s) = s else { continue };
            let st = Arc::clone(&st);
            let conf = conf.clone();
            let log = Arc::clone(&log);
            thread::spawn(move || {
                let _ = serve_gateway_conn(&mut s, &st, &conf, |served| {
                    if let oasis_rt::mbtcp_pilot::Served::Decided { cmd_seq, decision, .. } = served {
                        if let Ok(mut g) = log.lock() {
                            g.push((cmd_seq, decision));
                        }
                    }
                });
            });
        }
    });
    (addr, out)
}

pub fn spawn_gateway(conf: Config) -> String {
    let srv = TcpListener::bind(&conf.listen).unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    let st = Arc::new(Mutex::new(GatewayState::new(MeshRouter::new_v0b(conf.our_fp, conf.network_id, conf.our_seed.clone(), conf.registry.clone()), now_ms())));
    thread::spawn(move || {
        for s in srv.incoming() {
            let Ok(mut s) = s else { continue };
            let st = Arc::clone(&st);
            let conf = conf.clone();
            thread::spawn(move || {
                let _ = serve_gateway_conn(&mut s, &st, &conf, |_| {});
            });
        }
    });
    addr
}

/// `tag` separates one bench's sequence file from another's in the same process.
pub fn spawn_agent_tagged(conf: Config, tag: &str) -> String {
    let srv = TcpListener::bind(&conf.listen).unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    let p = std::env::temp_dir().join(format!("oasis_bench_mbtcp_{}_{}.seq", tag, std::process::id()));
    let _ = std::fs::remove_file(&p);
    let agent = Arc::new(Mutex::new(AgentState {
        origin: MeshRouter::new_v0b(fp(0xAA), NET, seed(0xAA), MeshPubRegistry::new()),
        seq: SeqStore::load(p.to_str().unwrap()).unwrap(),
        // No restart in a bench, so the send counter stays in RAM. A deployed agent must
        // persist it or a restart locks it out of its own gateway.
        txc: None,
    }));
    thread::spawn(move || {
        for s in srv.incoming() {
            let Ok(mut s) = s else { continue };
            let agent = Arc::clone(&agent);
            let conf = conf.clone();
            thread::spawn(move || {
                let _ = serve_hmi(&mut s, &agent, &conf);
            });
        }
    });
    addr
}

pub fn spawn_agent(conf: Config) -> String {
    spawn_agent_tagged(conf, "a")
}

/// One HMI write over an open socket, returning the round trip in microseconds, or `None`
/// if the peer did not answer with an ack (a refusal is not a latency sample).
pub fn hmi_write(sock: &mut TcpStream, tid: u16, value: u16) -> Option<u64> {
    hmi_write_reg(sock, tid, REG, value)
}

pub fn hmi_write_reg(sock: &mut TcpStream, tid: u16, reg: u16, value: u16) -> Option<u64> {
    match hmi_write_detail(sock, tid, reg, value) {
        Wr::Ack(us) => Some(us),
        // An exception is the gate refusing, which is not what a latency arm is timing.
        Wr::Exception(_, _) | Wr::Transport => None,
    }
}

/// What one HMI write got back. A bench that only counts acks cannot tell "slow" from
/// "refused", and the difference is the whole question when several HMIs share an agent.
pub enum Wr {
    Ack(u64),
    Exception(u8, u64),
    Transport,
}

pub fn hmi_write_detail(sock: &mut TcpStream, tid: u16, reg: u16, value: u16) -> Wr {
    let req = [(tid >> 8) as u8, tid as u8, 0, 0, 0, 6, UNIT, 0x06, (reg >> 8) as u8, reg as u8, (value >> 8) as u8, value as u8];
    let t0 = Instant::now();
    if sock.write_all(&req).is_err() || sock.flush().is_err() {
        return Wr::Transport;
    }
    let mut head = [0u8; 8];
    if sock.read_exact(&mut head).is_err() {
        return Wr::Transport;
    }
    let rest = (u16::from_be_bytes([head[4], head[5]]) as usize).saturating_sub(2);
    let mut tail = vec![0u8; rest];
    if sock.read_exact(&mut tail).is_err() {
        return Wr::Transport;
    }
    let us = t0.elapsed().as_micros() as u64;
    if head[7] & 0x80 != 0 {
        // The exception code is byte 8 of the PDU, i.e. the first byte after the header.
        return Wr::Exception(*tail.first().unwrap_or(&0), us);
    }
    Wr::Ack(us)
}

/// Open a socket to `peer` with the bench's usual options.
pub fn dial(peer: &str) -> Option<TcpStream> {
    let sock = connect_timeout(peer, Duration::from_millis(5000)).ok()?;
    sock.set_nodelay(true).ok();
    sock.set_read_timeout(Some(Duration::from_millis(5000))).ok();
    Some(sock)
}
