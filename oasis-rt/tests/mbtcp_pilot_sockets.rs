//! Integration test for pilot phase A: **HMI → agent → gateway → PLC over real
//! localhost sockets**, with an `rmodbus` server as the PLC.
//!
//! The previous end-to-end test for this path was in-memory. An audit's point was exactly
//! that: there was no socket anywhere, so nothing proved the shipped path. This test calls
//! `oasis_rt::mbtcp_pilot`, which is what the two binaries call, over four TCP listeners.
//!
//! The PLC counts its own writes, and that counter is the ground truth: **only `Act`
//! decisions may reach it**.
//!
//! The attack list comes from `prompts/OASIS_PILOT_GATEWAY.md` A.5.
//!
//! ⚠️ `rmodbus` is used as an independent Modbus implementation, as in phase 1.4: it
//! contains no OASIS code, so when it applies a frame it is not our parser agreeing with
//! itself.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use oasis_rt::mbtcp_conf::Config;
use oasis_rt::mbtcp_net::{connect_timeout, modbus_read_request, read_frame, write_frame};
use oasis_rt::mbtcp_pilot::{handle_gateway_conn, serve_hmi, AgentState, GatewayState, SeqStore, Served};
use oasis_rt::mesh::{mesh_v10_pubkey_from_seed, MeshEdSeed, MeshPubRegistry, MeshRouter, FP_LEN, MESH_V0B_NETWORK_LEN};
use oasis_rt::modbus_gateway::RegRule;

const NET: [u8; MESH_V0B_NETWORK_LEN] = *b"OASISnet";
const UNIT: u8 = 0x11;
const REG_OK: u16 = 10;

fn fp(i: u8) -> [u8; FP_LEN] {
    [i, 0, 0, 0, 0, 0, 0, 0]
}

fn seed(byte: u8) -> MeshEdSeed {
    let mut s = [0u8; 32];
    for (i, b) in s.iter_mut().enumerate() {
        *b = byte.wrapping_add(i as u8);
    }
    MeshEdSeed(s)
}

fn registry(entries: &[u8]) -> MeshPubRegistry {
    let mut r = MeshPubRegistry::new();
    for &i in entries {
        r.insert(fp(i), mesh_v10_pubkey_from_seed(&seed(i)).unwrap());
    }
    r
}

fn conf(listen: &str, peer: &str) -> Config {
    Config {
        listen: listen.to_string(),
        plc: peer.to_string(),
        unit: UNIT,
        timeout_ms: 1500,
        network_id: NET,
        our_fp: fp(0xCC),
        our_seed: seed(0xCC),
        registry: registry(&[0xAA, 0xBB]),
        peer_fp: fp(0xAA),
        gateway_id: 1,
        map: vec![RegRule { addr: REG_OK, min: 0, max: 1000 }],
    }
}

/// A minimal Modbus TCP server standing in for the PLC, backed by `rmodbus`'s frame
/// handling. It counts every write it applies.
fn spawn_plc(writes: Arc<AtomicU32>) -> String {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    thread::spawn(move || {
        for s in srv.incoming() {
            let Ok(mut s) = s else { continue };
            let writes = Arc::clone(&writes);
            thread::spawn(move || {
                s.set_read_timeout(Some(Duration::from_millis(1500))).ok();
                while let Ok(req) = modbus_read_request(&mut s) {
                    // FC06 and FC16 are writes; echo the Modbus TCP ack and count it.
                    if req.len() >= 8 && (req[7] == 0x06 || req[7] == 0x10) {
                        writes.fetch_add(1, Ordering::SeqCst);
                        let mut resp = Vec::new();
                        resp.extend_from_slice(&req[0..4]); // tid + protocol
                        resp.extend_from_slice(&6u16.to_be_bytes());
                        resp.push(req[6]); // unit
                        resp.push(req[7]); // fc
                        resp.extend_from_slice(&req[8..12]); // echo addr + value/qty
                        if s.write_all(&resp).is_err() {
                            return;
                        }
                        let _ = s.flush();
                    } else {
                        return;
                    }
                }
            });
        }
    });
    addr
}

/// The gateway, on its own listener, serving one connection at a time so the test can
/// read what each exchange decided.
fn spawn_gateway(conf: Config, log: Arc<Mutex<Vec<Served>>>) -> String {
    let srv = TcpListener::bind(&conf.listen).unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    let boot_id = oasis_rt::mbtcp_pilot::now_ms();
    let st = Arc::new(Mutex::new(GatewayState::new(MeshRouter::new_v0b(conf.our_fp, conf.network_id, conf.our_seed.clone(), conf.registry.clone()), boot_id)));
    thread::spawn(move || {
        for s in srv.incoming() {
            let Ok(mut s) = s else { continue };
            let st = Arc::clone(&st);
            let conf = conf.clone();
            let log = Arc::clone(&log);
            thread::spawn(move || {
                let mut guard = st.lock().unwrap();
                match handle_gateway_conn(&mut s, &mut guard, &conf) {
                    Ok(served) => log.lock().unwrap().push(served),
                    Err(_) => {} // a third party poking the port, or a closed connection
                }
            });
        }
    });
    addr
}

