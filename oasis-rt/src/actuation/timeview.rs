//! OASIS — a commander's view of an actuator's clock (part K of
//! `docs/AUTHORITY_HARDENING_SPEC.md`, closing `POSITIONING_GAPS.md` **C4**).
//!
//! # The problem C4 names
//!
//! The freshness guarantee of part F is stated in the **actuator's** time base: an order
//! carries `boot_id` and `deadline_ms`, and the gate refuses it unless `cmd_boot_id ==
//! actuator_boot_id` and `now_ms <= deadline_ms <= now_ms + MAX_VALIDITY_MS`. On silicon
//! up to 2026-10-08 the PC read those values **over USB** before every order. That is a
//! laboratory shortcut: in the field the commander has no cable.
//!
//! # The mechanism
//!
//! The actuator originates a signed `OTM1` time beacon (20 bytes of payload, 119 on the
//! wire) carrying `(boot_id, now_ms)`. A commander keeps a [`TimeView`]: the beacon's
//! values plus **its own** monotonic time at reception. To stamp an order it adds the
//! elapsed local time to the beacon's `now_ms`.
//!
//! The beacon is authenticated by the v0B envelope it travels in — origin, counter,
//! network and payload digest are all bound — so a forged or replayed beacon is refused by
//! the mesh before this module sees it. [`TimeView::apply`] adds one rule of its own:
//! within the same boot, a beacon whose `now_ms` goes **backwards** is refused.
//!
//! # Every error direction is the safe one
//!
//! This is the property that makes the mechanism acceptable rather than merely convenient.
//! If the commander's estimate of the actuator's clock is **too low**, the deadline is too
//! early and the order **expires**. If it is **too high**, `deadline - now` exceeds
//! `MAX_VALIDITY_MS` at the actuator and the gate refuses it as `Expired`. If the actuator
//! has rebooted, the `boot_id` no longer matches and the gate refuses. A stale or wrong
//! view therefore costs an order, never an unintended execution — and the commander learns
//! to refresh from the refusal.
//!
//! # Cost in radio messages
//!
//! One beacon is 119 bytes on the wire: **≈ 4,5 s at SF12** in raw LoRa, so 8 per hour and
//! per band out of the 36 s duty-cycle budget (`§J.2`). The RP2040 core clock was measured
//! at 124.9986 MHz against 125 MHz nominal — **1,1 × 10⁻⁵** — so an hour of extrapolation
//! drifts about **40 ms**, negligible against `MAX_VALIDITY_MS` = 10 s. One beacon per hour
//! is therefore ample for the clock; what forces a beacon is a **reboot**, which changes
//! `boot_id`. Hence the rule: **emit a beacon at boot**, then periodically.

use super::MAX_VALIDITY_MS;

/// How long a view may be extrapolated before it is refused.
///
/// Drift is not the reason — it is 40 ms per hour. The reason is that beyond this the
/// probability that the actuator rebooted unnoticed stops being negligible, and an order
/// stamped for a dead boot is a wasted radio slot. One hour.
pub const MAX_VIEW_AGE_MS: u64 = 3_600_000;

/// A commander's view of one actuator's clock, built from a signed `OTM1` beacon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeView {
    pub boot_id: u64,
    /// The actuator's `now_ms` as the beacon reported it.
    pub beacon_now_ms: u64,
    /// The commander's **own** monotonic time when it accepted that beacon.
    pub local_rx_ms: u64,
}

impl TimeView {
    pub fn new(boot_id: u64, beacon_now_ms: u64, local_rx_ms: u64) -> Self {
        TimeView { boot_id, beacon_now_ms, local_rx_ms }
    }

    /// Accept a beacon. A beacon from a **different boot** replaces the view outright: a
    /// reboot is exactly what the commander needs to learn. Within the same boot, a beacon
    /// whose `now_ms` does not advance is refused and changes nothing.
    pub fn apply(&mut self, boot_id: u64, beacon_now_ms: u64, local_rx_ms: u64) -> bool {
        if boot_id == self.boot_id && beacon_now_ms <= self.beacon_now_ms {
            return false;
        }
        *self = TimeView::new(boot_id, beacon_now_ms, local_rx_ms);
        true
    }

    /// The actuator's clock as estimated now, or `None` if the local clock went backwards
    /// or the view is older than [`MAX_VIEW_AGE_MS`].
    pub fn estimate(&self, local_now_ms: u64) -> Option<u64> {
        let elapsed = local_now_ms.checked_sub(self.local_rx_ms)?;
        if elapsed > MAX_VIEW_AGE_MS {
            return None;
        }
        self.beacon_now_ms.checked_add(elapsed)
    }

    /// Stamp an order: `(boot_id, deadline_ms)` in the actuator's time base.
    ///
    /// `None` if the view cannot be extrapolated, or if `validity_ms` is zero or beyond
    /// [`MAX_VALIDITY_MS`] — the bound the gate itself enforces, checked here so a
    /// commander cannot build an order it knows will be refused.
    pub fn stamp(&self, local_now_ms: u64, validity_ms: u64) -> Option<(u64, u64)> {
        if validity_ms == 0 || validity_ms > MAX_VALIDITY_MS {
            return None;
        }
        let now = self.estimate(local_now_ms)?;
        let deadline = now.checked_add(validity_ms)?;
        Some((self.boot_id, deadline))
    }

    /// Age of the view in the commander's own clock, for diagnostics.
    pub fn age_ms(&self, local_now_ms: u64) -> Option<u64> {
        local_now_ms.checked_sub(self.local_rx_ms)
    }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
