//! The machine-side half: verify, decide, and write to the PLC only on `Act`.

use std::io;
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::Duration;

use super::{encode_reply, now_ms, CLOCK_REPLY_LEN, CLOCK_REQ};
use crate::actuation::{Decision, OrderClass, Reason};
use crate::journal::{Journal, LoggedDecision};
use crate::mbtcp_conf::Config;
use crate::mbtcp_net::{connect_timeout, modbus_exchange, read_frame, write_frame};
use crate::mesh::{inner_slice, MeshDecision, MeshRouter};
use crate::modbus_gateway::{parse_omb1, Gateway, OrderContext, Response};
use crate::modbus_read::{check_read_response, decide_read, encode_omv1, parse_omq1, MbQuery, ReadCheck};
use crate::modbus_tcp::{check_tcp_response, decide_tcp, Outcome, TcpFrame};

/// The gateway's mutable state. One per process, behind the caller's mutex.
pub struct GatewayState {
    pub gw: Gateway,
    pub router: MeshRouter,
    pub journal: Journal,
    /// Fresh per process start: that is what makes an order from a previous run
    /// refusable, so reusing one would reopen the replay window the gate exists to close.
    pub boot_id: u64,
    /// The connection to the PLC, kept open between orders.
    ///
    /// It used to be dialled per order. `bench_mbtcp_pilot` put a number on that: of the
    /// 3.1 ms an authorised write cost end to end, 264 µs was sign + verify + gate and the
    /// rest was three TCP connects. A cached connection removes one of them. On any error
    /// it is dropped and redialled once, because a half-closed socket must not turn into a
    /// silent non-write — the PLC not answering has to reach the operator as an exception.
    plc: Option<TcpStream>,
}

impl GatewayState {
    pub fn new(router: MeshRouter, boot_id: u64) -> Self {
        Self { gw: Gateway::new(), router, journal: Journal::new(boot_id), boot_id, plc: None }
    }

    /// Exchange one frame with the PLC, redialling once if the cached socket is stale.
    ///
    /// A cached connection fails in a way a fresh one does not: the peer may have closed
    /// it while idle, and the failure then surfaces on the *write*, after the gate has
    /// already said `Act`. So one retry on a fresh socket, and no more — a second failure
    /// is the PLC being unreachable, which is an answer the operator needs, not a loop.
    fn plc_exchange(&mut self, conf: &Config, frame: &TcpFrame) -> io::Result<Vec<u8>> {
        let to = Duration::from_millis(conf.timeout_ms);
        if let Some(mut sock) = self.plc.take() {
            // On an error the stale socket is simply dropped here and the code falls
            // through to a fresh connection.
            if let Ok(resp) = modbus_exchange(&mut sock, frame.as_slice()) {
                self.plc = Some(sock);
                return Ok(resp);
            }
        }
        let mut sock = connect_timeout(&conf.plc, to)?;
        sock.set_nodelay(true).ok();
        let resp = modbus_exchange(&mut sock, frame.as_slice())?;
        self.plc = Some(sock);
        Ok(resp)
    }
}

/// What the gateway did with one exchange, for a caller that wants to log or count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Served {
    /// Answered the agent's clock request.
    Clock { boot_id: u64 },
    /// The mesh layer refused the envelope; the gate was never reached.
    MeshDrop(&'static str),
    /// Verified, and the gate decided. `plc_written` is the ground truth that matters.
    Decided { cmd_seq: u32, decision: Decision, outcome: Outcome, plc_written: bool },
    /// Verified, and it was an authenticated read. `served` is false for a refusal and
    /// for a device that did not answer; a read never writes.
    Read { check: ReadCheck, served: bool },
    /// Verified but neither an order nor a read.
    NotAnOrder,
}

