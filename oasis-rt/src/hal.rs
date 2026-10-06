//! OASIS-RT — Hardware Abstraction Layer
//!
//! Tension-native HAL: drivers communicate via the tension field.
//! Kill switch = entropy spike (R14/R15). Efference-integrated.
//! Migrated from TypeScript kernel/hal/ — only what's pertinent.
//!
//! Safety: force/torque/velocity clamping (R9), geofencing, kill switch.

use crate::vec::*;
#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec, vec::Vec};
#[cfg(not(feature = "std"))]
use core::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "std")]
use std::sync::atomic::{AtomicBool, Ordering};

// ─── Physical Constraints (R9) ──────────────────────────────

pub struct PhysicalConstraints {
    pub max_force_n: f64,
    pub max_torque_nm: f64,
    pub max_velocity_ms: f64,
    pub geofence: [f64; 6], // [min_x, min_y, min_z, max_x, max_y, max_z]
}

impl PhysicalConstraints {
    pub fn default_robot() -> Self {
        Self { max_force_n: 50.0, max_torque_nm: 10.0, max_velocity_ms: 2.0, geofence: [-10.0, -10.0, -1.0, 10.0, 10.0, 3.0] }
    }
}

/// Clamp a command and return which fields were clamped
pub struct ClampResult {
    pub force: f64,
    pub torque: f64,
    pub velocity: f64,
    pub clamped_force: bool,
    pub clamped_torque: bool,
    pub clamped_velocity: bool,
    pub geofence_breach: bool,
    /// A command or position input was NaN or infinite. The command is zeroed:
    /// comparisons against NaN are always false, so it would otherwise pass
    /// every cap and the geofence unchanged.
    pub non_finite_input: bool,
}

pub fn clamp_command(force: f64, torque: f64, velocity: f64, pos: &[f64; 3], constraints: &PhysicalConstraints) -> ClampResult {
    let mut r = ClampResult {
        force,
        torque,
        velocity,
        clamped_force: false,
        clamped_torque: false,
        clamped_velocity: false,
        geofence_breach: false,
        non_finite_input: false,
    };
    // Fail closed on NaN/inf before any comparison.
    if !(force.is_finite() && torque.is_finite() && velocity.is_finite() && pos.iter().all(|p| p.is_finite())) {
        r.non_finite_input = true;
        r.force = 0.0;
        r.torque = 0.0;
        r.velocity = 0.0;
        return r;
    }
    // Force capping
    if r.force.abs() > constraints.max_force_n {
        r.force = r.force.signum() * constraints.max_force_n;
        r.clamped_force = true;
    }
    // Torque capping
    if r.torque.abs() > constraints.max_torque_nm {
        r.torque = r.torque.signum() * constraints.max_torque_nm;
        r.clamped_torque = true;
    }
    // Velocity capping
    if r.velocity.abs() > constraints.max_velocity_ms {
        r.velocity = r.velocity.signum() * constraints.max_velocity_ms;
        r.clamped_velocity = true;
    }
    // Geofence check
    let g = &constraints.geofence;
    if pos[0] < g[0] || pos[0] > g[3] || pos[1] < g[1] || pos[1] > g[4] || pos[2] < g[2] || pos[2] > g[5] {
        r.geofence_breach = true;
        r.force = 0.0;
        r.torque = 0.0;
        r.velocity = 0.0;
    }
    r
}

// ─── Kill Switch (atomic, one-way, < 1ms) ───────────────────

// KillSwitch uses std::sync::Mutex. On MCU, use interrupt-masked
// atomics + per-interrupt panic handling instead of this abstraction.
// Gated behind `std` for now — replacing with `spin::Mutex` or similar
// is a future round.
#[cfg(feature = "std")]
pub struct KillSwitch {
    triggered: AtomicBool,
    reason: std::sync::Mutex<Option<PanicEvent>>,
}

