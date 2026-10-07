use super::*;

/// PROVE: the token bucket never hands out more than `burst` tokens before any refill,
/// and `try_take` only succeeds when a whole token is available. Bounded loop.
#[kani::proof]
#[kani::unwind(30)]
fn proof_budget_never_exceeds_burst() {
    let rate: u64 = kani::any();
    let burst: u64 = kani::any();
    kani::assume(rate <= 1000 && burst >= 1 && burst <= 24);
    let now: u64 = kani::any(); // one fixed instant: no refill happens across the loop
    let mut b = LinkBudget::new(rate, burst);
    let mut granted: u64 = 0;
    let mut i = 0u64;
    while i < burst + 2 {
        if b.try_take(now) {
            granted += 1;
        }
        i += 1;
    }
    // At a single instant the bucket starts full (burst) and never refills, so it grants
    // exactly `burst`, never more.
    assert!(granted == burst);
}

/// PROVE: refilling is monotone and capped — after any elapsed time the available tokens
/// never exceed `burst`, and a backwards clock never increases them.
#[kani::proof]
fn proof_budget_refill_capped() {
    let rate: u64 = kani::any();
    let burst: u64 = kani::any();
    kani::assume(rate <= 1000 && burst >= 1 && burst <= 1000);
    let t0: u64 = kani::any();
    let t1: u64 = kani::any();
    let mut b = LinkBudget::new(rate, burst);
    // spend down to (near) empty is not needed; just exercise refill at two instants.
    let _ = b.try_take(t0);
    let before = b.tokens();
    let _ = b.try_take(t1);
    assert!(b.tokens() <= burst);
    if t1 <= t0 {
        // backwards or equal clock: no tokens were added (a take may have removed one).
        assert!(b.tokens() <= before);
    }
}
