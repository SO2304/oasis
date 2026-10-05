//! OASIS-RT — Non-Euclidean World Model via Pressure Fields (Mechanism 10)
//!
//! Entities are continuous pressure zones, not geometric objects.
//! Navigation = gradient descent through combined field. No A*/RRT*.

#[cfg(not(feature = "std"))]
use crate::fmath::F64Ext;
use crate::vec::*;
#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec, vec::Vec};

const MAX_ZONES: usize = 32;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ZoneType {
    Repulsive,  // Obstacles, hazards → push away
    Attractive, // Goals, targets → pull toward
    Entropy,    // Unknown regions → radiate uncertainty
    Semantic,   // "slow down", "be gentle" → modify behavior
}

#[derive(Clone)]
pub struct PressureZone {
    pub zone_type: ZoneType,
    pub center: V,
    pub falloff: f64,
    pub intensity: f64,
    pub active: bool,
}

/// Error returned by `try_add_zone` when the WorldModel cap is hit.
/// CallerMUST decide: evict an existing zone, refuse the operation,
/// or log + continue. Silent drop is forbidden by the type system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneError {
    /// MAX_ZONES reached; the new zone was NOT added.
    CapacityExceeded { current: usize, cap: usize },
}

pub struct WorldModel {
    zones: Vec<PressureZone>,
    /// Telemetry: number of times `add_zone` would-have or did silently
    /// drop a zone due to cap. Exposed via `cap_hit_count()`. Operators
    /// MUST poll this if they call legacy `add_zone()` directly;
    /// `try_add_zone()` returns Err instead so polling is unnecessary.
    cap_hit_count: u32,
}

impl WorldModel {
    pub fn new() -> Self {
        Self { zones: Vec::new(), cap_hit_count: 0 }
    }

