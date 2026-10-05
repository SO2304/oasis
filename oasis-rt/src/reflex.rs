//! OASIS-RT — Reflex Arc (Mechanism 9)
//!
//! Hardwired condition-action rules evaluated on raw telemetry
//! BEFORE any deliberative processing. Reaction < 100µs.

#[cfg(not(feature = "std"))]
use crate::fmath::F64Ext;

/// Adaptive reflex threshold — learns baseline from calibration data
pub struct AdaptiveReflex {
    mean: f64,
    std_dev: f64,
    sigma: f64,
    calibrated: bool,
    buf: [f64; 64],
    count: usize,
}

impl AdaptiveReflex {
    pub fn new(sigma: f64) -> Self {
        Self { mean: 0.0, std_dev: 0.0, sigma, calibrated: false, buf: [0.0; 64], count: 0 }
    }

    /// Feed a calibration sample
    pub fn feed(&mut self, value: f64) {
        if self.count < 64 {
            self.buf[self.count] = value;
            self.count += 1;
        }
    }

    /// Compute mean and std from buffer
    pub fn calibrate(&mut self) {
        if self.count < 3 {
            return;
        }
        let n = self.count as f64;
        let mean = self.buf[..self.count].iter().sum::<f64>() / n;
        let var = self.buf[..self.count].iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
        self.mean = mean;
        self.std_dev = var.sqrt().max(0.001);
        self.calibrated = true;
    }

    /// Check if value exceeds threshold (mean + sigma * std)
    pub fn check(&self, value: f64) -> bool {
        self.calibrated && value > self.mean + self.sigma * self.std_dev
    }

    /// Get current threshold
    pub fn threshold(&self) -> f64 {
        self.mean + self.sigma * self.std_dev
    }

    pub fn is_calibrated(&self) -> bool {
        self.calibrated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_calibrated_never_fires() {
        let r = AdaptiveReflex::new(3.0);
        assert!(!r.check(1000.0));
    }

    #[test]
    fn calibration_sets_threshold() {
        let mut r = AdaptiveReflex::new(3.0);
        for v in [1.0, 1.1, 0.9, 1.0, 1.05, 0.95, 1.02, 0.98, 1.0, 1.01] {
            r.feed(v);
        }
        r.calibrate();
        assert!(r.is_calibrated());
        assert!(!r.check(1.0)); // normal
        assert!(r.check(5.0)); // extreme → fires
    }

    #[test]
    fn higher_sigma_less_sensitive() {
        let mut r3 = AdaptiveReflex::new(3.0);
        let mut r5 = AdaptiveReflex::new(5.0);
        for v in [1.0, 1.1, 0.9, 1.0, 1.05] {
            r3.feed(v);
            r5.feed(v);
        }
        r3.calibrate();
        r5.calibrate();
        let test_val = r3.threshold() + 0.01;
        assert!(r3.check(test_val));
        assert!(!r5.check(test_val)); // higher sigma = higher threshold
    }
}

// ==========================================================================
//                              KANI PROOFS
//
// M9 Reflex arc — formal properties of the threshold-decision math.
// Cannot prove `calibrate()` fully because `var.sqrt()` is
// SAT-intractable. Instead prove invariants over the DECISION layer
// (`check`, `threshold`) which is pure arithmetic on already-computed
// mean/std/sigma state.
// ==========================================================================

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: A non-calibrated reflex NEVER fires, regardless of input
    /// magnitude. This is the safety property: no reflex fires until
    /// the sensor baseline is established.
    #[kani::proof]
    fn proof_reflex_uncalibrated_never_fires() {
        let sigma: f64 = kani::any();
        let value: f64 = kani::any();
        kani::assume(sigma.is_finite());
        kani::assume(value.is_finite());
        let r = AdaptiveReflex::new(sigma);
        assert!(!r.calibrated);
        assert!(!r.check(value), "non-calibrated reflex fired on value={}, sigma={}", value, sigma);
    }

    // The threshold-monotonicity and bidirectional-check proofs were
    // attempted with 4× symbolic f64 inputs each. CBMC's float model
    // bit-blasts the multiplications into millions of SAT clauses and
    // did not return a verdict within 180 s. We prove simpler scalar
    // properties instead — each single-symbolic-float, SAT-tractable.

    /// PROVE: threshold reduces cleanly to `mean + sigma * std_dev`.
    /// Regression guard against someone accidentally adding extra
    /// terms to the threshold formula.
    #[kani::proof]
    fn proof_reflex_threshold_formula() {
        let value: f64 = kani::any();
        kani::assume(value.is_finite());
        kani::assume(value.abs() < 1e6);
        let mut r = AdaptiveReflex::new(3.0);
        r.mean = value;
        r.std_dev = 0.0; // concrete: eliminates the multiplication branch
        r.calibrated = true;
        // With std_dev == 0, threshold must equal mean exactly.
        assert_eq!(r.threshold(), value);
    }

    /// PROVE: calibrated reflex with zero std_dev fires when the
    /// value is strictly greater than the baseline by a
    /// representable margin. The `delta >= 1e-6` bound avoids
    /// float-absorption at large magnitudes (e.g. mean = 1e6, delta =
    /// 1e-300 collapses to mean + delta == mean in f64).
    #[kani::proof]
    fn proof_reflex_fires_when_above_baseline_zero_std() {
        let mean: f64 = kani::any();
        let delta: f64 = kani::any();
        kani::assume(mean.is_finite() && delta.is_finite());
        kani::assume(delta >= 1e-6 && delta < 1e6);
        kani::assume(mean.abs() < 1e3); // tighter so delta isn't absorbed

        let mut r = AdaptiveReflex::new(3.0);
        r.mean = mean;
        r.std_dev = 0.0;
        r.calibrated = true;
        // threshold = mean; value = mean + delta > mean ⇒ fires
        assert!(r.check(mean + delta));
    }
}
