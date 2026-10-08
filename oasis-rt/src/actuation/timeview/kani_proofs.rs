use super::*;

fn any_view() -> TimeView {
    TimeView { boot_id: kani::any(), beacon_now_ms: kani::any(), local_rx_ms: kani::any() }
}

/// PROVE: `stamp` is total — no overflow, no panic, for any view, any local clock and any
/// validity, including values at `u64::MAX`.
#[kani::proof]
fn proof_timeview_stamp_total() {
    let v = any_view();
    let _ = v.stamp(kani::any(), kani::any());
    let _ = v.estimate(kani::any());
    let _ = v.age_ms(kani::any());
}

/// PROVE (**the invariant that makes part K safe**): whenever `stamp` returns a deadline,
/// the window it opens is **at most** `MAX_VALIDITY_MS` wide measured from the commander's
/// own estimate, and the `boot_id` is the view's. A commander cannot build an order that
/// asks the gate for more time than the gate allows.
#[kani::proof]
fn proof_timeview_window_is_bounded() {
    let v = any_view();
    let local_now: u64 = kani::any();
    let validity: u64 = kani::any();
    if let Some((boot, deadline)) = v.stamp(local_now, validity) {
        assert!(boot == v.boot_id);
        let now = v.estimate(local_now).unwrap();
        assert!(deadline >= now, "a deadline is never in the past of the estimate");
        assert!(deadline - now <= MAX_VALIDITY_MS);
        assert!(validity >= 1);
    }
}

/// PROVE: a view is never extrapolated past `MAX_VIEW_AGE_MS`, and a local clock that went
/// backwards yields nothing rather than a wrapped estimate.
#[kani::proof]
fn proof_timeview_refuses_stale_and_backwards() {
    let v = any_view();
    let local_now: u64 = kani::any();
    match v.estimate(local_now) {
        Some(_) => {
            assert!(local_now >= v.local_rx_ms);
            assert!(local_now - v.local_rx_ms <= MAX_VIEW_AGE_MS);
        }
        None => {
            // Refused for exactly one of three reasons.
            let backwards = local_now < v.local_rx_ms;
            let stale = !backwards && local_now - v.local_rx_ms > MAX_VIEW_AGE_MS;
            let overflow = !backwards && !stale && v.beacon_now_ms.checked_add(local_now - v.local_rx_ms).is_none();
            assert!(backwards || stale || overflow);
        }
    }
}

/// PROVE: `apply` accepts a beacon from a different boot unconditionally, and within the
/// same boot only one that advances the clock. A replayed beacon changes nothing.
#[kani::proof]
fn proof_timeview_apply_is_monotone_within_a_boot() {
    let before = any_view();
    let mut v = before;
    let boot: u64 = kani::any();
    let now: u64 = kani::any();
    let rx: u64 = kani::any();
    let accepted = v.apply(boot, now, rx);
    if accepted {
        assert!(v.boot_id == boot);
        assert!(v.beacon_now_ms == now);
        assert!(v.local_rx_ms == rx);
        assert!(boot != before.boot_id || now > before.beacon_now_ms);
    } else {
        assert!(v == before, "a refused beacon changes nothing");
        assert!(boot == before.boot_id);
        assert!(now <= before.beacon_now_ms);
    }
}
