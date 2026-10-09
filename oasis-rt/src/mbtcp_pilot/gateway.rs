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
use crate::mesh_revocation::{parse_orv1, revocation_transition, signed_message, RevDecision, RevReject, RevState};
use crate::modbus_gateway::{parse_omb1, Gateway, OrderContext, Response};
use crate::modbus_read::{check_read_response, decide_read, encode_omv1, parse_omq1, MbQuery, ReadCheck};
use crate::modbus_tcp::{check_tcp_response, decide_tcp_for_origin, Outcome, TcpFrame};

/// Where the gateway writes its journal so it survives the process.
///
/// Until this existed the TCP gateway called `Journal::append`, **threw the returned bytes
/// away**, and kept the chain in RAM — so the journal died with the process and was not
/// evidence of anything, while the documentation said every decision was appended. Annex
/// III 1.1.9 ¶5 asks a machine to *collect* evidence; a record that vanishes on exit
/// collects nothing. The RP2040 firmware persisted via `jstore` from the start; the host
/// gateway simply never did.
///
/// Two files, in the order that makes a crash detectable — **entry first, then head**,
/// exactly as `jstore` does on the MCU. A cut between the two leaves one entry the head
/// does not cover, which `verify` reports as `Unconfirmed` rather than hiding.
///
/// The format is the one `oasis_journal_verify` already reads, so nothing new has to be
/// parsed: `cat <prefix>.head <prefix>.entries` is a valid input to it.
pub struct JournalSink {
    /// Both files are held **open**. Reopening them per decision cost 12.7 ms of the
    /// 13.2 ms an authorised write took through the real binaries, measured by the
    /// end-to-end campaign — two opens and two flushes on Windows. Holding the handles
    /// keeps the durability ordering identical and pays the open once.
    entries: std::fs::File,
    head: std::fs::File,
}

impl JournalSink {
    /// `prefix.entries` and `prefix.head`. Truncates both: a fresh process starts a fresh
    /// chain under a fresh `boot_id`, and mixing two boots' entries in one file would make
    /// the sequence appear to jump.
    pub fn create(prefix: &str, boot_id: u64) -> io::Result<Self> {
        let entries = std::fs::File::create(format!("{prefix}.entries"))?;
        let head = std::fs::File::create(format!("{prefix}.head"))?;
        let mut s = JournalSink { entries, head };
        let g = crate::journal::JournalHead::new(boot_id);
        s.write_head(&g)?;
        Ok(s)
    }

    /// Rewrite the head in place: two short lines, truncated first so a shorter head
    /// cannot leave a tail of the previous one behind.
    fn write_head(&mut self, head: &crate::journal::JournalHead) -> io::Result<()> {
        use std::io::{Seek, SeekFrom, Write};
        let seq = match head.seq {
            Some(s) => alloc_fmt(s),
            None => "none".to_string(),
        };
        let text = format!(
            "JRN_BOOT boot_id={}
JRN_HEAD seq={seq} hash={} overwritten={}
",
            head.boot_id,
            hex32(&head.hash),
            head.overwritten
        );
        self.head.seek(SeekFrom::Start(0))?;
        self.head.set_len(0)?;
        self.head.write_all(text.as_bytes())?;
        self.head.flush()
    }

    /// Append one entry, then rewrite the head. Both flushed: a campaign that kills this
    /// process must still find the record on disk.
    fn persist(&mut self, head: &crate::journal::JournalHead, entry: &[u8; crate::journal::ENTRY_LEN]) -> io::Result<()> {
        use std::io::Write;
        // Entry first, flushed, then the head: a crash between the two leaves one entry
        // the head does not cover, which `verify` reports as `Unconfirmed`.
        self.entries.write_all(
            format!(
                "JRN_E {}
",
                hex32(entry)
            )
            .as_bytes(),
        )?;
        self.entries.flush()?;
        self.write_head(head)
    }
}

fn alloc_fmt(v: u32) -> String {
    format!("{v}")
}

