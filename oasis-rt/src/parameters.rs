//! OASIS — Parameter server (rclcpp `Parameter` equivalent).
//!
//! Typed, bounded, versioned key-value config store. Single-threaded.
//! Designed for drone / robot config (PID gains, thresholds, feature flags)
//! that is set once at boot or occasionally updated at runtime.
//!
//! ## Contract
//!
//! - `set` on a known key with the correct type: stores the value (clamped
//!   for numerics if a descriptor bound is set) and increments `version`.
//! - `set` with wrong type or unknown key: returns `Err`, leaves store
//!   untouched, version does NOT change.
//! - `get` is read-only; never fails if key+type match.
//! - `version` is monotonically non-decreasing. Consumers can poll it to
//!   detect config changes without subscribing.
//!
//! ## Why BTreeMap not HashMap
//!
//! Same reason as `timers.rs`: HashMap's random seed defeats Kani. BTreeMap
//! also gives stable iteration order for `list_keys()` — useful for dumping
//! config deterministically to logs.
//!
//! ## What this is NOT
//!
//! - Not a distributed config (no DDS broadcast). If you need federated
//!   config, wrap this server in a topic publisher.
//! - Not persisted. Caller's responsibility to snapshot/restore on boot.
//! - Not thread-safe by itself. Wrap in `Arc<Mutex<_>>` if shared.

#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Parameter {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
}

/// Bounds descriptor. Only meaningful for Int / Float parameters.
/// `None` means unbounded on that end.
#[derive(Debug, Clone, Default)]
pub struct NumericDescriptor {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug)]
pub enum ParamError {
    UnknownKey,
    TypeMismatch { expected: &'static str, got: &'static str },
    InvalidValue(&'static str),
}

/// Pure clamp for f64 bounds. Extracted for Kani verification.
/// Semantics: if min is Some and v < min → min; if max is Some and v > max → max.
#[inline]
pub fn clamp_float(v: f64, min: Option<f64>, max: Option<f64>) -> f64 {
    let mut out = v;
    if let Some(lo) = min {
        if out < lo {
            out = lo;
        }
    }
    if let Some(hi) = max {
        if out > hi {
            out = hi;
        }
    }
    out
}

/// Pure clamp for i64 bounds. Note: min/max are f64 to match descriptor;
/// we saturate-cast to i64 range.
#[inline]
pub fn clamp_int(v: i64, min: Option<f64>, max: Option<f64>) -> i64 {
    let mut out = v;
    if let Some(lo) = min {
        let lo_i = if lo < i64::MIN as f64 {
            i64::MIN
        } else if lo > i64::MAX as f64 {
            i64::MAX
        } else {
            lo as i64
        };
        if out < lo_i {
            out = lo_i;
        }
    }
    if let Some(hi) = max {
        let hi_i = if hi > i64::MAX as f64 {
            i64::MAX
        } else if hi < i64::MIN as f64 {
            i64::MIN
        } else {
            hi as i64
        };
        if out > hi_i {
            out = hi_i;
        }
    }
    out
}

fn type_name(p: &Parameter) -> &'static str {
    match p {
        Parameter::Bool(_) => "Bool",
        Parameter::Int(_) => "Int",
        Parameter::Float(_) => "Float",
        Parameter::String(_) => "String",
        Parameter::Bytes(_) => "Bytes",
    }
}

struct Entry {
    value: Parameter,
    descriptor: Option<NumericDescriptor>,
}

pub struct ParameterServer {
    entries: BTreeMap<String, Entry>,
    version: u64,
}

impl Default for ParameterServer {
    fn default() -> Self {
        Self::new()
    }
}

impl ParameterServer {
    pub fn new() -> Self {
        Self { entries: BTreeMap::new(), version: 0 }
    }

    /// Declare a parameter with an initial value and optional numeric
    /// descriptor. If the key already exists, returns Err (mirrors rclcpp's
    /// declare_parameter semantics).
    pub fn declare(&mut self, key: &str, value: Parameter, descriptor: Option<NumericDescriptor>) -> Result<(), ParamError> {
        if self.entries.contains_key(key) {
            return Err(ParamError::InvalidValue("key already declared"));
        }
        // Clamp initial value to descriptor bounds if numeric.
        let clamped = clamp_if_needed(value, &descriptor);
        self.entries.insert(key.to_string(), Entry { value: clamped, descriptor });
        self.version = self.version.wrapping_add(1);
        Ok(())
    }

