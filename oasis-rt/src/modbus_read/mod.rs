//! Authenticated, address-limited **reads** through the gateway (pilot phase A.6).
//!
//! Until now the agent refused FC03 with exception 0x0A, on the reasoning that the gateway
//! must be the PLC's only path. That reasoning is right and the conclusion was wrong: it
//! forbids a *second path*, not reading. An HMI that cannot read is not an HMI, and an
//! operator who cannot see the machine has no way to decide whether to stop it — so
//! refusing reads pushed every real deployment towards a side channel straight to the PLC,
//! which is the exact thing the gateway exists to prevent.
//!
//! So a read goes **through** the gateway, with four properties and one deliberate
//! omission:
//!
//! 1. **Authenticated.** An `OMQ1` query travels inside the same v0B envelope as an order,
//!    so a forged read is dropped at the mesh layer before this module is reached. An
//!    unauthenticated read path would let anyone who can reach the agent enumerate the
//!    device, which is reconnaissance, not a read.
//! 2. **Address-limited.** Every register in `start .. start+count` must appear in the
//!    configured map, or the query is refused — the same map that bounds writes. Without
//!    this, FC03 with a large quantity is a scanner.
//! 3. **Bounded.** At most [`MAX_READ_REGS`] registers per query, so a reply fits a frame
//!    whose size is known at compile time and `no_std` needs no allocation.
//! 4. **Addressed.** `gateway_id` must match, so on a network with two gateways a query
//!    for one is not answered by the other.
//!
//! **A read is not put through the actuation gate, and that is the point.** The gate exists
//! to decide whether to *act*; a read changes nothing, so freshness, the sequence counter
//! and the value limits have nothing to judge. Two consequences are worth stating plainly
//! rather than discovering:
//!
//! - **A latched stop does not block reads.** It must not: a stop is exactly the moment an
//!   operator most needs to see the registers. Blocking reads during a stop would make the
//!   safest state also the blindest one.
//! - **Reads are not journalled.** Not for lack of care — Annex III 1.1.9 asks for a
//!   record of commands, "légitime **ou** illégitime", and a read is not a command. The
//!   positive reason is that an HMI polls: at 10 Hz a day of reads would evict the command
//!   history from the journal's ring, destroying the audit trail in order to log the thing
//!   that does not need auditing.
//!
//! ⚠️ **FC03 only.** FC04 (input registers) is refused: the map describes holding
//! registers, the ones FC06 and FC16 write, and serving FC04 from it would answer a
//! question about a different address space.
//! ⚠️ A read reveals register values to anyone holding the agent's key. The envelope gives
//! authenticity, not confidentiality — `sealed` (`OSE1`) is the module for that, and this
//! path does not use it.

use crate::modbus_gateway::{RegRule, MAX_REGS};
use crate::modbus_tcp::{MAX_TCP_FRAME, MBAP_LEN};

pub const OMQ1_MAGIC: [u8; 4] = *b"OMQ1";
/// `magic 4 | gateway_id u16 | unit u8 | fc u8 | start u16 | count u8`
pub const OMQ1_LEN: usize = 11;

pub const OMV1_MAGIC: [u8; 4] = *b"OMV1";
/// `magic 4 | count u8` then `count` little-endian registers.
pub const OMV1_HEADER_LEN: usize = 5;

/// Registers per query. Equal to [`MAX_REGS`] — the write path's bound — because the two
/// describe the same map, but kept as its own constant: a reply is read-only and could
/// safely grow without touching anything that writes.
pub const MAX_READ_REGS: usize = MAX_REGS;
pub const OMV1_MAX_LEN: usize = OMV1_HEADER_LEN + 2 * MAX_READ_REGS;

pub const FC_READ_HOLDING: u8 = 0x03;
/// Read of input registers. Named so it can be refused by name rather than by default.
pub const FC_READ_INPUT: u8 = 0x04;

/// A read the agent received from the HMI and will ask the gateway to serve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MbQuery {
    pub gateway_id: u16,
    pub unit: u8,
    pub fc: u8,
    pub start: u16,
    pub count: u8,
}

/// Why a query was refused, or that it was not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadCheck {
    Ok,
    WrongGateway(u16),
    WrongUnit(u8),
    UnsupportedFc(u8),
    CountOutOfRange(u8),
    /// The first register of the span that is not in the map.
    RegisterNotAllowed(u16),
}

impl ReadCheck {
    pub fn is_ok(self) -> bool {
        matches!(self, ReadCheck::Ok)
    }

