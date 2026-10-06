use super::*;

fn any_input() -> GateInput {
    GateInput {
        v0b_ok: kani::any(),
        authorized: kani::any(),
        revoked: kani::any(),
        cmd_boot_id: kani::any(),
        deadline_ms: kani::any(),
        actuator_boot_id: kani::any(),
        now_ms: kani::any(),
        r14_safe: kani::any(),
        within_limits: kani::any(),
        cmd_seq: kani::any(),
        last_executed_seq: if kani::any() { Some(kani::any()) } else { None },
    }
}

/// PROVE: `Act` is returned only if ALL seven conditions hold.
#[kani::proof]
fn proof_act_requires_all_conditions() {
    let i = any_input();
    if actuation_decision(&i) == Decision::Act {
        assert!(i.v0b_ok);
        assert!(i.authorized);
        assert!(!i.revoked);
        assert!(i.cmd_boot_id == i.actuator_boot_id);
        assert!(i.now_ms <= i.deadline_ms);
        assert!(i.deadline_ms - i.now_ms <= MAX_VALIDITY_MS);
        assert!(i.r14_safe);
        assert!(i.within_limits);
        if let Some(last) = i.last_executed_seq {
            assert!(i.cmd_seq > last);
        }
    }
}

/// PROVE: R14 unfavourable implies no action, whatever the other inputs.
#[kani::proof]
fn proof_r14_unsafe_never_acts() {
    let mut i = any_input();
    i.r14_safe = false;
    assert!(actuation_decision(&i) != Decision::Act);
}

/// PROVE: the decision is total (never panics) and deterministic.
#[kani::proof]
fn proof_decision_total_and_deterministic() {
    let i = any_input();
    assert!(actuation_decision(&i) == actuation_decision(&i));
}