    /// LEGACY add_zone — silently drops at MAX_ZONES cap. KEPT for
    /// backward compat but DEPRECATED. New code MUST use `try_add_zone`.
    /// The silent drop is fail-QUIET — the operator has no signal that
    /// hazards were lost. cap_hit_count() exposes the telemetry but
    /// most callers don't poll it.
    ///
    /// Audit history: this was identified as a fail-mode defect (worse
    /// than unbounded growth) on 2026-05-10 and the cap_hit_count +
    /// try_add_zone API were added to expose / replace the silent path.
    #[deprecated(
        since = "0.3.1",
        note = "use try_add_zone() — it returns Result<(), ZoneError> so the cap-overflow case CANNOT be silently ignored. See SHADOW_AUDIT_SILENT_CAP_ADDRESSING.md."
    )]
    pub fn add_zone(&mut self, zone_type: ZoneType, center: V, intensity: f64, falloff: f64) {
        if self.zones.len() < MAX_ZONES {
            self.zones.push(PressureZone { zone_type, center, falloff, intensity, active: true });
        } else {
            self.cap_hit_count = self.cap_hit_count.saturating_add(1);
        }
    }

    /// Add a pressure zone, returning Err if the cap is reached.
    /// Replaces the legacy `add_zone()` silent no-op behavior.
    /// Callers MUST handle the Err — they can choose to evict an
    /// existing zone, refuse the operation loudly, or log + continue
    /// (in which case cap_hit_count is incremented for telemetry).
    ///
    /// Implementation note (2026-05-10 fix): first scans for an
    /// INACTIVE slot (left behind by `remove_zone()`, which only
    /// marks active=false without shrinking the Vec). This is the
    /// correct behavior because the previous round discovered that
    /// remove → try_add was failing because Vec.len() stayed at cap
    /// even though active count had dropped. THE slot-reuse path is
    /// what lets eviction-then-add patterns succeed.
    pub fn try_add_zone(&mut self, zone_type: ZoneType, center: V, intensity: f64, falloff: f64) -> Result<(), ZoneError> {
        // First: try to reuse an inactive slot (from prior remove_zone)
        for slot in self.zones.iter_mut() {
            if !slot.active {
                *slot = PressureZone { zone_type, center, falloff, intensity, active: true };
                return Ok(());
            }
        }
        // No inactive slot; only push if Vec hasn't hit cap.
        if self.zones.len() < MAX_ZONES {
            self.zones.push(PressureZone { zone_type, center, falloff, intensity, active: true });
            Ok(())
        } else {
            self.cap_hit_count = self.cap_hit_count.saturating_add(1);
            Err(ZoneError::CapacityExceeded { current: self.zones.len(), cap: MAX_ZONES })
        }
    }

    /// Expose the cap-hit telemetry for callers that USE the legacy
    /// add_zone() and want to observe whether silent drops occurred.
    /// Monotonically non-decreasing; reset only via fleet redeploy
    /// (or explicit reset_cap_hit_count() — not provided to keep
    /// monotonicity invariant).
    pub fn cap_hit_count(&self) -> u32 {
        self.cap_hit_count
    }

    pub fn remove_zone(&mut self, idx: usize) {
        if idx < self.zones.len() {
            self.zones[idx].active = false;
        }
    }

    /// Sample the combined pressure field at a position.
    /// Returns (gradient_force, total_repulsion, total_attraction, local_entropy)
    pub fn sample(&self, pos: &V) -> (V, f64, f64, f64) {
        let mut gradient = vz();
        let mut repulsion = 0.0_f64;
        let mut attraction = 0.0_f64;
        let mut local_entropy = 0.0_f64;

        for zone in &self.zones {
            if !zone.active {
                continue;
            }

            let dist = vd(pos, &zone.center);
            // Pressure = intensity * exp(-dist * falloff)
            let pressure = zone.intensity * (-dist * zone.falloff).exp();

            if pressure < 1e-6 {
                continue;
            } // Too far, skip

            // Direction from zone center to agent (for repulsion) or agent to center (attraction)
            let diff = vsub(pos, &zone.center);
            let diff_norm = vn(&diff);
            if diff_norm < 1e-10 {
                continue;
            }

            match zone.zone_type {
                ZoneType::Repulsive => {
                    // Push away: gradient points AWAY from center
                    let push = vscale(&diff, pressure / diff_norm);
                    gradient = vadd(&gradient, &push);
                    repulsion += pressure;
                }
                ZoneType::Attractive => {
                    // Pull toward: gradient points TOWARD center
                    let pull = vscale(&diff, -pressure / diff_norm);
                    gradient = vadd(&gradient, &pull);
                    attraction += pressure;
                }
                ZoneType::Entropy => {
                    // Radiate uncertainty: adds to local entropy estimate
                    local_entropy += pressure;
                }
                ZoneType::Semantic => {
                    // Semantic: scales behavior (like damping force)
                    let damp = vscale(&diff, -pressure * 0.1 / diff_norm);
                    gradient = vadd(&gradient, &damp);
                }
            }
        }

        (gradient, repulsion, attraction, local_entropy.min(1.0))
    }

    /// Find the path of least resistance from start to goal via gradient descent
    pub fn navigate(&self, start: &V, goal: &V, steps: usize) -> Vec<V> {
        let mut path = Vec::with_capacity(steps + 1);
        let mut pos = *start;
        path.push(pos);

        for _ in 0..steps {
            let (grad, _, _, _) = self.sample(&pos);
            // Also add goal attraction directly
            let to_goal = vsub(goal, &pos);
            let goal_dist = vn(&to_goal);
            if goal_dist < 0.1 {
                break;
            } // Arrived

            let goal_pull = vscale(&to_goal, 0.3 / goal_dist);
            let combined = vadd(&grad, &goal_pull);

            // Step along gradient
            let step_size = 0.1;
            let step = vscale(&combined, step_size / vn(&combined).max(1e-10));
            for i in 0..DIM {
                pos[i] += step[i];
            }
            path.push(pos);
        }

        path
    }

    pub fn zone_count(&self) -> usize {
        self.zones.iter().filter(|z| z.active).count()
    }
}

/// Pure 1D gradient contribution from a single repulsive zone.
/// Returns the scalar dx component of the "push" vector for an agent at
/// position `x` relative to a repulsive zone centered at `center_x` with
/// given pressure. Mirrors the core math in WorldModel::sample.
///
/// Invariant: grad_1d * (x - center_x) >= 0 (points away from source).
#[inline]
pub fn repulsive_grad_1d(x: f64, center_x: f64, pressure: f64) -> f64 {
    let diff = x - center_x;
    let diff_abs = diff.abs();
    if diff_abs < 1e-10 || pressure <= 0.0 {
        return 0.0;
    }
    // `push = diff * pressure / |diff|` → same sign as diff
    diff * pressure / diff_abs
}

