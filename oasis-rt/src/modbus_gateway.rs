//! OASIS — Modbus RTU gateway for brownfield devices (spec
//! `docs/specs/MODBUS_GATEWAY_SPEC.md`).
//!
//! A gateway node sits alone on the bus of an unauthenticated Modbus RTU device and
//! writes to it only what the Part F actuation gate decides to execute. An order
//! (`OMB1`, a v0B payload) names registers and values; "within limits" means the
//! configured unit, registers present in the gateway's register map and values inside
//! their ranges. [`gateway_decision`] is pure: the RTU frame is built only in the
//! `Act` branch, from the decided order, so no frame exists without an `Act`
//! (Kani: `kani_proofs.rs`).
//!
//! Formats:
//! - order `"OMB1" | gateway_id u16 | cmd_seq u32 | boot_id u64 | deadline_ms u64 |
//!   unit u8 | fc u8 | start u16 | count u8 | values [u16; count]` (OASIS fields
//!   little-endian, 33..=47 bytes). Only FC06 (count 1) and FC16 (count 1..=8).
//! - RTU request (Modbus, big-endian, CRC-16/MODBUS low byte first):
//!   FC06 `unit 06 addr value crc` (8 B); FC16 `unit 10 start qty 2*qty values crc`.

use crate::actuation::{actuation_decision_ctx, Actuator, Decision, GateContext, GateInput};
use crc::{Crc, CRC_16_MODBUS};

pub const OMB1_MAGIC: [u8; 4] = *b"OMB1";
pub const OMB1_HEADER_LEN: usize = 31;
pub const MAX_REGS: usize = 8;
pub const OMB1_MAX_LEN: usize = OMB1_HEADER_LEN + 2 * MAX_REGS;
pub const FC_WRITE_SINGLE: u8 = 0x06;
pub const FC_WRITE_MULTIPLE: u8 = 0x10;
/// Largest request: FC16 with `MAX_REGS` registers.
pub const MAX_FRAME: usize = 9 + 2 * MAX_REGS;

const MODBUS_CRC: Crc<u16> = Crc::<u16>::new(&CRC_16_MODBUS);

pub fn crc16(b: &[u8]) -> u16 {
    MODBUS_CRC.checksum(b)
}

/// A decoded write order. `values[count..]` are zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MbOrder {
    pub gateway_id: u16,
    pub cmd_seq: u32,
    pub boot_id: u64,
    pub deadline_ms: u64,
    pub unit: u8,
    pub fc: u8,
    pub start: u16,
    pub count: u8,
    pub values: [u16; MAX_REGS],
}

fn count_ok(fc: u8, count: u8) -> bool {
    match fc {
        FC_WRITE_SINGLE => count == 1,
        FC_WRITE_MULTIPLE => count >= 1 && count as usize <= MAX_REGS,
        _ => false,
    }
}

/// Encode an order; returns the buffer and its length. `None` if fc/count invalid.
pub fn encode_omb1(o: &MbOrder) -> Option<([u8; OMB1_MAX_LEN], usize)> {
    if !count_ok(o.fc, o.count) {
        return None;
    }
    let mut b = [0u8; OMB1_MAX_LEN];
    b[0..4].copy_from_slice(&OMB1_MAGIC);
    b[4..6].copy_from_slice(&o.gateway_id.to_le_bytes());
    b[6..10].copy_from_slice(&o.cmd_seq.to_le_bytes());
    b[10..18].copy_from_slice(&o.boot_id.to_le_bytes());
    b[18..26].copy_from_slice(&o.deadline_ms.to_le_bytes());
    b[26] = o.unit;
    b[27] = o.fc;
    b[28..30].copy_from_slice(&o.start.to_le_bytes());
    b[30] = o.count;
    for i in 0..o.count as usize {
        b[OMB1_HEADER_LEN + 2 * i..OMB1_HEADER_LEN + 2 * i + 2].copy_from_slice(&o.values[i].to_le_bytes());
    }
    Some((b, OMB1_HEADER_LEN + 2 * o.count as usize))
}