fn hex32(b: &[u8]) -> String {
    use core::fmt::Write as _;
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        let _ = write!(s, "{x:02x}");
    }
    s
}

/// The gateway's mutable state. One per process, behind the caller's mutex.
pub struct GatewayState {
    pub gw: Gateway,
    pub router: MeshRouter,
    pub journal: Journal,
    /// Where decisions are written. `None` keeps the pre-2026-10-09 behaviour — a chain in
    /// RAM that dies with the process — which is why the binary always sets it.
    pub jsink: Option<JournalSink>,
    /// The revocation list this gateway has applied, with its epoch. Starts from whatever
    /// the config revoked and rises only by a signed `ORV1` with a strictly greater epoch.
    pub rev: RevState,
    /// Who may sign a revocation: one operator, or k of n. `None` refuses every `ORV1` —
    /// a gateway with no authority configured must not accept a list from anyone.
    pub authority: Option<oasis_operator_key::OperatorAuthority>,
    /// Where an applied list is written **before** it is applied, so a restart does not
    /// forget a revocation. `None` means it is applied in RAM only, and the binary says so.
    pub rev_path: Option<std::path::PathBuf>,
    /// The last executed `cmd_seq` **per origin**.
    ///
    /// One value for the whole gateway made the first commander's `cmd_seq=1` turn every
    /// other commander's into a replay, so two authorised senders could not both command
    /// one gateway (campaign C25). ⚠️ In RAM: a gateway restart forgets these, and an order
    /// from before the restart is then refused on its `boot_id` instead, which the gate
    /// checks first. So the sequence does not need to survive a restart — the boot id is
    /// what makes a previous run's order refusable.
    pub last_seq: Vec<([u8; 8], u32)>,
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
        Self {
            gw: Gateway::new(),
            router,
            journal: Journal::new(boot_id),
            boot_id,
            plc: None,
            jsink: None,
            rev: RevState::default(),
            last_seq: Vec::new(),
            authority: None,
            rev_path: None,
        }
    }

    /// Same, writing every decision to `prefix.entries` / `prefix.head`.
    pub fn with_journal(router: MeshRouter, boot_id: u64, prefix: &str) -> io::Result<Self> {
        let mut s = Self::new(router, boot_id);
        s.jsink = Some(JournalSink::create(prefix, boot_id)?);
        Ok(s)
    }

    /// Append a decision **and persist it**. Every journalling path in this module goes
    /// through here, so a future branch cannot record a decision in RAM only by
    /// forgetting a line — which is exactly how the original defect looked.
    ///
    /// A failed write is not silently swallowed: it returns the error to the caller, who
    /// drops the connection. A gateway that cannot write its journal must not keep
    /// deciding as if it could.
    /// Record a configuration change on the same chain, and persist it the same way.
    fn journal_and_persist_change(&mut self, authority_fp: [u8; 8], ident: u32, kind: crate::journal::ChangeKind, applied: bool) -> io::Result<()> {
        let bytes = self.journal.append_change(authority_fp, ident, kind, applied);
        if let Some(sink) = self.jsink.as_mut() {
            sink.persist(&self.journal.head, &bytes)?;
        }
        Ok(())
    }

    fn journal_and_persist(&mut self, origin_fp: [u8; 8], cmd_seq: u32, class: OrderClass, decision: LoggedDecision, flags: u8) -> io::Result<()> {
        let bytes = self.journal.append(origin_fp, cmd_seq, class, decision, flags);
        if let Some(sink) = self.jsink.as_mut() {
            sink.persist(&self.journal.head, &bytes)?;
        }
        Ok(())
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
    /// A signed revocation list arrived. `epoch` is this gateway's epoch afterwards.
    Revocation { decision: RevDecision, epoch: u64 },
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
            st.journal_and_persist([0u8; 8], 0, OrderClass::Act, LoggedDecision::Reject(Reason::NotVerified), 0)?;
            write_frame(sock, &encode_reply(0, &Outcome::NoAnswer))?;
            return Ok(Served::MeshDrop(why));
        }
        MeshDecision::Arrived { envelope, .. } => envelope,
    };

    let inner = inner_slice(&arrived);
    // Bytes 14..22 of a v0B envelope are the origin fingerprint, and it is signed — the
    // signature covers origin, counter, length and payload digest, so by the time the
    // mesh layer has accepted this frame the fingerprint is attributable.
    let mut origin_fp = [0u8; 8];
    if arrived.len() >= 22 {
        origin_fp.copy_from_slice(&arrived[14..22]);
    }

    // A read, if that is what arrived. It is authenticated by the envelope above and
    // bounded by the same register map as a write, but it does **not** go through the
    // actuation gate: there is nothing to actuate, so freshness, the sequence counter and
    // the value limits have nothing to judge. A latched stop therefore does not block it,
    // deliberately — a stop is when an operator most needs to see the registers.
    if let Some(query) = parse_omq1(inner) {
        return serve_read(sock, st, conf, &query);
    }

    // A signed revocation list. Checked before the order branch because a revocation is
    // not an order and must never be parsed as one.
    if inner.get(0..4) == Some(&b"ORV1"[..]) {
        return serve_revocation(sock, st, conf, inner, origin_fp);
    }

    let order = match parse_omb1(inner) {
        Some(o) => o,
        None => {
            write_frame(sock, &encode_reply(0, &Outcome::NoAnswer))?;
            return Ok(Served::NotAnOrder);
        }
    };

    // The gate is Part F's, unchanged — but it is only as good as what it is fed, and three
    // of these were constants until 2026-10-09 while the firmware fed the same gate real
    // values. `authorized: true` meant `gateway_id` was never compared, so an order
    // addressed to another gateway was executed by this one; `revoked: false` meant
    // revocation did nothing on this path at all.
    let ctx = OrderContext {
        // True by construction: the mesh layer verified this envelope above, and a frame
        // that failed is dropped before reaching here.
        v0b_ok: true,
        // The config's key registry is the authorisation list on this path, so holding a
        // key in it is what "authorised" means — plus the order must be addressed to THIS
        // gateway. ⚠️ Weaker than the firmware, which also requires the `ACTUATE`
        // permission from an enrolment attestation; the TCP config has keys, not
        // permissions, so a key in it can command anything in the register map.
        // Addressed to THIS gateway, **and** from an origin the config granted
        // `ACTUATE`. The second half used to be missing entirely: holding any key in the
        // registry was enough to command anything in the register map, where the firmware
        // requires `registry.allows(origin, ACTUATE)`. Same `Registry` type and same rule
        // on both paths now, so they cannot drift apart.
        authorized: order.gateway_id == conf.gateway_id && conf.perms.allows(&origin_fp, crate::enrollment::perm::ACTUATE),
        // Defence in depth, not the primary check: a revoked origin is dropped by the
        // **mesh layer** above, before the gate and before its signature is even verified
        // (v0B enforces revocation at the first hop, proved on silicon). So this can only
        // matter for a path that reaches the gate without that check — a locally injected
        // order, say. Measured in the campaign: a revoked origin produces a MeshDrop and a
        // `Reject(NotVerified)` entry, never a `Reject(Revoked)` one.
        revoked: st.router.is_revoked(&origin_fp),
        actuator_boot_id: st.boot_id,
        now_ms: now_ms(),
        // No sensor on a host: there is no entropy signal to read, so this is honest
        // rather than lenient. A gateway co-located with sensing must feed it.
        r14_safe: true,
    };
    let tid = (order.cmd_seq & 0xFFFF) as u16;
    // "Within limits" consults **this origin's** map when it has one, and the shared map
    // otherwise. The gate's nine conditions are untouched: only the register map it reads
    // narrows, so a per-origin line can tighten one sender and never widen another.
    // This origin's own last executed order, not the gateway's: see `last_seq`.
    let last = st.last_seq.iter().find(|(fp, _)| *fp == origin_fp).map(|(_, s)| *s);
    let (decision, rules, frame) = decide_tcp_for_origin(&mut st.gw, &ctx, &order, conf.unit, &origin_fp, &conf.origin_map, &conf.map, last, tid);
    if decision == Decision::Act {
        match st.last_seq.iter_mut().find(|(fp, _)| *fp == origin_fp) {
            Some(e) => e.1 = order.cmd_seq,
            None => st.last_seq.push((origin_fp, order.cmd_seq)),
        }
    }

    st.journal_and_persist(
        // The real origin, not eight zero bytes. Without it the journal records that a
        // decision happened and not who caused it, which is most of what makes a record
        // evidence of a *legitimate or illegitimate* intervention.
        //
        // Only on a **verified** decision. A dropped envelope keeps `0000` (see the
        // MeshDrop arm): its origin field is attacker-controlled until the signature has
        // been checked, and writing an unverified claim into the evidence as if it were
        // fact is worse than writing nothing.
        origin_fp,
        order.cmd_seq,
        OrderClass::Act,
        match decision {
            Decision::Act => LoggedDecision::Act,
            Decision::Reject(r) => LoggedDecision::Reject(r),
        },
        0,
    )?;

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

