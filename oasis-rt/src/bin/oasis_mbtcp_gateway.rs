//! `oasis_mbtcp_gateway` — the machine-side gateway (pilot phase A.1).
//!
//! Listens for v0B envelopes from the agent, verifies them, runs the **unchanged** Part F
//! gate, and writes to the PLC only on `Act`. Then it reads the PLC's answer with a
//! timeout, checks it, and returns the outcome to the agent in a length-prefixed reply.
//!
//! ```text
//! agent ──v0B over TCP (len-prefixed)──▶ gateway ──Modbus TCP──▶ PLC
//!       ◀──OMR1 reply─────────────────           ◀────────────
//! ```
//!
//! ```text
//! oasis_mbtcp_gateway --config gateway.conf
//! ```
//!
//! An audit found that `oasis-rt` contained **no socket at all**: the Modbus TCP layer was
//! pure, so it could decide and build frames but never reach a PLC. This binary is that
//! missing half, and it adds nothing to the decision: `decide_tcp` wraps the Part F gate
//! unchanged, and the frame is still built only in the `Act` branch.
//!
//! Every decision is appended to the **decision journal**, refusals included, which is
//! what Annex III 1.1.9 asks for and what only the RP2040 firmware did until now.
//!
//! ⚠️ **No PLC and no silicon.** It talks to whatever answers Modbus TCP at the configured
//! address; the integration test points it at an `rmodbus` server on localhost. The RTU
//! path is the one proven on three RP2040 against an independent device.
//! ⚠️ Keys come from the config file. There is **no compiled seed**, which was phase 1.2's
//! lesson, but the file itself is as readable as any file on the host.

use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use oasis_rt::actuation::{Decision, OrderClass};
use oasis_rt::journal::{Journal, LoggedDecision};
use oasis_rt::mbtcp_net::{connect_timeout, modbus_exchange, read_frame, write_frame};
use oasis_rt::mesh::{inner_slice, MeshDecision, MeshRouter};
use oasis_rt::modbus_gateway::{parse_omb1, Gateway, OrderContext, Response, RuleCheck};
use oasis_rt::modbus_tcp::{check_tcp_response, decide_tcp, refusal_code, Outcome, TcpFrame};

use oasis_rt::mbtcp_conf::Config;

/// Reply to the agent: `"OMR1" | cmd_seq u32 | kind u8 | code u8`.
pub const OMR1_MAGIC: [u8; 4] = *b"OMR1";
pub const OMR1_LEN: usize = 10;

fn encode_reply(cmd_seq: u32, outcome: &Outcome) -> [u8; OMR1_LEN] {
    let mut b = [0u8; OMR1_LEN];
    b[0..4].copy_from_slice(&OMR1_MAGIC);
    b[4..8].copy_from_slice(&cmd_seq.to_le_bytes());
    let (kind, code) = match outcome {
        Outcome::Done => (0u8, 0u8),
        Outcome::Refused(r, rules) => (1, refusal_code(*r, *rules)),
        Outcome::PlcException(c) => (2, *c),
        Outcome::NoAnswer => (3, 0),
    };
    b[8] = kind;
    b[9] = code;
    b
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

struct Shared {
    gw: Gateway,
    router: MeshRouter,
    journal: Journal,
    boot_id: u64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = match args.iter().position(|a| a == "--config").and_then(|i| args.get(i + 1)) {
        Some(p) => p.clone(),
        None => {
            eprintln!("usage: oasis_mbtcp_gateway --config <file>");
            std::process::exit(2);
        }
    };
    let conf = match Config::load(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config: {e}");
            std::process::exit(2);
        }
    };

    // A fresh boot id per process start is what makes an order from a previous run
    // refusable; reusing one would reopen the replay window the gate exists to close.
    let boot_id = now_ms();
    let shared = Arc::new(Mutex::new(Shared {
        gw: Gateway::new(),
        router: MeshRouter::new_v0b(conf.our_fp, conf.network_id, conf.our_seed.clone(), conf.registry.clone()),
        journal: Journal::new(boot_id),
        boot_id,
    }));

    let listener = match TcpListener::bind(&conf.listen) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {}: {e}", conf.listen);
            std::process::exit(2);
        }
    };
    let bound = listener.local_addr().map(|a| a.to_string()).unwrap_or_default();
    println!("GATEWAY listen={bound} plc={} unit=0x{:02x} boot_id={boot_id} registers={}", conf.plc, conf.unit, conf.map.len());

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let shared = Arc::clone(&shared);
                let conf = conf.clone();
                std::thread::spawn(move || {
                    if let Err(e) = serve(s, &shared, &conf) {
                        // The normal end of a short exchange, and also what a third party
                        // poking the port produces. Logged, never fatal: one bad peer must
                        // not stop the gateway.
                        println!("GATEWAY conn_end err={:?}", e.kind());
                    }
                });
            }
            Err(e) => println!("GATEWAY accept err={:?}", e.kind()),
        }
    }
}

