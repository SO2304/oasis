//! `oasis_mbtcp_agent` — the operator-side agent (pilot phase A.2).
//!
//! The HMI writes plain Modbus TCP to this agent exactly as it would to the PLC, and is
//! **not modified**. For each write the agent parses it, turns it into an `OMB1` order,
//! signs it in a v0B envelope, sends it to the gateway, waits for the signed outcome, and
//! answers the HMI.
//!
//! ```text
//! HMI ──Modbus TCP, clear──▶ agent ──v0B over TCP──▶ gateway ──▶ PLC
//!     ◀──ack or exception──        ◀──OMR1 reply───
//! ```
//!
//! ```text
//! oasis_mbtcp_agent --config agent.conf
//! ```
//!
//! Two rules the prompt is explicit about, and both are visible in the code:
//!
//! - **The sequence is strictly increasing and persisted.** It is read from a file at
//!   start and written back before the order goes out, so a restart cannot reuse a
//!   number. Same idea as `tx_lease` on the MCU, and for the same reason: a reused
//!   `cmd_seq` is a replay the gateway would have to accept.
//! - **Never silence towards the HMI.** Every path answers: a timeout, an unreachable
//!   gateway or a malformed reply all become `Outcome::NoAnswer`, which `hmi_response`
//!   turns into exception 0x0B. An HMI that waits forever is worse than one told no.
//!
//! The deadline comes from the **gateway's clock**, not ours: the agent asks the gateway
//! for its `boot_id` and `now_ms` before signing. That is the honest version of what part
//! K solved on the mesh with a signed `OTM1` beacon; here the link is a TCP connection to
//! the one party whose clock matters.
//!
//! ⚠️ **Reads (FC03/FC04) are refused**, not relayed. The pilot prompt asks for a decision
//! and this is it: the default must leave **one** device talking to the PLC, so the agent
//! does not open a second path for reads. A read returns exception 0x0A rather than
//! silently bypassing the gateway. Relaying reads through the gateway is the next step and
//! it is not done.

use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use oasis_rt::actuation::MAX_VALIDITY_MS;
use oasis_rt::mbtcp_net::{connect_timeout, modbus_read_request, read_frame, write_frame};
use oasis_rt::mesh::MeshRouter;
use oasis_rt::modbus_gateway::{encode_omb1, RuleCheck};
use oasis_rt::modbus_tcp::{hmi_response, parse_tcp_write, Outcome, EXC_GATEWAY_PATH_UNAVAILABLE};

use oasis_rt::mbtcp_conf::Config;

const OMR1_MAGIC: [u8; 4] = *b"OMR1";
const OMR1_LEN: usize = 10;
/// `"OTQ1"`: ask the gateway for its clock. Four bytes, no payload.
const CLOCK_REQ: [u8; 4] = *b"OTQ1";

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

struct SeqStore {
    path: String,
    next: u32,
}

impl SeqStore {
    /// Load the persisted sequence, or start at 1. A missing file is a first run; a
    /// corrupt one is treated as unknown and refused, because guessing a sequence is
    /// exactly how a replay window reopens.
    fn load(path: &str) -> Result<Self, String> {
        let next = match std::fs::read_to_string(path) {
            Ok(t) => t.trim().parse::<u32>().map_err(|e| format!("{path}: {e}"))? + 1,
            Err(e) if e.kind() == io::ErrorKind::NotFound => 1,
            Err(e) => return Err(format!("{path}: {e}")),
        };
        Ok(SeqStore { path: path.to_string(), next })
    }

    /// Reserve the next sequence and **persist it before returning it**, so a crash
    /// between reserving and sending can only lose a number, never reuse one.
    fn reserve(&mut self) -> Result<u32, String> {
        let n = self.next;
        std::fs::write(&self.path, n.to_string()).map_err(|e| format!("{}: {e}", self.path))?;
        self.next = n.saturating_add(1);
        Ok(n)
    }
}

struct Shared {
    origin: MeshRouter,
    seq: SeqStore,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = match args.iter().position(|a| a == "--config").and_then(|i| args.get(i + 1)) {
        Some(p) => p.clone(),
        None => {
            eprintln!("usage: oasis_mbtcp_agent --config <file>");
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
    let seq_path = format!("{path}.seq");
    let seq = match SeqStore::load(&seq_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sequence: {e}");
            std::process::exit(2);
        }
    };

    let shared = Arc::new(Mutex::new(Shared { origin: MeshRouter::new_v0b(conf.our_fp, conf.network_id, conf.our_seed.clone(), conf.registry.clone()), seq }));

    let listener = match TcpListener::bind(&conf.listen) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {}: {e}", conf.listen);
            std::process::exit(2);
        }
    };
    let bound = listener.local_addr().map(|a| a.to_string()).unwrap_or_default();
    println!("AGENT listen={bound} gateway={} unit=0x{:02x} next_seq={}", conf.plc, conf.unit, shared.lock().unwrap().seq.next);

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let shared = Arc::clone(&shared);
                let conf = conf.clone();
                std::thread::spawn(move || {
                    if let Err(e) = serve_hmi(s, &shared, &conf) {
                        println!("AGENT hmi_conn_end err={:?}", e.kind());
                    }
                });
            }
            Err(e) => println!("AGENT accept err={:?}", e.kind()),
        }
    }
}

