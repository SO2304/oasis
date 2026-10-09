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
use oasis_rt::mbtcp_pilot::{serve_gateway_conn, serve_hmi, AgentState, GatewayState, SeqStore, Served};
use oasis_rt::mesh::{mesh_v10_pubkey_from_seed, MeshEdSeed, MeshPubRegistry, MeshRouter, FP_LEN, MESH_V0B_NETWORK_LEN};
use oasis_rt::modbus_gateway::RegRule;
use rmodbus::server::context::ModbusContext;
use rmodbus::server::storage::ModbusStorageSmall;
use rmodbus::server::ModbusFrame;
use rmodbus::{ModbusFrameBuf, ModbusProto};

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

/// The permissions the gateway grants, as an `enrollment::Registry` — the same type and
/// the same `allows()` rule the firmware uses.
fn perm_registry(entries: &[u8]) -> oasis_rt::enrollment::Registry {
    let mut r = oasis_rt::enrollment::Registry::default();
    for &i in entries {
        r.entries.push(oasis_rt::enrollment::Entry {
            fp: fp(i),
            pk: mesh_v10_pubkey_from_seed(&seed(i)).unwrap().0,
            role: 0,
            permissions: oasis_rt::enrollment::perm::ACTUATE,
            seq: 0,
        });
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
        revoked: Vec::new(),
        // No per-origin rules here: every origin falls back to the shared map, which
        // is the pre-2026-10-09 behaviour and what these cases were written against.
        origin_map: Vec::new(),
        authority: None,
        // The two origins this test commands with, granted `ACTUATE`. The gate now
        // requires the permission and not merely a key in the registry, so without this
        // every order would be refused `NotAuthorized` — which is the point of the change.
        perms: perm_registry(&[0xAA, 0xBB]),
    }
}

/// The PLC: a Modbus TCP server whose frames are parsed and applied by **`rmodbus`**,
/// which contains no OASIS code. It counts the writes it applies and keeps the register
/// values, so a test can check both that a write happened and what landed.
///
/// This used to be a hand-written responder that echoed the request back, while the file
/// said three times that the PLC was an `rmodbus` server. It was not, so "an independent
/// implementation applied the frame" — the whole point of using one — was unearned here;
/// only the RTU tests had it. Now the frame really has to satisfy `rmodbus`'s parser in
/// `ModbusProto::TcpUdp`, and the value read back comes out of its storage.
fn spawn_plc(writes: Arc<AtomicU32>, store: Arc<Mutex<ModbusStorageSmall>>) -> String {
    spawn_plc_counting(writes, Arc::new(AtomicU32::new(0)), store)
}