#[derive(Clone, Debug)]
pub struct PanicEvent {
    pub reason: PanicReason,
    pub source: &'static str,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PanicReason {
    Manual,
    ForceExceeded,
    GeofenceBreach,
    DeadlineMiss,
    HardwareFault,
    WatchdogTimeout,
    VitalityDead,
}

#[cfg(feature = "std")]
impl KillSwitch {
    pub fn new() -> Self {
        Self { triggered: AtomicBool::new(false), reason: std::sync::Mutex::new(None) }
    }

    /// Trigger kill switch — one-way, idempotent, second call is no-op
    pub fn panic(&self, reason: PanicReason, source: &'static str, detail: String) -> bool {
        // Compare-and-swap: only first panic wins
        if self.triggered.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            if let Ok(mut r) = self.reason.lock() {
                *r = Some(PanicEvent { reason, source, detail });
            }
            true
        } else {
            false // Already triggered
        }
    }

    #[inline]
    pub fn is_triggered(&self) -> bool {
        self.triggered.load(Ordering::Relaxed)
    }

    pub fn get_event(&self) -> Option<PanicEvent> {
        self.reason.lock().ok().and_then(|r| r.clone())
    }
}

// ─── Semantic Manifolds (latent ↔ physical mapping) ─────────

pub struct SemanticManifold {
    pub name: &'static str,
    pub dims: &'static [usize],
    pub scales: &'static [f64],
    pub limits: &'static [(f64, f64)],
}

/// LOCOMOTION: linear x/y/z + angular x/y/z → dims 10-15
pub const LOCOMOTION: SemanticManifold = SemanticManifold {
    name: "LOCOMOTION",
    dims: &[10, 11, 12, 13, 14, 15],
    scales: &[1.0, 1.0, 0.5, 0.5, 0.5, 1.0],
    limits: &[(-2.0, 2.0), (-1.0, 1.0), (-0.5, 0.5), (-1.0, 1.0), (-1.0, 1.0), (-2.0, 2.0)],
};

/// MANIPULATION: 6-DOF joint angles → dims 16-21
pub const MANIPULATION: SemanticManifold = SemanticManifold {
    name: "MANIPULATION",
    dims: &[16, 17, 18, 19, 20, 21],
    scales: &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
    limits: &[(-3.15, 3.15), (-3.15, 3.15), (-3.15, 3.15), (-3.15, 3.15), (-3.15, 3.15), (-3.15, 3.15)],
};

/// NAVIGATION: goal x/y/z + heading + urgency + confidence → dims 22-27
pub const NAVIGATION: SemanticManifold = SemanticManifold {
    name: "NAVIGATION",
    dims: &[22, 23, 24, 25, 26, 27],
    scales: &[10.0, 10.0, 5.0, 3.15, 1.0, 1.0],
    limits: &[(-100.0, 100.0), (-100.0, 100.0), (-10.0, 10.0), (-3.15, 3.15), (0.0, 1.0), (0.0, 1.0)],
};

/// Encode physical values into latent vector via manifold
pub fn encode_manifold(manifold: &SemanticManifold, values: &[f64], out: &mut V) {
    for (i, &dim) in manifold.dims.iter().enumerate() {
        if i >= values.len() {
            break;
        }
        let s = manifold.scales.get(i).copied().unwrap_or(1.0);
        let (lo, hi) = manifold.limits.get(i).copied().unwrap_or((-1e6, 1e6));
        out[dim] = (values[i] / s).clamp(lo, hi);
    }
}

/// Decode latent vector back to physical values via manifold
pub fn decode_manifold(manifold: &SemanticManifold, tension: &V) -> Vec<f64> {
    manifold
        .dims
        .iter()
        .enumerate()
        .map(|(i, &dim)| {
            let s = manifold.scales.get(i).copied().unwrap_or(1.0);
            tension[dim] * s
        })
        .collect()
}

// ─── HAL Driver Trait ───────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DriverState {
    Unloaded,
    Ready,
    Active,
    Error,
}

