//! OASIS — a signed order carried inside MAVLink, and arming only on `Act`.
//!
//! **B1** of `partners/POSITIONING_GAPS.md`. The MAVLink v2 signing scheme is a
//! link-level HMAC with a shared key: it says a frame came from *someone holding the
//! link key*, not from a named operator, and it is not bound to a counter the vehicle
//! persists (`docs/compliance/MAVLINK_SIGNING_GAP.md`, limits L1–L9). The answer here is
//! not to replace it: it is to carry an OASIS v0B envelope **inside** a MAVLink message
//! and let the Part F gate decide, so the arming decision depends on the OASIS
//! signature, the OASIS counter and the OASIS revocation set — whatever the link does.
//!
//! **Carrier: `V2_EXTENSION`** (msgid 248, `CRC_EXTRA` 8, 254-byte payload, 249 opaque
//! bytes at offset 5). A v0B envelope carrying an `OAC1` order is 99 + 54 = **153 bytes**,
//! which fits with 94 to spare once the 2-byte length prefix is counted; `TUNNEL`
//! (msgid 385) offers **128** and does not fit. Both read from
//! `mavlink/message_definitions/v1.0/common.xml`; the `CRC_EXTRA` values were recomputed
//! and checked against the ten already in [`crate::mavlink_min`] — see
//! `docs/specs/MAVLINK_ORDER_SPEC.md`.
//!
//! The opaque payload is `len u16 LE | envelope[len] | zero padding`, which is what
//! the field's own description in `common.xml` asks for: "The length must be encoded in
//! the payload as part of the `message_type` protocol". It has to be, because MAVLink 2
//! senders may trim trailing zero bytes — [`truncate_trailing_zeros`] models one, and a
//! test reassembles after it.
//!
//! [`arm_decision_ctx`] is pure, and builds the `COMMAND_LONG` **only** in the `Act`
//! branch, so no ARM frame exists without an `Act` (Kani: `kani_proofs.rs`). Same shape
//! as [`crate::modbus_gateway`], for the same reason.
//!
//! ⚠️ **`OrderClass::Stop` is refused over this carrier.** For an airborne vehicle the
//! "safe direction" is not a disarm — a disarm in flight drops it — and OASIS does not
//! define what it is. Refusing is honest; inventing a landing behaviour here would be a
//! safety claim this project does not make.

use crate::actuation::{
    actuation_decision_ctx, ActCommand, Actuator, Decision, GateContext, GateInput, OrderClass,
};
use crate::mavlink_min::encode_command_long;

/// `V2_EXTENSION`, from `common.xml`.
pub const V2EXT_MSGID: u32 = 248;
pub const V2EXT_CRC_EXTRA: u8 = 8;
/// `message_type u16 | target_network u8 | target_system u8 | target_component u8`.
pub const V2EXT_PAYLOAD_OFF: usize = 5;
pub const V2EXT_OPAQUE_CAP: usize = 249;
pub const V2EXT_MSG_LEN: usize = V2EXT_PAYLOAD_OFF + V2EXT_OPAQUE_CAP;
/// 2 bytes of the opaque block are the length prefix.
pub const ENVELOPE_CAP: usize = V2EXT_OPAQUE_CAP - 2;

/// Above 32767, which `common.xml` reserves for "local experiments" that "should not be
/// checked in to any widely distributed codebase" — the right class for this, since no
/// `message_type` has been registered with the MAVLink project. `0x0B` recalls v0B.
pub const OASIS_MESSAGE_TYPE: u16 = 0xFA0B;

/// `MAV_CMD_COMPONENT_ARM_DISARM`, `COMMAND_LONG` param1 = 1 to arm, 0 to disarm.
pub const MAV_CMD_COMPONENT_ARM_DISARM: u16 = 400;

const MAV_V2_MAGIC: u8 = 0xFD;

/// What an accepted order asks the vehicle to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmAction {
    Arm,
    Disarm,
}

/// The vehicle's configured rules. "Within limits" for an arming order is this and
/// nothing else: the order names *this* vehicle, and `force` is exactly 0 or 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmRules {
    pub vehicle_id: u16,
}

/// `Some` only for an order this vehicle may consider at all.
pub fn action_of(o: &ActCommand, rules: &ArmRules) -> Option<ArmAction> {
    if o.actuator_id != rules.vehicle_id {
        return None;
    }
    // NaN compares false against everything, so a NaN `force` lands here and is refused.
    // That is deliberate and it is the opposite of `clamp_command`, which fails open on
    // NaN (noted in `docs/AUTHORITY_HARDENING_SPEC.md`).
    if o.force == 1.0 {
        Some(ArmAction::Arm)
    } else if o.force == 0.0 {
        Some(ArmAction::Disarm)
    } else {
        None
    }
}