    /// The Modbus exception an agent returns to the HMI for this refusal.
    ///
    /// Every refusal here is about *which* register or unit was asked for, so 0x02
    /// "illegal data address" is the honest code — except an unsupported function, which
    /// has its own.
    pub fn exception_code(self) -> u8 {
        match self {
            ReadCheck::Ok => 0,
            ReadCheck::UnsupportedFc(_) => 0x01, // illegal function
            _ => crate::modbus_tcp::EXC_ILLEGAL_ADDRESS,
        }
    }
}

fn span_allowed(start: u16, count: u8, map: &[RegRule]) -> ReadCheck {
    let mut i = 0u8;
    while i < count {
        // A span that would wrap the address space is not a span. Checked here rather
        // than trusted, because `start + count` is where an off-by-one becomes a read of
        // register 0 of someone else's map.
        let addr = match start.checked_add(i as u16) {
            Some(a) => a,
            None => return ReadCheck::RegisterNotAllowed(start),
        };
        let mut found = false;
        let mut j = 0usize;
        while j < map.len() {
            if map[j].addr == addr {
                found = true;
                break;
            }
            j += 1;
        }
        if !found {
            return ReadCheck::RegisterNotAllowed(addr);
        }
        i += 1;
    }
    ReadCheck::Ok
}

/// The pure rule. Ordered so the cheapest and most specific refusal wins, and so the
/// register walk only runs for a query that is otherwise well formed.
pub fn read_decision(q: &MbQuery, gateway_id: u16, unit: u8, map: &[RegRule]) -> ReadCheck {
    if q.gateway_id != gateway_id {
        return ReadCheck::WrongGateway(q.gateway_id);
    }
    if q.fc != FC_READ_HOLDING {
        return ReadCheck::UnsupportedFc(q.fc);
    }
    if q.unit != unit {
        return ReadCheck::WrongUnit(q.unit);
    }
    if q.count == 0 || q.count as usize > MAX_READ_REGS {
        return ReadCheck::CountOutOfRange(q.count);
    }
    span_allowed(q.start, q.count, map)
}

/// The decision **and** the PLC request, which exists only when the decision is `Ok`.
///
/// The same shape as `decide_tcp` on the write side, and for the same reason: a Kani
/// harness can then state "no frame reaches the PLC unless the rule allowed it" over the
/// read path exactly as it does over the write path.
pub fn decide_read(q: &MbQuery, gateway_id: u16, unit: u8, map: &[RegRule], tid: u16) -> (ReadCheck, Option<crate::modbus_tcp::TcpFrame>) {
    let check = read_decision(q, gateway_id, unit, map);
    if !check.is_ok() {
        return (check, None);
    }
    let mut bytes = [0u8; MAX_TCP_FRAME];
    bytes[0..2].copy_from_slice(&tid.to_be_bytes());
    bytes[2..4].copy_from_slice(&0u16.to_be_bytes());
    bytes[4..6].copy_from_slice(&6u16.to_be_bytes());
    bytes[6] = unit;
    bytes[7] = FC_READ_HOLDING;
    bytes[8..10].copy_from_slice(&q.start.to_be_bytes());
    bytes[10..12].copy_from_slice(&(q.count as u16).to_be_bytes());
    (check, Some(crate::modbus_tcp::TcpFrame { bytes, len: MBAP_LEN + 5 }))
}

pub fn encode_omq1(q: &MbQuery) -> [u8; OMQ1_LEN] {
    let mut b = [0u8; OMQ1_LEN];
    b[0..4].copy_from_slice(&OMQ1_MAGIC);
    b[4..6].copy_from_slice(&q.gateway_id.to_le_bytes());
    b[6] = q.unit;
    b[7] = q.fc;
    b[8..10].copy_from_slice(&q.start.to_le_bytes());
    b[10] = q.count;
    b
}

/// Total: every byte string either parses to one query or to `None`.
pub fn parse_omq1(b: &[u8]) -> Option<MbQuery> {
    if b.len() != OMQ1_LEN || b[0..4] != OMQ1_MAGIC {
        return None;
    }
    Some(MbQuery {
        gateway_id: u16::from_le_bytes([b[4], b[5]]),
        unit: b[6],
        fc: b[7],
        start: u16::from_le_bytes([b[8], b[9]]),
        count: b[10],
    })
}

/// The values reply. Returns `None` if `count` exceeds what a reply can carry, so an
/// oversized answer is never framed.
pub fn encode_omv1(values: &[u16]) -> Option<([u8; OMV1_MAX_LEN], usize)> {
    if values.is_empty() || values.len() > MAX_READ_REGS {
        return None;
    }
    let mut b = [0u8; OMV1_MAX_LEN];
    b[0..4].copy_from_slice(&OMV1_MAGIC);
    b[4] = values.len() as u8;
    let mut i = 0usize;
    while i < values.len() {
        b[OMV1_HEADER_LEN + 2 * i..OMV1_HEADER_LEN + 2 * i + 2].copy_from_slice(&values[i].to_le_bytes());
        i += 1;
    }
    Some((b, OMV1_HEADER_LEN + 2 * values.len()))
}

