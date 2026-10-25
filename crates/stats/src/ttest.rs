//! Student t-tests.
//!
//! M3.1 ships the one-sample test. Paired and Welch two-sample
//! tests land in M3.2.

use graphcraft_numeric::NumericError;

use crate::descriptive::Descriptives;
use crate::distributions::t::t_cdf;

/// Result of a one-sample t-test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OneSampleT {
    pub n: usize,
    pub mean: f64,
    pub sem: f64,
    pub t: f64,
    pub df: f64,
    /// Two-sided p-value `P(|T| ≥ |t_obs|)`.
    pub p_two_sided: f64,
}

/// One-sample Student t-test for `H₀: μ = μ₀`.
///
/// Returns `Err(TooFewValues { required: 2, got: n })` if `xs.len()`
/// is less than 2; `Err(NotFinite)` if any value is NaN/infinite or
/// if the sample variance is zero (`t` undefined).
pub fn one_sample_t(xs: &[f64], mu0: f64) -> Result<OneSampleT, NumericError> {
    if !mu0.is_finite() {
        return Err(NumericError::NotFinite);
    }
    let d = Descriptives::of(xs)?;
    if d.n < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: d.n,
        });
    }
    let sem = d.sem.ok_or(NumericError::TooFewValues {
        required: 2,
        got: d.n,
    })?;
    if sem == 0.0 {
        return Err(NumericError::DivisionByZero);
    }
    let t = (d.mean - mu0) / sem;
    let df = (d.n - 1) as f64;
    let p_two_sided = 2.0 * (1.0 - t_cdf(t.abs(), df)?);
    Ok(OneSampleT {
        n: d.n,
        mean: d.mean,
        sem,
        t,
        df,
        p_two_sided,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_sample_t_against_zero_textbook() {
        // Hand-worked example: xs = [2, 4, 4, 4, 5, 5, 7, 9],
        // mean = 5, sample sd = sqrt(32/7) ≈ 2.13809,
        // sem = sd/sqrt(8) ≈ 0.75593.
        // Test H0: μ = 0, two-sided.
        //   t = 5 / 0.75593 ≈ 6.6144
        //   df = 7.
        // t-distribution with df=7 at t=6.6144 — survival ≈ 0.000148
        // ⇒ two-sided p ≈ 0.000297.
        let r = one_sample_t(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0], 0.0).unwrap();
        assert_eq!(r.n, 8);
        assert_eq!(r.df, 7.0);
        assert!((r.mean - 5.0).abs() < 1e-15);
        let expected_t = 5.0 / ((32.0_f64 / 7.0).sqrt() / 8.0_f64.sqrt());
        assert!((r.t - expected_t).abs() < 1e-13);
        assert!(r.p_two_sided > 0.0 && r.p_two_sided < 1e-3);
    }

    #[test]
    fn one_sample_t_zero_variance_rejected() {
        let err = one_sample_t(&[3.0, 3.0, 3.0], 0.0).unwrap_err();
        assert_eq!(err, NumericError::DivisionByZero);
    }

    #[test]
    fn one_sample_t_too_few_rejected() {
        let err = one_sample_t(&[1.0], 0.0).unwrap_err();
        assert!(matches!(err, NumericError::TooFewValues { .. }));
    }
}