/// Part F conditions that the carrier cannot establish by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmContext {
    /// Full v0B verification succeeded.
    pub v0b_ok: bool,
    /// Origin is enrolled with `ACTUATE` and commands this vehicle.
    pub authorized: bool,
    pub revoked: bool,
    pub actuator_boot_id: u64,
    pub now_ms: u64,
    pub r14_safe: bool,
}

pub fn gate_input(ctx: &ArmContext, o: &ActCommand, rules: &ArmRules, last_executed_seq: Option<u32>) -> GateInput {
    GateInput {
        v0b_ok: ctx.v0b_ok,
        authorized: ctx.authorized,
        revoked: ctx.revoked,
        cmd_boot_id: o.boot_id,
        deadline_ms: o.deadline_ms,
        actuator_boot_id: ctx.actuator_boot_id,
        now_ms: ctx.now_ms,
        r14_safe: ctx.r14_safe,
        within_limits: action_of(o, rules).is_some(),
        cmd_seq: o.cmd_seq,
        last_executed_seq,
    }
}

/// Decide, and build the `COMMAND_LONG` only if the decision is `Act`.
///
/// `class` is the order class read from the signed `OAC1` byte. Only
/// [`OrderClass::Act`] is carried here; a `Stop` is refused (see the module note).
pub fn arm_decision_ctx(
    gctx: &GateContext,
    ctx: &ArmContext,
    o: &ActCommand,
    class: OrderClass,
    rules: &ArmRules,
    last_executed_seq: Option<u32>,
    seq: u8,
    sysid: u8,
    compid: u8,
    target_sys: u8,
    target_comp: u8,
) -> (Decision, Option<ArmAction>, Option<Vec<u8>>) {
    if class != OrderClass::Act {
        return (Decision::Reject(crate::actuation::Reason::OutOfLimits), None, None);
    }
    let d = actuation_decision_ctx(gctx, &gate_input(ctx, o, rules, last_executed_seq));
    match d {
        Decision::Act => {
            // `within_limits` was true, so `action_of` is `Some`; this is the same fact.
            let action = match action_of(o, rules) {
                Some(a) => a,
                None => return (Decision::Reject(crate::actuation::Reason::OutOfLimits), None, None),
            };
            let p1 = match action {
                ArmAction::Arm => 1.0,
                ArmAction::Disarm => 0.0,
            };
            let f = encode_command_long(
                seq, sysid, compid, target_sys, target_comp,
                MAV_CMD_COMPONENT_ARM_DISARM, 0, p1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            );
            (d, Some(action), Some(f))
        }
        Decision::Reject(_) => (d, None, None),
    }
}

/// Stateful vehicle side: the counter and the reject tally live in an `Actuator`.
#[derive(Debug, Clone)]
pub struct Vehicle {
    pub act: Actuator,
    pub rules: ArmRules,
    pub sysid: u8,
    pub compid: u8,
    pub target_sys: u8,
    pub target_comp: u8,
    pub seq: u8,
}

impl Vehicle {
    pub fn new(rules: ArmRules) -> Self {
        Self { act: Actuator::new(), rules, sysid: 1, compid: 191, target_sys: 1, target_comp: 1, seq: 0 }
    }

    /// Decide on one order; send the returned frame (if any) exactly once.
    pub fn decide(&mut self, ctx: &ArmContext, o: &ActCommand, class: OrderClass) -> (Decision, Option<ArmAction>, Option<Vec<u8>>) {
        let gctx = self.act.context(ctx.now_ms);
        let r = arm_decision_ctx(
            &gctx, ctx, o, class, &self.rules, self.act.last_executed_seq,
            self.seq, self.sysid, self.compid, self.target_sys, self.target_comp,
        );
        match r.0 {
            Decision::Act => {
                self.act.last_executed_seq = Some(o.cmd_seq);
                self.act.executed += 1;
                self.seq = self.seq.wrapping_add(1);
            }
            Decision::Reject(reason) => self.act.rejects[reason as usize] += 1,
        }
        r
    }
}

// ─── the carrier ───

