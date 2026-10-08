//! The gateway and agent logic, in the library so the integration test exercises **the
//! shipped path** and not a copy of it (pilot phase A).
//!
//! `oasis_mbtcp_gateway` and `oasis_mbtcp_agent` are thin wrappers over these functions:
//! they parse a config, bind a socket and call in. A test that reimplemented the exchange
//! would prove something about the test.
//!
//! That last sentence was aspirational when this module was written, and the gap it left
//! was not theoretical. Both binaries carried their own copy of the logic, and the agent's
//! copy still had the defect the library had already fixed: it rebuilt the refusal from an
//! invented `Reason::NotAuthorized` and `RuleCheck::Ok`, which `refusal_code` maps to 0x0A,
//! so an unlisted register reached the HMI as "gateway path unavailable" instead of
//! "illegal data address". The integration test was green throughout, because it tested
//! this module and the operator would have run the other one. Everything down to the HMI
//! loop now lives here, and the binaries hold argument parsing, a listener and a thread.

use std::io;
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::actuation::{Decision, OrderClass, Reason, MAX_VALIDITY_MS};
use crate::journal::{Journal, LoggedDecision};
use crate::mbtcp_conf::Config;
use crate::mbtcp_net::{connect_timeout, modbus_exchange, read_frame, write_frame};
use crate::mesh::{inner_slice, MeshDecision, MeshRouter};
use crate::modbus_gateway::{encode_omb1, parse_omb1, Gateway, OrderContext, Response, RuleCheck};
use crate::modbus_tcp::{check_tcp_response, decide_tcp, hmi_response, parse_tcp_write, refusal_code, Outcome, TcpWrite, EXC_GATEWAY_PATH_UNAVAILABLE};

/// Reply to the agent: `"OMR1" | cmd_seq u32 | kind u8 | code u8`.
pub const OMR1_MAGIC: [u8; 4] = *b"OMR1";
pub const OMR1_LEN: usize = 10;
/// The agent asks the gateway for its clock with these four bytes.
pub const CLOCK_REQ: [u8; 4] = *b"OTQ1";
/// And gets `"OTM1" | boot_id u64 | now_ms u64` back.
pub const CLOCK_REPLY_LEN: usize = 20;

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

