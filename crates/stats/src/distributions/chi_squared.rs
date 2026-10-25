//! Chi-squared distribution.
//!
//! The CDF rides on the regularized lower incomplete gamma:
//!   `P(χ²_k ≤ x) = P(k/2, x/2)`.
//!
//! Reference (behaviour): Abramowitz & Stegun §26.4 and the
//! Wikipedia article "Chi-squared distribution"
//! (<https://en.wikipedia.org/wiki/Chi-squared_distribution>). Pure
//! library math; no third-party or GraphPad code.

use graphcraft_numeric::NumericError;

use crate::special::inc_gamma_reg_p;

/// Chi-squared cumulative distribution function.
pub fn chi2_cdf(x: f64, df: f64) -> Result<f64, NumericError> {
    if !x.is_finite() || !df.is_finite() || df <= 0.0 || x < 0.0 {
        return Err(NumericError::NotFinite);
    }
    if x == 0.0 {
        return Ok(0.0);
    }
    inc_gamma_reg_p(0.5 * df, 0.5 * x)
}

/// Chi-squared survival function.
pub fn chi2_sf(x: f64, df: f64) -> Result<f64, NumericError> {
    Ok(1.0 - chi2_cdf(x, df)?)
}

/// Chi-squared inverse CDF via bisection.
pub fn chi2_inv_cdf(p: f64, df: f64) -> Result<f64, NumericError> {
    if !p.is_finite() || !df.is_finite() || df <= 0.0 || !(0.0..=1.0).contains(&p) {
        return Err(NumericError::NotFinite);
    }
    if p == 0.0 {
        return Ok(0.0);
    }
    if p == 1.0 {
        return Ok(f64::INFINITY);
    }
    let mut lo = 0.0_f64;
    let mut hi = (df + 10.0_f64 * df.sqrt()).max(10.0);
    while chi2_cdf(hi, df)? < p && hi < 1.0e300 {
        hi *= 2.0;
    }
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if hi - lo < 1.0e-14 * (1.0 + hi.abs()) {
            return Ok(mid);
        }
        if chi2_cdf(mid, df)? < p {
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
    fn chi2_df2_closed_form() {
        // P(χ²₂ ≤ x) = 1 − exp(−x/2).
        for x in [0.5_f64, 1.0, 3.0, 7.0] {
            let got = chi2_cdf(x, 2.0).unwrap();
            let expected = 1.0 - (-0.5 * x).exp();
            assert!((got - expected).abs() < 1e-13, "x={x}");
        }
    }

    #[test]
    fn chi2_df2_95th_percentile_closed_form() {
        // 95th percentile at df=2 is 2·ln(20) = −2·ln(0.05).
        let expected = 2.0 * 20.0_f64.ln();
        let got = chi2_inv_cdf(0.95, 2.0).unwrap();
        assert!((got - expected).abs() < 1e-10, "got={got} exp={expected}");
    }

    #[test]
    fn chi2_monotone() {
        let a = chi2_cdf(1.0, 5.0).unwrap();
        let b = chi2_cdf(5.0, 5.0).unwrap();
        let c = chi2_cdf(20.0, 5.0).unwrap();
        assert!(a < b && b < c);
    }

    #[test]
    fn chi2_inv_round_trip() {
        for p in [0.01_f64, 0.1, 0.5, 0.9, 0.99] {
            for df in [1.0_f64, 2.5, 5.0, 20.0] {
                let x = chi2_inv_cdf(p, df).unwrap();
                let back = chi2_cdf(x, df).unwrap();
                assert!((back - p).abs() < 1e-10, "p={p} df={df} x={x} back={back}");
            }
        }
    }
}
