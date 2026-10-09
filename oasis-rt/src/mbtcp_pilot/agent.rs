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
use crate::modbus_read::{encode_omq1, hmi_read_response, parse_omv1, parse_tcp_read, MbQuery};
use crate::modbus_tcp::{hmi_response, parse_tcp_write, Outcome, TcpWrite, EXC_GATEWAY_PATH_UNAVAILABLE, EXC_GATEWAY_TARGET_FAILED};

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

/// How many v0B send counters one disk write buys. Larger than [`SEQ_LEASE`] because a
/// mesh counter is 64 bits and skipping some costs nothing at all.
pub const TX_LEASE: u64 = 256;

/// The v0B **send counter**, leased to a file so a restart cannot reuse one.
///
/// This exists because the end-to-end campaign restarted the agent and every write after
/// it came back as exception 0x0B. The order sequence was persisted — `SeqStore` — but the
/// *mesh* counter was not, so a restarted agent began again at 1 and the gateway's counter
/// window refused its envelopes as stale, exactly as it should. The agent was locking
/// itself out. `tx_lease` solved the same problem on the MCU and was silicon-proven across
/// a real power cut; the host agent simply never got it, and no library test restarts a
/// process so nothing caught it.
///
/// Same guarantee as the lease on the order sequence: a crash loses up to [`TX_LEASE`]
/// counters and can never **reuse** one, because the file claimed them before any was
/// sent.
pub struct TxCounterStore {
    path: String,
    high_water: u64,
}

impl TxCounterStore {
    pub fn load(path: &str) -> Result<Self, String> {
        let high_water = match std::fs::read_to_string(path) {
            Ok(t) => t.trim().parse::<u64>().map_err(|e| format!("{path}: {e}"))?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => 0,
            Err(e) => return Err(format!("{path}: {e}")),
        };
        Ok(TxCounterStore { path: path.to_string(), high_water })
    }

    pub fn claimed(&self) -> u64 {
        self.high_water
    }

    /// Start this run strictly above everything the previous run could have sent.
    pub fn restore_into(&self, origin: &mut MeshRouter) {
        origin.set_tx_counter(self.high_water);
    }

    /// Make sure the **next** counter the router will use is already claimed on disk.
    /// Called before every send; writes only when a lease runs out.
    ///
    /// Takes the current counter rather than the router so a caller holding the router
    /// mutably can still call it.
    pub fn ensure(&mut self, current: u64) -> Result<(), String> {
        let next = current.saturating_add(1);
        if next > self.high_water {
            let want = next.saturating_add(TX_LEASE - 1);
            std::fs::write(&self.path, want.to_string()).map_err(|e| format!("{}: {e}", self.path))?;
            self.high_water = want;
        }
        Ok(())
    }
}

/// The agent's shared state: its signing identity, its order sequence, and its v0B send
/// counter.
pub struct AgentState {
    pub origin: MeshRouter,
    pub seq: SeqStore,
    /// `None` keeps the counter in RAM only, which means **a restart locks the agent out
    /// of its own gateway** until the window catches up. The binary always sets it; the
    /// in-process tests leave it `None` because they never restart.
    pub txc: Option<TxCounterStore>,
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
    // Every Modbus exchange is one small request and one small reply, strictly alternating
    // — the worst case for Nagle, which holds a small write waiting for an ACK that only
    // arrives once the peer has the write. The end-to-end campaign measured **13.6 ms**
    // per write through the real binaries against ~460 us in process, and this one line is
    // most of that difference. The gateway and the PLC link already set it; the socket
    // facing the HMI did not.
    hmi.set_nodelay(true).ok();
    let mut link = GatewayLink::new();

    loop {
        let req = crate::mbtcp_net::modbus_read_request(hmi)?;
        if let Some(w) = parse_tcp_write(&req) {
            let outcome = agent_forward(&mut link, agent, conf, &w);
            let (resp, n) = hmi_response(&w, outcome);
            write_all_flush(hmi, &resp[..n])?;
            continue;
        }
        // A read goes **through** the gateway, signed like an order and bounded by the
        // same register map. It used to be refused outright, which left the HMI
        // write-only and pushed every real deployment towards a side channel to the PLC —
        // the exact thing the gateway exists to prevent.
        if let Some(q) = parse_tcp_read(&req, conf.gateway_id) {
            let tid = u16::from_be_bytes([req[0], req[1]]);
            match agent_read(&mut link, agent, conf, &q) {
                Ok(values) => match hmi_read_response(&q, tid, &values) {
                    Some((resp, n)) => write_all_flush(hmi, &resp[..n])?,
                    None => write_all_flush(hmi, &exception_for(&req, EXC_GATEWAY_TARGET_FAILED))?,
                },
                Err(code) => write_all_flush(hmi, &exception_for(&req, code))?,
            }
            continue;
        }
        // Neither a write nor a read this agent understands. Answer, never stay silent.
        let exc = exception_for(&req, EXC_GATEWAY_PATH_UNAVAILABLE);
        write_all_flush(hmi, &exc)?;
    }
}