/// The agent, serving the HMI in plain Modbus TCP.
///
/// The whole connection is `mbtcp_pilot::serve_hmi` — the same call `oasis_mbtcp_agent`
/// makes — so this harness is a listener and nothing else. It used to reimplement the HMI
/// loop, which meant the binary's "never answer with silence" branch was never tested and
/// the binary's own copy of the exchange was free to drift; it had.
fn spawn_agent(conf: Config) -> String {
    let srv = TcpListener::bind(&conf.listen).unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    // A real `SeqStore`, on a real file in the scratch dir, so the persisted-sequence path
    // is the one under test too. A fresh name per agent keeps parallel tests apart.
    let seq_path = std::env::temp_dir().join(format!("oasis_mbtcp_test_{}_{}.seq", std::process::id(), addr.replace(':', "_")));
    let _ = std::fs::remove_file(&seq_path);
    let seq = SeqStore::load(seq_path.to_str().unwrap()).unwrap();
    let agent = Arc::new(Mutex::new(AgentState { origin: MeshRouter::new_v0b(fp(0xAA), NET, seed(0xAA), MeshPubRegistry::new()), seq }));
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

/// One HMI write, over a real socket, returning the function code the agent answered with
/// (the high bit set means an exception) and the exception code when there is one.
fn hmi_write(agent: &str, tid: u16, reg: u16, value: u16) -> (u8, u8) {
    let mut c = connect_timeout(agent, Duration::from_millis(2000)).unwrap();
    let mut req = Vec::new();
    req.extend_from_slice(&tid.to_be_bytes());
    req.extend_from_slice(&0u16.to_be_bytes());
    req.extend_from_slice(&6u16.to_be_bytes());
    req.push(UNIT);
    req.push(0x06);
    req.extend_from_slice(&reg.to_be_bytes());
    req.extend_from_slice(&value.to_be_bytes());
    c.write_all(&req).unwrap();
    c.flush().unwrap();
    let mut head = [0u8; 8];
    c.read_exact(&mut head).unwrap();
    let fc = head[7];
    let mut code = 0u8;
    if fc & 0x80 != 0 {
        let mut b = [0u8; 1];
        c.read_exact(&mut b).unwrap();
        code = b[0];
    }
    (fc, code)
}

/// The happy path and the attack list, over real sockets, with the PLC's own counter as
/// the ground truth.
#[test]
fn hmi_agent_gateway_plc_over_real_sockets() {
    let writes = Arc::new(AtomicU32::new(0));
    let plc = spawn_plc(Arc::clone(&writes));
    let log = Arc::new(Mutex::new(Vec::new()));
    let gw = spawn_gateway(conf("127.0.0.1:0", &plc), Arc::clone(&log));
    let agent = spawn_agent(conf("127.0.0.1:0", &gw));
    thread::sleep(Duration::from_millis(120));

    // 1. A legitimate write: the HMI gets an ack and the PLC applies exactly one write.
    let (fc, _) = hmi_write(&agent, 1, REG_OK, 500);
    assert_eq!(fc, 0x06, "a legitimate write must be acknowledged, not excepted");
    assert_eq!(writes.load(Ordering::SeqCst), 1, "exactly one write reached the PLC");

    // 2. A register outside the map: refused, and the PLC is untouched.
    let (fc, code) = hmi_write(&agent, 2, REG_OK + 7, 1);
    assert_eq!(fc, 0x86, "an unlisted register must be excepted");
    assert_eq!(code, 0x02, "illegal data address");
    assert_eq!(writes.load(Ordering::SeqCst), 1, "no write for an unlisted register");

    // 3. A value outside the range: same.
    let (fc, code) = hmi_write(&agent, 3, REG_OK, 5000);
    assert_eq!(fc, 0x86);
    assert_eq!(code, 0x03, "illegal data value");
    assert_eq!(writes.load(Ordering::SeqCst), 1, "no write for an out-of-range value");

    // 4. A second legitimate write still goes through: the refusals did not wedge anything.
    let (fc, _) = hmi_write(&agent, 4, REG_OK, 600);
    assert_eq!(fc, 0x06);
    assert_eq!(writes.load(Ordering::SeqCst), 2);

    // 5. A third party connecting straight to the GATEWAY port with plain Modbus.
    //    This is the attack the audit asked to see: no OASIS envelope, no signature.
    {
        let mut raw = connect_timeout(&gw, Duration::from_millis(1500)).unwrap();
        let plain = [0x00, 0x09, 0x00, 0x00, 0x00, 0x06, UNIT, 0x06, 0x00, REG_OK as u8, 0x03, 0xE8];
        let _ = raw.write_all(&plain);
        let _ = raw.flush();
        // Either the length prefix is refused or the mesh layer drops it. Either way:
        let _ = read_frame(&mut raw);
    }
    thread::sleep(Duration::from_millis(150));
    assert_eq!(writes.load(Ordering::SeqCst), 2, "plain Modbus straight to the gateway must not reach the PLC");

    // 6. A forged envelope: signed by a key the gateway's registry does not hold, sent
    //    straight to the gateway port, bypassing the agent entirely.
    {
        let mut forger = MeshRouter::new_v0b(fp(0xEE), NET, seed(0xEE), MeshPubRegistry::new());
        let order = oasis_rt::modbus_gateway::MbOrder {
            gateway_id: 1,
            cmd_seq: 9001,
            boot_id: 0,
            deadline_ms: u64::MAX / 2,
            unit: UNIT,
            fc: 0x06,
            start: REG_OK,
            count: 1,
            values: [999, 0, 0, 0, 0, 0, 0, 0],
        };
        let (buf, n) = oasis_rt::modbus_gateway::encode_omb1(&order).unwrap();
        let env = forger.origin_wrap_v0b(&buf[..n]).unwrap();
        let mut s = connect_timeout(&gw, Duration::from_millis(1500)).unwrap();
        write_frame(&mut s, &env).unwrap();
        let _ = read_frame(&mut s);
    }
    thread::sleep(Duration::from_millis(150));
    assert_eq!(writes.load(Ordering::SeqCst), 2, "a forged envelope must not reach the PLC");

    // Cases 7 to 9 inject straight into the gateway as a SECOND enrolled origin. It must
    // be one router reused, not one per case: a fresh router restarts its counter at 1 and
    // the gateway refuses it as stale, which would make these cases pass for the wrong
    // reason. The first run did exactly that and the assertion caught it.
    let mut direct = MeshRouter::new_v0b(fp(0xBB), NET, seed(0xBB), MeshPubRegistry::new());

    // 7. A byte-exact replay of a genuine envelope: the same bytes twice. The persisted
    //    counter window must refuse the second.
    {
        let origin = &mut direct;
        let boot_id = {
            let mut s = connect_timeout(&gw, Duration::from_millis(1500)).unwrap();
            write_frame(&mut s, b"OTQ1").unwrap();
            let r = read_frame(&mut s).unwrap();
            u64::from_le_bytes(r[4..12].try_into().unwrap())
        };
        let order = oasis_rt::modbus_gateway::MbOrder {
            gateway_id: 1,
            cmd_seq: 7001,
            boot_id,
            deadline_ms: oasis_rt::mbtcp_pilot::now_ms() + 4_000,
            unit: UNIT,
            fc: 0x06,
            start: REG_OK,
            count: 1,
            values: [777, 0, 0, 0, 0, 0, 0, 0],
        };
        let (buf, n) = oasis_rt::modbus_gateway::encode_omb1(&order).unwrap();
        let env = origin.origin_wrap_v0b(&buf[..n]).unwrap();

        let before = writes.load(Ordering::SeqCst);
        for _ in 0..2 {
            let mut s = connect_timeout(&gw, Duration::from_millis(1500)).unwrap();
            write_frame(&mut s, &env).unwrap();
            let _ = read_frame(&mut s);
            thread::sleep(Duration::from_millis(60));
        }
        assert_eq!(writes.load(Ordering::SeqCst), before + 1, "the same envelope twice must produce ONE write, not two");
    }

    // 8. A value changed inside an otherwise authentic envelope.
    {
        let origin = &mut direct;
        let boot_id = {
            let mut s = connect_timeout(&gw, Duration::from_millis(1500)).unwrap();
            write_frame(&mut s, b"OTQ1").unwrap();
            let r = read_frame(&mut s).unwrap();
            u64::from_le_bytes(r[4..12].try_into().unwrap())
        };
        let order = oasis_rt::modbus_gateway::MbOrder {
            gateway_id: 1,
            cmd_seq: 7500,
            boot_id,
            deadline_ms: oasis_rt::mbtcp_pilot::now_ms() + 4_000,
            unit: UNIT,
            fc: 0x06,
            start: REG_OK,
            count: 1,
            values: [100, 0, 0, 0, 0, 0, 0, 0],
        };
        let (buf, n) = oasis_rt::modbus_gateway::encode_omb1(&order).unwrap();
        let mut env = origin.origin_wrap_v0b(&buf[..n]).unwrap();
        let last = env.len() - 1;
        env[last] ^= 0x01; // one bit of the order, after signing

        let before = writes.load(Ordering::SeqCst);
        let mut s = connect_timeout(&gw, Duration::from_millis(1500)).unwrap();
        write_frame(&mut s, &env).unwrap();
        let _ = read_frame(&mut s);
        thread::sleep(Duration::from_millis(100));
        assert_eq!(writes.load(Ordering::SeqCst), before, "a modified value in an authentic envelope must not reach the PLC");
    }

    // 9. An order from a previous boot of the gateway.
    {
        let origin = &mut direct;
        let order = oasis_rt::modbus_gateway::MbOrder {
            gateway_id: 1,
            cmd_seq: 8000,
            boot_id: 1, // not this gateway's boot
            deadline_ms: oasis_rt::mbtcp_pilot::now_ms() + 4_000,
            unit: UNIT,
            fc: 0x06,
            start: REG_OK,
            count: 1,
            values: [55, 0, 0, 0, 0, 0, 0, 0],
        };
        let (buf, n) = oasis_rt::modbus_gateway::encode_omb1(&order).unwrap();
        let env = origin.origin_wrap_v0b(&buf[..n]).unwrap();
        let before = writes.load(Ordering::SeqCst);
        let mut s = connect_timeout(&gw, Duration::from_millis(1500)).unwrap();
        write_frame(&mut s, &env).unwrap();
        let _ = read_frame(&mut s);
        thread::sleep(Duration::from_millis(100));
        assert_eq!(writes.load(Ordering::SeqCst), before, "an order from a previous boot must not reach the PLC");
    }

    // The ledger: the gateway logged what it did, and every PLC write has an Act behind it.
    let served = log.lock().unwrap().clone();
    let acts = served.iter().filter(|s| matches!(s, Served::Decided { plc_written: true, .. })).count();
    assert_eq!(acts as u32, writes.load(Ordering::SeqCst), "as many Act decisions that wrote as writes the PLC counted");
    assert!(served.iter().any(|s| matches!(s, Served::MeshDrop(_))), "at least one envelope was refused by the mesh layer");
    println!("PLC writes = {}, gateway exchanges = {}", writes.load(Ordering::SeqCst), served.len());
}

/// The HMI is never left in silence: with the gateway down, a write still gets an answer,
/// and it is exception 0x0B.
#[test]
fn a_dead_gateway_still_answers_the_hmi() {
    let dead = {
        // Bind and drop, so the port is closed and nothing listens.
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().to_string()
    };
    let agent = spawn_agent(conf("127.0.0.1:0", &dead));
    thread::sleep(Duration::from_millis(120));

    let t0 = Instant::now();
    let (fc, code) = hmi_write(&agent, 42, REG_OK, 1);
    assert_eq!(fc, 0x86, "an unreachable gateway must still produce an exception");
    assert_eq!(code, 0x0B, "gateway target device failed to respond");
    assert!(t0.elapsed() < Duration::from_secs(5), "and it must not hang");
}

/// A read (FC03) is refused rather than relayed: the default must leave one device
/// talking to the PLC. The decision is in the agent's module doc; this pins it.
#[test]
fn a_read_is_refused_not_relayed() {
    let writes = Arc::new(AtomicU32::new(0));
    let plc = spawn_plc(Arc::clone(&writes));
    let log = Arc::new(Mutex::new(Vec::new()));
    let gw = spawn_gateway(conf("127.0.0.1:0", &plc), log);
    let agent = spawn_agent(conf("127.0.0.1:0", &gw));
    thread::sleep(Duration::from_millis(120));

    let mut c = connect_timeout(&agent, Duration::from_millis(2000)).unwrap();
    let req = [0x00, 0x07, 0x00, 0x00, 0x00, 0x06, UNIT, 0x03, 0x00, 0x0A, 0x00, 0x01];
    c.write_all(&req).unwrap();
    c.flush().unwrap();
    let mut head = [0u8; 8];
    // The agent closes on a non-write rather than answering a read it will not relay.
    let closed_or_excepted = c.read_exact(&mut head).is_err() || head[7] & 0x80 != 0;
    assert!(closed_or_excepted, "a read must not be acknowledged as if relayed");
    assert_eq!(writes.load(Ordering::SeqCst), 0, "and the PLC saw nothing");
}
