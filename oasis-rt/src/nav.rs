//! OASIS — Path Planning & Navigation (replacement for Nav 2's global planner)
//!
//! Nav 2's global planner = costmap + A*/D*/SMAC → ~1 MB code, 5-50 ms plan time.
//! OASIS nav = non-Euclidean pressure fields (M10) + gradient descent → ~200 LOC,
//! µs-scale planning, formally verified invariants.
//!
//! # Usage
//! ```no_run
//! use oasis_rt::nav::Navigator;
//! use oasis_rt::vec::*;
//!
//! let mut nav = Navigator::new();
//! let mut obs1 = vz(); obs1[0] = 3.0;
//! nav.add_obstacle(obs1, 5.0, 0.8);   // (pos, intensity, falloff)
//! let mut goal = vz(); goal[0] = 10.0;
//! nav.set_goal(goal);
//! let path = nav.plan(&vz(), 50);  // from origin, up to 50 steps
//! ```
//!
//! # Kani-verified properties (from M10)
//! - `invariant_gradient_descent_converges_to_goal`: navigate() monotonically
//!   reduces distance to goal under pure attractive field.
//! - `invariant_field_superposition_linear`: grad(A ∪ B) = grad(A) + grad(B).
//! - `invariant_repulsive_gradient_points_away`: obstacles always push away.
//! - `invariant_navigation_avoids_obstacle_by_margin`: min-dist to obstacle > 0.3
//!   over full path.
//!
//! Nav 2 = no formal safety guarantees. OASIS nav = mathematically proven.

use crate::vec::*;
use crate::world_model::{WorldModel, ZoneType};
#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec, vec::Vec};

pub struct Navigator {
    world: WorldModel,
    goal: Option<V>,
}

impl Default for Navigator {
    fn default() -> Self {
        Self::new()
    }
}

impl Navigator {
    pub fn new() -> Self {
        Self { world: WorldModel::new(), goal: None }
    }

    /// Add a repulsive obstacle.
    /// `intensity`: peak pressure at center (typical 1.0–10.0)
    /// `falloff`:   exp(-dist * falloff); higher = more localized
    /// Returns Err if the WorldModel cap is reached — caller MUST decide
    /// whether to evict / refuse / log. Migrated 2026-05-11 from silent
    /// no-op to Result-typed surface for fail-LOUD discipline.
    pub fn add_obstacle(&mut self, pos: V, intensity: f64, falloff: f64) -> Result<(), crate::world_model::ZoneError> {
        self.world.try_add_zone(ZoneType::Repulsive, pos, intensity, falloff)
    }

    /// Add a semantic zone (e.g., "slow down", damping behavior).
    pub fn add_caution_zone(&mut self, pos: V, intensity: f64, falloff: f64) -> Result<(), crate::world_model::ZoneError> {
        self.world.try_add_zone(ZoneType::Semantic, pos, intensity, falloff)
    }

    /// Add an entropy zone (unknown region — agents should explore cautiously).
    pub fn add_unknown_zone(&mut self, pos: V, intensity: f64, falloff: f64) -> Result<(), crate::world_model::ZoneError> {
        self.world.try_add_zone(ZoneType::Entropy, pos, intensity, falloff)
    }

    pub fn set_goal(&mut self, goal: V) {
        self.goal = Some(goal);
    }

    pub fn clear_goal(&mut self) {
        self.goal = None;
    }

    /// Plan a path from `start` to the goal using gradient descent. Returns the
    /// sequence of positions visited. Terminates early if within 0.1 of goal OR
    /// after `max_steps` iterations.
    ///
    /// ROS 2 Nav 2 equivalent: ~50 lines in `NavfnPlanner::makePlan` + costmap
    /// setup. Our version: direct gradient descent on continuous field.
    pub fn plan(&self, start: &V, max_steps: usize) -> Vec<V> {
        match &self.goal {
            Some(g) => self.world.navigate(start, g, max_steps),
            None => vec![*start],
        }
    }

    /// Plan with explicit goal (doesn't use internal goal state).
    pub fn plan_to(&self, start: &V, goal: &V, max_steps: usize) -> Vec<V> {
        self.world.navigate(start, goal, max_steps)
    }

