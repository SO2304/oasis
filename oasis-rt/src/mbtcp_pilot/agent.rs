//! The operator-side half: take a plain Modbus write from the HMI, sign it as an order
//! against the gateway's clock, and report what the gate decided.

use std::io;
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::Duration;

use super::{decode_reply, CLOCK_REPLY_LEN, CLOCK_REQ};
use crate::actuation::{TimeView, MAX_VALIDITY_MS};
use crate::mbtcp_conf::Config;
use crate::mbtcp_net::{connect_timeout, read_frame, write_frame};
use crate::mesh::MeshRouter;
use crate::modbus_gateway::encode_omb1;
use crate::modbus_tcp::{hmi_response, parse_tcp_write, Outcome, TcpWrite, EXC_GATEWAY_PATH_UNAVAILABLE};

/// How long the agent reuses its view of the gateway's clock before asking again.
///
/// Not [`crate::actuation::MAX_VIEW_AGE_MS`], which is an hour and sized for a
/// radio slot. Here the clock costs one TCP round trip, and the risk a cached view carries
/// is a gateway **restart**: its `boot_id` changes, and orders stamped for the dead boot
/// are refused as `Expired` until the view refreshes. One second bounds that window to one
/// second of refused writes, which the HMI sees as exceptions and the operator can retry.
/// Refusing is the correct failure here — part K's property, proven by Kani and on silicon,
/// is that every error in a commander's clock view ends in a refusal and never in an
/// unintended execution.
pub const CLOCK_REFRESH_MS: u64 = 1_000;

/// How many sequence numbers one disk write buys.
///
/// The file holds a **high-water mark**: "every number up to this may already have been
/// used". `reserve` hands out numbers below it from memory and only writes when the lease
/// runs out, so the cost per order is one write in [`SEQ_LEASE`] instead of one per order.
///
/// This is `tx_lease` on the MCU, for the same reason and with the same guarantee: a crash
/// can lose up to [`SEQ_LEASE`] numbers and can never **reuse** one, because the file
/// already claimed them. The gate wants strictly newer, not consecutive, so a gap costs
/// nothing; a repeat is a replay it would have to accept.
///
/// 64 was chosen by measurement, not taste: `bench_mbtcp_pilot` could not account for
/// 628 µs of an authorised write's 887 µs, and one `fs::write` of this file measured
/// **258 µs median** on this host — as much as the whole sign + verify + gate. At 64 the
/// amortised cost is ~4 µs, and the worst case is a restart skipping 63 numbers out of
/// 2^32.
pub const SEQ_LEASE: u32 = 64;

/// A strictly increasing command sequence, leased from a file.
///
/// A number is never handed out before the file claims it, so a crash between reserving
/// and sending can only lose a sequence, never reuse one. A reused `cmd_seq` is a replay
/// the gateway would have to accept.
pub struct SeqStore {
    path: String,
    /// The next number to hand out.
    next: u32,
    /// The largest number the file has claimed. Numbers up to here need no write.
    high_water: u32,
}

impl SeqStore {
    /// A missing file is a first run and starts at 1. A corrupt one is an error, not a
    /// guess: guessing a sequence is exactly how a replay window reopens.
    ///
    /// The on-disk format did not change when leasing arrived — it was "the last number
    /// used" and is now "the last number claimed", and both are read as `next = file + 1`,
    /// so an existing file is still safe to resume from. Only the meaning widened, in the
    /// direction that cannot cause a reuse.
    pub fn load(path: &str) -> Result<Self, String> {
        let claimed = match std::fs::read_to_string(path) {
            Ok(t) => t.trim().parse::<u32>().map_err(|e| format!("{path}: {e}"))?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => 0,
            Err(e) => return Err(format!("{path}: {e}")),
        };
        Ok(SeqStore { path: path.to_string(), next: claimed.saturating_add(1), high_water: claimed })
    }

    pub fn peek(&self) -> u32 {
        self.next
    }

    /// The number the file has claimed, for a test or a diagnostic that wants to check
    /// that nothing is handed out beyond it.
    pub fn claimed(&self) -> u32 {
        self.high_water
    }

    pub fn reserve(&mut self) -> Result<u32, String> {
        if self.next > self.high_water {
            let want = self.next.saturating_add(SEQ_LEASE - 1);
            std::fs::write(&self.path, want.to_string()).map_err(|e| format!("{}: {e}", self.path))?;
            self.high_water = want;
        }
        let n = self.next;
        self.next = n.saturating_add(1);
        Ok(n)
    }
}

/// The agent's shared state: its signing identity and its sequence.
pub struct AgentState {
    pub origin: MeshRouter,
    pub seq: SeqStore,
}

/// One HMI connection's link to the gateway: the socket, and the view of its clock.
///
/// Both used to be rebuilt per write — a connect for the clock, a connect for the order.
/// `bench_mbtcp_pilot` measured what that cost: 3.1 ms per authorised write, of which
/// 264 µs was sign + verify + gate. Worse than the median, the spread reached ±724 %,
/// because a connection per write also means thousands of ephemeral ports per minute. An
/// unpredictable write latency is harder to live with than a slow one.
///
/// Not shared between HMI connections: a `TcpStream` has one read cursor, so two threads
/// interleaving frames on it would read each other's replies.
pub struct GatewayLink {
    sock: Option<TcpStream>,
    view: Option<TimeView>,
    /// The agent's own monotonic-ish clock, in the same base as `view.local_rx_ms`.
    last_refresh_ms: u64,
}

impl GatewayLink {
    pub fn new() -> Self {
        GatewayLink { sock: None, view: None, last_refresh_ms: 0 }
    }

