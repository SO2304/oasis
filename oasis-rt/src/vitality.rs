//! OASIS-RT — Vitality System (bio-inspired graceful degradation)
//! A living organism doesn't die when one sensor fails.
//! It adapts, compensates, limps, and only dies when EVERYTHING is gone.
//!
//! Healthy → Degraded → Critical → Dead
//!
//! R14 threshold scales with vitality. Reflexes always fire.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec, vec::Vec};

/// How critical a sensor/actuator is to survival
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Vitality {
    Vital,     // IMU, Gyro — core self-localization
    Important, // Baro, Compass — reference frames
    Optional,  // GPS, Light, Steps — environmental context
}

/// System health level — drives R14 threshold and behavior gating
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VitalityLevel {
    Healthy,  // All vital sensors alive → full capability
    Degraded, // 1-2 vital dead → reduced decisions, no dreams
    Critical, // 3+ vital dead → reflexes only, zero actuation
    Dead,     // Unrecoverable → controlled shutdown
}

/// Tracks which organs are alive and computes system health
pub struct VitalityState {
    pub level: VitalityLevel,
    pub vital_alive: usize,
    pub vital_total: usize,
    pub important_alive: usize,
    pub important_total: usize,
    pub entropy_contribution: f64,
    pub ticks_in_critical: u32,
    critical_timeout: u32, // ticks in Critical before Dead
}

impl VitalityState {
    pub fn new() -> Self {
        Self {
            level: VitalityLevel::Healthy,
            vital_alive: 0,
            vital_total: 0,
            important_alive: 0,
            important_total: 0,
            entropy_contribution: 0.0,
            ticks_in_critical: 0,
            critical_timeout: 100, // 100 ticks (~5 min at 3s/tick) before death
        }
    }

    /// Recalculate vitality from a list of (alive, vitality) pairs
    pub fn update(&mut self, sensors: &[(bool, Vitality)]) {
        let mut v_alive = 0usize;
        let mut v_total = 0usize;
        let mut i_alive = 0usize;
        let mut i_total = 0usize;
        let mut entropy = 0.0_f64;

        for &(alive, vit) in sensors {
            match vit {
                Vitality::Vital => {
                    v_total += 1;
                    if alive {
                        v_alive += 1;
                    } else {
                        entropy += 0.30;
                    }
                }
                Vitality::Important => {
                    i_total += 1;
                    if alive {
                        i_alive += 1;
                    } else {
                        entropy += 0.15;
                    }
                }
                Vitality::Optional => {
                    if !alive {
                        entropy += 0.05;
                    }
                }
            }
        }

        self.vital_alive = v_alive;
        self.vital_total = v_total;
        self.important_alive = i_alive;
        self.important_total = i_total;
        self.entropy_contribution = entropy.min(1.0);

        let vital_dead = v_total.saturating_sub(v_alive);
        self.level = if vital_dead == 0 {
            self.ticks_in_critical = 0;
            VitalityLevel::Healthy
        } else if vital_dead <= 2 {
            self.ticks_in_critical = 0;
            VitalityLevel::Degraded
        } else {
            self.ticks_in_critical += 1;
            if self.ticks_in_critical >= self.critical_timeout {
                VitalityLevel::Dead
            } else {
                VitalityLevel::Critical
            }
        };
    }

    /// Adaptive R14 entropy threshold — lower = more conservative
    pub fn r14_threshold(&self) -> f64 {
        match self.level {
            VitalityLevel::Healthy => 0.85,
            VitalityLevel::Degraded => 0.60,
            VitalityLevel::Critical => 0.30,
            VitalityLevel::Dead => 0.0,
        }
    }

    /// Should the system shut down? Only on Dead.
    pub fn should_shutdown(&self) -> bool {
        self.level == VitalityLevel::Dead
    }

    /// Diagnostic string for logging
    pub fn diagnostic(&self) -> String {
        format!(
            "{:?} vital={}/{} imp={}/{} entropy={:.2} crit_ticks={}",
            self.level, self.vital_alive, self.vital_total, self.important_alive, self.important_total, self.entropy_contribution, self.ticks_in_critical
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_alive_is_healthy() {
        let mut v = VitalityState::new();
        v.update(&[(true, Vitality::Vital), (true, Vitality::Vital), (true, Vitality::Important), (true, Vitality::Optional)]);
        assert_eq!(v.level, VitalityLevel::Healthy);
        assert!((v.r14_threshold() - 0.85).abs() < 1e-10);
        assert!(!v.should_shutdown());
        assert!(v.entropy_contribution < 0.01);
    }

    #[test]
    fn one_vital_dead_is_degraded() {
        let mut v = VitalityState::new();
        v.update(&[(false, Vitality::Vital), (true, Vitality::Vital), (true, Vitality::Important)]);
        assert_eq!(v.level, VitalityLevel::Degraded);
        assert!((v.r14_threshold() - 0.60).abs() < 1e-10);
        assert!(!v.should_shutdown());
        assert!((v.entropy_contribution - 0.30).abs() < 1e-10);
    }

    #[test]
    fn three_vital_dead_is_critical() {
        let mut v = VitalityState::new();
        v.update(&[(false, Vitality::Vital), (false, Vitality::Vital), (false, Vitality::Vital), (true, Vitality::Optional)]);
        assert_eq!(v.level, VitalityLevel::Critical);
        assert!((v.r14_threshold() - 0.30).abs() < 1e-10);
        assert!(!v.should_shutdown()); // not Dead yet
    }

    #[test]
    fn critical_timeout_causes_death() {
        let mut v = VitalityState::new();
        v.critical_timeout = 5; // fast timeout for test
        let sensors = [(false, Vitality::Vital), (false, Vitality::Vital), (false, Vitality::Vital)];
        for _ in 0..4 {
            v.update(&sensors);
        }
        assert_eq!(v.level, VitalityLevel::Critical);
        v.update(&sensors); // 5th tick
        assert_eq!(v.level, VitalityLevel::Dead);
        assert!(v.should_shutdown());
    }

    #[test]
    fn recovery_from_critical() {
        let mut v = VitalityState::new();
        let dead = [(false, Vitality::Vital), (false, Vitality::Vital), (false, Vitality::Vital)];
        for _ in 0..3 {
            v.update(&dead);
        }
        assert_eq!(v.level, VitalityLevel::Critical);
        assert_eq!(v.ticks_in_critical, 3);
        // Sensors recover
        let alive = [(true, Vitality::Vital), (true, Vitality::Vital), (true, Vitality::Vital)];
        v.update(&alive);
        assert_eq!(v.level, VitalityLevel::Healthy);
        assert_eq!(v.ticks_in_critical, 0); // reset
    }

    #[test]
    fn optional_death_stays_healthy() {
        let mut v = VitalityState::new();
        v.update(&[(true, Vitality::Vital), (true, Vitality::Vital), (false, Vitality::Optional), (false, Vitality::Optional), (false, Vitality::Optional)]);
        assert_eq!(v.level, VitalityLevel::Healthy);
        assert!((v.entropy_contribution - 0.15).abs() < 1e-10); // 3 * 0.05
    }
}