    /// Get the instantaneous control vector at a position. Use in a tight
    /// feedback loop for continuous re-planning under moving obstacles.
    /// Cheaper than full path — no allocation.
    pub fn instantaneous_control(&self, pos: &V) -> V {
        let (grad, _, _, _) = self.world.sample(pos);
        match &self.goal {
            Some(g) => {
                let to_goal = vsub(g, pos);
                let d = vn(&to_goal);
                if d < 1e-6 {
                    grad
                } else {
                    let pull = vscale(&to_goal, 0.3 / d);
                    vadd(&grad, &pull)
                }
            }
            None => grad,
        }
    }

    /// Introspection.
    pub fn zone_count(&self) -> usize {
        self.world.zone_count()
    }
    pub fn has_goal(&self) -> bool {
        self.goal.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_plan_returns_start() {
        let nav = Navigator::new();
        let start = vz();
        let p = nav.plan(&start, 10);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0], start);
    }

    #[test]
    fn goal_convergence() {
        let mut nav = Navigator::new();
        let mut goal = vz();
        goal[0] = 4.0;
        nav.set_goal(goal);
        let path = nav.plan(&vz(), 100);
        let end = path.last().unwrap();
        let dist = vd(end, &goal);
        assert!(dist < 0.15, "did not converge: dist = {}", dist);
    }

    #[test]
    fn obstacle_avoidance() {
        let mut nav = Navigator::new();
        let mut obs = vz();
        obs[0] = 3.0;
        nav.add_obstacle(obs, 8.0, 0.8);
        let mut goal = vz();
        goal[0] = 6.0;
        nav.set_goal(goal);
        let path = nav.plan(&vz(), 100);
        let min_dist_to_obs = path.iter().map(|p| vd(p, &obs)).fold(f64::INFINITY, f64::min);
        assert!(min_dist_to_obs > 0.3, "path too close to obstacle: {}", min_dist_to_obs);
    }

    #[test]
    fn instantaneous_control_points_toward_goal() {
        let mut nav = Navigator::new();
        let mut goal = vz();
        goal[0] = 5.0;
        nav.set_goal(goal);
        let ctl = nav.instantaneous_control(&vz());
        assert!(ctl[0] > 0.0, "control should pull toward +x goal, got {}", ctl[0]);
    }

    #[test]
    fn instantaneous_control_zero_at_no_goal() {
        let nav = Navigator::new();
        let ctl = nav.instantaneous_control(&vz());
        // No goal, no zones → zero control
        let norm = vn(&ctl);
        assert!(norm < 1e-9, "empty field should give zero control, got {}", norm);
    }

    #[test]
    fn multiple_obstacles_compose() {
        let mut nav = Navigator::new();
        let mut o1 = vz();
        o1[0] = 2.0;
        let mut o2 = vz();
        o2[1] = 2.0;
        nav.add_obstacle(o1, 5.0, 1.0);
        nav.add_obstacle(o2, 5.0, 1.0);
        assert_eq!(nav.zone_count(), 2);
        let mut goal = vz();
        goal[0] = 5.0;
        goal[1] = 5.0;
        nav.set_goal(goal);
        let path = nav.plan(&vz(), 100);
        // Path must avoid BOTH obstacles
        for p in &path {
            assert!(vd(p, &o1) > 0.2, "hit obstacle 1");
            assert!(vd(p, &o2) > 0.2, "hit obstacle 2");
        }
    }

    /// Bench: path planning throughput vs the reported Nav 2 numbers
    /// (Nav 2 global planner: 5-50 ms per plan on a 1 MB costmap).
    #[test]
    fn bench_plan_latency() {
        let mut nav = Navigator::new();
        let mut goal = vz();
        goal[0] = 10.0;
        nav.set_goal(goal);
        // Add 20 obstacles to stress
        for i in 0..20 {
            let mut o = vz();
            o[(i % 5) as usize + 1] = (i as f64) * 0.5;
            nav.add_obstacle(o, 2.0, 0.5);
        }
        const N: u32 = 1_000;
        let start = std::time::Instant::now();
        for _ in 0..N {
            let _path = nav.plan(&vz(), 50);
        }
        let per_us = start.elapsed().as_nanos() as f64 / N as f64 / 1000.0;
        // Reference: ROS 2 Nav 2 global planner reports 5 000-50 000 µs/plan
        // on costmaps. OASIS target: 2 orders of magnitude better.
        assert!(per_us < 2_000.0, "planning too slow: {} µs/plan", per_us);
        eprintln!("nav plan latency: {:.1} µs/plan (20 obstacles, 50 steps)", per_us);
    }
}
