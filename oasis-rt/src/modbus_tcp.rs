//! OASIS — Modbus TCP transport for the gateway (spec `docs/specs/MODBUS_TCP_SPEC.md`).
//!
//! The gate is the Modbus RTU gateway's, unchanged ([`gateway_decision`]): only the
//! transport differs. A TCP request (MBAP header + PDU) is derived from the RTU frame
//! of an `Act` decision, so no TCP frame exists without an `Act` either (Kani:
//! `kani_proofs.rs`).
//!
//! Two sides, both pure:
//! - **operator agent** (next to the HMI/SCADA): [`parse_tcp_write`] reads the HMI's
//!   plain Modbus TCP write, [`TcpWrite::to_order`] turns it into an `OMB1` order the
//!   agent signs (v0B), and [`hmi_response`] answers the HMI once the gateway decided;
//! - **gateway** (next to the PLC): [`gateway_decision_tcp`] / [`decide_tcp`] build the
//!   request for the PLC, [`check_tcp_response`] checks its answer.
//!
//! Formats (Modbus TCP, big-endian): `tid u16 | protocol 0 u16 | len u16 | unit u8 | PDU`,
//! `len` = 1 + PDU length. Only FC06 and FC16 (1..=8 registers), as for RTU. Reads are
//! out of scope here: they do not change the machine's state.

use crate::actuation::{Decision, Reason};
use crate::modbus_gateway::{gateway_decision, Frame, Gateway, MbOrder, RegRule, Response, RuleCheck, FC_WRITE_MULTIPLE, FC_WRITE_SINGLE, MAX_REGS};

pub const MBAP_LEN: usize = 7;
/// Largest request: FC16 with `MAX_REGS` registers (MBAP 7 + fc 1 + start 2 + qty 2 + bc 1 + values).
pub const MAX_TCP_FRAME: usize = MBAP_LEN + 6 + 2 * MAX_REGS;
/// Write echo (FC06 or FC16) and exception lengths.
pub const ACK_LEN: usize = MBAP_LEN + 5;
pub const EXC_LEN: usize = MBAP_LEN + 2;

/// Exception codes the agent returns to the HMI when the gateway refuses (Modbus
/// application protocol v1.1b3 §7). Policy refusals have no dedicated code.
pub const EXC_ILLEGAL_ADDRESS: u8 = 0x02;
pub const EXC_ILLEGAL_VALUE: u8 = 0x03;
pub const EXC_GATEWAY_PATH_UNAVAILABLE: u8 = 0x0A;
pub const EXC_GATEWAY_TARGET_FAILED: u8 = 0x0B;

/// A Modbus TCP request frame, built only for a decided order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpFrame {
    pub bytes: [u8; MAX_TCP_FRAME],
    pub len: usize,
}

impl TcpFrame {
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

fn u16_be(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([b[i], b[i + 1]])
}

/// MBAP header for `pdu_len` bytes of PDU.
fn mbap(b: &mut [u8], tid: u16, unit: u8, pdu_len: usize) {
    b[0..2].copy_from_slice(&tid.to_be_bytes());
    b[2..4].copy_from_slice(&0u16.to_be_bytes());
    b[4..6].copy_from_slice(&((1 + pdu_len) as u16).to_be_bytes());
    b[6] = unit;
}

/// The TCP form of an RTU request: same unit and PDU, MBAP instead of CRC.
fn from_rtu(rtu: &Frame, tid: u16) -> TcpFrame {
    let pdu = &rtu.bytes[1..rtu.len - 2];
    let mut b = [0u8; MAX_TCP_FRAME];
    mbap(&mut b, tid, rtu.bytes[0], pdu.len());
    b[MBAP_LEN..MBAP_LEN + pdu.len()].copy_from_slice(pdu);
    TcpFrame { bytes: b, len: MBAP_LEN + pdu.len() }
}

/// The gateway rule over TCP. Pure. The frame is `Some` only when the decision is `Act`.
pub fn gateway_decision_tcp(ctx: &crate::modbus_gateway::OrderContext, o: &MbOrder, unit: u8, map: &[RegRule], last_executed_seq: Option<u32>, tid: u16) -> (Decision, RuleCheck, Option<TcpFrame>) {
    let (d, rules, rtu) = gateway_decision(ctx, o, unit, map, last_executed_seq);
    (d, rules, rtu.map(|f| from_rtu(&f, tid)))
}

/// Stateful form: same bookkeeping as [`Gateway::decide`]. Send the frame exactly once.
pub fn decide_tcp(gw: &mut Gateway, ctx: &crate::modbus_gateway::OrderContext, o: &MbOrder, unit: u8, map: &[RegRule], tid: u16) -> (Decision, RuleCheck, Option<TcpFrame>) {
    let (d, rules, rtu) = gw.decide(ctx, o, unit, map);
    (d, rules, rtu.map(|f| from_rtu(&f, tid)))
}

/// Check the PLC's answer to `req`. Total: any `resp` is accepted as input.
pub fn check_tcp_response(req: &TcpFrame, resp: &[u8]) -> Response {
    if req.len < ACK_LEN || req.len > MAX_TCP_FRAME {
        return Response::Mismatch;
    }
    let req = &req.bytes[..req.len];
    if resp.len() < EXC_LEN {
        return Response::Short;
    }
    if resp[0..4] != req[0..4] || resp[6] != req[6] || u16_be(resp, 4) as usize != resp.len() - 6 {
        return Response::Mismatch;
    }
    if resp.len() == EXC_LEN && resp[7] == (req[7] | 0x80) {
        return Response::Exception(resp[8]);
    }
    if resp.len() == ACK_LEN && resp[7..ACK_LEN] == req[7..ACK_LEN] {
        return Response::Ack;
    }
    Response::Mismatch
}

/// A plain Modbus TCP write received from the HMI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpWrite {
    pub tid: u16,
    pub unit: u8,
    pub fc: u8,
    pub start: u16,
    pub count: u8,
    pub values: [u16; MAX_REGS],
}