pub struct DriverHealth {
    pub healthy: bool,
    pub state: DriverState,
    pub error_count: u32,
}

pub trait HalDriver {
    fn id(&self) -> &str;
    fn is_simulated(&self) -> bool;
    fn state(&self) -> DriverState;
    fn read(&mut self) -> V; // Returns sensor data as tension vector
    fn write(&mut self, command: &V, constraints: &PhysicalConstraints) -> ClampResult;
    fn health(&self) -> DriverHealth;
    fn emergency_stop(&mut self); // Must complete < 1ms
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: clamp_command output force is always bounded by max_force_n.
    #[kani::proof]
    fn proof_hal_clamp_force_bounded() {
        let force: f64 = kani::any();
        let torque: f64 = kani::any();
        let velocity: f64 = kani::any();
        kani::assume(force.is_finite() && torque.is_finite() && velocity.is_finite());
        let pos: [f64; 3] = [0.0, 0.0, 0.0];
        let c = PhysicalConstraints::default_robot();
        let r = clamp_command(force, torque, velocity, &pos, &c);
        assert!(r.force.abs() <= c.max_force_n + 1e-9);
    }

    /// PROVE: clamp_command output torque is always bounded.
    #[kani::proof]
    fn proof_hal_clamp_torque_bounded() {
        let force: f64 = kani::any();
        let torque: f64 = kani::any();
        let velocity: f64 = kani::any();
        kani::assume(force.is_finite() && torque.is_finite() && velocity.is_finite());
        let pos: [f64; 3] = [0.0, 0.0, 0.0];
        let c = PhysicalConstraints::default_robot();
        let r = clamp_command(force, torque, velocity, &pos, &c);
        assert!(r.torque.abs() <= c.max_torque_nm + 1e-9);
    }

    /// PROVE: clamp_command output velocity is always bounded.
    #[kani::proof]
    fn proof_hal_clamp_velocity_bounded() {
        let force: f64 = kani::any();
        let torque: f64 = kani::any();
        let velocity: f64 = kani::any();
        kani::assume(force.is_finite() && torque.is_finite() && velocity.is_finite());
        let pos: [f64; 3] = [0.0, 0.0, 0.0];
        let c = PhysicalConstraints::default_robot();
        let r = clamp_command(force, torque, velocity, &pos, &c);
        assert!(r.velocity.abs() <= c.max_velocity_ms + 1e-9);
    }

    /// PROVE: geofence breach zeroes ALL outputs.
    #[kani::proof]
    fn proof_hal_geofence_breach_zeroes_everything() {
        let force: f64 = kani::any();
        let torque: f64 = kani::any();
        let velocity: f64 = kani::any();
        kani::assume(force.is_finite() && torque.is_finite() && velocity.is_finite());
        let pos: [f64; 3] = [100.0, 0.0, 0.0]; // outside x=[-10,10]
        let c = PhysicalConstraints::default_robot();
        let r = clamp_command(force, torque, velocity, &pos, &c);
        assert!(r.geofence_breach);
        assert_eq!(r.force, 0.0);
        assert_eq!(r.torque, 0.0);
        assert_eq!(r.velocity, 0.0);
    }

