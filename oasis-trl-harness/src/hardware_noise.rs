//! AJ — Hardware physical-layer noise model for OASIS soak harness.
//!
//! Up through AI round, the harness modeled clean DIGITAL noise:
//! Gaussian sensor drift, Gilbert-Elliott packet loss, adversary
//! injection. The PHYSICAL hardware reality on a real drone has
//! additional perturbations the simulator must approximate:
//!
//!   1. EMI (electromagnetic interference) — periodic bit-flip
//!      bursts on the radio path, typically caused by motor PWM
//!      transients coupling into the antenna or BLDC ESC harmonics.
//!      Manifests as Drop("bad sig") / Drop("bad MAC") on the mesh.
//!
//!   2. TAMP0 voltage drop — the secure-element's tamper-detect pin
//!      can fire SPURIOUSLY when supply rail dips below ~2.8V during
//!      high-current motor draw. False tamper = node atomization.
//!      Costly false positive: the fleet loses a legitimate node.
//!
//!   3. I2C clock-stretch from motor vibration — physical shock on
//!      the SE chip's I2C clock line corrupts timing. Each SE access
//!      (sign / verify) has a small chance of clock-stretch failure,
//!      requiring retry or causing the signing operation to drop.
//!
//!   4. Brownout — momentary supply drops below the SE's POR
//!      threshold reset the volatile tx_counter. After brownout,
//!      the mesh router's monotonic counter starts over at 0,
//!      risking msg_id collisions with in-flight envelopes from
//!      before the brownout.
//!
//! Each perturbation has a per-tick probability + severity. Sane
//! defaults match published characteristics of typical UAV BLDC
//! electronics (Pixhawk/Holybro class).
//!
//! HONEST scope: this is STILL software simulation. It approximates
//! the EFFECT of hardware perturbations on the mesh layer + SE
//! interactions, not the underlying physics. Real hardware-in-the-loop
//! (HIL) remains the next TRL gate. But this round closes the gap
//! between "perfect digital noise only" and "drone-realistic stack
//! perturbations" — meaningful progress toward TRL 6-hardware.

use crate::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareEvent {
    /// EMI corrupted an envelope's bytes. The mesh layer's MAC/sig
    /// verification will reject it as Drop("bad MAC") / Drop("bad sig").
    EmiBitFlip,
    /// TAMP0 false-positive tamper event. In production, this would
    /// trigger SE self-wipe → node atomization → operator-driven
    /// revocation cascade. False alarm cost: legitimate node lost.
    Tamp0FalseAlarm,
    /// I2C clock-stretch from motor vibration. The SE sign() call
    /// fails (timeout). Caller must retry or skip this envelope.
    I2cClockStretch,
    /// Brownout reset. tx_counter lost; on restart, msg_id sequence
    /// restarts from 1. Without persistent tx_counter snapshot, msg_id
    /// collisions become possible. (Already documented in mesh.rs.)
    BrownoutTxCounterLoss,
}

/// Hardware perturbation model. Each tick, call `tick()` to roll
/// the dice; returns a list of any events that fired. Operators
/// configure rates to match their target deployment's electrical
/// environment.
#[derive(Debug, Clone)]
pub struct HardwareNoiseModel {
    /// Per-envelope probability that EMI corrupts at least one byte.
    /// Typical UAV with quality antenna + RC filtering: 0.001 (1 in 1000).
    /// Heavy EMI environment (cheap ESCs, no filtering): 0.05 (1 in 20).
    pub emi_per_envelope_prob: f64,
    /// Per-tick probability that TAMP0 fires spuriously. Voltage
    /// transients during motor spin-up are the main culprit. Sane
    /// default: 1 / 100 000 ticks (≈ once per 28 virtual hours at 1 Hz).
    /// Properly-filtered supply: 1 / 1 000 000 ticks.
    pub tamp0_false_alarm_per_tick_prob: f64,
    /// Per-SE-access probability that I2C clock-stretch fails the
    /// transaction. Typical: 0.001 per access. Bad mechanical mount
    /// near a motor: 0.05.
    pub i2c_clock_stretch_per_access_prob: f64,
    /// Per-tick probability of brownout. UAV with good battery +
    /// brown-out protection: 1 / 500 000 ticks. Without protection:
    /// 1 / 10 000 ticks during max-throttle maneuvers.
    pub brownout_per_tick_prob: f64,
    /// Telemetry counters (incremented every time an event fires).
    pub emi_events_total: u64,
    pub tamp0_false_alarms_total: u64,
    pub i2c_clock_stretch_total: u64,
    pub brownout_events_total: u64,
}

