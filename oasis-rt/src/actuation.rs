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
//!   | force f32 | torque f32 | velocity f32 | pos [f32;3] | class u8 | reserved [u8;3] = 0`
//!   (54 bytes; the reserved bytes must be zero, so a command has exactly one encoding)
//! - time beacon `"OTM1" | boot_id u64 | now_ms u64` (20 bytes), originated by the
//!   actuator; commanders stamp `deadline_ms` in the actuator's clock.
//!
//! Part G (`docs/AUTHORITY_HARDENING_SPEC.md`) adds the **order class**. `class` takes
//! byte 50, one of the four bytes `parse_oac1` has required to be zero since the phase 2.3
//! fuzzing fix — so an order encoded before part G decodes as [`OrderClass::Act`], and
//! [`parse_oac1`] keeps its exact previous behaviour (it accepts `Act` only).
//!
//! A **stop order goes in the safe direction**, so it must not be blocked by a condition
//! that does not concern it: [`stop_decision`] keeps 3 of the 9 conditions. The constraint
//! behind that asymmetry is ISO 13849-1:2023 — a component that can disable a safety
//! function is treated as a safety function in its own right, which OASIS must not become.
//! See `docs/compliance/IEC_TS_63074.md`. **Nothing here is a certified safety function**,
//! and the network stop is not an emergency stop: ISO 13850:2015 4.1.1.3 makes the
//! emergency stop a complementary protective measure on its own dedicated circuit, which
//! OASIS cannot reach.

use crate::hal::ClampResult;

pub const OAC1_MAGIC: [u8; 4] = *b"OAC1";
pub const OTM1_MAGIC: [u8; 4] = *b"OTM1";
pub const OAC1_LEN: usize = 54;
pub const OTM1_LEN: usize = 20;
/// Offset of the order-class byte inside `OAC1` (part G).
pub const OAC1_CLASS_OFF: usize = 50;
/// Default validity a commander grants (must exceed ~hops x 180 ms verify + 175 ms sign on M0+).
pub const DEFAULT_VALIDITY_MS: u64 = 3_000;
/// Deadlines further ahead than this are refused (no long-lived commands).
pub const MAX_VALIDITY_MS: u64 = 10_000;

mod supervision;
mod timeview;
mod wire;
pub use supervision::*;
pub use timeview::*;
pub use wire::*;

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
    /// Part G: a stop is latched. Only a local action clears it, never a network order.
    Stopped = 7,
    /// Part H: no live supervision beacon. Applies to `Act` only, never to a stop.
    SupervisionLost = 8,
    /// C9: a critical order arrived without k valid signatures from distinct authorised
    /// operators. See `crate::quorum`.
    QuorumMissing = 9,
}
pub const REASON_COUNT: usize = 10;

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

/// Actuator state the rule reads but does not own (parts G and H). `default()` is the
/// permissive context — not latched, supervision live — so that
/// `actuation_decision(i) == actuation_decision_ctx(&GateContext::default(), i)`, which
/// `proof_act_rule_unchanged` proves rather than assumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GateContext {
    /// A stop order has been accepted and not yet cleared locally (part G).
    pub stopped: bool,
    /// No live supervision beacon (part H). Inverted so that `false` is permissive.
    pub supervision_expired: bool,
}

/// The gate, as it was before part G: equivalent to [`actuation_decision_ctx`] with the
/// default context. Pure.
pub fn actuation_decision(i: &GateInput) -> Decision {
    actuation_decision_ctx(&GateContext::default(), i)
}