    /// Set a declared parameter. Type must match the declared type.
    /// Numeric values are clamped to descriptor bounds.
    pub fn set(&mut self, key: &str, value: Parameter) -> Result<(), ParamError> {
        let entry = self.entries.get_mut(key).ok_or(ParamError::UnknownKey)?;
        let expected = type_name(&entry.value);
        let got = type_name(&value);
        if expected != got {
            return Err(ParamError::TypeMismatch { expected, got });
        }
        entry.value = clamp_if_needed(value, &entry.descriptor);
        self.version = self.version.wrapping_add(1);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&Parameter> {
        self.entries.get(key).map(|e| &e.value)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        if let Some(Parameter::Bool(b)) = self.get(key) {
            Some(*b)
        } else {
            None
        }
    }
    pub fn get_int(&self, key: &str) -> Option<i64> {
        if let Some(Parameter::Int(i)) = self.get(key) {
            Some(*i)
        } else {
            None
        }
    }
    pub fn get_float(&self, key: &str) -> Option<f64> {
        if let Some(Parameter::Float(f)) = self.get(key) {
            Some(*f)
        } else {
            None
        }
    }
    pub fn get_string(&self, key: &str) -> Option<&str> {
        if let Some(Parameter::String(s)) = self.get(key) {
            Some(s.as_str())
        } else {
            None
        }
    }

    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn list_keys(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }
}

fn clamp_if_needed(value: Parameter, desc: &Option<NumericDescriptor>) -> Parameter {
    match (value, desc) {
        (Parameter::Float(f), Some(d)) => Parameter::Float(clamp_float(f, d.min, d.max)),
        (Parameter::Int(i), Some(d)) => Parameter::Int(clamp_int(i, d.min, d.max)),
        (v, _) => v,
    }
}

// ==========================================================================
//                                TESTS
// ==========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declare_then_get_roundtrip() {
        let mut p = ParameterServer::new();
        p.declare("max_speed", Parameter::Float(5.0), None).unwrap();
        assert_eq!(p.get_float("max_speed"), Some(5.0));
    }

    #[test]
    fn declare_twice_fails() {
        let mut p = ParameterServer::new();
        p.declare("k", Parameter::Int(1), None).unwrap();
        assert!(p.declare("k", Parameter::Int(2), None).is_err());
    }

    #[test]
    fn set_updates_value_and_bumps_version() {
        let mut p = ParameterServer::new();
        p.declare("gain", Parameter::Float(1.0), None).unwrap();
        let v0 = p.version();
        p.set("gain", Parameter::Float(2.5)).unwrap();
        assert_eq!(p.get_float("gain"), Some(2.5));
        assert!(p.version() > v0);
    }

    #[test]
    fn set_type_mismatch_rejects_and_keeps_version() {
        let mut p = ParameterServer::new();
        p.declare("gain", Parameter::Float(1.0), None).unwrap();
        let v0 = p.version();
        assert!(p.set("gain", Parameter::Int(42)).is_err());
        // Value unchanged
        assert_eq!(p.get_float("gain"), Some(1.0));
        // Version unchanged
        assert_eq!(p.version(), v0);
    }

    #[test]
    fn set_unknown_key_rejected() {
        let mut p = ParameterServer::new();
        let v0 = p.version();
        assert!(p.set("nope", Parameter::Int(1)).is_err());
        assert_eq!(p.version(), v0);
    }

    #[test]
    fn float_clamps_to_descriptor_bounds() {
        let mut p = ParameterServer::new();
        let d = NumericDescriptor { min: Some(0.0), max: Some(10.0) };
        p.declare("speed", Parameter::Float(5.0), Some(d)).unwrap();
        p.set("speed", Parameter::Float(99.0)).unwrap();
        assert_eq!(p.get_float("speed"), Some(10.0));
        p.set("speed", Parameter::Float(-5.0)).unwrap();
        assert_eq!(p.get_float("speed"), Some(0.0));
    }

    #[test]
    fn int_clamps_to_descriptor_bounds() {
        let mut p = ParameterServer::new();
        let d = NumericDescriptor { min: Some(1.0), max: Some(100.0) };
        p.declare("retries", Parameter::Int(5), Some(d)).unwrap();
        p.set("retries", Parameter::Int(500)).unwrap();
        assert_eq!(p.get_int("retries"), Some(100));
        p.set("retries", Parameter::Int(-1)).unwrap();
        assert_eq!(p.get_int("retries"), Some(1));
    }

    #[test]
    fn declare_clamps_initial_value() {
        let mut p = ParameterServer::new();
        let d = NumericDescriptor { min: Some(0.0), max: Some(1.0) };
        // Declare with out-of-bounds initial — should be clamped.
        p.declare("x", Parameter::Float(5.0), Some(d)).unwrap();
        assert_eq!(p.get_float("x"), Some(1.0));
    }

    #[test]
    fn bool_and_string_and_bytes_roundtrip() {
        let mut p = ParameterServer::new();
        p.declare("enabled", Parameter::Bool(true), None).unwrap();
        p.declare("name", Parameter::String("drone-42".into()), None).unwrap();
        p.declare("secret", Parameter::Bytes(vec![1, 2, 3]), None).unwrap();
        assert_eq!(p.get_bool("enabled"), Some(true));
        assert_eq!(p.get_string("name"), Some("drone-42"));
        if let Some(Parameter::Bytes(b)) = p.get("secret") {
            assert_eq!(b, &vec![1, 2, 3]);
        } else {
            panic!();
        }
    }

    #[test]
    fn list_keys_is_sorted() {
        let mut p = ParameterServer::new();
        p.declare("zzz", Parameter::Int(1), None).unwrap();
        p.declare("aaa", Parameter::Int(2), None).unwrap();
        p.declare("mmm", Parameter::Int(3), None).unwrap();
        assert_eq!(p.list_keys(), vec!["aaa", "mmm", "zzz"]);
    }

    #[test]
    fn bench_get_float_latency() {
        let mut p = ParameterServer::new();
        for i in 0..50 {
            p.declare(&format!("key_{}", i), Parameter::Float(i as f64), None).unwrap();
        }
        const N: u32 = 100_000;
        let start = std::time::Instant::now();
        let mut sum = 0.0;
        for i in 0..N {
            let k = format!("key_{}", i % 50);
            if let Some(v) = p.get_float(&k) {
                sum += v;
            }
        }
        let per_ns = start.elapsed().as_nanos() as f64 / N as f64;
        eprintln!("parameter get_float (50 keys): {:.0} ns/op (sum={})", per_ns, sum);
        assert!(per_ns < 5_000.0, "get too slow: {} ns", per_ns);
    }
}