/// Apply a signed `ORV1` revocation list, and **record the change**.
///
/// Annex III 1.1.9 ¶5 asks for evidence of "a modification of the software installed …
/// **or its configuration**", and who may command is configuration. So the decision is
/// journalled whether the list was applied or refused — "legitimate **or illegitimate**".
///
/// The order of operations is the one the spec fixes and silicon proved: verify, **persist
/// before applying**, then apply. A crash between the write and the apply leaves a list on
/// disk that the next start applies; a crash the other way round would leave a node
/// enforcing a list it cannot prove it received.
///
/// Single **or k-of-n**, delegated to `oasis-operator-key::OperatorAuthority`, which owns
/// the rule. Until 2026-10-09 that crate was reachable only as a dev-dependency (a cycle),
/// so this function refused every multi-signed list by name. Delegating also buys the
/// property the hand-rolled single-key check never had: `verify_authorization` requires **k
/// distinct keys**, so one operator signing twice is one vote, not two.
fn serve_revocation(sock: &mut TcpStream, st: &mut GatewayState, conf: &Config, inner: &[u8], origin_fp: [u8; 8]) -> io::Result<Served> {
    let outcome = (|| -> Result<RevDecision, RevReject> {
        let authority = st.authority.as_ref().ok_or(RevReject::BadOperatorSig)?;
        let p = parse_orv1(inner)?;
        // The authority decides what a quorum is. A `Single` authority still requires
        // exactly one signature from exactly that key; a `Multisig { k, pub_keys }`
        // requires k DISTINCT keys drawn from the n.
        let sig_ok = authority.verify_authorization(&signed_message(&p), &p.sigs).is_ok();
        let (d, new) = revocation_transition(&st.rev, &conf.network_id, &p, sig_ok);
        if let (RevDecision::Applied, Some(new)) = (d, new) {
            // Persisted before applied.
            if let Some(path) = &st.rev_path {
                std::fs::write(path, inner).map_err(|_| RevReject::PersistFailed)?;
            }
            for fp in &new.revoked {
                st.router.revoke(*fp);
            }
            st.rev = new;
        }
        Ok(d)
    })();

    let (decision, applied) = match outcome {
        Ok(d) => (d, d == RevDecision::Applied),
        Err(r) => (RevDecision::Reject(r), false),
    };
    // The epoch is 32 bits in an entry; a revocation epoch rises by one per published
    // list, so its low half is the identifier an auditor compares.
    st.journal_and_persist_change(origin_fp, st.rev.epoch as u32, crate::journal::ChangeKind::Revocation, applied)?;
    write_frame(sock, &encode_reply(0, &if applied { Outcome::Done } else { Outcome::NoAnswer }))?;
    Ok(Served::Revocation { decision, epoch: st.rev.epoch })
}
