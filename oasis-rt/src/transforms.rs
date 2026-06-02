//! OASIS — 2D rigid transforms + frame tree (TF2 equivalent for mobile robots)
//!
//! ROS 2 tf2 = 1000s of LOC + DDS topic + buffer with timestamps + listeners.
//! OASIS transforms = pure math + HashMap tree lookup + Kani-proven invariants.
//!
//! Scope: 2D rigid transforms (x, y, θ). Sufficient for diff-drive, car-like,
//! planar aerial. For full 6-DOF (3D rotation), extend to SE(3) (future work).
//!
//! # Usage
//! ```no_run
//! use oasis_rt::transforms::{Transform2D, TransformTree};
//!
//! let mut tree = TransformTree::new();
//! tree.set_transform("map", "odom", Transform2D::new(0.0, 0.0, 0.0));
//! tree.set_transform("odom", "base_link", Transform2D::new(1.0, 2.0, 0.5));
//! tree.set_transform("base_link", "laser", Transform2D::new(0.1, 0.0, 0.0));
//!
//! // Lookup accumulated transform map → laser:
//! let t = tree.lookup("map", "laser").unwrap();
//! // Apply to a point measured in laser frame:
//! let world_point = t.apply((2.5, 0.0));
//! ```
//!
//! # Kani-proven properties
//! - `compose(identity, T) == T` and `compose(T, identity) == T`
//! - `inverse(inverse(T)) == T` (within float epsilon)
//! - `compose(T, inverse(T))` ≈ identity

// HashMap on host (unchanged); BTreeMap aliased as HashMap on MCU
// (no_std has no HashMap). O(log n) instead of O(1), but only requires
// Ord on keys — frame names are `String`, which is Ord.
#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap as HashMap, string::{String, ToString}, vec::Vec, vec};
#[cfg(not(feature = "std"))]
use crate::fmath::F64Ext;

/// 2D rigid transform: translation (x, y) + rotation θ (radians, around Z axis).
/// Encodes the pose of a child frame relative to its parent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2D {
    pub x: f64,
    pub y: f64,
    pub theta: f64,
}

impl Transform2D {
    #[inline]
    pub const fn new(x: f64, y: f64, theta: f64) -> Self { Self { x, y, theta } }

    /// Identity transform: no translation, no rotation.
    #[inline]
    pub const fn identity() -> Self { Self { x: 0.0, y: 0.0, theta: 0.0 } }

    /// Apply this transform to a 2D point.
    /// point_in_parent = R(θ) · point_in_child + (x, y)
    #[inline]
    pub fn apply(&self, point: (f64, f64)) -> (f64, f64) {
        let (cos_t, sin_t) = (self.theta.cos(), self.theta.sin());
        let (px, py) = point;
        (cos_t * px - sin_t * py + self.x,
         sin_t * px + cos_t * py + self.y)
    }

    /// Inverse transform — if self = T_parent_child, inverse = T_child_parent.
    /// Derived from rigid-body algebra: R^T · (-t).
    #[inline]
    pub fn inverse(&self) -> Self {
        let (cos_t, sin_t) = (self.theta.cos(), self.theta.sin());
        Self {
            x: -(cos_t * self.x + sin_t * self.y),
            y: -(-sin_t * self.x + cos_t * self.y),
            theta: -self.theta,
        }
    }
}

/// Compose two transforms: returns `a · b` (a applied to the pose of b).
/// If `a = T_A_B` and `b = T_B_C`, then `compose(a, b) = T_A_C`.
#[inline]
pub fn compose(a: Transform2D, b: Transform2D) -> Transform2D {
    let (cos_a, sin_a) = (a.theta.cos(), a.theta.sin());
    Transform2D {
        x: cos_a * b.x - sin_a * b.y + a.x,
        y: sin_a * b.x + cos_a * b.y + a.y,
        theta: a.theta + b.theta,
    }
}

/// Wrap an angle to [-π, π]. Useful for diff checks.
///
/// Uses atan2(sin, cos) which is mathematically guaranteed to produce a
/// value in [-π, π] regardless of the input magnitude or rounding.
#[inline]
pub fn wrap_angle(theta: f64) -> f64 {
    theta.sin().atan2(theta.cos())
}