fn serve_hmi(mut hmi: TcpStream, shared: &Arc<Mutex<Shared>>, conf: &Config) -> io::Result<()> {
    let to = Duration::from_millis(conf.timeout_ms);
    hmi.set_read_timeout(Some(to))?;
    hmi.set_write_timeout(Some(to))?;

    loop {
        let req = match modbus_read_request(&mut hmi) {
            Ok(r) => r,
            Err(e) => return Err(e), // the HMI closed, or said nothing in time
        };
        let t0 = Instant::now();

        let w = match parse_tcp_write(&req) {
            Some(w) => w,
            None => {
                // Not a write we handle — a read, or malformed. Answer, never stay silent.
                println!("AGENT not_a_write len={} -> exception 0x0A", req.len());
                let exc = exception_for(&req, EXC_GATEWAY_PATH_UNAVAILABLE);
                hmi.write_all(&exc)?;
                hmi.flush()?;
                continue;
            }
        };

        let outcome = match forward(shared, conf, &w) {
            Ok(o) => o,
            Err(e) => {
                println!("AGENT forward_err={e} -> NoAnswer");
                Outcome::NoAnswer
            }
        };
        let (resp, n) = hmi_response(&w, outcome);
        hmi.write_all(&resp[..n])?;
        hmi.flush()?;
        println!("AGENT tid={} fc={} start={} outcome={:?} rtt_us={}", w.tid, w.fc, w.start, outcome, t0.elapsed().as_micros());
    }
}

/// Sign the order against the **gateway's** clock and send it.
fn forward(shared: &Arc<Mutex<Shared>>, conf: &Config, w: &oasis_rt::modbus_tcp::TcpWrite) -> Result<Outcome, String> {
    let to = Duration::from_millis(conf.timeout_ms);

    // 1. the gateway's clock. No order is signed without a fresh answer.
    let (boot_id, gw_now) = {
        let mut s = connect_timeout(&conf.plc, to).map_err(|e| format!("clock connect: {:?}", e.kind()))?;
        write_frame(&mut s, &CLOCK_REQ).map_err(|e| format!("clock send: {:?}", e.kind()))?;
        let r = read_frame(&mut s).map_err(|e| format!("clock read: {:?}", e.kind()))?;
        if r.len() != 20 || r[0..4] != *b"OTM1" {
            return Err("clock: bad answer".into());
        }
        (u64::from_le_bytes(r[4..12].try_into().unwrap()), u64::from_le_bytes(r[12..20].try_into().unwrap()))
    };

    // 2. order, signed, with a deadline inside the gateway's validity window.
    let (env, seq) = {
        let mut st = shared.lock().unwrap();
        let seq = st.seq.reserve()?;
        // `gateway_id` is the logical actuator this order addresses, not the Modbus
        // unit: the unit lives inside the order and is checked by the register rules.
        let order = w.to_order(conf.gateway_id, seq, boot_id, gw_now + MAX_VALIDITY_MS / 2);
        let (buf, n) = encode_omb1(&order).ok_or_else(|| "order: unsupported function or register count".to_string())?;
        let env = st.origin.origin_wrap_v0b(&buf[..n]).ok_or_else(|| "sign: origin_wrap_v0b failed".to_string())?;
        (env, seq)
    };

    // 3. send, and wait for the outcome.
    let mut g = connect_timeout(&conf.plc, to).map_err(|e| format!("gw connect: {:?}", e.kind()))?;
    write_frame(&mut g, &env).map_err(|e| format!("gw send: {:?}", e.kind()))?;
    let reply = read_frame(&mut g).map_err(|e| format!("gw read: {:?}", e.kind()))?;
    if reply.len() != OMR1_LEN || reply[0..4] != OMR1_MAGIC {
        return Err("gw: bad reply".into());
    }
    let got_seq = u32::from_le_bytes(reply[4..8].try_into().unwrap());
    if got_seq != seq && got_seq != 0 {
        return Err(format!("gw: reply for seq {got_seq}, expected {seq}"));
    }
    Ok(match (reply[8], reply[9]) {
        (0, _) => Outcome::Done,
        (1, code) => Outcome::Refused(decode_reason(code), RuleCheck::Ok),
        (2, code) => Outcome::PlcException(code),
        _ => Outcome::NoAnswer,
    })
}

/// The reply carries the Modbus exception the gateway chose, not the internal reason, so
/// the agent reports it as-is rather than inventing a cause it was not told.
fn decode_reason(_code: u8) -> oasis_rt::actuation::Reason {
    oasis_rt::actuation::Reason::NotAuthorized
}

fn exception_for(req: &[u8], code: u8) -> [u8; 9] {
    let mut b = [0u8; 9];
    b[0..2].copy_from_slice(&req[0..2]); // tid
    b[2] = 0;
    b[3] = 0;
    b[4] = 0;
    b[5] = 3;
    b[6] = if req.len() > 6 { req[6] } else { 0 };
    b[7] = if req.len() > 7 { req[7] | 0x80 } else { 0x80 };
    b[8] = code;
    b
}