fn u16_le(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn u64_le(b: &[u8], i: usize) -> u64 {
    let mut x = [0u8; 8];
    x.copy_from_slice(&b[i..i + 8]);
    u64::from_le_bytes(x)
}

/// Parse an order. Exact length only; FC06/FC16 only; bounded count.
pub fn parse_omb1(b: &[u8]) -> Option<MbOrder> {
    if b.len() < OMB1_HEADER_LEN || b[0..4] != OMB1_MAGIC {
        return None;
    }
    let (fc, count) = (b[27], b[30]);
    if !count_ok(fc, count) || b.len() != OMB1_HEADER_LEN + 2 * count as usize {
        return None;
    }
    let mut values = [0u16; MAX_REGS];
    for (i, v) in values.iter_mut().enumerate().take(count as usize) {
        *v = u16_le(b, OMB1_HEADER_LEN + 2 * i);
    }
    Some(MbOrder {
        gateway_id: u16_le(b, 4),
        cmd_seq: u32_le(b, 6),
        boot_id: u64_le(b, 10),
        deadline_ms: u64_le(b, 18),
        unit: b[26],
        fc,
        start: u16_le(b, 28),
        count,
        values,
    })
}

/// One writable register of the device and its allowed range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegRule {
    pub addr: u16,
    pub min: u16,
    pub max: u16,
}

/// Why an order is or is not within the gateway's register rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleCheck {
    Ok,
    WrongUnit,
    RegisterNotAllowed(u16),
    ValueOutOfRange(u16, u16),
}

/// "Within limits" for a Modbus order: configured unit, every written register in
/// the map (no wrap past 0xFFFF), every value in its register's range.
pub fn check_rules(o: &MbOrder, unit: u8, map: &[RegRule]) -> RuleCheck {
    if o.unit != unit {
        return RuleCheck::WrongUnit;
    }
    for i in 0..o.count as usize {
        let addr = match o.start.checked_add(i as u16) {
            Some(a) => a,
            None => return RuleCheck::RegisterNotAllowed(0xFFFF),
        };
        match map.iter().find(|r| r.addr == addr) {
            None => return RuleCheck::RegisterNotAllowed(addr),
            Some(r) if o.values[i] < r.min || o.values[i] > r.max => return RuleCheck::ValueOutOfRange(addr, o.values[i]),
            Some(_) => {}
        }
    }
    RuleCheck::Ok
}

/// One register rule that applies to **one origin only**.
///
/// A flat table of these, rather than a map of maps: this module is `no_std` and allocates
/// nothing, and a flat slice is also what a Kani harness can quantify over without an
/// unwind bound on a nested structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OriginRule {
    /// The v0B origin fingerprint this rule belongs to.
    pub origin: [u8; 8],
    pub rule: RegRule,
}

/// Does `origin` have a register map of its own?
///
/// This is what decides whether the shared map still applies to it. An origin named
/// anywhere in the table is governed **only** by its own entries, so adding a rule for an
/// origin can never widen what that origin may write.
pub fn origin_has_own_map(origin: &[u8; 8], per_origin: &[OriginRule]) -> bool {
    let mut i = 0usize;
    while i < per_origin.len() {
        if &per_origin[i].origin == origin {
            return true;
        }
        i += 1;
    }
    false
}

/// "Within limits" for an order from a **named origin**.
///
/// Identical to [`check_rules`] for an origin with no map of its own — the same function is
/// called, not a copy of it — and strictly narrower for one that has. The gate's nine
/// conditions are untouched: this only decides which register map "within limits" consults,
/// so configuring a per-origin map can tighten what an origin may write and never widen it.
///
/// Why per-origin at all: with one shared map, any key the config authorises can write
/// every register in it. A pilot wants "this HMI may move axis 1 between 0 and 100, that
/// one may only reset the counter" — which is an authorisation question, not a range check.
pub fn check_rules_for_origin(o: &MbOrder, unit: u8, origin: &[u8; 8], per_origin: &[OriginRule], shared: &[RegRule]) -> RuleCheck {
    if !origin_has_own_map(origin, per_origin) {
        return check_rules(o, unit, shared);
    }
    if o.unit != unit {
        return RuleCheck::WrongUnit;
    }
    let mut i = 0usize;
    while i < o.count as usize {
        let addr = match o.start.checked_add(i as u16) {
            Some(a) => a,
            // A span that wraps past 0xFFFF is not a span, as in `check_rules`.
            None => return RuleCheck::RegisterNotAllowed(0xFFFF),
        };
        let mut found: Option<RegRule> = None;
        let mut j = 0usize;
        while j < per_origin.len() {
            let e = per_origin[j];
            if &e.origin == origin && e.rule.addr == addr {
                found = Some(e.rule);
                break;
            }
            j += 1;
        }
        match found {
            None => return RuleCheck::RegisterNotAllowed(addr),
            Some(r) if o.values[i] < r.min || o.values[i] > r.max => return RuleCheck::ValueOutOfRange(addr, o.values[i]),
            Some(_) => {}
        }
        i += 1;
    }
    RuleCheck::Ok
}

