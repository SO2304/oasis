//! OASIS-RT — Emotional Gain Modulation (Mechanism 5)
//!
//! Five signals (curiosity, fear, satisfaction, frustration, urgency)
//! act as gain multipliers on the tension field.

use crate::vec::*;
#[cfg(not(feature = "std"))]
use alloc::{vec::Vec, string::String, vec, format};
#[cfg(not(feature = "std"))]
use crate::fmath::F64Ext;

const MAX_PAIN: usize = 128;

/// Pure no_std configuration for EmotionalState.
/// Built explicitly on MCU targets; built via `from_env()` on desktop/Android.
#[derive(Debug, Clone, Copy)]
pub struct PainConfig {
    pub decay_rate: f64,               // per-tick decay of fear contribution from each pain
    pub motor_dampening_enabled: bool, // if true, motor_dampening() returns pain-count-scaled factor
    pub reanchor_enabled: bool,
    pub reanchor_bounds: (f64, f64, f64, f64, f64, f64), // xmin,xmax,ymin,ymax,zmin,zmax
    pub world_anchor: Option<(f64, f64, f64, f64)>,      // px,py,pz,heading_rad
}

impl PainConfig {
    /// Defaults suitable for MCU / no_std — matches legacy behavior when env vars unset.
    pub const fn new() -> Self {
        Self {
            decay_rate: 0.98,
            motor_dampening_enabled: false,
            reanchor_enabled: false,
            reanchor_bounds: (-4.0, 4.0, -3.0, 3.0, 0.5, 2.0),
            world_anchor: None,
        }
    }

    /// Build PainConfig from environment variables. Only available with std.
    #[cfg(feature = "std_env")]
    pub fn from_env() -> Self {
        let mut c = Self::new();
        if let Ok(s) = std::env::var("OASIS_PAIN_DECAY") {
            if let Ok(v) = s.parse::<f64>() { c.decay_rate = v; }
        }
        c.motor_dampening_enabled = std::env::var("OASIS_MOTOR_DAMPENING")
            .ok().map(|s| s == "1").unwrap_or(false);
        if let Ok(s) = std::env::var("OASIS_PAIN_WORLD_ANCHOR") {
            let p: Vec<f64> = s.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            if p.len() == 4 { c.world_anchor = Some((p[0], p[1], p[2], p[3])); }
        }
        if c.world_anchor.is_none() {
            c.reanchor_enabled = std::env::var("OASIS_PAIN_REANCHOR")
                .ok().map(|s| s == "1").unwrap_or(false);
            if c.reanchor_enabled {
                if let Ok(s) = std::env::var("OASIS_PAIN_REANCHOR_BOUNDS") {
                    let p: Vec<f64> = s.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                    if p.len() == 6 {
                        c.reanchor_bounds = (p[0], p[1], p[2], p[3], p[4], p[5]);
                    }
                }
            }
        }
        c
    }
}

impl Default for PainConfig {
    fn default() -> Self { Self::new() }
}

pub struct EmotionalState {
    pub curiosity: f64,
    pub fear: f64,
    pub satisfaction: f64,
    pub frustration: f64,
    pub urgency: f64,
    // Pain memory
    // pain_pos was `[V; MAX_PAIN]` inline = 131 KiB on stack — overflowed MCU
    // stacks (M0+ default ~8 KiB). Moved to heap-backed Vec<V> with fixed
    // length MAX_PAIN initialized at construction. Access pattern (indexing,
    // iter over slot i's V) is byte-identical via Deref; host tests stay green.
    // Other two arrays are <1.5 KiB, kept inline.
    pain_pos: Vec<V>,
    pain_intensity: [f64; MAX_PAIN],
    pain_tick: [u32; MAX_PAIN],
    pain_count: usize,
    // Goal distance history
    goal_dist: [f64; 8],
    goal_count: usize,
}