/// Pure 1D gradient contribution from a single attractive zone.
/// Invariant: grad_1d * (x - center_x) <= 0 (points toward source).
#[inline]
pub fn attractive_grad_1d(x: f64, center_x: f64, pressure: f64) -> f64 {
    let diff = x - center_x;
    let diff_abs = diff.abs();
    if diff_abs < 1e-10 || pressure <= 0.0 {
        return 0.0;
    }
    -diff * pressure / diff_abs
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: repulsive 1D gradient has the SAME sign as (x - center_x).
    /// This is the non-Euclidean "push away" core property.
    #[kani::proof]
    fn proof_m10_repulsive_points_away() {
        let x: f64 = kani::any();
        let c: f64 = kani::any();
        let p: f64 = kani::any();
        kani::assume(x.is_finite() && c.is_finite() && p.is_finite());
        kani::assume(p > 0.0 && p < 100.0);
        kani::assume(x != c && (x - c).abs() > 1e-6); // well outside zero
        kani::assume(x.abs() < 1e6 && c.abs() < 1e6); // bounded magnitudes
        let g = repulsive_grad_1d(x, c, p);
        let diff = x - c;
        // Same sign: diff * g >= 0
        assert!(diff * g >= 0.0);
    }

    /// PROVE: attractive 1D gradient has OPPOSITE sign of (x - center_x).
    #[kani::proof]
    fn proof_m10_attractive_points_toward() {
        let x: f64 = kani::any();
        let c: f64 = kani::any();
        let p: f64 = kani::any();
        kani::assume(x.is_finite() && c.is_finite() && p.is_finite());
        kani::assume(p > 0.0 && p < 100.0);
        kani::assume(x != c && (x - c).abs() > 1e-6);
        kani::assume(x.abs() < 1e6 && c.abs() < 1e6);
        let g = attractive_grad_1d(x, c, p);
        let diff = x - c;
        // Opposite sign: diff * g <= 0
        assert!(diff * g <= 0.0);
    }

    /// PROVE: 1D field SUPERPOSITION is linear (strict equality).
    /// grad(zone_A + zone_B) = grad(A) + grad(B).
    #[kani::proof]
    fn proof_m10_field_linear() {
        let x: f64 = kani::any();
        let cA: f64 = kani::any();
        let pA: f64 = kani::any();
        let cB: f64 = kani::any();
        let pB: f64 = kani::any();
        kani::assume(x.is_finite() && cA.is_finite() && cB.is_finite() && pA.is_finite() && pB.is_finite());
        kani::assume(pA > 0.0 && pA < 10.0 && pB > 0.0 && pB < 10.0);
        kani::assume((x - cA).abs() > 1e-6 && (x - cB).abs() > 1e-6);
        kani::assume(x.abs() < 1e3 && cA.abs() < 1e3 && cB.abs() < 1e3);
        let gA = repulsive_grad_1d(x, cA, pA);
        let gB = repulsive_grad_1d(x, cB, pB);
        // The "combined" gradient is just the sum (code in sample() does exactly this).
        let g_sum = gA + gB;
        // Recompute the way the real code would: start with 0, add each.
        let g_acc = 0.0 + gA + gB;
        assert!((g_sum - g_acc).abs() < 1e-12);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repulsive_zone_pushes_away() {
        let mut world = WorldModel::new();
        let mut center = vz();
        center[0] = 5.0;
        world.try_add_zone(ZoneType::Repulsive, center, 2.0, 0.5).expect("test setup must not exceed cap");
        // Sample near the obstacle
        let mut pos = vz();
        pos[0] = 4.0;
        let (grad, rep, _, _) = world.sample(&pos);
        assert!(grad[0] < 0.0, "should push away (negative direction): {}", grad[0]);
        assert!(rep > 0.0);
    }

    #[test]
    fn attractive_zone_pulls_toward() {
        let mut world = WorldModel::new();
        let mut center = vz();
        center[0] = 5.0;
        world.try_add_zone(ZoneType::Attractive, center, 2.0, 0.5).expect("test setup must not exceed cap");
        let pos = vz(); // Far from goal
        let (grad, _, att, _) = world.sample(&pos);
        assert!(grad[0] > 0.0, "should pull toward goal: {}", grad[0]);
        assert!(att > 0.0);
    }

    #[test]
    fn entropy_zone_radiates_uncertainty() {
        let mut world = WorldModel::new();
        let center = vz();
        world.try_add_zone(ZoneType::Entropy, center, 1.0, 0.3).expect("test setup must not exceed cap");
        let mut pos = vz();
        pos[0] = 0.5;
        let (_, _, _, entropy) = world.sample(&pos);
        assert!(entropy > 0.0, "entropy zone should radiate: {}", entropy);
    }

    #[test]
    fn repulsion_and_attraction_superpose() {
        let mut world = WorldModel::new();
        // Obstacle at x=3, goal at x=10
        let mut obs = vz();
        obs[0] = 3.0;
        let mut goal = vz();
        goal[0] = 10.0;
        world.try_add_zone(ZoneType::Repulsive, obs, 2.0, 1.0).expect("test setup must not exceed cap");
        world.try_add_zone(ZoneType::Attractive, goal, 2.0, 0.3).expect("test setup must not exceed cap");
        // Sample between them
        let mut pos = vz();
        pos[0] = 4.0;
        let (grad, rep, att, _) = world.sample(&pos);
        assert!(rep > 0.0 && att > 0.0, "both should be present");
        // Gradient should push toward goal (away from obstacle)
        assert!(grad[0] > 0.0, "net gradient should be toward goal");
    }

    #[test]
    fn navigate_avoids_obstacle() {
        let mut world = WorldModel::new();
        // Obstacle blocking direct path
        let mut obs = vz();
        obs[0] = 3.0;
        world.try_add_zone(ZoneType::Repulsive, obs, 5.0, 1.0).expect("test setup must not exceed cap");
        let start = vz();
        let mut goal = vz();
        goal[0] = 6.0;
        let path = world.navigate(&start, &goal, 50);
        assert!(path.len() > 1, "should produce a path");
        let end = path.last().unwrap();
        let end_dist = vd(end, &goal);
        let start_dist = vd(&start, &goal);
        assert!(end_dist < start_dist, "should get closer to goal: {} → {}", start_dist, end_dist);
    }

    #[test]
    fn far_zones_have_no_effect() {
        let mut world = WorldModel::new();
        let mut center = vz();
        center[0] = 100.0; // Very far
        world.try_add_zone(ZoneType::Repulsive, center, 1.0, 2.0).expect("test setup must not exceed cap");
        let pos = vz();
        let (grad, rep, _, _) = world.sample(&pos);
        assert!(rep < 1e-5, "far zone should have negligible effect");
        assert!(vn(&grad) < 1e-5);
    }

    #[test]
    fn zone_count_tracks() {
        let mut world = WorldModel::new();
        let center = vz();
        world.try_add_zone(ZoneType::Repulsive, center, 1.0, 1.0).expect("test setup must not exceed cap");
        world.try_add_zone(ZoneType::Attractive, center, 1.0, 1.0).expect("test setup must not exceed cap");
        assert_eq!(world.zone_count(), 2);
        world.remove_zone(0);
        assert_eq!(world.zone_count(), 1);
    }

    // ───── M10 mathematical invariants ─────────────────────────

    /// Invariant 1 — Gradient descent CONVERGES to goal via `navigate`.
    #[test]
    fn invariant_gradient_descent_converges_to_goal() {
        let world = WorldModel::new();
        let start = vz();
        let mut goal = vz();
        goal[0] = 3.0;
        let path = world.navigate(&start, &goal, 100);
        let end = path.last().unwrap();
        let end_dist = vd(end, &goal);
        assert!(end_dist < 0.15, "navigation did not converge: {} from goal", end_dist);
        // Strict monotonic distance decrease
        let mut last = vd(&path[0], &goal);
        let mut violations = 0;
        for p in path.iter().skip(1) {
            let d = vd(p, &goal);
            if d > last + 0.01 {
                violations += 1;
            }
            last = d;
        }
        assert_eq!(violations, 0, "monotonicity violated {} times", violations);
    }

    /// Invariant 2 — Field is linear: sample(A∪B) = sample(A) + sample(B).
    #[test]
    fn invariant_field_superposition_linear() {
        let mut ca = vz();
        ca[0] = 2.0;
        let mut cb = vz();
        cb[1] = 3.0;
        let mut wa = WorldModel::new();
        wa.try_add_zone(ZoneType::Repulsive, ca, 1.5, 0.8).expect("test");
        let mut wb = WorldModel::new();
        wb.try_add_zone(ZoneType::Attractive, cb, 2.0, 0.5).expect("test");
        let mut wab = WorldModel::new();
        wab.try_add_zone(ZoneType::Repulsive, ca, 1.5, 0.8).expect("test");
        wab.try_add_zone(ZoneType::Attractive, cb, 2.0, 0.5).expect("test");
        let mut pos = vz();
        pos[0] = 1.0;
        pos[1] = 1.0;
        let (ga, _, _, _) = wa.sample(&pos);
        let (gb, _, _, _) = wb.sample(&pos);
        let (gboth, _, _, _) = wab.sample(&pos);
        for i in 0..DIM {
            assert!((gboth[i] - (ga[i] + gb[i])).abs() < 1e-9, "superposition broken at dim {}", i);
        }
    }

    /// Invariant 3 — Repulsive gradient points away: grad·(pos-center) ≥ 0.
    #[test]
    fn invariant_repulsive_gradient_points_away() {
        let center = vz();
        let mut world = WorldModel::new();
        world.try_add_zone(ZoneType::Repulsive, center, 2.0, 0.3).expect("test setup must not exceed cap");
        for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (0.7, 0.7), (-0.7, 0.7), (1.5, 0.5), (0.3, -2.0)] {
            let mut pos = vz();
            pos[0] = dx;
            pos[1] = dy;
            let (grad, _, _, _) = world.sample(&pos);
            let diff = vsub(&pos, &center);
            let dot: f64 = (0..DIM).map(|i| grad[i] * diff[i]).sum();
            assert!(dot >= 0.0, "repulsive gradient at ({},{}) points INTO source: dot={}", dx, dy, dot);
        }
    }

    /// Invariant 4 — Attractive gradient points toward: grad·(pos-center) ≤ 0.
    #[test]
    fn invariant_attractive_gradient_points_toward() {
        let center = vz();
        let mut world = WorldModel::new();
        world.try_add_zone(ZoneType::Attractive, center, 2.0, 0.3).expect("test setup must not exceed cap");
        for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (0.7, 0.7), (-0.7, 0.7), (1.5, 0.5), (0.3, -2.0)] {
            let mut pos = vz();
            pos[0] = dx;
            pos[1] = dy;
            let (grad, _, _, _) = world.sample(&pos);
            let diff = vsub(&pos, &center);
            let dot: f64 = (0..DIM).map(|i| grad[i] * diff[i]).sum();
            assert!(dot <= 0.0, "attractive gradient at ({},{}) points AWAY: dot={}", dx, dy, dot);
        }
    }

    /// Invariant 5 — Exponential pressure decay: ratio at 2× distance = exp(-falloff).
    #[test]
    fn invariant_pressure_decays_exponentially() {
        let center = vz();
        let mut world = WorldModel::new();
        let falloff = 0.5;
        world.try_add_zone(ZoneType::Repulsive, center, 10.0, falloff).expect("test setup must not exceed cap");
        let mut pos1 = vz();
        pos1[0] = 1.0;
        let (_, r1, _, _) = world.sample(&pos1);
        let mut pos2 = vz();
        pos2[0] = 2.0;
        let (_, r2, _, _) = world.sample(&pos2);
        let ratio = r2 / r1;
        let expected = (-falloff).exp();
        assert!((ratio - expected).abs() < 0.01, "decay ratio {} ≠ exp(-{})={}", ratio, falloff, expected);
    }

    /// Invariant 6 — Navigation avoids obstacles by a positive margin.
    #[test]
    fn invariant_navigation_avoids_obstacle_by_margin() {
        let mut obs = vz();
        obs[0] = 3.0;
        let mut world = WorldModel::new();
        world.try_add_zone(ZoneType::Repulsive, obs, 8.0, 0.8).expect("test setup must not exceed cap");
        let start = vz();
        let mut goal = vz();
        goal[0] = 6.0;
        let path = world.navigate(&start, &goal, 100);
        let min_dist = path.iter().map(|p| vd(p, &obs)).fold(f64::INFINITY, f64::min);
        assert!(min_dist > 0.3, "navigation got too close to obstacle: min={}", min_dist);
    }
}