/// Total. The declared count must match the actual length exactly: a reply claiming more
/// registers than it carries is refused rather than padded with zeros, because a zero an
/// operator believes is a reading is worse than no reading.
pub fn parse_omv1(b: &[u8]) -> Option<([u16; MAX_READ_REGS], u8)> {
    if b.len() < OMV1_HEADER_LEN || b[0..4] != OMV1_MAGIC {
        return None;
    }
    let count = b[4];
    if count == 0 || count as usize > MAX_READ_REGS || b.len() != OMV1_HEADER_LEN + 2 * count as usize {
        return None;
    }
    let mut out = [0u16; MAX_READ_REGS];
    let mut i = 0usize;
    while i < count as usize {
        out[i] = u16::from_le_bytes([b[OMV1_HEADER_LEN + 2 * i], b[OMV1_HEADER_LEN + 2 * i + 1]]);
        i += 1;
    }
    Some((out, count))
}

/// Parse the HMI's plain Modbus TCP read. Exact length, protocol 0, FC03 or FC04 — FC04 is
/// parsed so [`read_decision`] can refuse it by name rather than the parser hiding it.
pub fn parse_tcp_read(b: &[u8], gateway_id: u16) -> Option<MbQuery> {
    if b.len() != MBAP_LEN + 5 || u16::from_be_bytes([b[2], b[3]]) != 0 || u16::from_be_bytes([b[4], b[5]]) as usize != b.len() - 6 {
        return None;
    }
    let fc = b[7];
    if fc != FC_READ_HOLDING && fc != FC_READ_INPUT {
        return None;
    }
    let qty = u16::from_be_bytes([b[10], b[11]]);
    if qty == 0 || qty > u8::MAX as u16 {
        return None;
    }
    Some(MbQuery { gateway_id, unit: b[6], fc, start: u16::from_be_bytes([b[8], b[9]]), count: qty as u8 })
}

/// Check the PLC's FC03 answer against the request and extract the values.
///
/// `Err(Some(code))` is a Modbus exception from the device, `Err(None)` a malformed or
/// mismatched answer. Neither is turned into values: an operator reading a register must
/// see an exception rather than a number the device did not send.
pub fn check_read_response(req: &crate::modbus_tcp::TcpFrame, resp: &[u8], count: u8) -> Result<([u16; MAX_READ_REGS], u8), Option<u8>> {
    let r = req.as_slice();
    if resp.len() < MBAP_LEN + 2 || resp[0..2] != r[0..2] {
        return Err(None);
    }
    if resp[7] == FC_READ_HOLDING | 0x80 {
        return Err(Some(resp[8]));
    }
    let n = count as usize;
    if resp[7] != FC_READ_HOLDING || resp.len() != MBAP_LEN + 2 + 2 * n || resp[8] as usize != 2 * n {
        return Err(None);
    }
    let mut out = [0u16; MAX_READ_REGS];
    let mut i = 0usize;
    while i < n {
        out[i] = u16::from_be_bytes([resp[9 + 2 * i], resp[10 + 2 * i]]);
        i += 1;
    }
    Ok((out, count))
}

/// The FC03 answer the agent gives the HMI.
///
/// `tid` is the HMI's own transaction id and must be echoed: an HMI matches a reply to its
/// request by that field, and `MbQuery` does not carry it because it is local to the HMI
/// link and has no meaning to the gateway.
pub fn hmi_read_response(q: &MbQuery, tid: u16, values: &[u16]) -> Option<([u8; MAX_TCP_FRAME], usize)> {
    if values.is_empty() || values.len() > MAX_READ_REGS {
        return None;
    }
    let n = values.len();
    let mut b = [0u8; MAX_TCP_FRAME];
    b[0..2].copy_from_slice(&tid.to_be_bytes());
    b[2..4].copy_from_slice(&0u16.to_be_bytes());
    b[4..6].copy_from_slice(&((3 + 2 * n) as u16).to_be_bytes());
    b[6] = q.unit;
    b[7] = FC_READ_HOLDING;
    b[8] = (2 * n) as u8;
    let mut i = 0usize;
    while i < n {
        b[9 + 2 * i..11 + 2 * i].copy_from_slice(&values[i].to_be_bytes());
        i += 1;
    }
    Some((b, MBAP_LEN + 2 + 2 * n))
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
