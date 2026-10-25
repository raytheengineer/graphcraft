//! Fisher–Snedecor F distribution.
//!
//! CDF is the standard incomplete-beta identity:
//!   `F(x; d1, d2) = I_u(d1/2, d2/2)` with
//!   `u = d1·x / (d1·x + d2)`.
//! Reference (behaviour): the identity is standard; see
//! Abramowitz & Stegun §26.6 and the Wikipedia article
//! "F-distribution" (<https://en.wikipedia.org/wiki/F-distribution>).
//!
//! The inverse CDF uses bisection on the monotone CDF, matching the
//! t-distribution module. `d1, d2` are real-valued (`> 0`) so this
//! file also serves non-integer-df cases that Welch-style corrections
//! produce in two-way designs later on.

use graphcraft_numeric::NumericError;

use crate::special::inc_beta_reg;

/// Fisher–Snedecor F cumulative distribution function.
pub fn f_cdf(x: f64, d1: f64, d2: f64) -> Result<f64, NumericError> {
    if !x.is_finite() || !d1.is_finite() || !d2.is_finite() || d1 <= 0.0 || d2 <= 0.0 || x < 0.0 {
        return Err(NumericError::NotFinite);
    }
    if x == 0.0 {
        return Ok(0.0);
    }
    let u = d1 * x / (d1 * x + d2);
    inc_beta_reg(u, 0.5 * d1, 0.5 * d2)
}

/// F survival function `P(X > x) = 1 − F(x)`.
pub fn f_sf(x: f64, d1: f64, d2: f64) -> Result<f64, NumericError> {
    Ok(1.0 - f_cdf(x, d1, d2)?)
}

/// F inverse CDF via bisection. `p ∈ [0, 1]`.
pub fn f_inv_cdf(p: f64, d1: f64, d2: f64) -> Result<f64, NumericError> {
    if !p.is_finite() || !d1.is_finite() || !d2.is_finite() {
        return Err(NumericError::NotFinite);
    }
    if d1 <= 0.0 || d2 <= 0.0 || !(0.0..=1.0).contains(&p) {
        return Err(NumericError::NotFinite);
    }
    if p == 0.0 {
        return Ok(0.0);
    }
    if p == 1.0 {
        return Ok(f64::INFINITY);
    }
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    while f_cdf(hi, d1, d2)? < p && hi < 1.0e300 {
        hi *= 2.0;
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if hi - lo < 1.0e-14 * (1.0 + hi.abs()) {
            return Ok(mid);
        }
        let f = f_cdf(mid, d1, d2)?;
        if f < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(0.5 * (lo + hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_cdf_at_zero() {
        assert_eq!(f_cdf(0.0, 2.0, 2.0).unwrap(), 0.0);
    }

    #[test]
    fn f_cdf_median_d1_d2_2() {
        // At d1 = d2 = 2 and x = 1, u = 2/(2+2) = 0.5, and
        // I_{0.5}(1, 1) = 0.5 (B(1,1) is uniform on [0,1]).
        let got = f_cdf(1.0, 2.0, 2.0).unwrap();
        assert!((got - 0.5).abs() < 1e-14, "got={got}");
    }

    #[test]
    fn f_cdf_median_d1_d2_1() {
        // At d1 = d2 = 1 and x = 1, u = 0.5, and
        // I_{0.5}(0.5, 0.5) = 0.5 (arcsine distribution symmetry).
        let got = f_cdf(1.0, 1.0, 1.0).unwrap();
        assert!((got - 0.5).abs() < 1e-13, "got={got}");
    }

    #[test]
    fn f_reciprocal_identity() {
        // F(x; d1, d2) + F(1/x; d2, d1) = 1 (reciprocal property).
        for &(x, d1, d2) in &[(2.0_f64, 3.0, 7.0), (0.5, 5.0, 2.0), (10.0, 2.0, 10.0)] {
            let a = f_cdf(x, d1, d2).unwrap();
            let b = f_cdf(1.0 / x, d2, d1).unwrap();
            assert!((a + b - 1.0).abs() < 1e-13, "x={x} d1={d1} d2={d2}");
        }
    }

    #[test]
    fn f_inv_cdf_round_trip() {
        for p in [0.01, 0.1, 0.5, 0.9, 0.99] {
            for &(d1, d2) in &[(2.0_f64, 2.0), (3.0, 5.0), (10.0, 20.0), (1.0, 100.0)] {
                let x = f_inv_cdf(p, d1, d2).unwrap();
                let back = f_cdf(x, d1, d2).unwrap();
                assert!(
                    (back - p).abs() < 1e-10,
                    "p={p} d1={d1} d2={d2} x={x} back={back}"
                );
            }
        }
    }
}