/// An RTU request frame, built only for a decided order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub bytes: [u8; MAX_FRAME],
    pub len: usize,
}

impl Frame {
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

fn encode_request(o: &MbOrder) -> Frame {
    let mut b = [0u8; MAX_FRAME];
    b[0] = o.unit;
    b[1] = o.fc;
    b[2..4].copy_from_slice(&o.start.to_be_bytes());
    let mut n = 4;
    if o.fc == FC_WRITE_SINGLE {
        b[4..6].copy_from_slice(&o.values[0].to_be_bytes());
        n = 6;
    } else {
        b[4..6].copy_from_slice(&(o.count as u16).to_be_bytes());
        b[6] = 2 * o.count;
        n += 3;
        for i in 0..o.count as usize {
            b[n..n + 2].copy_from_slice(&o.values[i].to_be_bytes());
            n += 2;
        }
    }
    let crc = crc16(&b[..n]);
    b[n..n + 2].copy_from_slice(&crc.to_le_bytes());
    Frame { bytes: b, len: n + 2 }
}

/// Everything the decision depends on besides the order, already evaluated by the
/// caller (same meaning as in `actuation::GateInput`).
#[derive(Debug, Clone, Copy)]
pub struct OrderContext {
    pub v0b_ok: bool,
    /// Enrolled with `ACTUATE`, and the order targets this gateway.
    pub authorized: bool,
    pub revoked: bool,
    pub actuator_boot_id: u64,
    pub now_ms: u64,
    pub r14_safe: bool,
}

/// The gate input for an order: the Part F conditions, with "within limits" = the
/// register rules.
pub fn gate_input(ctx: &OrderContext, o: &MbOrder, rules: RuleCheck, last_executed_seq: Option<u32>) -> GateInput {
    GateInput {
        v0b_ok: ctx.v0b_ok,
        authorized: ctx.authorized,
        revoked: ctx.revoked,
        cmd_boot_id: o.boot_id,
        deadline_ms: o.deadline_ms,
        actuator_boot_id: ctx.actuator_boot_id,
        now_ms: ctx.now_ms,
        r14_safe: ctx.r14_safe,
        within_limits: rules == RuleCheck::Ok,
        cmd_seq: o.cmd_seq,
        last_executed_seq,
    }
}

/// The gateway rule. Pure. The frame is `Some` only when the decision is `Act`.
pub fn gateway_decision(ctx: &OrderContext, o: &MbOrder, unit: u8, map: &[RegRule], last_executed_seq: Option<u32>) -> (Decision, RuleCheck, Option<Frame>) {
    gateway_decision_ctx(&GateContext::default(), ctx, o, unit, map, last_executed_seq)
}

/// The same rule with the actuator state of parts G and H: a latched stop or a dead
/// supervision link must refuse the order and, above all, **produce no frame**.
///
/// This exists because the silicon run of 2026-10-08 caught the gateway writing to the
/// device while the stop latch was set: setting `gw.act.stopped` was not enough, because
/// the pure rule never read it.
pub fn gateway_decision_ctx(gctx: &GateContext, ctx: &OrderContext, o: &MbOrder, unit: u8, map: &[RegRule], last_executed_seq: Option<u32>) -> (Decision, RuleCheck, Option<Frame>) {
    gateway_decision_with_rules(gctx, ctx, o, check_rules(o, unit, map), last_executed_seq)
}

/// Same decision, for a caller that has already computed "within limits".
///
/// Extracted so the shared map and a per-origin map share **one** site that builds a
/// frame, which is the property rule 2 of the pilot prompt asks for and what
/// `proof_mb_no_frame_without_act` proves. The gate still receives "within limits" as a
/// verdict, exactly as it receives `v0b_ok`: which register map produced it is this
/// module's business and none of the gate's.
pub fn gateway_decision_with_rules(gctx: &GateContext, ctx: &OrderContext, o: &MbOrder, rules: RuleCheck, last_executed_seq: Option<u32>) -> (Decision, RuleCheck, Option<Frame>) {
    let d = actuation_decision_ctx(gctx, &gate_input(ctx, o, rules, last_executed_seq));
    let frame = match d {
        Decision::Act => Some(encode_request(o)),
        Decision::Reject(_) => None,
    };
    (d, rules, frame)
}

/// [`gateway_decision_ctx`] with a **per-origin** register map.
///
/// Identical for an origin with no map of its own — [`check_rules_for_origin`] calls
/// [`check_rules`] on the shared map in that case — and strictly narrower for one that has.
pub fn gateway_decision_for_origin(
    gctx: &GateContext,
    ctx: &OrderContext,
    o: &MbOrder,
    unit: u8,
    origin: &[u8; 8],
    per_origin: &[OriginRule],
    shared: &[RegRule],
    last_executed_seq: Option<u32>,
) -> (Decision, RuleCheck, Option<Frame>) {
    let rules = check_rules_for_origin(o, unit, origin, per_origin, shared);
    gateway_decision_with_rules(gctx, ctx, o, rules, last_executed_seq)
}

/// Stateful wrapper: the last executed `cmd_seq` and counters live in an
/// `Actuator` of their own (not the LED's).
#[derive(Debug, Clone, Default)]
pub struct Gateway {
    pub act: Actuator,
}

impl Gateway {
    pub fn new() -> Self {
        Self::default()
    }
    /// Decide on an order; send the returned frame (if any) exactly once.
    pub fn decide(&mut self, ctx: &OrderContext, o: &MbOrder, unit: u8, map: &[RegRule]) -> (Decision, RuleCheck, Option<Frame>) {
        let gctx = self.act.context(ctx.now_ms);
        let r = gateway_decision_ctx(&gctx, ctx, o, unit, map, self.act.last_executed_seq);
        self.record(o, r.0);
        r
    }