pub fn encode_reply(cmd_seq: u32, outcome: &Outcome) -> [u8; OMR1_LEN] {
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

pub fn decode_reply(reply: &[u8]) -> Option<(u32, Outcome)> {
    if reply.len() != OMR1_LEN || reply[0..4] != OMR1_MAGIC {
        return None;
    }
    let seq = u32::from_le_bytes(reply[4..8].try_into().unwrap());
    let outcome = match (reply[8], reply[9]) {
        (0, _) => Outcome::Done,
        // The reply carries the Modbus exception the gateway chose. The agent must render
        // **that** code and nothing else: an earlier version threw it away and recomputed
        // one from a reason it invented, so an unlisted register reached the HMI as 0x0A
        // "gateway path unavailable" instead of 0x02 "illegal data address". The
        // integration test caught it. Reconstructing the rule from the code is how the
        // agent reports the gateway's decision without pretending to know its reason.
        (1, code) => Outcome::Refused(
            Reason::OutOfLimits,
            match code {
                crate::modbus_tcp::EXC_ILLEGAL_ADDRESS => RuleCheck::RegisterNotAllowed(0),
                crate::modbus_tcp::EXC_ILLEGAL_VALUE => RuleCheck::ValueOutOfRange(0, 0),
                _ => RuleCheck::Ok,
            },
        ),
        (2, code) => Outcome::PlcException(code),
        _ => Outcome::NoAnswer,
    };
    Some((seq, outcome))
}

/// The gateway's mutable state. One per process, behind the caller's mutex.
pub struct GatewayState {
    pub gw: Gateway,
    pub router: MeshRouter,
    pub journal: Journal,
    /// Fresh per process start: that is what makes an order from a previous run
    /// refusable, so reusing one would reopen the replay window the gate exists to close.
    pub boot_id: u64,
}

impl GatewayState {
    pub fn new(router: MeshRouter, boot_id: u64) -> Self {
        Self { gw: Gateway::new(), router, journal: Journal::new(boot_id), boot_id }
    }
}

/// What the gateway did with one connection, for a caller that wants to log or count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Served {
    /// Answered the agent's clock request.
    Clock { boot_id: u64 },
    /// The mesh layer refused the envelope; the gate was never reached.
    MeshDrop(&'static str),
    /// Verified, and the gate decided. `plc_written` is the ground truth that matters.
    Decided { cmd_seq: u32, decision: Decision, outcome: Outcome, plc_written: bool },
    /// Verified but not an order.
    NotAnOrder,
}

/// Handle one agent connection: read a frame, answer a clock request, or verify → decide
/// → write to the PLC **only on `Act`** → check the answer → reply.
///
/// Every decision is appended to the journal, refusals included: Annex III 1.1.9 asks for
/// "légitime **ou** illégitime", and until this existed only the RP2040 firmware did it.
pub fn handle_gateway_conn(sock: &mut TcpStream, st: &mut GatewayState, conf: &Config) -> io::Result<Served> {
    let to = Duration::from_millis(conf.timeout_ms);
    sock.set_read_timeout(Some(to))?;
    sock.set_write_timeout(Some(to))?;

    // A third party speaking plain Modbus to this port lands here: its bytes are not a
    // length-prefixed v0B envelope, so either the prefix is refused by `read_frame` or the
    // mesh layer drops them below. Either way nothing reaches the PLC.
    let env = read_frame(sock)?;

    if env.len() == CLOCK_REQ.len() && env[..] == CLOCK_REQ[..] {
        let mut out = [0u8; CLOCK_REPLY_LEN];
        out[0..4].copy_from_slice(b"OTM1");
        out[4..12].copy_from_slice(&st.boot_id.to_le_bytes());
        out[12..20].copy_from_slice(&now_ms().to_le_bytes());
        write_frame(sock, &out)?;
        return Ok(Served::Clock { boot_id: st.boot_id });
    }

    let arrived = match st.router.process(&env) {
        MeshDecision::Drop(why) => {
            // A dropped envelope has no order to name, so it is journalled as a refusal
            // at the verification step with sequence 0 — enough to count it, and honest
            // that nothing else is known.
            st.journal.append([0u8; 8], 0, OrderClass::Act, LoggedDecision::Reject(Reason::NotVerified), 0);
            write_frame(sock, &encode_reply(0, &Outcome::NoAnswer))?;
            return Ok(Served::MeshDrop(why));
        }
        MeshDecision::Arrived { envelope, .. } => envelope,
    };

    let order = match parse_omb1(inner_slice(&arrived)) {
        Some(o) => o,
        None => {
            write_frame(sock, &encode_reply(0, &Outcome::NoAnswer))?;
            return Ok(Served::NotAnOrder);
        }
    };

    let ctx = OrderContext { v0b_ok: true, authorized: true, revoked: false, actuator_boot_id: st.boot_id, now_ms: now_ms(), r14_safe: true };
    let tid = (order.cmd_seq & 0xFFFF) as u16;
    let (decision, rules, frame) = decide_tcp(&mut st.gw, &ctx, &order, conf.unit, &conf.map, tid);

    st.journal.append(
        [0u8; 8],
        order.cmd_seq,
        OrderClass::Act,
        match decision {
            Decision::Act => LoggedDecision::Act,
            Decision::Reject(r) => LoggedDecision::Reject(r),
        },
        0,
    );

    let (outcome, plc_written) = match (decision, frame) {
        (Decision::Act, Some(f)) => {
            // Only here does a byte reach the PLC.
            match connect_timeout(&conf.plc, to).and_then(|mut p| modbus_exchange(&mut p, f.as_slice())) {
                Ok(resp) => (
                    match check_tcp_response(&f, &resp) {
                        Response::Ack => Outcome::Done,
                        Response::Exception(c) => Outcome::PlcException(c),
                        _ => Outcome::NoAnswer,
                    },
                    true,
                ),
                Err(_) => (Outcome::NoAnswer, false),
            }
        }
        (Decision::Reject(r), _) => (Outcome::Refused(r, rules), false),
        (Decision::Act, None) => (Outcome::NoAnswer, false),
    };

    write_frame(sock, &encode_reply(order.cmd_seq, &outcome))?;
    Ok(Served::Decided { cmd_seq: order.cmd_seq, decision, outcome, plc_written })
}

/// A strictly increasing command sequence, persisted to a file.
///
/// `reserve` writes the number **before** returning it, so a crash between reserving and
/// sending can only lose a sequence, never reuse one. Same reasoning as `tx_lease` on the
/// MCU: a reused `cmd_seq` is a replay the gateway would have to accept.
pub struct SeqStore {
    path: String,
    next: u32,
}

impl SeqStore {
    /// A missing file is a first run and starts at 1. A corrupt one is an error, not a
    /// guess: guessing a sequence is exactly how a replay window reopens.
    pub fn load(path: &str) -> Result<Self, String> {
        let next = match std::fs::read_to_string(path) {
            Ok(t) => t.trim().parse::<u32>().map_err(|e| format!("{path}: {e}"))? + 1,
            Err(e) if e.kind() == io::ErrorKind::NotFound => 1,
            Err(e) => return Err(format!("{path}: {e}")),
        };
        Ok(SeqStore { path: path.to_string(), next })
    }

    pub fn peek(&self) -> u32 {
        self.next
    }

    pub fn reserve(&mut self) -> Result<u32, String> {
        let n = self.next;
        std::fs::write(&self.path, n.to_string()).map_err(|e| format!("{}: {e}", self.path))?;
        self.next = n.saturating_add(1);
        Ok(n)
    }
}

/// The agent's mutable state: its signing identity and its sequence.
pub struct AgentState {
    pub origin: MeshRouter,
    pub seq: SeqStore,
}

/// Serve one HMI connection for as long as it sends requests.
///
/// The HMI speaks plain Modbus TCP and is **not modified**. Returns when it closes or goes
/// quiet past the timeout — which is the normal end of a connection, not a failure.
///
/// Never leaves the HMI waiting: a read, a malformed frame, an unreachable gateway and a
/// timeout all produce an answer. Silence towards an operator's screen is worse than a no.
pub fn serve_hmi(hmi: &mut TcpStream, agent: &Mutex<AgentState>, conf: &Config) -> io::Result<()> {
    let to = Duration::from_millis(conf.timeout_ms);
    hmi.set_read_timeout(Some(to))?;
    hmi.set_write_timeout(Some(to))?;

    loop {
        let req = crate::mbtcp_net::modbus_read_request(hmi)?;
        match parse_tcp_write(&req) {
            // Not a write this agent handles — a read, or malformed. ⚠️ Reads are refused
            // rather than relayed, so the gateway stays the PLC's only path; see the
            // binary's header for the decision.
            None => {
                let exc = exception_for(&req, EXC_GATEWAY_PATH_UNAVAILABLE);
                write_all_flush(hmi, &exc)?;
            }
            Some(w) => {
                let outcome = agent_forward(agent, conf, &w);
                let (resp, n) = hmi_response(&w, outcome);
                write_all_flush(hmi, &resp[..n])?;
            }
        }
    }
}

fn write_all_flush(sock: &mut TcpStream, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    sock.write_all(bytes)?;
    sock.flush()
}

/// A Modbus TCP exception for a request the agent will not forward. Built from whatever
/// arrived, so even a truncated frame gets an answer rather than silence.
pub fn exception_for(req: &[u8], code: u8) -> [u8; 9] {
    let mut b = [0u8; 9];
    b[0] = *req.first().unwrap_or(&0);
    b[1] = *req.get(1).unwrap_or(&0);
    b[5] = 3;
    b[6] = *req.get(6).unwrap_or(&0);
    b[7] = req.get(7).map_or(0x80, |fc| fc | 0x80);
    b[8] = code;
    b
}

/// The agent side of one HMI write: ask the gateway for its clock, sign an order against
/// **that** clock, send it, and return the outcome.
///
/// Never returns silence: every failure becomes `Outcome::NoAnswer`, which `hmi_response`
/// turns into exception 0x0B. An HMI left waiting is worse than one told no.
pub fn agent_forward(agent: &Mutex<AgentState>, conf: &Config, w: &TcpWrite) -> Outcome {
    match agent_try(agent, conf, w) {
        Ok(o) => o,
        Err(_) => Outcome::NoAnswer,
    }
}

/// Same, with the reason preserved — the binaries log it, the tests assert on the outcome.
pub fn agent_try(agent: &Mutex<AgentState>, conf: &Config, w: &TcpWrite) -> Result<Outcome, String> {
    let to = Duration::from_millis(conf.timeout_ms);

    // 1. The gateway's clock. No order is signed against ours: freshness is judged in the
    // gateway's `boot_id` and `now_ms`, so those are what the deadline is built from.
    let (boot_id, gw_now) = {
        let mut s = connect_timeout(&conf.plc, to).map_err(|e| format!("clock: {:?}", e.kind()))?;
        write_frame(&mut s, &CLOCK_REQ).map_err(|e| format!("clock send: {:?}", e.kind()))?;
        let r = read_frame(&mut s).map_err(|e| format!("clock read: {:?}", e.kind()))?;
        if r.len() != CLOCK_REPLY_LEN || r[0..4] != *b"OTM1" {
            return Err("clock: bad answer".into());
        }
        (u64::from_le_bytes(r[4..12].try_into().unwrap()), u64::from_le_bytes(r[12..20].try_into().unwrap()))
    };

    // 2. Sign under the lock, and only that: the round trip below must not hold it.
    let (env, seq) = {
        let mut st = agent.lock().map_err(|_| "agent state poisoned".to_string())?;
        let seq = st.seq.reserve()?;
        // `gateway_id` is the logical actuator, not the Modbus unit: the unit is inside
        // the order and is checked by the register rules.
        let order = w.to_order(conf.gateway_id, seq, boot_id, gw_now + MAX_VALIDITY_MS / 2);
        let (buf, n) = encode_omb1(&order).ok_or("order: unsupported write")?;
        let env = st.origin.origin_wrap_v0b(&buf[..n]).ok_or("sign failed")?;
        (env, seq)
    };

    // 3. Send, and wait for the outcome.
    let mut g = connect_timeout(&conf.plc, to).map_err(|e| format!("gw: {:?}", e.kind()))?;
    write_frame(&mut g, &env).map_err(|e| format!("gw send: {:?}", e.kind()))?;
    let reply = read_frame(&mut g).map_err(|e| format!("gw read: {:?}", e.kind()))?;
    let (got, outcome) = decode_reply(&reply).ok_or("gw: bad reply")?;
    if got != seq && got != 0 {
        return Err(format!("gw: reply for {got}, expected {seq}"));
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> String {
        let p = std::env::temp_dir().join(format!("oasis_seqstore_{}_{}.seq", std::process::id(), name));
        let _ = std::fs::remove_file(&p);
        p.to_str().unwrap().to_string()
    }

    /// The property the agent's header claims and nothing checked until now: the number is
    /// on disk **before** the caller has it, so a crash between reserving and sending can
    /// only lose a sequence. Losing one is harmless — the gateway wants strictly newer,
    /// not consecutive — while reusing one is a replay it would have to accept.
    #[test]
    fn seq_store_persists_before_returning_and_never_repeats() {
        let p = scratch("persist");
        let mut s = SeqStore::load(&p).unwrap();
        assert_eq!(s.peek(), 1, "a missing file is a first run");
        assert_eq!(s.reserve().unwrap(), 1);
        assert_eq!(std::fs::read_to_string(&p).unwrap().trim(), "1", "on disk already");
        assert_eq!(s.reserve().unwrap(), 2);

        // A restart, which is the case that matters.
        let mut after = SeqStore::load(&p).unwrap();
        assert_eq!(after.reserve().unwrap(), 3, "a restart must not reuse 2");
        let _ = std::fs::remove_file(&p);
    }

    /// A corrupt file is an error, not a guess. Starting over at 1 would hand the gateway
    /// sequences it has already executed.
    #[test]
    fn seq_store_refuses_a_corrupt_file() {
        let p = scratch("corrupt");
        std::fs::write(&p, "not a number").unwrap();
        assert!(SeqStore::load(&p).is_err());
        let _ = std::fs::remove_file(&p);
    }

    /// "Never silence towards the HMI" has to hold for a frame too short to parse, which
    /// is where an exception builder normally panics on a slice.
    #[test]
    fn exception_for_answers_even_a_truncated_frame() {
        for len in 0..9usize {
            let req: Vec<u8> = (0..len).map(|i| i as u8 + 1).collect();
            let e = exception_for(&req, EXC_GATEWAY_PATH_UNAVAILABLE);
            assert_eq!(e.len(), 9);
            assert_eq!(e[8], EXC_GATEWAY_PATH_UNAVAILABLE);
            assert_eq!(e[7] & 0x80, 0x80, "an exception sets the high bit of the function code");
            assert_eq!(e[5], 3, "MBAP length covers unit + fc + code");
        }
        // And for a complete request it echoes the transaction id, so the HMI can match it.
        let req = [0xAB, 0xCD, 0, 0, 0, 6, 0x11, 0x06, 0, 10, 0, 1];
        let e = exception_for(&req, crate::modbus_tcp::EXC_ILLEGAL_ADDRESS);
        assert_eq!(&e[0..2], &[0xAB, 0xCD]);
        assert_eq!(e[6], 0x11, "unit echoed");
        assert_eq!(e[7], 0x86, "FC06 with the high bit");
    }
}