fn serve(mut sock: TcpStream, shared: &Arc<Mutex<Shared>>, conf: &Config) -> io::Result<()> {
    let to = Duration::from_millis(conf.timeout_ms);
    sock.set_read_timeout(Some(to))?;
    sock.set_write_timeout(Some(to))?;
    let peer = sock.peer_addr().map(|a| a.to_string()).unwrap_or_default();

    // A third party speaking plain Modbus to this port lands here: its bytes are not a
    // length-prefixed v0B envelope, so either the prefix is refused or the mesh layer
    // drops it. Either way nothing reaches the PLC.
    let env = read_frame(&mut sock)?;

    // A.3, the gateway's clock. The agent must not sign an order against its own clock —
    // freshness is judged here, in this process's boot_id and now_ms — so it asks, and
    // this is the answer: `"OTM1" | boot_id u64 | now_ms u64`. The same shape part K
    // signs as a beacon on the mesh; over a TCP connection to the one party whose clock
    // matters, the connection is the binding.
    if env.len() == 4 && env[..] == *b"OTQ1" {
        let boot_id = { shared.lock().unwrap().boot_id };
        let mut out = [0u8; 20];
        out[0..4].copy_from_slice(b"OTM1");
        out[4..12].copy_from_slice(&boot_id.to_le_bytes());
        out[12..20].copy_from_slice(&now_ms().to_le_bytes());
        return write_frame(&mut sock, &out);
    }

    let act: Option<(TcpFrame, u32)> = {
        let mut st = shared.lock().unwrap();
        let arrived = match st.router.process(&env) {
            MeshDecision::Drop(why) => {
                st.journal.append([0u8; 8], 0, OrderClass::Act, LoggedDecision::Reject(oasis_rt::actuation::Reason::NotVerified), 0);
                println!("GATEWAY {peer} MESH_DROP why={why:?} -> no PLC write");
                return write_frame(&mut sock, &encode_reply(0, &Outcome::NoAnswer));
            }
            MeshDecision::Arrived { envelope, .. } => envelope,
        };
        let order = match parse_omb1(inner_slice(&arrived)) {
            Some(o) => o,
            None => {
                println!("GATEWAY {peer} NOT_AN_ORDER -> no PLC write");
                return write_frame(&mut sock, &encode_reply(0, &Outcome::NoAnswer));
            }
        };
        let ctx = OrderContext { v0b_ok: true, authorized: true, revoked: false, actuator_boot_id: st.boot_id, now_ms: now_ms(), r14_safe: true };
        let tid = (order.cmd_seq & 0xFFFF) as u16;
        let (d, rules, frame) = decide_tcp(&mut st.gw, &ctx, &order, conf.unit, &conf.map, tid);

        st.journal.append(
            [0u8; 8],
            order.cmd_seq,
            OrderClass::Act,
            match d {
                Decision::Act => LoggedDecision::Act,
                Decision::Reject(r) => LoggedDecision::Reject(r),
            },
            0,
        );

        match (d, frame) {
            (Decision::Act, Some(f)) => Some((f, order.cmd_seq)),
            (Decision::Reject(r), _) => {
                println!("GATEWAY {peer} seq={} Reject({r:?}) -> no PLC write", order.cmd_seq);
                return write_frame(&mut sock, &encode_reply(order.cmd_seq, &Outcome::Refused(r, rules)));
            }
            (Decision::Act, None) => return write_frame(&mut sock, &encode_reply(order.cmd_seq, &Outcome::NoAnswer)),
        }
    };

    // Only here does a byte reach the PLC.
    let (frame, cmd_seq) = act.unwrap();
    let outcome = match talk_to_plc(conf, &frame) {
        Ok(resp) => match check_tcp_response(&frame, &resp) {
            Response::Ack => Outcome::Done,
            Response::Exception(c) => Outcome::PlcException(c),
            other => {
                println!("GATEWAY plc_bad_response={other:?}");
                Outcome::NoAnswer
            }
        },
        Err(e) => {
            println!("GATEWAY plc_err={:?}", e.kind());
            Outcome::NoAnswer
        }
    };

    println!("GATEWAY {peer} seq={cmd_seq} Act -> PLC, outcome={outcome:?}");
    write_frame(&mut sock, &encode_reply(cmd_seq, &outcome))
}

fn talk_to_plc(conf: &Config, frame: &TcpFrame) -> io::Result<Vec<u8>> {
    let mut plc = connect_timeout(&conf.plc, Duration::from_millis(conf.timeout_ms))?;
    let t0 = Instant::now();
    let resp = modbus_exchange(&mut plc, frame.as_slice())?;
    println!("GATEWAY plc_rtt_us={}", t0.elapsed().as_micros());
    Ok(resp)
}

#[allow(dead_code)]
fn unused_rulecheck_marker(_: RuleCheck) {}