    /// Same, consulting the named origin's register map **and** that origin's own last
    /// executed sequence.
    ///
    /// `last_executed_seq` is passed in rather than read from `self.act`, because a single
    /// value for the whole gateway makes the first commander's `cmd_seq=1` turn every other
    /// commander's `cmd_seq=1` into a replay. Campaign C25 demonstrated it: a legitimate
    /// order from a second authorised sender was journalled `Reject(StaleOrReplayed)`.
    ///
    /// The nine conditions are unchanged and so is "strictly newer than the last executed";
    /// only *what it is newer than* changes. A replay of an origin's order is still caught
    /// by that origin's counter, and one sender can no longer consume numbers that block
    /// another.
    pub fn decide_for_origin(
        &mut self,
        ctx: &OrderContext,
        o: &MbOrder,
        unit: u8,
        origin: &[u8; 8],
        per_origin: &[OriginRule],
        shared: &[RegRule],
        last_executed_seq: Option<u32>,
    ) -> (Decision, RuleCheck, Option<Frame>) {
        let gctx = self.act.context(ctx.now_ms);
        let r = gateway_decision_for_origin(&gctx, ctx, o, unit, origin, per_origin, shared, last_executed_seq);
        self.record(o, r.0);
        r
    }

    /// The bookkeeping both paths share: an executed order advances `last_executed_seq`,
    /// a refused one increments its reason's counter. Shared rather than copied, because
    /// a second copy is how the sequence counter stops being monotone on one path only.
    fn record(&mut self, o: &MbOrder, d: Decision) {
        match d {
            Decision::Act => {
                self.act.last_executed_seq = Some(o.cmd_seq);
                self.act.executed += 1;
            }
            Decision::Reject(reason) => self.act.rejects[reason as usize] += 1,
        }
    }
}

/// What the device answered to a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    /// FC06: exact echo; FC16: unit, fc, start and quantity echoed.
    Ack,
    Exception(u8),
    BadCrc,
    /// Well-formed but not the answer to this request.
    Mismatch,
    Short,
}

fn crc_ok(b: &[u8]) -> bool {
    b.len() >= 3 && crc16(&b[..b.len() - 2]).to_le_bytes() == [b[b.len() - 2], b[b.len() - 1]]
}

/// Check the device's answer to `req`. Total: any `resp` is accepted as input.
pub fn check_response(req: &Frame, resp: &[u8]) -> Response {
    if req.len < 8 || req.len > MAX_FRAME {
        return Response::Mismatch;
    }
    let req = &req.bytes[..req.len];
    if resp.len() < 5 {
        return Response::Short;
    }
    if !crc_ok(resp) {
        return Response::BadCrc;
    }
    if resp.len() == 5 && resp[0] == req[0] && resp[1] == (req[1] | 0x80) {
        return Response::Exception(resp[2]);
    }
    if resp.len() == 8 && resp[..6] == req[..6] {
        return Response::Ack;
    }
    Response::Mismatch
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