// ==========================================================================
//                              KANI PROOFS
//
// Container-free: all proofs target the pure `clamp_float` / `clamp_int`
// helpers. BTreeMap+String state is not Kani-tractable (drop loops, String
// alloc). The clamp functions are where ALL the numeric invariants live.
// ==========================================================================

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// PROVE: clamp_float with bounded inputs always produces output in [min, max].
    #[kani::proof]
    fn proof_params_clamp_float_respects_bounds() {
        let v: f64 = kani::any();
        let lo: f64 = kani::any();
        let hi: f64 = kani::any();
        kani::assume(v.is_finite() && lo.is_finite() && hi.is_finite());
        kani::assume(lo <= hi);
        kani::assume(v.abs() < 1e9 && lo.abs() < 1e9 && hi.abs() < 1e9);

        let out = clamp_float(v, Some(lo), Some(hi));
        assert!(out >= lo);
        assert!(out <= hi);
    }

    /// PROVE: clamp_float with no bounds is the identity function.
    #[kani::proof]
    fn proof_params_clamp_float_unbounded_is_identity() {
        let v: f64 = kani::any();
        kani::assume(v.is_finite());
        let out = clamp_float(v, None, None);
        assert_eq!(out, v);
    }

    /// PROVE: clamp_float is idempotent — clamping twice equals clamping once.
    #[kani::proof]
    fn proof_params_clamp_float_idempotent() {
        let v: f64 = kani::any();
        let lo: f64 = kani::any();
        let hi: f64 = kani::any();
        kani::assume(v.is_finite() && lo.is_finite() && hi.is_finite());
        kani::assume(lo <= hi);
        kani::assume(v.abs() < 1e9 && lo.abs() < 1e9 && hi.abs() < 1e9);

        let once = clamp_float(v, Some(lo), Some(hi));
        let twice = clamp_float(once, Some(lo), Some(hi));
        assert_eq!(once, twice);
    }

    /// PROVE: clamp_int respects bounded integer bounds.
    #[kani::proof]
    fn proof_params_clamp_int_respects_bounds() {
        let v: i64 = kani::any();
        let lo_raw: i32 = kani::any();
        let hi_raw: i32 = kani::any();
        kani::assume(lo_raw <= hi_raw);
        let lo = lo_raw as f64;
        let hi = hi_raw as f64;

        let out = clamp_int(v, Some(lo), Some(hi));
        assert!(out >= lo_raw as i64);
        assert!(out <= hi_raw as i64);
    }

    /// PROVE: clamp_int with value already in bounds is a pass-through.
    #[kani::proof]
    fn proof_params_clamp_int_in_bounds_is_identity() {
        let v: i32 = kani::any(); // bounded domain so f64 conversion is exact
        let lo_raw: i32 = kani::any();
        let hi_raw: i32 = kani::any();
        kani::assume(lo_raw <= v && v <= hi_raw);

        let out = clamp_int(v as i64, Some(lo_raw as f64), Some(hi_raw as f64));
        assert_eq!(out, v as i64);
    }
}
