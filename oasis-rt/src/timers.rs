//! OASIS — Timer / executor primitive (rclcpp `create_timer` equivalent).
//!
//! Monotonic, tick-driven timer registry. No threads, no OS dependency.
//! The caller owns the clock: feed `tick(now_ms)` from whatever monotonic
//! source they have (std::time::Instant, Android SystemClock.elapsedRealtime,
//! embedded HAL counter). This keeps the module `no_std`-friendly and
//! fully deterministic for replay / simulation / Kani proofs.
//!
//! ## Semantics
//!
//! - `register(period_ms, callback)` returns a `TimerId`.
//! - `tick(now_ms)` fires every timer whose `next_fire_ms <= now_ms` in
//!   **stable id order** (FIFO by registration). Multiple missed periods
//!   are coalesced into a single call — we do NOT replay past ticks.
//! - Timers are deterministic: same sequence of `tick` inputs ⇒ same
//!   sequence of callbacks, regardless of process.
//! - No starvation: every registered timer with period ≤ (now - last_tick)
//!   will fire exactly once per `tick` call.
//!
//! ## Wire format
//!
//! None. This is a runtime primitive, not a network envelope.
//!
//! ## Kani-provable invariants
//!
//! 1. `next_fire_ms` is monotonically non-decreasing after a tick.
//! 2. After firing, `next_fire_ms - now_ms >= 0` (no immediate re-fire).
//! 3. `register` with period > 0 never panics for finite inputs.

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

pub type TimerId = u32;
pub type TimerCallback = fn();

pub struct Timer {
    pub id: TimerId,
    pub period_ms: u64,
    pub next_fire_ms: u64,
    pub callback: TimerCallback,
    pub fire_count: u64,
}

/// Pure per-timer tick: if `t.next_fire_ms <= now_ms`, fire once, advance
/// `next_fire_ms` to `now_ms + period_ms`, and return `true`. Otherwise
/// leave untouched and return `false`.
///
/// Split out from `TimerRegistry::tick` so the core scheduling invariants
/// are Kani-verifiable without allocating a container.
#[inline]
pub fn tick_one(t: &mut Timer, now_ms: u64) -> bool {
    if t.next_fire_ms <= now_ms {
        (t.callback)();
        t.fire_count = t.fire_count.saturating_add(1);
        t.next_fire_ms = now_ms.saturating_add(t.period_ms);
        true
    } else {
        false
    }
}

/// BTreeMap keeps iteration in id order — giving us stable FIFO fire order
/// at zero extra cost. Also avoids the HashMap random seed, which Kani
/// cannot symbolically evaluate (getrandom loop explodes the solver).
pub struct TimerRegistry {
    timers: BTreeMap<TimerId, Timer>,
    next_id: TimerId,
    last_tick_ms: u64,
}

impl Default for TimerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TimerRegistry {
    pub fn new() -> Self {
        Self { timers: BTreeMap::new(), next_id: 0, last_tick_ms: 0 }
    }

    /// Register a timer that fires every `period_ms` starting at
    /// `last_tick_ms + period_ms`. Period must be > 0.
    pub fn register(&mut self, period_ms: u64, callback: TimerCallback) -> Option<TimerId> {
        if period_ms == 0 {
            return None;
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let t = Timer { id, period_ms, next_fire_ms: self.last_tick_ms.saturating_add(period_ms), callback, fire_count: 0 };
        self.timers.insert(id, t);
        Some(id)
    }

    pub fn unregister(&mut self, id: TimerId) -> bool {
        self.timers.remove(&id).is_some()
    }

    pub fn len(&self) -> usize {
        self.timers.len()
    }
    pub fn is_empty(&self) -> bool {
        self.timers.is_empty()
    }

    /// Advance the clock to `now_ms` and fire every due timer.
    ///
    /// Returns the number of callbacks invoked. Firing is in stable id order
    /// (FIFO). Missed periods are coalesced — if a timer has period 100 ms
    /// and 500 ms have elapsed, it fires once, not five times.
    pub fn tick(&mut self, now_ms: u64) -> u32 {
        if now_ms < self.last_tick_ms {
            // Clock went backwards — treat as no-op rather than firing everything.
            return 0;
        }
        self.last_tick_ms = now_ms;

        // BTreeMap iterates in key order → stable FIFO.
        let mut fired = 0u32;
        for t in self.timers.values_mut() {
            if tick_one(t, now_ms) {
                fired += 1;
            }
        }
        fired
    }

    /// Peek the soonest fire time across all timers. `None` if empty.
    pub fn next_fire_ms(&self) -> Option<u64> {
        self.timers.values().map(|t| t.next_fire_ms).min()
    }

    /// Total fires across all timers. Useful for tests and telemetry.
    pub fn total_fires(&self) -> u64 {
        self.timers.values().map(|t| t.fire_count).sum()
    }
}

// ==========================================================================
//                                TESTS
// ==========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn noop() {}