/// Build a `V2_EXTENSION` frame carrying `envelope` (a v0B/v0C mesh envelope).
/// `None` if the envelope does not fit in [`ENVELOPE_CAP`].
pub fn encode_v2_extension(
    seq: u8,
    sysid: u8,
    compid: u8,
    target_network: u8,
    target_sys: u8,
    target_comp: u8,
    message_type: u16,
    envelope: &[u8],
) -> Option<Vec<u8>> {
    if envelope.len() > ENVELOPE_CAP {
        return None;
    }
    let mut payload = vec![0u8; V2EXT_MSG_LEN];
    payload[0..2].copy_from_slice(&message_type.to_le_bytes());
    payload[2] = target_network;
    payload[3] = target_sys;
    payload[4] = target_comp;
    payload[V2EXT_PAYLOAD_OFF..V2EXT_PAYLOAD_OFF + 2]
        .copy_from_slice(&(envelope.len() as u16).to_le_bytes());
    payload[V2EXT_PAYLOAD_OFF + 2..V2EXT_PAYLOAD_OFF + 2 + envelope.len()].copy_from_slice(envelope);
    Some(frame_with_payload(seq, sysid, compid, &payload))
}

fn frame_with_payload(seq: u8, sysid: u8, compid: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(12 + payload.len());
    frame.push(MAV_V2_MAGIC);
    frame.push(payload.len() as u8);
    frame.push(0); // incompat_flags: unsigned
    frame.push(0); // compat_flags
    frame.push(seq);
    frame.push(sysid);
    frame.push(compid);
    frame.push((V2EXT_MSGID & 0xFF) as u8);
    frame.push(((V2EXT_MSGID >> 8) & 0xFF) as u8);
    frame.push(((V2EXT_MSGID >> 16) & 0xFF) as u8);
    frame.extend_from_slice(payload);
    let crc = crate::mavlink_min::crc_over(&frame[1..], V2EXT_CRC_EXTRA);
    frame.push((crc & 0xFF) as u8);
    frame.push((crc >> 8) as u8);
    frame
}

/// Model a conforming MAVLink 2 sender: trim trailing zero bytes of the payload (never
/// the first), and recompute `len` and the CRC. A frame that goes through this is still
/// a valid MAVLink 2 frame, and the receiver must put the zeros back.
pub fn truncate_trailing_zeros(frame: &[u8]) -> Option<Vec<u8>> {
    let (hdr, payload) = split_frame(frame)?;
    let mut n = payload.len();
    while n > 1 && payload[n - 1] == 0 {
        n -= 1;
    }
    Some(frame_with_payload(hdr.0, hdr.1, hdr.2, &payload[..n]))
}

/// `(seq, sysid, compid)` and the payload of an unsigned MAVLink 2 `V2_EXTENSION`
/// frame whose CRC checks out. Total: every malformed input returns `None`.
fn split_frame(frame: &[u8]) -> Option<((u8, u8, u8), &[u8])> {
    if frame.len() < 12 || frame[0] != MAV_V2_MAGIC {
        return None;
    }
    let len = frame[1] as usize;
    if frame[2] & 0x01 != 0 {
        return None; // signed frames carry 13 more bytes; not handled here
    }
    let msgid = u32::from(frame[7]) | (u32::from(frame[8]) << 8) | (u32::from(frame[9]) << 16);
    if msgid != V2EXT_MSGID || frame.len() != 12 + len {
        return None;
    }
    let crc = crate::mavlink_min::crc_over(&frame[1..10 + len], V2EXT_CRC_EXTRA);
    if frame[10 + len] != (crc & 0xFF) as u8 || frame[11 + len] != (crc >> 8) as u8 {
        return None;
    }
    Some(((frame[4], frame[5], frame[6]), &frame[10..10 + len]))
}

/// The OASIS envelope carried by a `V2_EXTENSION` frame, zero-extended back to the
/// length the payload declares. `None` if the frame is not one of ours, its CRC is
/// wrong, or the declared length does not fit.
///
/// Zero-extension is safe by construction: the v0B signature covers
/// `SHA-256(payload)`, so a wrongly reassembled envelope fails verification instead of
/// being acted on.
pub fn extract_envelope(frame: &[u8], message_type: u16) -> Option<Vec<u8>> {
    let (_, payload) = split_frame(frame)?;
    // Everything the sender may have trimmed was zero.
    let mut opaque = [0u8; V2EXT_OPAQUE_CAP];
    if payload.len() < V2EXT_PAYLOAD_OFF + 2 {
        return None;
    }
    if u16::from_le_bytes([payload[0], payload[1]]) != message_type {
        return None;
    }
    let tail = &payload[V2EXT_PAYLOAD_OFF..];
    opaque[..tail.len()].copy_from_slice(tail);
    let n = u16::from_le_bytes([opaque[0], opaque[1]]) as usize;
    if n == 0 || n > ENVELOPE_CAP {
        return None;
    }
    Some(opaque[2..2 + n].to_vec())
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