/// Same, with the **read** requests counted too: a refused read must not reach the device
/// any more than a refused write must, and only a counter at the device can show that.
fn spawn_plc_counting(writes: Arc<AtomicU32>, reads: Arc<AtomicU32>, store: Arc<Mutex<ModbusStorageSmall>>) -> String {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap().to_string();
    thread::spawn(move || {
        for s in srv.incoming() {
            let Ok(mut s) = s else { continue };
            let writes = Arc::clone(&writes);
            let reads = Arc::clone(&reads);
            let store = Arc::clone(&store);
            thread::spawn(move || {
                s.set_read_timeout(Some(Duration::from_millis(1500))).ok();
                while let Ok(req) = modbus_read_request(&mut s) {
                    let mut buf: ModbusFrameBuf = [0; 256];
                    if req.len() > buf.len() {
                        return;
                    }
                    buf[..req.len()].copy_from_slice(&req);
                    let mut resp = Vec::new();
                    let mut f = ModbusFrame::new(UNIT, &buf, ModbusProto::TcpUdp, &mut resp);
                    if f.parse().is_err() {
                        return; // rmodbus rejected the frame: nothing is applied
                    }
                    if f.processing_required {
                        let mut st = store.lock().unwrap();
                        let r = if f.readonly { f.process_read(&mut *st) } else { f.process_write(&mut *st) };
                        if r.is_err() {
                            return;
                        }
                        if f.readonly {
                            reads.fetch_add(1, Ordering::SeqCst);
                        } else {
                            writes.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    if f.response_required && f.finalize_response().is_err() {
                        return;
                    }
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
                // `serve_gateway_conn`, not one call to `handle_gateway_conn`: the agent
                // keeps its connection open across writes, so the gateway must serve more
                // than one frame on it. It takes the state lock per frame, which is also
                // what lets a second agent in while the first holds a connection.
                let _ = serve_gateway_conn(&mut s, &st, &conf, |served| log.lock().unwrap().push(served));
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
    let agent = Arc::new(Mutex::new(AgentState {
        origin: MeshRouter::new_v0b(fp(0xAA), NET, seed(0xAA), MeshPubRegistry::new()),
        seq,
        // This test never restarts the agent, which is exactly why it did not catch
        // the send counter not being persisted; the end-to-end campaign did.
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
    let store = Arc::new(Mutex::new(ModbusStorageSmall::new()));
    let plc = spawn_plc(Arc::clone(&writes), Arc::clone(&store));
    let log = Arc::new(Mutex::new(Vec::new()));
    let gw = spawn_gateway(conf("127.0.0.1:0", &plc), Arc::clone(&log));
    let agent = spawn_agent(conf("127.0.0.1:0", &gw));
    thread::sleep(Duration::from_millis(120));

    // 1. A legitimate write: the HMI gets an ack and the PLC applies exactly one write.
    let (fc, _) = hmi_write(&agent, 1, REG_OK, 500);
    assert_eq!(fc, 0x06, "a legitimate write must be acknowledged, not excepted");
    assert_eq!(writes.load(Ordering::SeqCst), 1, "exactly one write reached the PLC");
    assert_eq!(store.lock().unwrap().get_holding(REG_OK).unwrap(), 500, "rmodbus parsed the gateway's frame and stored the ordered value");

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

/// Reads go **through** the gateway: authenticated, bounded by the same register map, and
/// never around it. This replaces a test that pinned the opposite — reads were refused
/// outright, which left the HMI write-only and pushed a real deployment towards a side
/// channel straight to the PLC, the exact thing the gateway exists to prevent.
///
/// Three things are checked here, and the device's own counters are the ground truth:
/// a listed read returns what `rmodbus` holds, an unlisted one is refused **without the
/// device being asked**, and a forged query dies at the mesh layer.
#[test]
fn reads_are_served_through_the_gateway_and_bounded_by_the_map() {
    let writes = Arc::new(AtomicU32::new(0));
    let reads = Arc::new(AtomicU32::new(0));
    let store = Arc::new(Mutex::new(ModbusStorageSmall::new()));
    let plc = spawn_plc_counting(Arc::clone(&writes), Arc::clone(&reads), Arc::clone(&store));
    let log = Arc::new(Mutex::new(Vec::new()));
    let gw = spawn_gateway(conf("127.0.0.1:0", &plc), Arc::clone(&log));
    let agent = spawn_agent(conf("127.0.0.1:0", &gw));
    thread::sleep(Duration::from_millis(120));

    // Put a known value in the register through the authorised write path, so the read
    // has something to find that the test did not write into the store behind the PLC.
    let (fc, _) = hmi_write(&agent, 1, REG_OK, 437);
    assert_eq!(fc, 0x06, "the write must be acknowledged first");
    assert_eq!(writes.load(Ordering::SeqCst), 1);

    // 1. A listed register: the HMI gets an FC03 answer carrying what rmodbus holds.
    let (fc, code, values) = hmi_read(&agent, 0x5101, REG_OK, 1);
    assert_eq!(fc, 0x03, "a listed read must be answered, not excepted (code {code:#04x})");
    assert_eq!(values, vec![437], "and carry the value the device actually holds");
    assert_eq!(store.lock().unwrap().get_holding(REG_OK).unwrap(), 437, "cross-checked in rmodbus");
    assert_eq!(reads.load(Ordering::SeqCst), 1, "exactly one read reached the device");
    assert_eq!(writes.load(Ordering::SeqCst), 1, "and a read writes nothing");

    // 2. An unlisted register: refused with 0x02, and the device is never asked. This is
    // what stops FC03 being a scanner.
    let (fc, code, _) = hmi_read(&agent, 0x5102, REG_OK + 7, 1);
    assert_eq!(fc, 0x83, "an unlisted read must be excepted");
    assert_eq!(code, 0x02, "illegal data address");
    assert_eq!(reads.load(Ordering::SeqCst), 1, "a refused read must not reach the device");

    // 3. A span that starts inside the map and runs out of it is refused whole.
    let (fc, code, _) = hmi_read(&agent, 0x5103, REG_OK, 4);
    assert_eq!(fc, 0x83, "a span crossing the map is refused");
    assert_eq!(code, 0x02);
    assert_eq!(reads.load(Ordering::SeqCst), 1, "and still nothing reached the device");

    // 4. A forged query — an OMQ1 signed by a key the gateway does not know — dies at the
    // mesh layer, before the register rules are even consulted.
    let mut stranger = MeshRouter::new_v0b(fp(0xEE), NET, seed(0xEE), MeshPubRegistry::new());
    let payload = oasis_rt::modbus_read::encode_omq1(&oasis_rt::modbus_read::MbQuery { gateway_id: 1, unit: UNIT, fc: 0x03, start: REG_OK, count: 1 });
    let env = stranger.origin_wrap_v0b(&payload).unwrap();
    let mut g = connect_timeout(&gw, Duration::from_millis(2000)).unwrap();
    write_frame(&mut g, &env).unwrap();
    let reply = read_frame(&mut g).unwrap();
    assert!(oasis_rt::modbus_read::parse_omv1(&reply).is_none(), "a forged query must not be answered with values");
    assert_eq!(reads.load(Ordering::SeqCst), 1, "a forged query must not reach the device");

    // 5. FC04 is refused by name: the map describes holding registers.
    let (fc, code, _) = hmi_read_fc(&agent, 0x5105, REG_OK, 1, 0x04);
    assert_eq!(fc, 0x84, "FC04 is excepted");
    assert_eq!(code, 0x01, "illegal function, not illegal address");
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

/// A read (FC03) through the agent, returning the function code the agent answered with
/// (the high bit set means an exception), the exception code, and the values when there
/// are any.
fn hmi_read(agent: &str, tid: u16, start: u16, count: u16) -> (u8, u8, Vec<u16>) {
    hmi_read_fc(agent, tid, start, count, 0x03)
}

fn hmi_read_fc(agent: &str, tid: u16, start: u16, count: u16, fc: u8) -> (u8, u8, Vec<u16>) {
    let mut c = connect_timeout(agent, Duration::from_millis(2000)).unwrap();
    c.set_read_timeout(Some(Duration::from_millis(2000))).unwrap();
    let req = [(tid >> 8) as u8, tid as u8, 0, 0, 0, 6, UNIT, fc, (start >> 8) as u8, start as u8, (count >> 8) as u8, count as u8];
    c.write_all(&req).unwrap();
    c.flush().unwrap();
    let mut head = [0u8; 8];
    c.read_exact(&mut head).unwrap();
    assert_eq!(&head[0..2], &req[0..2], "the reply must echo the transaction id");
    let body = (u16::from_be_bytes([head[4], head[5]]) as usize).saturating_sub(2);
    let mut tail = vec![0u8; body];
    c.read_exact(&mut tail).unwrap();
    if head[7] & 0x80 != 0 {
        return (head[7], tail[0], Vec::new());
    }
    // head[7] is the function code; tail[0] is the byte count, then the registers.
    let n = tail[0] as usize / 2;
    let vals = (0..n).map(|i| u16::from_be_bytes([tail[1 + 2 * i], tail[2 + 2 * i]])).collect();
    (head[7], 0, vals)
}

/// An idle connection must not stop the gateway serving another one.
///
/// This pins a defect of mine. `serve_gateway_conn` first took the state lock and *then*
/// called a function that blocks in `read_frame`, so a peer that opened a connection and
/// said nothing held the gateway's state for a whole read timeout. There is no attacker in
/// that scenario — just a connection nobody closed — and it made every other write fail
/// with 0x0B. The read test above hit it by leaving one connection open, and the suite
/// went from 0.14 s to 1.64 s while a case starved.
///
/// The measurement is the point: a write behind an idle peer must complete in well under
/// the gateway's own timeout, not just eventually.
#[test]
fn an_idle_connection_does_not_block_another() {
    let writes = Arc::new(AtomicU32::new(0));
    let store = Arc::new(Mutex::new(ModbusStorageSmall::new()));
    let plc = spawn_plc(Arc::clone(&writes), store);
    let log = Arc::new(Mutex::new(Vec::new()));
    let c = conf("127.0.0.1:0", &plc);
    let timeout_ms = c.timeout_ms;
    let gw = spawn_gateway(c, log);
    let agent = spawn_agent(conf("127.0.0.1:0", &gw));
    thread::sleep(Duration::from_millis(120));

    // A peer that connects to the gateway and says nothing at all, held open for the rest
    // of the test.
    let _idle = connect_timeout(&gw, Duration::from_millis(2000)).unwrap();
    thread::sleep(Duration::from_millis(50));

    let t0 = Instant::now();
    let (fc, _) = hmi_write(&agent, 1, REG_OK, 321);
    let elapsed = t0.elapsed();
    assert_eq!(fc, 0x06, "a write must succeed with an idle peer connected");
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    assert!(elapsed < Duration::from_millis(timeout_ms / 2), "took {elapsed:?}, which is within the gateway's {timeout_ms} ms timeout: the idle peer is holding the lock");
}
