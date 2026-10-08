use super::*;

#[test]
fn tv_stamps_in_the_actuator_clock() {
    let v = TimeView::new(7, 1_000, 500);
    // 300 ms of local time elapsed: the actuator is estimated at 1_300.
    assert_eq!(v.estimate(800), Some(1_300));
    assert_eq!(v.stamp(800, 3_000), Some((7, 4_300)));
    assert_eq!(v.age_ms(800), Some(300));
}

#[test]
fn tv_refuses_a_stale_view() {
    let v = TimeView::new(7, 1_000, 0);
    assert!(v.estimate(MAX_VIEW_AGE_MS).is_some(), "exactly at the bound is still usable");
    assert_eq!(v.estimate(MAX_VIEW_AGE_MS + 1), None);
    assert_eq!(v.stamp(MAX_VIEW_AGE_MS + 1, 3_000), None);
}

#[test]
fn tv_refuses_a_local_clock_that_went_backwards() {
    let v = TimeView::new(7, 1_000, 500);
    assert_eq!(v.estimate(499), None);
    assert_eq!(v.stamp(499, 3_000), None);
    assert_eq!(v.age_ms(499), None);
}

/// The bound the gate enforces is checked here too, so a commander cannot build an order
/// it already knows will be refused.
#[test]
fn tv_refuses_a_validity_the_gate_would_refuse() {
    let v = TimeView::new(7, 1_000, 0);
    assert_eq!(v.stamp(0, 0), None, "zero validity");
    assert_eq!(v.stamp(0, MAX_VALIDITY_MS), Some((7, 1_000 + MAX_VALIDITY_MS)));
    assert_eq!(v.stamp(0, MAX_VALIDITY_MS + 1), None);
    assert_eq!(v.stamp(0, u64::MAX), None);
}

#[test]
fn tv_a_new_boot_replaces_the_view_and_a_replay_does_not() {
    let mut v = TimeView::new(7, 5_000, 100);
    // Same boot, clock does not advance: refused, nothing changes.
    assert!(!v.apply(7, 5_000, 200));
    assert!(!v.apply(7, 4_000, 200));
    assert_eq!(v, TimeView::new(7, 5_000, 100));
    // Same boot, clock advances: accepted.
    assert!(v.apply(7, 6_000, 300));
    assert_eq!(v, TimeView::new(7, 6_000, 300));
    // Different boot: replaces outright, even with a smaller now_ms — a reboot restarts
    // the actuator's millisecond counter, and learning that is the whole point.
    assert!(v.apply(8, 12, 400));
    assert_eq!(v, TimeView::new(8, 12, 400));
}

/// End to end against the real gate: a commander with a view stamps an order the gate
/// accepts, and the same order stamped from a view of the PREVIOUS boot is refused.
#[test]
fn tv_order_stamped_from_a_view_is_accepted_and_a_stale_boot_is_not() {
    use crate::actuation::{actuation_decision, Decision, GateInput, Reason};

    let actuator_boot = 42u64;
    let actuator_now = 100_000u64;
    // The commander received a beacon 250 ms of its own time ago.
    let v = TimeView::new(actuator_boot, actuator_now - 250, 1_000);
    let (boot, deadline) = v.stamp(1_250, 3_000).unwrap();
    assert_eq!(boot, actuator_boot);

    let gate = |cmd_boot: u64, deadline_ms: u64| GateInput {
        v0b_ok: true,
        authorized: true,
        revoked: false,
        cmd_boot_id: cmd_boot,
        deadline_ms,
        actuator_boot_id: actuator_boot,
        // The order arrives 40 ms later in the actuator's clock.
        now_ms: actuator_now + 40,
        r14_safe: true,
        within_limits: true,
        cmd_seq: 1,
        last_executed_seq: None,
    };
    assert_eq!(actuation_decision(&gate(boot, deadline)), Decision::Act);

    // A view from the previous boot: the gate refuses, and that is how the commander
    // learns to refresh.
    let old = TimeView::new(actuator_boot - 1, actuator_now - 250, 1_000);
    let (old_boot, old_deadline) = old.stamp(1_250, 3_000).unwrap();
    assert_eq!(actuation_decision(&gate(old_boot, old_deadline)), Decision::Reject(Reason::Expired));
}

/// Every error direction costs an order, never an unintended execution. An estimate that
/// is too low expires; one that is too high breaks the gate's `MAX_VALIDITY_MS` window.
#[test]
fn tv_estimation_errors_only_ever_refuse() {
    use crate::actuation::{actuation_decision, Decision, GateInput};

    let boot = 9u64;
    let real_now = 50_000u64;
    let gate = |deadline_ms: u64| GateInput {
        v0b_ok: true,
        authorized: true,
        revoked: false,
        cmd_boot_id: boot,
        deadline_ms,
        actuator_boot_id: boot,
        now_ms: real_now,
        r14_safe: true,
        within_limits: true,
        cmd_seq: 1,
        last_executed_seq: None,
    };
    // Estimate 20 s too low: the deadline is already past.
    let low = TimeView::new(boot, real_now - 20_000, 0);
    let (_, d_low) = low.stamp(0, 3_000).unwrap();
    assert!(d_low < real_now);
    assert_ne!(actuation_decision(&gate(d_low)), Decision::Act, "too low: expired");

    // Estimate 20 s too high: deadline - now exceeds MAX_VALIDITY_MS.
    let high = TimeView::new(boot, real_now + 20_000, 0);
    let (_, d_high) = high.stamp(0, 3_000).unwrap();
    assert!(d_high - real_now > MAX_VALIDITY_MS);
    assert_ne!(actuation_decision(&gate(d_high)), Decision::Act, "too high: outside the window");

    // And a correct estimate works, which is what makes the two above meaningful.
    let ok = TimeView::new(boot, real_now, 0);
    let (_, d_ok) = ok.stamp(0, 3_000).unwrap();
    assert_eq!(actuation_decision(&gate(d_ok)), Decision::Act);
}

#[test]
fn tv_otm1_roundtrip_feeds_a_view() {
    use crate::actuation::{encode_otm1, parse_otm1};
    let bytes = encode_otm1(77, 123_456);
    let (boot, now) = parse_otm1(&bytes).unwrap();
    let v = TimeView::new(boot, now, 10);
    assert_eq!(v.boot_id, 77);
    assert_eq!(v.estimate(20), Some(123_466));
    assert_eq!(bytes.len(), 20, "a beacon is 20 bytes of payload, 119 on the wire");
}