    fn connect(&mut self, conf: &Config) -> Result<&mut TcpStream, String> {
        if self.sock.is_none() {
            let to = Duration::from_millis(conf.timeout_ms);
            let s = connect_timeout(&conf.plc, to).map_err(|e| format!("gw connect: {:?}", e.kind()))?;
            s.set_nodelay(true).ok();
            s.set_read_timeout(Some(to)).ok();
            s.set_write_timeout(Some(to)).ok();
            self.sock = Some(s);
        }
        Ok(self.sock.as_mut().expect("just set"))
    }

    /// Drop the socket so the next call redials. Called on any I/O error, because a
    /// half-closed connection would otherwise fail every write from here on.
    fn reset(&mut self) {
        self.sock = None;
    }

    /// `(boot_id, deadline_ms)` in the gateway's time base, refreshing the view when it is
    /// older than [`CLOCK_REFRESH_MS`].
    fn stamp(&mut self, conf: &Config, local_now: u64) -> Result<(u64, u64), String> {
        let stale = match self.view {
            None => true,
            Some(_) => local_now.saturating_sub(self.last_refresh_ms) >= CLOCK_REFRESH_MS,
        };
        if stale {
            let (boot_id, gw_now) = self.ask_clock(conf)?;
            match self.view.as_mut() {
                // `apply` refuses a beacon whose clock does not advance within a boot, and
                // replaces the view outright across a boot — a restart is exactly what the
                // agent needs to learn. A refused beacon leaves the old view in place,
                // which then expires on its own rather than being trusted silently.
                Some(v) => {
                    v.apply(boot_id, gw_now, local_now);
                }
                None => self.view = Some(TimeView::new(boot_id, gw_now, local_now)),
            }
            self.last_refresh_ms = local_now;
        }
        let v = self.view.as_ref().ok_or("clock: no view")?;
        // Half the gate's window: enough slack for the round trip, far from the bound.
        v.stamp(local_now, MAX_VALIDITY_MS / 2).ok_or_else(|| "clock: view cannot be extrapolated".to_string())
    }

    fn ask_clock(&mut self, conf: &Config) -> Result<(u64, u64), String> {
        let sock = self.connect(conf)?;
        let r = (|| -> io::Result<Vec<u8>> {
            write_frame(sock, &CLOCK_REQ)?;
            read_frame(sock)
        })();
        let r = match r {
            Ok(r) => r,
            Err(e) => {
                self.reset();
                return Err(format!("clock: {:?}", e.kind()));
            }
        };
        if r.len() != CLOCK_REPLY_LEN || r[0..4] != *b"OTM1" {
            self.reset();
            return Err("clock: bad answer".into());
        }
        Ok((u64::from_le_bytes(r[4..12].try_into().unwrap()), u64::from_le_bytes(r[12..20].try_into().unwrap())))
    }
}

impl Default for GatewayLink {
    fn default() -> Self {
        Self::new()
    }
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
    let mut link = GatewayLink::new();

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
                let outcome = agent_forward(&mut link, agent, conf, &w);
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

/// The agent side of one HMI write: stamp it against the gateway's clock, sign it, send
/// it, and return the outcome.
///
/// Never returns silence: every failure becomes `Outcome::NoAnswer`, which `hmi_response`
/// turns into exception 0x0B. An HMI left waiting is worse than one told no.
pub fn agent_forward(link: &mut GatewayLink, agent: &Mutex<AgentState>, conf: &Config, w: &TcpWrite) -> Outcome {
    match agent_try(link, agent, conf, w) {
        Ok(o) => o,
        Err(_) => Outcome::NoAnswer,
    }
}

/// Same, with the reason preserved — the binaries log it, the tests assert on the outcome.
pub fn agent_try(link: &mut GatewayLink, agent: &Mutex<AgentState>, conf: &Config, w: &TcpWrite) -> Result<Outcome, String> {
    // 1. The gateway's clock, cached. No order is stamped against ours: freshness is
    // judged in the gateway's `boot_id` and `now_ms`, so those are what the deadline is
    // built from, extrapolated by part K's `TimeView` between refreshes.
    let local_now = super::now_ms();
    let (boot_id, deadline_ms) = link.stamp(conf, local_now)?;

    // 2. Sign under the lock, and only that: the round trip below must not hold it.
    let (env, seq) = {
        let mut st = agent.lock().map_err(|_| "agent state poisoned".to_string())?;
        let seq = st.seq.reserve()?;
        // `gateway_id` is the logical actuator, not the Modbus unit: the unit is inside
        // the order and is checked by the register rules.
        let order = w.to_order(conf.gateway_id, seq, boot_id, deadline_ms);
        let (buf, n) = encode_omb1(&order).ok_or("order: unsupported write")?;
        let env = st.origin.origin_wrap_v0b(&buf[..n]).ok_or("sign failed")?;
        (env, seq)
    };

    // 3. Send on the kept connection, and wait for the outcome.
    let reply = {
        let sock = link.connect(conf)?;
        let r = (|| -> io::Result<Vec<u8>> {
            write_frame(sock, &env)?;
            read_frame(sock)
        })();
        match r {
            Ok(r) => r,
            Err(e) => {
                link.reset();
                return Err(format!("gw: {:?}", e.kind()));
            }
        }
    };
    let (got, outcome) = decode_reply(&reply).ok_or("gw: bad reply")?;
    if got != seq && got != 0 {
        // The reply does not match the order: the connection is out of step, so it is
        // dropped rather than reused for a write whose answer would be the previous one's.
        link.reset();
        return Err(format!("gw: reply for {got}, expected {seq}"));
    }
    Ok(outcome)
}
