//! Student's t-distribution.
//!
//! The CDF is computed through the regularized incomplete beta
//! identity:
//!   `F(t; ν) = 1 − 0.5 · I_{x}(ν/2, 1/2)` for `t ≥ 0`, with
//!   `x = ν / (ν + t²)` and the ν>0 real-valued degrees of freedom.
//!   Reflection handles `t < 0`.
//!
//! The inverse CDF uses bisection on the CDF — the Student-t CDF
//! is monotone, so bisection is unconditionally safe and converges
//! to full double precision in ≤ 70 iterations over `[-200, 200]`.
//! A Newton polish would halve iteration count but adds complexity
//! that M3.1 doesn't yet need.
//!
//! Reference (behaviour): the identity is standard; see the
//! Wikipedia article "Student's t-distribution"
//! (<https://en.wikipedia.org/wiki/Student%27s_t-distribution>) and
//! Abramowitz & Stegun §26.7.

use graphcraft_numeric::NumericError;

use crate::special::inc_beta_reg;

/// Student-t cumulative distribution function `P(T ≤ t)` with real
/// `df > 0`.
pub fn t_cdf(t: f64, df: f64) -> Result<f64, NumericError> {
    if !t.is_finite() || !df.is_finite() || df <= 0.0 {
        return Err(NumericError::NotFinite);
    }
    if t == 0.0 {
        return Ok(0.5);
    }
    let x = df / (df + t * t);
    let ib = inc_beta_reg(x, 0.5 * df, 0.5)?;
    if t > 0.0 {
        Ok(1.0 - 0.5 * ib)
    } else {
        Ok(0.5 * ib)
    }
}

/// Student-t survival function `P(T > t) = 1 − F(t)`.
pub fn t_sf(t: f64, df: f64) -> Result<f64, NumericError> {
    Ok(1.0 - t_cdf(t, df)?)
}

/// Student-t inverse CDF (quantile function). `p` in `(0, 1)`;
/// `df > 0`. Boundary values return `±∞`.
pub fn t_inv_cdf(p: f64, df: f64) -> Result<f64, NumericError> {
    if !p.is_finite() || !df.is_finite() || df <= 0.0 || !(0.0..=1.0).contains(&p) {
        return Err(NumericError::NotFinite);
    }
    if p == 0.0 {
        return Ok(f64::NEG_INFINITY);
    }
    if p == 1.0 {
        return Ok(f64::INFINITY);
    }
    if p == 0.5 {
        return Ok(0.0);
    }
    // Reflect to upper tail for better numerical behaviour on both
    // sides; bisection on the straight CDF would work too.
    let (p_target, sign) = if p < 0.5 { (1.0 - p, -1.0) } else { (p, 1.0) };

    let mut lo = 0.0_f64;
    let mut hi = 200.0_f64;
    // Expand `hi` just in case (should never trigger for df ≥ 1 at
    // sensible p — kept as belt-and-braces).
    while t_cdf(hi, df)? < p_target && hi < 1.0e300 {
        hi *= 2.0;
    }
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if hi - lo < 1.0e-14 * (1.0 + hi.abs()) {
            return Ok(sign * mid);
        }
        let f = t_cdf(mid, df)?;
        if f < p_target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(sign * 0.5 * (lo + hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_cdf_at_zero_is_half() {
        for df in [1.0, 5.0, 10.0, 200.0] {
            assert!((t_cdf(0.0, df).unwrap() - 0.5).abs() < 1e-15);
        }
    }

    #[test]
    fn t_cdf_symmetry() {
        for &(t, df) in &[(0.5, 10.0), (2.1, 5.0), (3.0, 1.0)] {
            let left = t_cdf(-t, df).unwrap();
            let right = t_cdf(t, df).unwrap();
            assert!((left + right - 1.0).abs() < 1e-13);
        }
    }

    #[test]
    fn t_cdf_df_one_is_cauchy() {
        // Cauchy CDF: F(t) = 1/2 + arctan(t)/π.
        for t in [-3.0_f64, -0.3, 0.4, 2.5, 12.0] {
            let expected = 0.5 + t.atan() / std::f64::consts::PI;
            let got = t_cdf(t, 1.0).unwrap();
            assert!((got - expected).abs() < 1e-13, "df=1 t={t}");
        }
    }

    #[test]
    fn t_inv_cdf_round_trip() {
        for p in [0.01, 0.1, 0.4, 0.5, 0.6, 0.95, 0.999] {
            for df in [1.0, 2.5, 10.0, 100.0] {
                let q = t_inv_cdf(p, df).unwrap();
                let p_back = t_cdf(q, df).unwrap();
                assert!(
                    (p_back - p).abs() < 1e-11,
                    "p={p} df={df} q={q} p_back={p_back}"
                );
            }
        }
    }

    #[test]
    fn t_inv_cdf_df1_cauchy_closed_form() {
        // Cauchy quantile: F^{-1}(p) = tan(π(p - 1/2)).
        let p = 0.975;
        let expected = (std::f64::consts::PI * (p - 0.5)).tan();
        let got = t_inv_cdf(p, 1.0).unwrap();
        assert!(
            (got - expected).abs() < 1e-10,
            "got={got} expected={expected}"
        );
    }
}