/// Serve an authenticated read: apply the register rules, ask the device, answer.
///
/// Refusals come back to the agent as an `OMR1` with the exception code the rule chose, so
/// the agent renders the gateway's decision rather than inventing one — the defect the
/// write path already had once.
fn serve_read(sock: &mut TcpStream, st: &mut GatewayState, conf: &Config, query: &MbQuery) -> io::Result<Served> {
    let (check, frame) = decide_read(query, conf.gateway_id, conf.unit, &conf.map, (query.start as u32 & 0xFFFF) as u16);
    let Some(f) = frame else {
        write_frame(sock, &encode_reply(0, &Outcome::PlcException(check.exception_code())))?;
        return Ok(Served::Read { check, served: false });
    };
    // Only here does a read reach the device, and only for a span the map allows.
    match st.plc_exchange(conf, &f) {
        Ok(resp) => match check_read_response(&f, &resp, query.count) {
            Ok((values, n)) => match encode_omv1(&values[..n as usize]) {
                Some((buf, len)) => {
                    write_frame(sock, &buf[..len])?;
                    Ok(Served::Read { check, served: true })
                }
                None => {
                    write_frame(sock, &encode_reply(0, &Outcome::NoAnswer))?;
                    Ok(Served::Read { check, served: false })
                }
            },
            // The device's own exception is forwarded as itself; a malformed answer is
            // not turned into values an operator would read as measurements.
            Err(code) => {
                let out = match code {
                    Some(c) => Outcome::PlcException(c),
                    None => Outcome::NoAnswer,
                };
                write_frame(sock, &encode_reply(0, &out))?;
                Ok(Served::Read { check, served: false })
            }
        },
        Err(_) => {
            write_frame(sock, &encode_reply(0, &Outcome::NoAnswer))?;
            Ok(Served::Read { check, served: false })
        }
    }
}

/// Serve one agent connection for as long as it sends frames.
///
/// **The frame is read before the lock is taken.** The first version of this loop locked
/// and then called a function that blocks in `read_frame`, so an idle connection held the
/// gateway's state for a whole read timeout and every other peer stalled behind it. The
/// read test found it: one connection left open by an earlier case starved the next one
/// into a 0x0B. An idle peer must not be able to stop the gateway — that is a denial of
/// service with no attacker in it, just a connection nobody closed.
///
/// Returns when the agent closes or goes quiet — the normal end of a connection, not a
/// failure.
pub fn serve_gateway_conn(sock: &mut TcpStream, st: &Mutex<GatewayState>, conf: &Config, mut on_served: impl FnMut(Served)) -> io::Result<()> {
    let to = Duration::from_millis(conf.timeout_ms);
    sock.set_read_timeout(Some(to))?;
    sock.set_write_timeout(Some(to))?;
    sock.set_nodelay(true).ok();
    loop {
        // No lock held here: this blocks until the agent speaks or the timeout expires.
        let env = read_frame(sock)?;
        let served = {
            let mut g = st.lock().map_err(|_| io::Error::other("gateway state poisoned"))?;
            handle_frame(sock, &mut g, conf, env)?
        };
        on_served(served);
    }
}

/// Read one frame and handle it. Kept for a caller that wants a single exchange; the
/// serving loop reads the frame itself so it can do that without holding the lock.
pub fn handle_gateway_conn(sock: &mut TcpStream, st: &mut GatewayState, conf: &Config) -> io::Result<Served> {
    let to = Duration::from_millis(conf.timeout_ms);
    sock.set_read_timeout(Some(to))?;
    sock.set_write_timeout(Some(to))?;
    // A third party speaking plain Modbus to this port lands here: its bytes are not a
    // length-prefixed v0B envelope, so either the prefix is refused by `read_frame` or the
    // mesh layer drops them below. Either way nothing reaches the PLC.
    let env = read_frame(sock)?;
    handle_frame(sock, st, conf, env)
}

/// Handle one frame from the agent: answer a clock request, or verify → decide → write to
/// the PLC **only on `Act`** → check the answer → reply.
///
/// Every decision is appended to the journal, refusals included: Annex III 1.1.9 asks for
/// "légitime **ou** illégitime", and until this existed only the RP2040 firmware did it.
pub fn handle_frame(sock: &mut TcpStream, st: &mut GatewayState, conf: &Config, env: Vec<u8>) -> io::Result<Served> {
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

    let inner = inner_slice(&arrived);

    // A read, if that is what arrived. It is authenticated by the envelope above and
    // bounded by the same register map as a write, but it does **not** go through the
    // actuation gate: there is nothing to actuate, so freshness, the sequence counter and
    // the value limits have nothing to judge. A latched stop therefore does not block it,
    // deliberately — a stop is when an operator most needs to see the registers.
    if let Some(query) = parse_omq1(inner) {
        return serve_read(sock, st, conf, &query);
    }

    let order = match parse_omb1(inner) {
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
            match st.plc_exchange(conf, &f) {
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