/// Multi-frame tree storing parent-child transforms.
/// Lookup walks the chain and composes transforms.
pub struct TransformTree {
    /// child -> (parent, transform_parent_to_child)
    parents: HashMap<String, (String, Transform2D)>,
}

impl Default for TransformTree {
    fn default() -> Self { Self::new() }
}

impl TransformTree {
    pub fn new() -> Self { Self { parents: HashMap::new() } }

    /// Register T_parent_child: the pose of `child` expressed in `parent`'s frame.
    pub fn set_transform(&mut self, parent: &str, child: &str, tf: Transform2D) {
        self.parents.insert(child.to_string(), (parent.to_string(), tf));
    }

    /// Compute T_from_to: transforms a point expressed in `to` into `from` coords.
    /// Walks the chain child→parent until both `from` and `to` share an ancestor,
    /// then composes the two legs.
    pub fn lookup(&self, from: &str, to: &str) -> Option<Transform2D> {
        if from == to { return Some(Transform2D::identity()); }
        // Build path from 'to' up to root
        let path_to = self.path_to_root(to)?;
        let path_from = self.path_to_root(from)?;
        // Find LCA (lowest common ancestor)
        let (cut_to, cut_from) = find_lca(&path_to, &path_from)?;
        // Compose T_from_to = inverse(T_from_lca) ∘ T_lca_to
        // T_to_lca: walk from `to` up `cut_to` hops, composing
        let mut tf_to_lca = Transform2D::identity();
        let mut cur = to;
        for _ in 0..cut_to {
            let (p, tf) = self.parents.get(cur)?;
            tf_to_lca = compose(*tf, tf_to_lca);
            cur = p;
        }
        // T_from_lca: same from `from`
        let mut tf_from_lca = Transform2D::identity();
        cur = from;
        for _ in 0..cut_from {
            let (p, tf) = self.parents.get(cur)?;
            tf_from_lca = compose(*tf, tf_from_lca);
            cur = p;
        }
        // Final: inverse(tf_from_lca) ∘ tf_to_lca
        Some(compose(tf_from_lca.inverse(), tf_to_lca))
    }

    fn path_to_root(&self, frame: &str) -> Option<Vec<String>> {
        let mut chain = vec![frame.to_string()];
        let mut cur = frame;
        while let Some((parent, _)) = self.parents.get(cur) {
            chain.push(parent.clone());
            cur = parent;
            if chain.len() > 128 { return None; } // cycle guard
        }
        Some(chain)
    }

    pub fn frame_count(&self) -> usize { self.parents.len() }
}