    /// PROVE: if input force is within bounds, output == input (no spurious clamp).
    #[kani::proof]
    fn proof_hal_clamp_identity_when_in_bounds() {
        let force: f64 = kani::any();
        kani::assume(force.is_finite());
        let c = PhysicalConstraints::default_robot();
        kani::assume(force.abs() <= c.max_force_n);
        let pos: [f64; 3] = [0.0, 0.0, 0.0];
        let r = clamp_command(force, 0.0, 0.0, &pos, &c);
        assert_eq!(r.force, force);
        assert!(!r.clamped_force);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_fails_closed_on_non_finite_input() {
        let c = PhysicalConstraints::default_robot();
        let ok = [0.0, 0.0, 0.0];
        for (f, t, v, pos) in [(f64::NAN, 0.0, 0.0, ok), (0.0, f64::INFINITY, 0.0, ok), (0.0, 0.0, f64::NEG_INFINITY, ok), (1.0, 1.0, 1.0, [f64::NAN, 0.0, 0.0])] {
            let r = clamp_command(f, t, v, &pos, &c);
            assert!(r.non_finite_input);
            assert_eq!((r.force, r.torque, r.velocity), (0.0, 0.0, 0.0));
        }
        assert!(!clamp_command(1.0, 1.0, 1.0, &ok, &c).non_finite_input);
    }

    #[test]
    fn clamp_caps_force() {
        let c = PhysicalConstraints::default_robot();
        let r = clamp_command(100.0, 5.0, 1.0, &[0.0, 0.0, 0.0], &c);
        assert!(r.clamped_force);
        assert!((r.force - 50.0).abs() < 1e-10);
        assert!(!r.clamped_torque);
    }

    #[test]
    fn clamp_caps_velocity() {
        let c = PhysicalConstraints::default_robot();
        let r = clamp_command(10.0, 5.0, 5.0, &[0.0, 0.0, 0.0], &c);
        assert!(r.clamped_velocity);
        assert!((r.velocity - 2.0).abs() < 1e-10);
    }

    #[test]
    fn geofence_breach_zeros_all() {
        let c = PhysicalConstraints::default_robot();
        let r = clamp_command(10.0, 5.0, 1.0, &[999.0, 0.0, 0.0], &c);
        assert!(r.geofence_breach);
        assert!(r.force.abs() < 1e-10);
        assert!(r.velocity.abs() < 1e-10);
    }

    #[test]
    fn inside_geofence_no_breach() {
        let c = PhysicalConstraints::default_robot();
        let r = clamp_command(10.0, 5.0, 1.0, &[0.0, 0.0, 0.0], &c);
        assert!(!r.geofence_breach);
    }

    #[test]
    fn kill_switch_one_way() {
        let ks = KillSwitch::new();
        assert!(!ks.is_triggered());
        let first = ks.panic(PanicReason::Manual, "TEST", "test".into());
        assert!(first);
        assert!(ks.is_triggered());
        // Second panic is no-op
        let second = ks.panic(PanicReason::ForceExceeded, "TEST2", "test2".into());
        assert!(!second);
        // First reason preserved
        let ev = ks.get_event().unwrap();
        assert_eq!(ev.reason, PanicReason::Manual);
    }

    #[test]
    fn kill_switch_idempotent() {
        let ks = KillSwitch::new();
        ks.panic(PanicReason::GeofenceBreach, "HAL", "x=999".into());
        ks.panic(PanicReason::ForceExceeded, "HAL", "f=100".into());
        assert_eq!(ks.get_event().unwrap().reason, PanicReason::GeofenceBreach);
    }

    #[test]
    fn encode_decode_locomotion() {
        let mut v = vz();
        encode_manifold(&LOCOMOTION, &[1.5, 0.5, 0.3, 0.2, 0.1, 1.0], &mut v);
        assert!((v[10] - 1.5).abs() < 1e-10);
        assert!((v[11] - 0.5).abs() < 1e-10);
        let decoded = decode_manifold(&LOCOMOTION, &v);
        assert!((decoded[0] - 1.5).abs() < 1e-10);
    }

    #[test]
    fn encode_clamps_to_limits() {
        let mut v = vz();
        encode_manifold(&LOCOMOTION, &[999.0], &mut v); // limit is [-2, 2]
        assert!((v[10] - 2.0).abs() < 1e-10);
    }

    #[test]
    fn negative_force_clamps_correctly() {
        let c = PhysicalConstraints::default_robot();
        let r = clamp_command(-100.0, -20.0, -5.0, &[0.0, 0.0, 0.0], &c);
        assert!((r.force - (-50.0)).abs() < 1e-10);
        assert!((r.torque - (-10.0)).abs() < 1e-10);
        assert!((r.velocity - (-2.0)).abs() < 1e-10);
    }
}