impl HardwareNoiseModel {
    /// Realistic-drone defaults (a Holybro X500 V2 with name-brand ESCs,
    /// quality antenna placement, moderate filtering). Numbers
    /// approximated from public failure-mode literature on UAV
    /// avionics; specific values are conservative engineering
    /// estimates, NOT measured on real hardware.
    pub fn realistic_drone() -> Self {
        Self {
            emi_per_envelope_prob:             0.005,         // 1 in 200 envelopes corrupted
            tamp0_false_alarm_per_tick_prob:   1.0 / 200_000.0, // ~once per 56 virtual hours
            i2c_clock_stretch_per_access_prob: 0.002,         // 1 in 500 SE accesses fails
            brownout_per_tick_prob:            1.0 / 500_000.0, // ~once per 139 virtual hours
            emi_events_total: 0,
            tamp0_false_alarms_total: 0,
            i2c_clock_stretch_total: 0,
            brownout_events_total: 0,
        }
    }

    /// Harsh environment: high-EMI ESCs, no chassis vibration isolation,
    /// poor power filtering. Stress test for the OASIS stack.
    pub fn harsh_environment() -> Self {
        Self {
            emi_per_envelope_prob:             0.05,           // 1 in 20 envelopes
            tamp0_false_alarm_per_tick_prob:   1.0 / 20_000.0, // ~once per 5.5 virtual hours
            i2c_clock_stretch_per_access_prob: 0.02,           // 1 in 50 SE accesses fail
            brownout_per_tick_prob:            1.0 / 50_000.0, // ~once per 13.9 virtual hours
            emi_events_total: 0,
            tamp0_false_alarms_total: 0,
            i2c_clock_stretch_total: 0,
            brownout_events_total: 0,
        }
    }

    /// Disabled (legacy harness behavior — no hardware noise).
    pub fn disabled() -> Self {
        Self {
            emi_per_envelope_prob: 0.0,
            tamp0_false_alarm_per_tick_prob: 0.0,
            i2c_clock_stretch_per_access_prob: 0.0,
            brownout_per_tick_prob: 0.0,
            emi_events_total: 0,
            tamp0_false_alarms_total: 0,
            i2c_clock_stretch_total: 0,
            brownout_events_total: 0,
        }
    }

    /// Roll for tamper + brownout events that fire on the TICK clock.
    /// Returns a Vec of any that fired. EMI + I2C events are rolled
    /// separately via per-access methods below.
    pub fn tick(&mut self, rng: &mut Rng) -> Vec<HardwareEvent> {
        let mut events = Vec::new();
        if rng.next_f64() < self.tamp0_false_alarm_per_tick_prob {
            self.tamp0_false_alarms_total += 1;
            events.push(HardwareEvent::Tamp0FalseAlarm);
        }
        if rng.next_f64() < self.brownout_per_tick_prob {
            self.brownout_events_total += 1;
            events.push(HardwareEvent::BrownoutTxCounterLoss);
        }
        events
    }

    /// Roll for EMI on one envelope. If true, the caller should corrupt
    /// the envelope (e.g., flip one random byte) before feeding to the
    /// receiver — the receiver's MAC/sig verify will reject it.
    pub fn roll_emi(&mut self, rng: &mut Rng) -> bool {
        if rng.next_f64() < self.emi_per_envelope_prob {
            self.emi_events_total += 1;
            true
        } else { false }
    }

    /// Roll for I2C clock-stretch on one SE access. If true, the caller
    /// should treat this SE access as having failed (skip the operation
    /// or retry next tick).
    pub fn roll_i2c_clock_stretch(&mut self, rng: &mut Rng) -> bool {
        if rng.next_f64() < self.i2c_clock_stretch_per_access_prob {
            self.i2c_clock_stretch_total += 1;
            true
        } else { false }
    }

    /// Total events of any class.
    pub fn total_events(&self) -> u64 {
        self.emi_events_total + self.tamp0_false_alarms_total
            + self.i2c_clock_stretch_total + self.brownout_events_total
    }
}