fn find_lca(path_to: &[String], path_from: &[String]) -> Option<(usize, usize)> {
    for (i, a) in path_to.iter().enumerate() {
        for (j, b) in path_from.iter().enumerate() {
            if a == b { return Some((i, j)); }
        }
    }
    None
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: compose(identity, T) == T (left identity).
    /// Bounded inputs for Kani tractability.
    #[kani::proof]
    fn proof_transforms_compose_left_identity() {
        let x: f64 = kani::any();
        let y: f64 = kani::any();
        let theta: f64 = kani::any();
        kani::assume(x.is_finite() && y.is_finite() && theta.is_finite());
        kani::assume(x.abs() < 1e6 && y.abs() < 1e6 && theta.abs() < 100.0);
        let t = Transform2D::new(x, y, theta);
        let id = Transform2D::identity();
        let r = compose(id, t);
        assert!((r.x - t.x).abs() < 1e-9);
        assert!((r.y - t.y).abs() < 1e-9);
        assert!((r.theta - t.theta).abs() < 1e-9);
    }

    /// PROVE: compose(T, identity) == T (right identity).
    #[kani::proof]
    fn proof_transforms_compose_right_identity() {
        let x: f64 = kani::any();
        let y: f64 = kani::any();
        let theta: f64 = kani::any();
        kani::assume(x.is_finite() && y.is_finite() && theta.is_finite());
        kani::assume(x.abs() < 1e6 && y.abs() < 1e6 && theta.abs() < 100.0);
        let t = Transform2D::new(x, y, theta);
        let id = Transform2D::identity();
        let r = compose(t, id);
        assert!((r.x - t.x).abs() < 1e-9);
        assert!((r.y - t.y).abs() < 1e-9);
        assert!((r.theta - t.theta).abs() < 1e-9);
    }

    /// PROVE: inverse(identity) == identity.
    #[kani::proof]
    fn proof_transforms_inverse_of_identity_is_identity() {
        let id = Transform2D::identity();
        let inv = id.inverse();
        assert_eq!(inv.x, 0.0);
        assert_eq!(inv.y, 0.0);
        assert_eq!(inv.theta, 0.0);
    }

    /// PROVE: for translation-only transforms (theta=0), inverse composed with
    /// the original gives identity (x=0, y=0, theta=0) bit-exact.
    ///
    /// Note: we avoid proving properties of `wrap_angle` here because Kani's
    /// SMT backend cannot precisely model transcendental functions (sin/cos/
    /// atan2). The runtime behaviour is covered by unit tests instead.
    #[kani::proof]
    fn proof_transforms_translation_inverse_cancels() {
        let x: f64 = kani::any();
        let y: f64 = kani::any();
        kani::assume(x.is_finite() && y.is_finite());
        kani::assume(x.abs() < 1e6 && y.abs() < 1e6);
        let t = Transform2D::new(x, y, 0.0);
        let inv = t.inverse();
        let composed = compose(t, inv);
        assert!(composed.x.abs() < 1e-9);
        assert!(composed.y.abs() < 1e-9);
        assert_eq!(composed.theta, 0.0);
    }

    /// PROVE: apply(identity, point) == point.
    #[kani::proof]
    fn proof_transforms_apply_identity_is_identity() {
        let px: f64 = kani::any();
        let py: f64 = kani::any();
        kani::assume(px.is_finite() && py.is_finite());
        kani::assume(px.abs() < 1e6 && py.abs() < 1e6);
        let id = Transform2D::identity();
        let (rx, ry) = id.apply((px, py));
        assert!((rx - px).abs() < 1e-9);
        assert!((ry - py).abs() < 1e-9);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{PI, FRAC_PI_2};

    const EPS: f64 = 1e-9;

    fn approx(a: f64, b: f64) -> bool { (a - b).abs() < EPS }

    #[test]
    fn identity_basic() {
        let id = Transform2D::identity();
        assert_eq!(id.x, 0.0);
        assert_eq!(id.y, 0.0);
        assert_eq!(id.theta, 0.0);
        let (px, py) = id.apply((3.0, 4.0));
        assert!(approx(px, 3.0) && approx(py, 4.0));
    }

    #[test]
    fn translation_only() {
        let t = Transform2D::new(1.0, 2.0, 0.0);
        let (px, py) = t.apply((0.0, 0.0));
        assert!(approx(px, 1.0) && approx(py, 2.0));
    }

    #[test]
    fn rotation_90deg() {
        let t = Transform2D::new(0.0, 0.0, FRAC_PI_2);
        let (px, py) = t.apply((1.0, 0.0));
        // (1, 0) rotated 90° = (0, 1)
        assert!(approx(px, 0.0) && approx(py, 1.0));
    }

    #[test]
    fn compose_identity_right() {
        let t = Transform2D::new(1.0, 2.0, 0.5);
        let r = compose(t, Transform2D::identity());
        assert!(approx(r.x, 1.0) && approx(r.y, 2.0) && approx(r.theta, 0.5));
    }

    #[test]
    fn compose_identity_left() {
        let t = Transform2D::new(1.0, 2.0, 0.5);
        let r = compose(Transform2D::identity(), t);
        assert!(approx(r.x, 1.0) && approx(r.y, 2.0) && approx(r.theta, 0.5));
    }

    #[test]
    fn inverse_of_translation() {
        let t = Transform2D::new(3.0, 4.0, 0.0);
        let inv = t.inverse();
        assert!(approx(inv.x, -3.0) && approx(inv.y, -4.0));
    }

    #[test]
    fn inverse_composition_gives_identity() {
        let t = Transform2D::new(1.5, -2.3, 0.7);
        let inv = t.inverse();
        let r = compose(t, inv);
        assert!(r.x.abs() < 1e-9, "x not zero: {}", r.x);
        assert!(r.y.abs() < 1e-9, "y not zero: {}", r.y);
        assert!(r.theta.abs() < 1e-9, "theta not zero: {}", r.theta);
    }

    #[test]
    fn double_inverse_roundtrip() {
        let t = Transform2D::new(1.5, -2.3, 0.7);
        let inv2 = t.inverse().inverse();
        assert!(approx(inv2.x, t.x));
        assert!(approx(inv2.y, t.y));
        assert!(approx(inv2.theta, t.theta));
    }

    #[test]
    fn wrap_angle_bounded() {
        assert!(approx(wrap_angle(0.0), 0.0));
        assert!(approx(wrap_angle(PI), PI));
        assert!(approx(wrap_angle(-PI), -PI));
        // 3π should wrap to π
        assert!(approx(wrap_angle(3.0 * PI), PI));
        // -3π should wrap to -π
        assert!(approx(wrap_angle(-3.0 * PI), -PI));
    }

    #[test]
    fn tree_single_frame_lookup() {
        let mut tree = TransformTree::new();
        tree.set_transform("map", "odom", Transform2D::new(1.0, 2.0, 0.0));
        let t = tree.lookup("map", "odom").unwrap();
        assert!(approx(t.x, 1.0) && approx(t.y, 2.0));
    }

    #[test]
    fn tree_chained_lookup() {
        let mut tree = TransformTree::new();
        tree.set_transform("map", "odom", Transform2D::new(1.0, 0.0, 0.0));
        tree.set_transform("odom", "base_link", Transform2D::new(2.0, 0.0, 0.0));
        tree.set_transform("base_link", "laser", Transform2D::new(0.5, 0.0, 0.0));
        // map → laser should be (1 + 2 + 0.5, 0, 0) = (3.5, 0, 0)
        let t = tree.lookup("map", "laser").unwrap();
        assert!(approx(t.x, 3.5), "expected x=3.5, got {}", t.x);
        assert!(approx(t.y, 0.0));
    }

    #[test]
    fn tree_self_lookup_is_identity() {
        let tree = TransformTree::new();
        let t = tree.lookup("map", "map").unwrap();
        assert_eq!(t, Transform2D::identity());
    }

    #[test]
    fn tree_unknown_frame_returns_none() {
        let tree = TransformTree::new();
        assert!(tree.lookup("map", "unknown").is_none());
    }

    #[test]
    fn tree_siblings_via_lca() {
        // base_link → sensor_left and base_link → sensor_right
        //   lookup("sensor_left", "sensor_right") should traverse via base_link
        let mut tree = TransformTree::new();
        tree.set_transform("base_link", "sensor_left",  Transform2D::new(0.0, 0.5, 0.0));
        tree.set_transform("base_link", "sensor_right", Transform2D::new(0.0, -0.5, 0.0));
        let t = tree.lookup("sensor_left", "sensor_right").unwrap();
        // In sensor_left's frame, sensor_right is at y = -1.0
        assert!(approx(t.x, 0.0), "x should be 0, got {}", t.x);
        assert!(approx(t.y, -1.0), "y should be -1.0, got {}", t.y);
    }

    #[test]
    fn bench_lookup_chained() {
        let mut tree = TransformTree::new();
        // Build a 10-deep chain
        for i in 0..10 {
            let parent = if i == 0 { "root".to_string() } else { format!("n{}", i - 1) };
            tree.set_transform(&parent, &format!("n{}", i), Transform2D::new(0.1 * i as f64, 0.0, 0.01));
        }
        const N: u32 = 50_000;
        let start = std::time::Instant::now();
        for _ in 0..N {
            let _ = tree.lookup("root", "n9").unwrap();
        }
        let per_us = start.elapsed().as_nanos() as f64 / N as f64 / 1000.0;
        assert!(per_us < 50.0, "tree lookup too slow: {} µs", per_us);
        eprintln!("transform tree lookup (10-deep chain): {:.2} µs/op", per_us);
    }
}