impl EmotionalState {
    pub fn new() -> Self {
        Self {
            curiosity: 0.0,
            fear: 0.0,
            satisfaction: 0.0,
            frustration: 0.0,
            urgency: 1.0,
            pain_pos: {
                // Init MAX_PAIN zero vectors on heap without any stack-resident
                // temporary array (would be 131 KiB and blow the MCU stack).
                let mut v = Vec::with_capacity(MAX_PAIN);
                for _ in 0..MAX_PAIN { v.push(vz()); }
                v
            },
            pain_intensity: [0.0; MAX_PAIN],
            pain_tick: [0; MAX_PAIN],
            pain_count: 0,
            goal_dist: [0.0; 8],
            goal_count: 0,
        }
    }

    /// Record a pain event at a position
    pub fn record_pain(&mut self, pos: &V, intensity: f64, tick: u32) {
        if self.pain_count < MAX_PAIN {
            self.pain_pos[self.pain_count] = *pos;
            self.pain_intensity[self.pain_count] = intensity;
            self.pain_tick[self.pain_count] = tick;
            self.pain_count += 1;
        } else {
            // Evict WEAKEST pain, not oldest — strong traumas persist (biological)
            let weakest = self.pain_intensity.iter().enumerate().min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap()).map(|(i, _)| i).unwrap_or(0);
            if intensity > self.pain_intensity[weakest] {
                self.pain_pos[weakest] = *pos;
                self.pain_intensity[weakest] = intensity;
                self.pain_tick[weakest] = tick;
            } // else: new pain too weak, ignored — buffer keeps strong memories
        }
    }

    /// Record goal distance for satisfaction tracking
    pub fn record_goal_distance(&mut self, dist: f64) {
        if self.goal_count < 8 {
            self.goal_dist[self.goal_count] = dist;
            self.goal_count += 1;
        } else {
            for i in 0..7 {
                self.goal_dist[i] = self.goal_dist[i + 1];
            }
            self.goal_dist[7] = dist;
        }
    }

    /// no_std-friendly update: takes explicit PainConfig instead of reading env.
    /// The std-compatible `update()` is a thin wrapper around this.
    pub fn update_with_config(&mut self, pos: &V, entropy: f64, tick: u32, cfg: &PainConfig) {
        // Curiosity: thrives in moderate entropy
        if entropy >= 0.3 && entropy <= 0.7 {
            self.curiosity = (self.curiosity + 0.1).min(1.0);
        } else {
            self.curiosity = (self.curiosity - 0.05).max(0.0);
        }
        let habituation = 1.0 / (1.0 + self.pain_count as f64 * 0.1);
        let mut max_fear = 0.0_f64;
        for i in 0..self.pain_count {
            let age = tick.saturating_sub(self.pain_tick[i]);
            let decay = cfg.decay_rate.powi(age as i32);
            let dist = vd(pos, &self.pain_pos[i]);
            if dist < 2.0 {
                let fear = self.pain_intensity[i] * decay * (1.0 - dist / 2.0) * habituation;
                if fear > max_fear { max_fear = fear; }
            }
        }
        self.fear = max_fear.min(5.0);

        if self.goal_count >= 3 {
            let (a, b, c) = (self.goal_dist[self.goal_count - 3], self.goal_dist[self.goal_count - 2], self.goal_dist[self.goal_count - 1]);
            if c < b && b < a { self.satisfaction = (self.satisfaction + 0.15).min(1.0); }
            else { self.satisfaction = (self.satisfaction - 0.05).max(0.0); }
        }
        self.curiosity *= 1.0 - self.fear * 0.7;
        self.frustration *= 1.0 - self.satisfaction * 0.8;
        self.curiosity = (self.curiosity + self.frustration * 0.3).min(1.0);
        if self.fear > 0.5 {
            self.urgency = (self.urgency * (1.0 + self.fear)).min(5.0);
        } else {
            self.urgency = (self.urgency * 0.9 + 1.0 * 0.1).max(1.0);
        }
    }

    /// no_std-friendly motor_dampening: takes config explicitly.
    pub fn motor_dampening_with_config(&self, cfg: &PainConfig) -> f64 {
        if !cfg.motor_dampening_enabled { return 1.0; }
        let n = self.pain_count as f64;
        (1.0 - (n * 0.003).min(0.4)).max(0.6)
    }

    /// Update all emotional signals based on current state
    pub fn update(&mut self, pos: &V, entropy: f64, tick: u32) {
        // Curiosity: thrives in moderate entropy
        if entropy >= 0.3 && entropy <= 0.7 {
            self.curiosity = (self.curiosity + 0.1).min(1.0);
        } else {
            self.curiosity = (self.curiosity - 0.05).max(0.0);
        }

        // Fear: proximity to pain + habituation (O(n), not O(n²))
        // More pain memories = more familiar = less scary per memory
        let habituation = 1.0 / (1.0 + self.pain_count as f64 * 0.1); // 0→1.0, 10→0.5, 100→0.09
        // Pain decay rate tunable via OASIS_PAIN_DECAY env var. Default 0.98 (aggressive ~34t halflife).
        // Setting e.g. 0.9999 gives ~7000t halflife, biologically plausible.
        #[cfg(feature = "std_env")]
        let decay_rate: f64 = std::env::var("OASIS_PAIN_DECAY")
            .ok().and_then(|s| s.parse().ok()).unwrap_or(0.98);
        #[cfg(not(feature = "std_env"))]
        let decay_rate: f64 = 0.98;
        let mut max_fear = 0.0_f64;
        for i in 0..self.pain_count {
            let age = tick.saturating_sub(self.pain_tick[i]);
            let decay = decay_rate.powi(age as i32);
            let dist = vd(pos, &self.pain_pos[i]);
            if dist < 2.0 {
                let fear = self.pain_intensity[i] * decay * (1.0 - dist / 2.0) * habituation;
                if fear > max_fear {
                    max_fear = fear;
                }
            }
        }
        self.fear = max_fear.min(5.0);

        // Satisfaction: goal distance decreasing over time
        if self.goal_count >= 3 {
            let (a, b, c) = (self.goal_dist[self.goal_count - 3], self.goal_dist[self.goal_count - 2], self.goal_dist[self.goal_count - 1]);
            if c < b && b < a {
                self.satisfaction = (self.satisfaction + 0.15).min(1.0);
            } else {
                self.satisfaction = (self.satisfaction - 0.05).max(0.0);
            }
        }

        // Cross-modulation
        self.curiosity *= 1.0 - self.fear * 0.7;
        self.frustration *= 1.0 - self.satisfaction * 0.8;
        self.curiosity = (self.curiosity + self.frustration * 0.3).min(1.0);

        // Urgency from fear — rises fast, decays slow
        if self.fear > 0.5 {
            self.urgency = (self.urgency * (1.0 + self.fear)).min(5.0);
        } else {
            self.urgency = (self.urgency * 0.9 + 1.0 * 0.1).max(1.0);
        }
    }

    // ─── Pain Memory Persistence ───────────────────────────────
    //
    // Format: OASISPAIN\x01 + count(u16) + [pain]*
    // Pain: ndims(u8) + [(idx(u8), val(f64))]* + intensity(f64) + tick(u32)

    const PAIN_MAGIC: &'static [u8] = b"OASISPAIN\x01";

    /// Save pain memories to file
    #[cfg(feature = "std")]
    pub fn save_pain(&self, path: &str) -> Result<(), &'static str> {
        let mut buf: Vec<u8> = Vec::with_capacity(4096);
        buf.extend_from_slice(Self::PAIN_MAGIC);
        let count = self.pain_count.min(MAX_PAIN) as u16;
        buf.extend_from_slice(&count.to_le_bytes());

        for i in 0..count as usize {
            let nz: Vec<(u8, f64)> = self.pain_pos[i].iter().enumerate().filter(|(_, &v)| v.abs() > 1e-6).map(|(idx, &v)| (idx as u8, v)).collect();
            buf.push(nz.len().min(255) as u8);
            for &(idx, val) in &nz {
                buf.push(idx);
                buf.extend_from_slice(&val.to_le_bytes());
            }
            buf.extend_from_slice(&self.pain_intensity[i].to_le_bytes());
            buf.extend_from_slice(&self.pain_tick[i].to_le_bytes());
        }
        std::fs::write(path, &buf).map_err(|_| "write failed")
    }

    /// Load pain memories from file.
    ///
    /// Three anchor modes (priority order):
    /// 1. OASIS_PAIN_WORLD_ANCHOR="px,py,pz,heading_rad" — WORLD-FRAME conversion:
    ///    for each loaded pain, assume stored xyz is in save-time body frame,
    ///    convert to world as `world[i] = anchor + R(heading) * body[i]`.
    ///    This is the DEFENSE-USEFUL mode (phone records pain; drone uses pain in world coords).
    /// 2. OASIS_PAIN_REANCHOR=1 — random remap to arena bounds (test kludge).
    /// 3. Neither — use stored positions as-is (may cause coordinate mismatch).
    #[cfg(feature = "std")]
    pub fn load_pain(&mut self, path: &str) -> Result<u32, &'static str> {
        // World-frame anchor takes priority over random reanchor.
        let world_anchor: Option<(f64, f64, f64, f64)> =
            std::env::var("OASIS_PAIN_WORLD_ANCHOR").ok().and_then(|s| {
                let p: Vec<f64> = s.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                if p.len() == 4 { Some((p[0], p[1], p[2], p[3])) } else { None }
            });
        let reanchor: bool = world_anchor.is_none() && std::env::var("OASIS_PAIN_REANCHOR")
            .ok().map(|s| s == "1").unwrap_or(false);
        let (xmin, xmax, ymin, ymax, zmin, zmax) = if reanchor {
            let bounds_str = std::env::var("OASIS_PAIN_REANCHOR_BOUNDS")
                .unwrap_or_else(|_| "-4,4,-3,3,0.5,2.0".into());
            let parts: Vec<f64> = bounds_str.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            if parts.len() == 6 { (parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]) }
            else { (-4.0, 4.0, -3.0, 3.0, 0.5, 2.0) }
        } else { (0.0, 0.0, 0.0, 0.0, 0.0, 0.0) };
        // Deterministic LCG seeded from path hash for reproducibility across restarts
        let mut rng_state: u64 = path.bytes().map(|b| b as u64).sum::<u64>().wrapping_mul(2654435761);
        let mut next_rand = || -> f64 {
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((rng_state >> 33) as u32) as f64 / u32::MAX as f64
        };
        let data = std::fs::read(path).map_err(|_| "read failed")?;
        if data.len() < Self::PAIN_MAGIC.len() + 2 {
            return Err("too short");
        }
        if &data[..Self::PAIN_MAGIC.len()] != Self::PAIN_MAGIC {
            return Err("bad magic");
        }

        let mut pos = Self::PAIN_MAGIC.len();
        let count = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        let mut loaded = 0u32;

        for _ in 0..count.min(MAX_PAIN) {
            if pos >= data.len() {
                break;
            }
            let ndims = data[pos] as usize;
            pos += 1;

            let mut pain_pos = crate::vec::vz();
            for _ in 0..ndims {
                if pos + 9 > data.len() {
                    return Err("truncated");
                }
                let idx = data[pos] as usize;
                pos += 1;
                let val = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
                pos += 8;
                if idx < crate::vec::DIM {
                    pain_pos[idx] = val;
                }
            }
            if pos + 12 > data.len() {
                return Err("truncated");
            }
            let intensity = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
            pos += 8;
            let tick = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
            pos += 4;

            if self.pain_count < MAX_PAIN {
                if let Some((ax, ay, az, heading)) = world_anchor {
                    // World-frame conversion: rotate body-frame xyz by heading, translate by anchor.
                    // Standard 2D planar rotation in xy, z passed through.
                    let (bx, by, bz) = (pain_pos[0], pain_pos[1], pain_pos[2]);
                    let (cos_h, sin_h) = (heading.cos(), heading.sin());
                    let mut world_pos = crate::vec::vz();
                    world_pos[0] = ax + cos_h * bx - sin_h * by;
                    world_pos[1] = ay + sin_h * bx + cos_h * by;
                    world_pos[2] = az + bz;
                    self.pain_pos[self.pain_count] = world_pos;
                } else if reanchor {
                    let mut anchored = crate::vec::vz();
                    anchored[0] = xmin + (xmax - xmin) * next_rand();
                    anchored[1] = ymin + (ymax - ymin) * next_rand();
                    anchored[2] = zmin + (zmax - zmin) * next_rand();
                    self.pain_pos[self.pain_count] = anchored;
                } else {
                    self.pain_pos[self.pain_count] = pain_pos;
                }
                self.pain_intensity[self.pain_count] = intensity;
                self.pain_tick[self.pain_count] = tick;
                self.pain_count += 1;
                loaded += 1;
            }
        }
        Ok(loaded)
    }

    /// Count of pain memories
    /// Motor dampening factor — habituation of MOTOR RESPONSE based on sustained pain pressure.
    /// When many pain memories accumulate, motor commands are attenuated (biological withdrawal).
    /// Opt-in via OASIS_MOTOR_DAMPENING=1. Returns 1.0 if disabled (no-op).
    ///
    /// When enabled, dampening = 1.0 - min(0.4, pain_count * 0.003).
    /// With 128 pain memories → factor 0.616 (motor output at 62% of command).
    pub fn motor_dampening(&self) -> f64 {
        #[cfg(feature = "std_env")]
        let enabled: bool = std::env::var("OASIS_MOTOR_DAMPENING")
            .ok().map(|s| s == "1").unwrap_or(false);
        #[cfg(not(feature = "std_env"))]
        let enabled = false;
        if !enabled { return 1.0; }
        let n = self.pain_count as f64;
        (1.0 - (n * 0.003).min(0.4)).max(0.6)
    }

    pub fn pain_count(&self) -> usize {
        self.pain_count
    }

    /// Get dominant emotion name
    pub fn dominant(&self) -> &'static str {
        let vals = [self.curiosity, self.fear, self.satisfaction, self.frustration];
        let names = ["CUR", "FEAR", "SAT", "FRU"];
        let max = vals.iter().cloned().fold(0.0_f64, f64::max);
        if max < 0.05 {
            "CALM"
        } else {
            names[vals.iter().position(|&x| (x - max).abs() < 1e-9).unwrap_or(0)]
        }
    }
}