/// One HMI read: sign the query, send it, and return the values the gateway reported.
///
/// `Err(code)` is the Modbus exception to give the HMI. There is no silent path: an
/// unreachable gateway becomes 0x0B, a refused span the code the gateway's rule chose.
pub fn agent_read(link: &mut GatewayLink, agent: &Mutex<AgentState>, conf: &Config, q: &MbQuery) -> Result<Vec<u16>, u8> {
    match agent_read_try(link, agent, conf, q) {
        Ok(v) => Ok(v),
        Err(ReadFail::Exception(c)) => Err(c),
        Err(ReadFail::Transport) => Err(EXC_GATEWAY_TARGET_FAILED),
    }
}

enum ReadFail {
    /// The gateway (or the device) refused, with this Modbus exception code.
    Exception(u8),
    /// Nothing usable came back.
    Transport,
}

fn agent_read_try(link: &mut GatewayLink, agent: &Mutex<AgentState>, conf: &Config, q: &MbQuery) -> Result<Vec<u16>, ReadFail> {
    // A read carries no `cmd_seq`, no `boot_id` and no deadline: it authorises nothing, so
    // there is nothing for a gate to judge. Its authenticity and its freshness come from
    // the v0B envelope — whose counter window and Bloom filter are what refuse a replayed
    // query — and not from fields this payload would otherwise have to carry.
    let env = {
        let mut st = agent.lock().map_err(|_| ReadFail::Transport)?;
        let tx_now = st.origin.tx_counter();
        if let Some(t) = st.txc.as_mut() {
            t.ensure(tx_now).map_err(|_| ReadFail::Transport)?;
        }
        let payload = encode_omq1(q);
        st.origin.origin_wrap_v0b(&payload).ok_or(ReadFail::Transport)?
    };

    let reply = {
        let sock = link.connect(conf).map_err(|_| ReadFail::Transport)?;
        let r = (|| -> io::Result<Vec<u8>> {
            write_frame(sock, &env)?;
            read_frame(sock)
        })();
        match r {
            Ok(r) => r,
            Err(_) => {
                link.reset();
                return Err(ReadFail::Transport);
            }
        }
    };

    if let Some((values, n)) = parse_omv1(&reply) {
        return Ok(values[..n as usize].to_vec());
    }
    // Not values: the gateway refused, and its `OMR1` carries the code it chose. Rendering
    // that code rather than one computed here is the lesson the write path already paid
    // for — an unlisted register must reach the HMI as 0x02, not as 0x0A.
    match decode_reply(&reply) {
        Some((_, Outcome::PlcException(c))) => Err(ReadFail::Exception(c)),
        Some((_, Outcome::Refused(r, rules))) => Err(ReadFail::Exception(crate::modbus_tcp::refusal_code(r, rules))),
        _ => {
            link.reset();
            Err(ReadFail::Transport)
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

    // 2 and 3 in ONE critical section: sign **and** send.
    //
    // This lock used to be released after signing, so the round trip did not hold it.
    // That was faster and wrong. Every HMI connection shares this agent's identity, so
    // they share one `cmd_seq` space, and the gateway requires strictly increasing **per
    // origin**. With the round trip outside the lock, two HMIs could reserve 5 and 6 and
    // have 6 arrive first, after which 5 is `Reject(StaleOrReplayed)` — a legitimate
    // write refused, and the HMI shown 0x0A, which an operator reads as "not authorised".
    // `bench_mbtcp_concurrency` measured **16 such refusals against 1784 acts** at N=5
    // and N=10 (2026-10-09), read from the gateway's own decision record rather than
    // inferred from the exception code.
    //
    // The cost is throughput: 3065 → ~1500 ack/s at N=10 on loopback. Paid willingly,
    // because the gateway serialises behind its own state lock anyway — so most of that
    // 3065 was writes queueing with a ~0.9 % chance of being refused for it. An HMI
    // issues a handful of writes per second.
    //
    // Real parallelism comes from **distinct origins** — one enrolled identity per HMI —
    // which the per-origin register map and per-origin sequence (A.1) make possible. Not
    // from racing one identity's counter.
    //
    // An idle HMI still holds nothing: it blocks in `modbus_read_request` before ever
    // reaching here, so `an_idle_connection_does_not_block_another` is unaffected.
    let (seq, reply) = {
        let mut st = agent.lock().map_err(|_| "agent state poisoned".to_string())?;
        // The v0B counter this envelope will use must be claimed on disk first, or a
        // restart would reuse it and the gateway would refuse everything that follows.
        let tx_now = st.origin.tx_counter();
        if let Some(t) = st.txc.as_mut() {
            t.ensure(tx_now)?;
        }
        let seq = st.seq.reserve()?;
        // `gateway_id` is the logical actuator, not the Modbus unit: the unit is inside
        // the order and is checked by the register rules.
        let order = w.to_order(conf.gateway_id, seq, boot_id, deadline_ms);
        let (buf, n) = encode_omb1(&order).ok_or("order: unsupported write")?;
        let env = st.origin.origin_wrap_v0b(&buf[..n]).ok_or("sign failed")?;

        // Send on the kept connection and wait for the outcome, still holding the lock so
        // that orders leave and are answered in the order their numbers were handed out.
        let sock = link.connect(conf)?;
        let r = (|| -> io::Result<Vec<u8>> {
            write_frame(sock, &env)?;
            read_frame(sock)
        })();
        match r {
            Ok(r) => (seq, r),
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
