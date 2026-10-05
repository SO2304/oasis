//! OASIS-RT — N-dimensional vector algebra
//!
//! All vectors are stack-allocated [f64; DIM]. Zero heap allocation.
//! Hot-path ops use #[inline(always)] for cross-module inlining.

#[cfg(not(feature = "std"))]
use crate::fmath::F64Ext;

pub const DIM: usize = 128;
pub type V = [f64; DIM];

#[inline(always)]
pub fn vz() -> V {
    [0.0; DIM]
}

/// Accumulate: d += s
#[inline(always)]
pub fn vacc(d: &mut V, s: &V) {
    for i in 0..DIM {
        d[i] += s[i];
    }
}

/// Scale-into: d = s * k
#[inline(always)]
pub fn vsi(d: &mut V, s: &V, k: f64) {
    for i in 0..DIM {
        d[i] = s[i] * k;
    }
}

/// Norm (magnitude)
#[inline(always)]
pub fn vn(v: &V) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Distance between two vectors
#[inline(always)]
pub fn vd(a: &V, b: &V) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum::<f64>().sqrt()
}

/// Cosine similarity
#[inline(always)]
pub fn vcos(a: &V, b: &V) -> f64 {
    let (mut d, mut na, mut nb) = (0.0, 0.0, 0.0);
    for i in 0..DIM {
        d += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let dn = na.sqrt() * nb.sqrt();
    if dn < 1e-12 {
        0.0
    } else {
        d / dn
    }
}

/// Normalize to unit vector
#[inline(always)]
pub fn vnorm(v: &V) -> V {
    let n = vn(v);
    if n < 1e-12 {
        return vz();
    }
    let mut r = vz();
    for i in 0..DIM {
        r[i] = v[i] / n;
    }
    r
}

/// Add two vectors (allocating)
#[inline(always)]
pub fn vadd(a: &V, b: &V) -> V {
    let mut r = vz();
    for i in 0..DIM {
        r[i] = a[i] + b[i];
    }
    r
}

/// Subtract: a - b
#[inline(always)]
pub fn vsub(a: &V, b: &V) -> V {
    let mut r = vz();
    for i in 0..DIM {
        r[i] = a[i] - b[i];
    }
    r
}

/// Scale (allocating)
#[inline(always)]
pub fn vscale(v: &V, k: f64) -> V {
    let mut r = vz();
    for i in 0..DIM {
        r[i] = v[i] * k;
    }
    r
}

/// Dot product
#[inline(always)]
pub fn vdot(a: &V, b: &V) -> f64 {
    let mut d = 0.0;
    for i in 0..DIM {
        d += a[i] * b[i];
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_vector() {
        let z = vz();
        assert_eq!(vn(&z), 0.0);
    }

    #[test]
    fn norm_unit() {
        let mut v = vz();
        v[0] = 3.0;
        v[1] = 4.0;
        assert!((vn(&v) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn cosine_identical() {
        let mut v = vz();
        v[10] = 1.0;
        v[11] = 2.0;
        assert!((vcos(&v, &v) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn cosine_opposite() {
        let mut a = vz();
        a[0] = 1.0;
        let mut b = vz();
        b[0] = -1.0;
        assert!((vcos(&a, &b) + 1.0).abs() < 1e-10);
    }

    #[test]
    fn cosine_orthogonal() {
        let mut a = vz();
        a[0] = 1.0;
        let mut b = vz();
        b[1] = 1.0;
        assert!(vcos(&a, &b).abs() < 1e-10);
    }

    #[test]
    fn normalize_preserves_direction() {
        let mut v = vz();
        v[5] = 3.0;
        v[6] = 4.0;
        let n = vnorm(&v);
        assert!((vn(&n) - 1.0).abs() < 1e-10);
        assert!((vcos(&v, &n) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn distance_symmetric() {
        let mut a = vz();
        a[0] = 1.0;
        let mut b = vz();
        b[0] = 4.0;
        assert!((vd(&a, &b) - 3.0).abs() < 1e-10);
        assert!((vd(&b, &a) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn add_sub_inverse() {
        let mut a = vz();
        a[0] = 1.0;
        a[1] = 2.0;
        let mut b = vz();
        b[0] = 3.0;
        b[1] = -1.0;
        let sum = vadd(&a, &b);
        let back = vsub(&sum, &b);
        assert!((back[0] - a[0]).abs() < 1e-10);
        assert!((back[1] - a[1]).abs() < 1e-10);
    }
}