/// The gate for an `Act` order. `Act` only if ALL NINE conditions hold; otherwise the
/// first failing one, in spec order. Pure.
///
/// `stopped` is checked after `revoked` and `supervision_expired` after `fresh`, both
/// deliberately: an order that is not authentic, not authorized or revoked learns
/// nothing about the actuator's internal state, and the costly conditions stay last.
pub fn actuation_decision_ctx(ctx: &GateContext, i: &GateInput) -> Decision {
    if !i.v0b_ok {
        return Decision::Reject(Reason::NotVerified);
    }
    if !i.authorized {
        return Decision::Reject(Reason::NotAuthorized);
    }
    if i.revoked {
        return Decision::Reject(Reason::Revoked);
    }
    if ctx.stopped {
        return Decision::Reject(Reason::Stopped);
    }
    if !fresh(i) {
        return Decision::Reject(Reason::Expired);
    }
    if ctx.supervision_expired {
        return Decision::Reject(Reason::SupervisionLost);
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

/// Everything a **stop** order's decision depends on. Deliberately small: the fields of
/// [`GateInput`] that are absent cannot block a stop, which is the point of part G.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopInput {
    /// Full v0B verification succeeded. Kept: an unauthenticated stop would be a free
    /// denial of service for anyone within radio range.
    pub v0b_ok: bool,
    /// Origin holds the `STOP` permission, which is distinct from `ACTUATE`: a supervisor
    /// may be allowed to stop without being allowed to act.
    pub stop_authorized: bool,
    /// Origin is in the revocation set. Kept — the one place where a security condition
    /// can still refuse a stop, because a revoked node is precisely the one being defended
    /// against and would otherwise hold a permanent denial of service. Deliberate, bounded
    /// (it takes an operator-signed revocation), and documented.
    pub revoked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopDecision {
    Stop,
    Reject(Reason),
}

/// The stop rule: 3 conditions, and no others.
///
/// Not applied, each for a stated reason: `fresh` (a stale stop is still a stop, and
/// replaying a stop onto an already stopped machine does nothing), `r14_safe` (**the sense
/// is inverted** — a lost sensor is a reason to stop, not to refuse stopping),
/// `within_limits` (a stop has no magnitude), `cmd_seq` (see `fresh`; v0B's Bloom dedup
/// already drops exact replays at the mesh layer).
pub fn stop_decision(i: &StopInput) -> StopDecision {
    if !i.v0b_ok {
        return StopDecision::Reject(Reason::NotVerified);
    }
    if !i.stop_authorized {
        return StopDecision::Reject(Reason::NotAuthorized);
    }
    if i.revoked {
        return StopDecision::Reject(Reason::Revoked);
    }
    StopDecision::Stop
}

/// Stateful wrapper: records the executed sequence, the stop latch, the supervision
/// deadline, and per-reason rejection counters.
#[derive(Debug, Clone, Default)]
pub struct Actuator {
    pub last_executed_seq: Option<u32>,
    pub rejects: [u32; REASON_COUNT],
    pub executed: u32,
    /// Part G: set by an accepted stop, cleared only by [`Actuator::clear_stop`].
    pub stopped: bool,
    pub stops: u32,
    /// Part H: `None` until the first valid beacon; afterwards the deadline in the
    /// actuator's own clock.
    pub supervision_until_ms: Option<u64>,
    /// Highest accepted beacon sequence, to refuse a replayed beacon.
    pub last_beacon_seq: Option<u32>,
    /// Part H: whether a live supervision beacon is a precondition to act.
    ///
    /// **Not a silent default.** [`Actuator::new`] leaves it `false` and
    /// [`Actuator::new_supervised`] sets it, because the two cases are genuinely different
    /// machines: a fixed actuator (a valve on a Modbus bus) has no supervisor and never
    /// will, while autonomous mobile machinery under Annex III part 3 of regulation (EU)
    /// 2023/1230 may not operate without one.
    ///
    /// ⚠️ **An `Actuator::new()` is therefore not configured for Annex III part 3.** That
    /// is a deployment decision the integrator must make and document; OASIS cannot infer
    /// which machine it is running on.
    pub supervision_required: bool,
}

impl Actuator {
    pub fn new() -> Self {
        Self::default()
    }
    /// Decide on an `Act` order. The caller must drive the actuator iff this returns
    /// `Act` (exactly once). `input.last_executed_seq` is overridden by the state.
    ///
    /// `now_ms` is needed to evaluate the supervision deadline; it is already present in
    /// `input.now_ms`, so the caller passes nothing extra.
    pub fn decide(&mut self, mut input: GateInput) -> Decision {
        input.last_executed_seq = self.last_executed_seq;
        let ctx = self.context(input.now_ms);
        let d = actuation_decision_ctx(&ctx, &input);
        match d {
            Decision::Act => {
                self.last_executed_seq = Some(input.cmd_seq);
                self.executed += 1;
            }
            Decision::Reject(r) => self.rejects[r as usize] += 1,
        }
        d
    }

    /// An actuator on machinery that may not operate without a live supervision link
    /// (Annex III part 3). Prefer this for autonomous mobile machinery.
    pub fn new_supervised() -> Self {
        Self { supervision_required: true, ..Self::default() }
    }

    /// The context the rule reads, derived from the state at `now_ms`.
    pub fn context(&self, now_ms: u64) -> GateContext {
        let supervision_expired = self.supervision_required
            && match self.supervision_until_ms {
                Some(until) => now_ms > until,
                // No beacon ever received: supervision is not live. Annex III part 3 —
                // "if the supervisory function is not active, the machinery shall not be
                // able to operate".
                None => true,
            };
        GateContext { stopped: self.stopped, supervision_expired }
    }

    /// Decide on a **stop** order, and latch on acceptance.
    pub fn decide_stop(&mut self, input: &StopInput) -> StopDecision {
        let d = stop_decision(input);
        match d {
            StopDecision::Stop => {
                self.stopped = true;
                self.stops += 1;
            }
            StopDecision::Reject(r) => self.rejects[r as usize] += 1,
        }
        d
    }

    /// Clear the stop latch. ISO 13850:2015 4.1.1.2 requires a stop to be "maintained
    /// until it is manually reset" by "intentional human action", so this is reachable
    /// only from a local interface — **never** from a network order.
    pub fn clear_stop(&mut self) {
        self.stopped = false;
    }

    /// Apply a supervision beacon already verified by the caller (signature, `SUPERVISE`
    /// permission, not revoked, matching `boot_id`). Returns `false` if the beacon is a
    /// replay or out of order, in which case nothing changes.
    pub fn apply_beacon(&mut self, b: &SupervisionBeacon, now_ms: u64) -> bool {
        if let Some(last) = self.last_beacon_seq {
            if b.beacon_seq <= last {
                return false;
            }
        }
        self.last_beacon_seq = Some(b.beacon_seq);
        // Bounded, and saturating: a beacon claiming validity_ms near u64::MAX must not
        // wrap into the past.
        let granted = b.validity_ms.min(MAX_SUPERVISION_MS);
        self.supervision_until_ms = Some(now_ms.saturating_add(granted));
        true
    }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
