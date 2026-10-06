//! OASIS — actuation gate (spec `docs/REVOCATION_AND_ACTUATION_SPEC.md` Part F).
//!
//! An actuator moves only if a command is authenticated, fresh, from an authorized
//! and non-revoked origin, unexpired in the actuator's own time base, allowed by
//! R14, inside the physical limits, and newer than the last executed command.
//!
//! [`actuation_decision`] is a pure function of its inputs (no I/O, no clock read,
//! no mutation), so it is provable by Kani. [`Actuator`] wraps it with the only
//! state the gate needs: the last executed sequence number and per-reason
//! rejection counters.
//!
//! Formats (v0B payload content types, little-endian):
//! - command `"OAC1" | actuator_id u16 | cmd_seq u32 | boot_id u64 | deadline_ms u64
//!   | force f32 | torque f32 | velocity f32 | pos [f32;3]` (54 bytes)
//! - time beacon `"OTM1" | boot_id u64 | now_ms u64` (20 bytes), originated by the
//!   actuator; commanders stamp `deadline_ms` in the actuator's clock.

use crate::hal::ClampResult;

pub const OAC1_MAGIC: [u8; 4] = *b"OAC1";
pub const OTM1_MAGIC: [u8; 4] = *b"OTM1";
pub const OAC1_LEN: usize = 54;
pub const OTM1_LEN: usize = 20;
/// Default validity a commander grants (must exceed ~hops x 180 ms verify + 175 ms sign on M0+).
pub const DEFAULT_VALIDITY_MS: u64 = 3_000;
/// Deadlines further ahead than this are refused (no long-lived commands).
pub const MAX_VALIDITY_MS: u64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActCommand {
    pub actuator_id: u16,
    pub cmd_seq: u32,
    pub boot_id: u64,
    pub deadline_ms: u64,
    pub force: f32,
    pub torque: f32,
    pub velocity: f32,
    pub pos: [f32; 3],
}

pub fn encode_oac1(c: &ActCommand) -> [u8; OAC1_LEN] {
    let mut b = [0u8; OAC1_LEN];
    b[0..4].copy_from_slice(&OAC1_MAGIC);
    b[4..6].copy_from_slice(&c.actuator_id.to_le_bytes());
    b[6..10].copy_from_slice(&c.cmd_seq.to_le_bytes());
    b[10..18].copy_from_slice(&c.boot_id.to_le_bytes());
    b[18..26].copy_from_slice(&c.deadline_ms.to_le_bytes());
    b[26..30].copy_from_slice(&c.force.to_le_bytes());
    b[30..34].copy_from_slice(&c.torque.to_le_bytes());
    b[34..38].copy_from_slice(&c.velocity.to_le_bytes());
    for i in 0..3 {
        b[38 + 4 * i..42 + 4 * i].copy_from_slice(&c.pos[i].to_le_bytes());
    }
    b
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
fn u64_at(b: &[u8], at: usize) -> u64 {
    let mut x = [0u8; 8];
    x.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(x)
}

pub fn parse_oac1(b: &[u8]) -> Option<ActCommand> {
    if b.len() != OAC1_LEN || b[0..4] != OAC1_MAGIC {
        return None;
    }
    Some(ActCommand {
        actuator_id: u16::from_le_bytes([b[4], b[5]]),
        cmd_seq: u32::from_le_bytes([b[6], b[7], b[8], b[9]]),
        boot_id: u64_at(b, 10),
        deadline_ms: u64_at(b, 18),
        force: f32_at(b, 26),
        torque: f32_at(b, 30),
        velocity: f32_at(b, 34),
        pos: [f32_at(b, 38), f32_at(b, 42), f32_at(b, 46)],
    })
}

pub fn encode_otm1(boot_id: u64, now_ms: u64) -> [u8; OTM1_LEN] {
    let mut b = [0u8; OTM1_LEN];
    b[0..4].copy_from_slice(&OTM1_MAGIC);
    b[4..12].copy_from_slice(&boot_id.to_le_bytes());
    b[12..20].copy_from_slice(&now_ms.to_le_bytes());
    b
}

/// `(boot_id, now_ms)` from an actuator time beacon.
pub fn parse_otm1(b: &[u8]) -> Option<(u64, u64)> {
    if b.len() != OTM1_LEN || b[0..4] != OTM1_MAGIC {
        return None;
    }
    Some((u64_at(b, 4), u64_at(b, 12)))
}

/// `true` iff `clamp_command` changed nothing and saw no geofence breach.
/// Out-of-limit commands are REJECTED, not clamped (spec F.3, condition 6).
pub fn limits_ok(r: &ClampResult) -> bool {
    !r.clamped_force && !r.clamped_torque && !r.clamped_velocity && !r.geofence_breach && !r.non_finite_input
}

/// Condition 6 for a decoded command. `clamp_command` fails OPEN on NaN
/// (`NaN.abs() > max` is false, geofence comparisons with NaN are false), so a
/// NaN or infinite setpoint would raise no flag. Every field must be finite first.
pub fn command_within_limits(c: &ActCommand, k: &crate::hal::PhysicalConstraints) -> bool {
    let fields = [c.force, c.torque, c.velocity, c.pos[0], c.pos[1], c.pos[2]];
    if fields.iter().any(|x| !x.is_finite()) {
        return false;
    }
    let pos = [c.pos[0] as f64, c.pos[1] as f64, c.pos[2] as f64];
    limits_ok(&crate::hal::clamp_command(c.force as f64, c.torque as f64, c.velocity as f64, &pos, k))
}

/// Why a command was not executed. Order = evaluation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    NotVerified = 0,
    NotAuthorized = 1,
    Revoked = 2,
    Expired = 3,
    R14Unsafe = 4,
    OutOfLimits = 5,
    StaleOrReplayed = 6,
}
pub const REASON_COUNT: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Act,
    Reject(Reason),
}