impl TcpWrite {
    /// The order the agent signs. Sequence, boot id and deadline come from the agent.
    pub fn to_order(&self, gateway_id: u16, cmd_seq: u32, boot_id: u64, deadline_ms: u64) -> MbOrder {
        MbOrder {
            gateway_id,
            cmd_seq,
            boot_id,
            deadline_ms,
            unit: self.unit,
            fc: self.fc,
            start: self.start,
            count: self.count,
            values: self.values,
        }
    }
}

/// Parse an HMI write. Exact length only; protocol 0; FC06/FC16 only; FC16 byte count
/// = 2 x quantity, quantity 1..=`MAX_REGS`. `None` for anything else, reads included.
pub fn parse_tcp_write(b: &[u8]) -> Option<TcpWrite> {
    if b.len() < MBAP_LEN + 5 || u16_be(b, 2) != 0 || u16_be(b, 4) as usize != b.len() - 6 {
        return None;
    }
    let (unit, fc, start) = (b[6], b[7], u16_be(b, 8));
    let mut values = [0u16; MAX_REGS];
    let count = match fc {
        FC_WRITE_SINGLE if b.len() == ACK_LEN => {
            values[0] = u16_be(b, 10);
            1
        }
        FC_WRITE_MULTIPLE if b.len() >= MBAP_LEN + 8 => {
            let qty = u16_be(b, 10) as usize;
            if qty == 0 || qty > MAX_REGS || b[12] as usize != 2 * qty || b.len() != MBAP_LEN + 6 + 2 * qty {
                return None;
            }
            for (i, v) in values.iter_mut().enumerate().take(qty) {
                *v = u16_be(b, 13 + 2 * i);
            }
            qty as u8
        }
        _ => return None,
    };
    Some(TcpWrite { tid: u16_be(b, 0), unit, fc, start, count, values })
}

/// What the agent learned about an HMI write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The gateway acted and the PLC acknowledged.
    Done,
    /// The gateway refused the order.
    Refused(Reason, RuleCheck),
    /// The gateway acted but the PLC answered with this exception.
    PlcException(u8),
    /// No usable answer (timeout, bad frame, gateway unreachable).
    NoAnswer,
}

/// Exception code for a refusal: the register rules have their own codes, every other
/// refusal of the gate is reported as "gateway path unavailable".
pub fn refusal_code(reason: Reason, rules: RuleCheck) -> u8 {
    match (reason, rules) {
        (Reason::OutOfLimits, RuleCheck::RegisterNotAllowed(_)) => EXC_ILLEGAL_ADDRESS,
        (Reason::OutOfLimits, RuleCheck::ValueOutOfRange(..)) => EXC_ILLEGAL_VALUE,
        _ => EXC_GATEWAY_PATH_UNAVAILABLE,
    }
}

/// The answer the agent gives the HMI: the write echo on `Done`, an exception otherwise.
pub fn hmi_response(w: &TcpWrite, outcome: Outcome) -> ([u8; ACK_LEN], usize) {
    let mut b = [0u8; ACK_LEN];
    let code = match outcome {
        Outcome::Done => {
            mbap(&mut b, w.tid, w.unit, 5);
            b[7] = w.fc;
            b[8..10].copy_from_slice(&w.start.to_be_bytes());
            let tail = if w.fc == FC_WRITE_SINGLE { w.values[0] } else { w.count as u16 };
            b[10..12].copy_from_slice(&tail.to_be_bytes());
            return (b, ACK_LEN);
        }
        Outcome::Refused(r, rules) => refusal_code(r, rules),
        Outcome::PlcException(c) => c,
        Outcome::NoAnswer => EXC_GATEWAY_TARGET_FAILED,
    };
    mbap(&mut b, w.tid, w.unit, 2);
    b[7] = w.fc | 0x80;
    b[8] = code;
    (b, EXC_LEN)
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
