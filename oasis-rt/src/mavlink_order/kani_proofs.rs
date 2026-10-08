//! Kani proofs for B1 — the MAVLink carrier and the arming rule.
//!
//! The invariant worth proving is the same one the Modbus gateway proves, because it is
//! the same shape: **no ARM frame exists unless the gate returned `Act`**. The frame is
//! built in the `Act` branch and nowhere else, and these harnesses say so for every
//! input, not for the ones a test happened to pick.
//!
//! ⚠️ **Unwind bound.** The two harnesses that reach `encode_command_long` must unwind
//! `crc_over`'s loop over a 43-byte frame. With `unwind(2)` both came back
//! `VERIFICATION:- FAILED` on an *unwinding assertion* — which is CBMC saying "I could
//! not look far enough", not a counterexample. `unwind(64)` covers it. The distinction is
//! recorded here because the log of the first run says FAILED and that log is kept.
//!
//! `encode_command_long` allocates, so the harnesses check `frame.is_some()`, never the
//! bytes: what is proved is *whether* a frame exists, which is the safety-relevant half.
//! The bytes are covered by `tests.rs`, which reads them back with `parse_frame`.

use super::*;
use crate::actuation::Reason;

fn any_act_command() -> ActCommand {
    ActCommand {
        actuator_id: kani::any(),
        cmd_seq: kani::any(),
        boot_id: kani::any(),
        deadline_ms: kani::any(),
        force: kani::any(),
        torque: kani::any(),
        velocity: kani::any(),
        pos: [kani::any(), kani::any(), kani::any()],
    }
}

fn any_ctx() -> ArmContext {
    ArmContext {
        v0b_ok: kani::any(),
        authorized: kani::any(),
        revoked: kani::any(),
        actuator_boot_id: kani::any(),
        now_ms: kani::any(),
        r14_safe: kani::any(),
    }
}

fn any_class() -> OrderClass {
    if kani::any() {
        OrderClass::Act
    } else {
        OrderClass::Stop
    }
}

/// **The B1 invariant.** For every order, every actuator state and every class, a frame
/// exists if and only if the decision is `Act` — and then it is accompanied by exactly
/// one named action.
#[kani::proof]
#[kani::unwind(64)]
fn proof_no_arm_frame_without_act() {
    let gctx = GateContext { stopped: kani::any(), supervision_expired: kani::any() };
    let ctx = any_ctx();
    let o = any_act_command();
    let rules = ArmRules { vehicle_id: kani::any() };
    let last: Option<u32> = if kani::any() { Some(kani::any()) } else { None };
    let (d, action, frame) = arm_decision_ctx(&gctx, &ctx, &o, any_class(), &rules, last, kani::any(), kani::any(), kani::any(), kani::any(), kani::any());
    match d {
        Decision::Act => {
            assert!(frame.is_some(), "an Act must produce the command");
            assert!(action.is_some());
        }
        Decision::Reject(_) => {
            assert!(frame.is_none(), "a reject must produce nothing");
            assert!(action.is_none());
        }
    }
}

/// An `Act` decision implies every Part F condition held **and** the order named this
/// vehicle with a binary `force`. The gate is not weakened by the carrier.
#[kani::proof]
#[kani::unwind(64)]
fn proof_arm_implies_every_condition() {
    let gctx = GateContext { stopped: kani::any(), supervision_expired: kani::any() };
    let ctx = any_ctx();
    let o = any_act_command();
    let rules = ArmRules { vehicle_id: kani::any() };
    let last: Option<u32> = if kani::any() { Some(kani::any()) } else { None };
    let class = any_class();
    let (d, _, _) = arm_decision_ctx(&gctx, &ctx, &o, class, &rules, last, kani::any(), kani::any(), kani::any(), kani::any(), kani::any());
    if d == Decision::Act {
        assert!(class == OrderClass::Act);
        assert!(ctx.v0b_ok);
        assert!(ctx.authorized);
        assert!(!ctx.revoked);
        assert!(!gctx.stopped);
        assert!(!gctx.supervision_expired);
        assert!(ctx.r14_safe);
        assert!(o.boot_id == ctx.actuator_boot_id);
        assert!(o.actuator_id == rules.vehicle_id);
        assert!(action_of(&o, &rules).is_some());
        match last {
            Some(prev) => assert!(o.cmd_seq > prev),
            None => (),
        }
    }
}

/// A `Stop` is refused by this carrier whatever else is true, and the reason given is
/// the same one an out-of-limits order gets — it tells an attacker nothing new.
#[kani::proof]
#[kani::unwind(2)]
fn proof_stop_class_never_arms() {
    let gctx = GateContext { stopped: kani::any(), supervision_expired: kani::any() };
    let ctx = any_ctx();
    let o = any_act_command();
    let rules = ArmRules { vehicle_id: kani::any() };
    let last: Option<u32> = if kani::any() { Some(kani::any()) } else { None };
    let (d, action, frame) = arm_decision_ctx(&gctx, &ctx, &o, OrderClass::Stop, &rules, last, kani::any(), kani::any(), kani::any(), kani::any(), kani::any());
    assert!(d == Decision::Reject(Reason::OutOfLimits));
    assert!(action.is_none());
    assert!(frame.is_none());
}

/// `action_of` is total and binary: never both, and `Some` only for an exact 0.0 or 1.0
/// addressed to this vehicle. A NaN `force` is refused (NaN compares false everywhere).
#[kani::proof]
fn proof_action_is_total_and_binary() {
    let o = any_act_command();
    let rules = ArmRules { vehicle_id: kani::any() };
    match action_of(&o, &rules) {
        Some(ArmAction::Arm) => {
            assert!(o.force == 1.0);
            assert!(o.actuator_id == rules.vehicle_id);
        }
        Some(ArmAction::Disarm) => {
            assert!(o.force == 0.0);
            assert!(o.actuator_id == rules.vehicle_id);
        }
        None => assert!(o.actuator_id != rules.vehicle_id || (o.force != 1.0 && o.force != 0.0)),
    }
}