/// Everything the rule depends on, already evaluated by the caller.
#[derive(Debug, Clone, Copy)]
pub struct GateInput {
    /// Full v0B verification succeeded (origin, content, freshness, network).
    pub v0b_ok: bool,
    /// Origin is a command authority for this actuator.
    pub authorized: bool,
    /// Origin is in the revocation set held by the actuator.
    pub revoked: bool,
    pub cmd_boot_id: u64,
    pub deadline_ms: u64,
    pub actuator_boot_id: u64,
    pub now_ms: u64,
    /// `hyper_state::is_action_safe(agent, threshold)`.
    pub r14_safe: bool,
    /// `limits_ok(&clamp_command(...))`.
    pub within_limits: bool,
    pub cmd_seq: u32,
    /// Last executed `cmd_seq` for this actuator since boot (`None` = none yet).
    pub last_executed_seq: Option<u32>,
}

/// Not expired in the actuator's own time base.
pub fn fresh(i: &GateInput) -> bool {
    i.cmd_boot_id == i.actuator_boot_id && i.now_ms <= i.deadline_ms && i.deadline_ms - i.now_ms <= MAX_VALIDITY_MS
}

/// The gate. `Act` only if ALL seven conditions hold; otherwise the first failing
/// one, in spec order. Pure.
pub fn actuation_decision(i: &GateInput) -> Decision {
    if !i.v0b_ok {
        return Decision::Reject(Reason::NotVerified);
    }
    if !i.authorized {
        return Decision::Reject(Reason::NotAuthorized);
    }
    if i.revoked {
        return Decision::Reject(Reason::Revoked);
    }
    if !fresh(i) {
        return Decision::Reject(Reason::Expired);
    }
    if !i.r14_safe {
        return Decision::Reject(Reason::R14Unsafe);
    }
    if !i.within_limits {
        return Decision::Reject(Reason::OutOfLimits);
    }
    if let Some(last) = i.last_executed_seq {
        if i.cmd_seq <= last {
            return Decision::Reject(Reason::StaleOrReplayed);
        }
    }
    Decision::Act
}

/// Stateful wrapper: records the executed sequence and counts rejections.
#[derive(Debug, Clone, Default)]
pub struct Actuator {
    pub last_executed_seq: Option<u32>,
    pub rejects: [u32; REASON_COUNT],
    pub executed: u32,
}

impl Actuator {
    pub fn new() -> Self {
        Self::default()
    }
    /// Decide on a command. The caller must drive the actuator iff this returns
    /// `Act` (exactly once). `input.last_executed_seq` is overridden by the state.
    pub fn decide(&mut self, mut input: GateInput) -> Decision {
        input.last_executed_seq = self.last_executed_seq;
        let d = actuation_decision(&input);
        match d {
            Decision::Act => {
                self.last_executed_seq = Some(input.cmd_seq);
                self.executed += 1;
            }
            Decision::Reject(r) => self.rejects[r as usize] += 1,
        }
        d
    }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