    #[test]
    fn register_returns_id_and_length_grows() {
        let mut r = TimerRegistry::new();
        let id1 = r.register(100, noop).unwrap();
        let id2 = r.register(200, noop).unwrap();
        assert_ne!(id1, id2);
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn register_rejects_zero_period() {
        let mut r = TimerRegistry::new();
        assert!(r.register(0, noop).is_none());
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn tick_fires_due_timers() {
        let mut r = TimerRegistry::new();
        let a = r.register(100, noop).unwrap();
        let b = r.register(250, noop).unwrap();
        assert_eq!(r.tick(99), 0);
        assert_eq!(r.timers[&a].fire_count, 0);
        assert_eq!(r.tick(100), 1);
        assert_eq!(r.timers[&a].fire_count, 1);
        assert_eq!(r.timers[&b].fire_count, 0);
        assert_eq!(r.tick(260), 2);
        assert_eq!(r.timers[&a].fire_count, 2);
        assert_eq!(r.timers[&b].fire_count, 1);
    }

    #[test]
    fn tick_coalesces_missed_periods() {
        let mut r = TimerRegistry::new();
        let id = r.register(100, noop).unwrap();
        // 500 ms elapse without any tick. Should fire exactly once, not 5×.
        assert_eq!(r.tick(500), 1);
        assert_eq!(r.timers[&id].fire_count, 1);
    }

    #[test]
    fn tick_backwards_is_noop() {
        let mut r = TimerRegistry::new();
        let id = r.register(100, noop).unwrap();
        r.tick(200);
        assert_eq!(r.timers[&id].fire_count, 1);
        // Clock skewed back — must NOT re-fire.
        assert_eq!(r.tick(150), 0);
        assert_eq!(r.timers[&id].fire_count, 1);
    }

    #[test]
    fn unregister_removes_timer() {
        let mut r = TimerRegistry::new();
        let id = r.register(100, noop).unwrap();
        assert!(r.unregister(id));
        assert_eq!(r.len(), 0);
        assert!(!r.unregister(id));
    }

    #[test]
    fn next_fire_ms_returns_minimum() {
        let mut r = TimerRegistry::new();
        r.register(500, noop).unwrap();
        r.register(100, noop).unwrap();
        assert_eq!(r.next_fire_ms(), Some(100));
    }

    #[test]
    fn next_fire_ms_none_when_empty() {
        let r = TimerRegistry::new();
        assert_eq!(r.next_fire_ms(), None);
    }

    #[test]
    fn fire_count_accumulates() {
        let mut r = TimerRegistry::new();
        let id = r.register(10, noop).unwrap();
        for now in 1..=100u64 {
            r.tick(now);
        }
        assert_eq!(r.timers[&id].fire_count, 10);
        assert_eq!(r.total_fires(), 10);
    }

    #[test]
    fn stable_order_across_ticks() {
        // Same registration order ⇒ same fire order, independent of HashMap.
        let mut r = TimerRegistry::new();
        let id1 = r.register(100, noop).unwrap();
        let id2 = r.register(100, noop).unwrap();
        assert!(id1 < id2);
        // Both due at the same time — must fire.
        assert_eq!(r.tick(200), 2);
        assert_eq!(r.timers[&id1].fire_count, 1);
        assert_eq!(r.timers[&id2].fire_count, 1);
    }

    #[test]
    fn bench_tick_latency() {
        let mut r = TimerRegistry::new();
        for _ in 0..10 {
            r.register(100, noop).unwrap();
        }
        const N: u32 = 50_000;
        let start = std::time::Instant::now();
        for i in 0..N {
            r.tick(i as u64);
        }
        let per_ns = start.elapsed().as_nanos() as f64 / N as f64;
        eprintln!("timer tick (10 timers, no fires): {:.0} ns/op", per_ns);
        assert!(per_ns < 5_000.0, "tick too slow: {} ns", per_ns);
    }
}

// ==========================================================================
//                              KANI PROOFS
// ==========================================================================

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    fn noop() {}

    fn mk(period: u64, next: u64) -> Timer {
        Timer { id: 0, period_ms: period, next_fire_ms: next, callback: noop, fire_count: 0 }
    }

    /// PROVE: tick_one does NOT fire when now < next_fire_ms.
    #[kani::proof]
    fn proof_timers_not_due_does_not_fire() {
        let now: u64 = kani::any();
        let next: u64 = kani::any();
        let period: u64 = kani::any();
        kani::assume(now < next);
        kani::assume(period > 0 && period < 1_000_000);

        let mut t = mk(period, next);
        let fired = tick_one(&mut t, now);
        assert!(!fired);
        assert_eq!(t.fire_count, 0);
        // next_fire_ms unchanged.
        assert_eq!(t.next_fire_ms, next);
    }

    /// PROVE: tick_one fires when now >= next_fire_ms, increments fire_count,
    /// and re-arms next_fire_ms to exactly now + period.
    #[kani::proof]
    fn proof_timers_due_fires_and_rearms() {
        let now: u64 = kani::any();
        let next: u64 = kani::any();
        let period: u64 = kani::any();
        kani::assume(next <= now);
        kani::assume(period > 0 && period < 1_000_000);
        kani::assume(now < u64::MAX - period);

        let mut t = mk(period, next);
        let fired = tick_one(&mut t, now);
        assert!(fired);
        assert_eq!(t.fire_count, 1);
        assert_eq!(t.next_fire_ms, now + period);
    }

    /// PROVE: after firing, next_fire_ms is strictly greater than now
    /// (no immediate re-fire on the same tick).
    #[kani::proof]
    fn proof_timers_no_immediate_refire() {
        let now: u64 = kani::any();
        let next: u64 = kani::any();
        let period: u64 = kani::any();
        kani::assume(next <= now);
        kani::assume(period > 0 && period < 1_000_000);
        kani::assume(now < u64::MAX - period);

        let mut t = mk(period, next);
        let _ = tick_one(&mut t, now);
        assert!(t.next_fire_ms > now);
        // Second call on the same `now` must NOT fire.
        let fired2 = tick_one(&mut t, now);
        assert!(!fired2);
    }

    /// PROVE: tick_one coalesces missed periods — fires exactly once even
    /// if many periods have elapsed since next_fire_ms.
    #[kani::proof]
    fn proof_timers_coalesces_missed_periods() {
        let period: u64 = kani::any();
        let skew: u64 = kani::any();
        kani::assume(period > 0 && period < 1_000);
        kani::assume(skew > period && skew < 1_000_000);

        let mut t = mk(period, 0);
        let now = skew;
        let fired = tick_one(&mut t, now);
        assert!(fired);
        assert_eq!(t.fire_count, 1); // not 5, not 10 — exactly 1
        assert_eq!(t.next_fire_ms, now + period);
    }

    /// PROVE: fire_count saturates at u64::MAX rather than wrapping.
    #[kani::proof]
    fn proof_timers_fire_count_saturates() {
        let period: u64 = kani::any();
        kani::assume(period > 0 && period < 1_000_000);
        let mut t = mk(period, 0);
        t.fire_count = u64::MAX;
        let _ = tick_one(&mut t, period);
        // saturating_add keeps it at MAX, never wraps to 0.
        assert_eq!(t.fire_count, u64::MAX);
    }
}
