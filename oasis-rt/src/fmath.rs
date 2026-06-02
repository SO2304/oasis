//! Floating-point math facade for std + no_std targets.
//!
//! On std, `f64` has inherent methods `cos`, `sin`, `sqrt`, etc. — they
//! live in `std`, not `core`. On no_std they don't resolve. This module
//! provides `F64Ext`, an extension trait with the SAME method names that
//! delegates to `libm`. On std, the trait is empty (inherent methods
//! take precedence). On no_std, the trait fills in the gaps.
//!
//! Usage in any module that needs cross-target trig/sqrt/exp:
//!
//! ```ignore
//! #[cfg(not(feature = "std"))]
//! use crate::fmath::F64Ext;
//! // Now `x.cos()` etc. resolve on both std and no_std.
//! ```
//!
//! NOTE: `powi(n: i32)` is delegated via `libm::pow` with f64 cast on
//! no_std — bit-equivalent for integer powers within range, but a
//! marginally slower path. For perf-critical hot loops, prefer manual
//! multiplication on no_std.

#[cfg(not(feature = "std"))]
pub trait F64Ext {
    fn cos(self) -> f64;
    fn sin(self) -> f64;
    fn atan2(self, other: f64) -> f64;
    fn sqrt(self) -> f64;
    fn exp(self) -> f64;
    fn ln(self) -> f64;
    fn powi(self, n: i32) -> f64;
    fn abs(self) -> f64;
    fn floor(self) -> f64;
    fn ceil(self) -> f64;
    fn round(self) -> f64;
    fn powf(self, n: f64) -> f64;
}

#[cfg(not(feature = "std"))]
impl F64Ext for f64 {
    #[inline]
    fn cos(self) -> f64 {
        libm::cos(self)
    }
    #[inline]
    fn sin(self) -> f64 {
        libm::sin(self)
    }
    #[inline]
    fn atan2(self, other: f64) -> f64 {
        libm::atan2(self, other)
    }
    #[inline]
    fn sqrt(self) -> f64 {
        libm::sqrt(self)
    }
    #[inline]
    fn exp(self) -> f64 {
        libm::exp(self)
    }
    #[inline]
    fn ln(self) -> f64 {
        libm::log(self)
    }
    #[inline]
    fn powi(self, n: i32) -> f64 {
        libm::pow(self, n as f64)
    }
    #[inline]
    fn abs(self) -> f64 {
        libm::fabs(self)
    }
    #[inline]
    fn floor(self) -> f64 {
        libm::floor(self)
    }
    #[inline]
    fn ceil(self) -> f64 {
        libm::ceil(self)
    }
    #[inline]
    fn round(self) -> f64 {
        libm::round(self)
    }
    #[inline]
    fn powf(self, n: f64) -> f64 {
        libm::pow(self, n)
    }
}