/// Pure function for fear saturation — floor at 0, ceiling at 5.
/// Kani-verifiable. Mirrors the `.min(5.0)` semantics used by EmotionalState.
#[inline]
pub fn saturate_fear(raw: f64) -> f64 {
    if raw.is_nan() { return 0.0; }
    raw.max(0.0).min(5.0)
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: saturate_fear output ∈ [0, 5] for any finite input.
    #[kani::proof]
    fn proof_m5_fear_bounded() {
        let raw: f64 = kani::any();
        kani::assume(raw.is_finite());
        let s = saturate_fear(raw);
        assert!(s >= 0.0);
        assert!(s <= 5.0);
    }

    /// PROVE: saturate_fear is IDEMPOTENT.
    #[kani::proof]
    fn proof_m5_fear_idempotent() {
        let raw: f64 = kani::any();
        kani::assume(raw.is_finite());
        let once = saturate_fear(raw);
        let twice = saturate_fear(once);
        assert!(once == twice);
    }

    /// PROVE: saturate_fear is MONOTONE.
    #[kani::proof]
    fn proof_m5_fear_monotone() {
        let a: f64 = kani::any();
        let b: f64 = kani::any();
        kani::assume(a.is_finite() && b.is_finite());
        kani::assume(a <= b);
        assert!(saturate_fear(a) <= saturate_fear(b));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_calm() {
        let emo = EmotionalState::new();
        assert_eq!(emo.dominant(), "CALM");
    }

    #[test]
    fn curiosity_in_moderate_entropy() {
        let mut emo = EmotionalState::new();
        let pos = vz();
        for _ in 0..10 {
            emo.update(&pos, 0.5, 1);
        }
        assert!(emo.curiosity > 0.5);
    }

    #[test]
    fn pain_memory_roundtrip() {
        let mut emo = EmotionalState::new();
        let mut pos1 = vz();
        pos1[10] = 0.5;
        pos1[22] = -0.3;
        let mut pos2 = vz();
        pos2[50] = 1.0;
        emo.record_pain(&pos1, 0.8, 100);
        emo.record_pain(&pos2, 0.3, 200);
        assert_eq!(emo.pain_count, 2);

        let path = "/tmp/oasis_test_pain.bin";
        emo.save_pain(path).unwrap();

        let mut emo2 = EmotionalState::new();
        let loaded = emo2.load_pain(path).unwrap();
        assert_eq!(loaded, 2);
        assert_eq!(emo2.pain_count, 2);

        // Verify positions preserved
        assert!((emo2.pain_pos[0][10] - 0.5).abs() < 1e-6);
        assert!((emo2.pain_pos[0][22] - (-0.3)).abs() < 1e-6);
        assert!((emo2.pain_pos[1][50] - 1.0).abs() < 1e-6);
        assert!((emo2.pain_intensity[0] - 0.8).abs() < 1e-6);
        assert_eq!(emo2.pain_tick[1], 200);

        // Verify loaded pain actually generates fear
        emo2.update(&pos1, 0.5, 101);
        assert!(emo2.fear > 0.3, "loaded pain should generate fear, got {}", emo2.fear);

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn pain_rejects_corrupt() {
        let path = "/tmp/oasis_test_pain_bad.bin";
        std::fs::write(path, b"NOT_PAIN_DATA").unwrap();
        let mut emo = EmotionalState::new();
        assert!(emo.load_pain(path).is_err());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn fear_near_pain() {
        let mut emo = EmotionalState::new();
        let pos = vz();
        emo.record_pain(&pos, 1.0, 1);
        emo.update(&pos, 0.5, 2);
        assert!(emo.fear > 0.5);
    }

    #[test]
    fn fear_saturates_at_5x() {
        let mut emo = EmotionalState::new();
        let pos = vz();
        // Stack 50 pain memories on top of each other
        for t in 0..50 {
            emo.record_pain(&pos, 1.0, t);
        }
        emo.update(&pos, 0.5, 51);
        assert!(emo.fear <= 5.0, "fear should saturate at 5.0, got {}", emo.fear);
    }

    #[test]
    fn fear_suppresses_curiosity() {
        let mut emo = EmotionalState::new();
        let pos = vz();
        emo.record_pain(&pos, 1.0, 1);
        for _ in 0..5 {
            emo.update(&pos, 0.5, 2);
        }
        assert!(emo.curiosity < 0.3, "fear should suppress curiosity");
    }

    #[test]
    fn no_std_config_pattern_works() {
        // Exercise update_with_config + motor_dampening_with_config — the no_std path.
        let mut cfg = PainConfig::new();
        cfg.decay_rate = 0.999; // slow decay for biological plausibility
        cfg.motor_dampening_enabled = true;

        let mut emo = EmotionalState::new();
        let mut pos = vz();
        pos[0] = 1.0;

        // Record 50 pain events at this position
        for t in 0..50 {
            emo.record_pain(&pos, 0.8, t);
        }

        // Update at same position → fear should rise
        emo.update_with_config(&pos, 0.5, 51, &cfg);
        assert!(emo.fear > 0.0, "fear should fire with re-anchored pain in range");

        // Motor dampening should kick in with 50 pain memories + enabled
        let damp = emo.motor_dampening_with_config(&cfg);
        assert!(damp < 1.0, "50 pain events + dampening enabled should give damp < 1, got {:.3}", damp);
        assert!(damp >= 0.6, "damp floor is 0.6");

        // Disable dampening → returns 1.0
        cfg.motor_dampening_enabled = false;
        let no_damp = emo.motor_dampening_with_config(&cfg);
        assert_eq!(no_damp, 1.0);
    }
}