/// Helper: corrupt one random byte of an envelope (simulates EMI
/// bit-flip; mesh MAC/sig verify will reject).
pub fn corrupt_envelope(envelope: &mut [u8], rng: &mut Rng) {
    if envelope.is_empty() { return; }
    let byte_idx = (rng.next_u64() as usize) % envelope.len();
    let bit_idx = (rng.next_u64() as u8) & 7;
    envelope[byte_idx] ^= 1 << bit_idx;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realistic_drone_defaults_are_nonzero() {
        let m = HardwareNoiseModel::realistic_drone();
        assert!(m.emi_per_envelope_prob > 0.0);
        assert!(m.tamp0_false_alarm_per_tick_prob > 0.0);
        assert!(m.i2c_clock_stretch_per_access_prob > 0.0);
        assert!(m.brownout_per_tick_prob > 0.0);
    }

    #[test]
    fn disabled_model_never_fires() {
        let mut m = HardwareNoiseModel::disabled();
        let mut rng = Rng::new(42);
        for _ in 0..10_000 {
            assert!(m.tick(&mut rng).is_empty());
            assert!(!m.roll_emi(&mut rng));
            assert!(!m.roll_i2c_clock_stretch(&mut rng));
        }
        assert_eq!(m.total_events(), 0);
    }

    #[test]
    fn realistic_drone_rates_within_band() {
        let mut m = HardwareNoiseModel::realistic_drone();
        let mut rng = Rng::new(7);
        const N: u64 = 100_000;
        let mut tick_events = 0u64;
        for _ in 0..N {
            tick_events += m.tick(&mut rng).len() as u64;
        }
        let mut emi_events = 0u64;
        for _ in 0..N {
            if m.roll_emi(&mut rng) { emi_events += 1; }
        }
        // EMI expected: 100_000 × 0.005 = 500 ± 100 (3σ)
        assert!(emi_events > 350 && emi_events < 650,
            "EMI rate out of band: {} (expected ~500)", emi_events);
        // Tick events (tamp0 + brownout): 100_000 × (1/200_000 + 1/500_000)
        // = 0.5 + 0.2 = 0.7 → expect 0-3 over 100k ticks
        assert!(tick_events < 10,
            "tick events too frequent: {}", tick_events);
    }

    #[test]
    fn harsh_environment_higher_rates() {
        let realistic = HardwareNoiseModel::realistic_drone();
        let harsh = HardwareNoiseModel::harsh_environment();
        assert!(harsh.emi_per_envelope_prob > realistic.emi_per_envelope_prob);
        assert!(harsh.tamp0_false_alarm_per_tick_prob > realistic.tamp0_false_alarm_per_tick_prob);
        assert!(harsh.i2c_clock_stretch_per_access_prob > realistic.i2c_clock_stretch_per_access_prob);
        assert!(harsh.brownout_per_tick_prob > realistic.brownout_per_tick_prob);
    }

    #[test]
    fn corrupt_envelope_changes_bytes() {
        let mut env = vec![0u8; 64];
        let mut rng = Rng::new(13);
        corrupt_envelope(&mut env, &mut rng);
        let any_nonzero = env.iter().any(|&b| b != 0);
        assert!(any_nonzero, "corrupt_envelope must flip at least one bit");
    }

    #[test]
    fn corrupt_envelope_empty_is_safe() {
        let mut env: Vec<u8> = vec![];
        let mut rng = Rng::new(13);
        corrupt_envelope(&mut env, &mut rng);   // must not panic
    }

    #[test]
    fn counters_increment_on_events() {
        let mut m = HardwareNoiseModel {
            emi_per_envelope_prob: 1.0,            // always
            tamp0_false_alarm_per_tick_prob: 1.0,
            i2c_clock_stretch_per_access_prob: 1.0,
            brownout_per_tick_prob: 1.0,
            ..HardwareNoiseModel::disabled()
        };
        let mut rng = Rng::new(99);
        for _ in 0..5 {
            assert!(m.roll_emi(&mut rng));
            assert!(m.roll_i2c_clock_stretch(&mut rng));
            let evts = m.tick(&mut rng);
            assert!(evts.contains(&HardwareEvent::Tamp0FalseAlarm));
            assert!(evts.contains(&HardwareEvent::BrownoutTxCounterLoss));
        }
        assert_eq!(m.emi_events_total, 5);
        assert_eq!(m.i2c_clock_stretch_total, 5);
        assert_eq!(m.tamp0_false_alarms_total, 5);
        assert_eq!(m.brownout_events_total, 5);
        assert_eq!(m.total_events(), 20);
    }
}
