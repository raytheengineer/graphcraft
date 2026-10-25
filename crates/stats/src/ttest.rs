//! Student t-tests.
//!
//! Three flavours ship in M3.2 alongside the M3.1 one-sample test:
//! - [`one_sample_t`] — `H₀: μ = μ₀` on a single sample.
//! - [`paired_t`] — paired-sample test (one-sample t on the pairwise
//!   differences).
//! - [`unpaired_t`] — two-sample test with [`TwoSampleKind::Welch`]
//!   (unequal variances, Welch-Satterthwaite df; the default per the
//!   public Prism manual) or [`TwoSampleKind::Pooled`] (equal
//!   variances assumed).
//!
//! Behaviour reference: Welch, B. L. (1947) "The generalization of
//! Student's problem when several different population variances are
//! involved", Biometrika 34, 28–35; the Satterthwaite df approximation
//! as given in Welch (1947) and in Satterthwaite (1946), Biometrics 2,
//! 110–114. Pooled form from Student (1908) "The probable error of a
//! mean", Biometrika 6, 1–25.

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

/// Which two-sample t-test formula to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwoSampleKind {
    /// Welch's test — unequal variances, Welch-Satterthwaite df.
    /// This is the default documented in the public Prism manual.
    Welch,
    /// Student's pooled-variance test — assumes equal variances.
    Pooled,
}

/// Result of a two-sample (unpaired) t-test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoSampleT {
    pub kind: TwoSampleKind,
    pub n1: usize,
    pub n2: usize,
    pub mean1: f64,
    pub mean2: f64,
    /// `mean2 − mean1` (second group minus first).
    pub mean_diff: f64,
    /// Standard error of the mean difference under the chosen model.
    pub se_diff: f64,
    pub t: f64,
    /// Real-valued df. Integer for Pooled, non-integer for Welch.
    pub df: f64,
    pub p_two_sided: f64,
}

/// Result of a paired t-test (one-sample t on `ys − xs`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairedT {
    pub n: usize,
    pub mean_diff: f64,
    pub sem_diff: f64,
    pub t: f64,
    pub df: f64,
    pub p_two_sided: f64,
}

/// One-sample Student t-test for `H₀: μ = μ₀`.
///
/// Returns `Err(TooFewValues { required: 2, got: n })` if `xs.len()`
/// is less than 2; `Err(NotFinite)` if any value is NaN/infinite or
/// `Err(DivisionByZero)` if the sample variance is zero.
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

/// Paired t-test on two equal-length samples. The test statistic is
/// the one-sample t on the pairwise differences `ys[i] − xs[i]`
/// against `μ = 0`.
///
/// `Err(TooFewValues)` if either sample has fewer than 2 values or if
/// the lengths disagree; `Err(DivisionByZero)` if all differences are
/// identical.
pub fn paired_t(xs: &[f64], ys: &[f64]) -> Result<PairedT, NumericError> {
    if xs.len() != ys.len() {
        return Err(NumericError::TooFewValues {
            required: xs.len(),
            got: ys.len(),
        });
    }
    let diffs: Vec<f64> = xs.iter().zip(ys.iter()).map(|(a, b)| b - a).collect();
    let r = one_sample_t(&diffs, 0.0)?;
    Ok(PairedT {
        n: r.n,
        mean_diff: r.mean,
        sem_diff: r.sem,
        t: r.t,
        df: r.df,
        p_two_sided: r.p_two_sided,
    })
}

/// Two-sample unpaired t-test. `mean_diff = mean(ys) − mean(xs)`.
///
/// For [`TwoSampleKind::Welch`] (Prism's documented default), the
/// standard error of the difference is
/// `√(s₁²/n₁ + s₂²/n₂)` and the Welch–Satterthwaite df is
/// `(s₁²/n₁ + s₂²/n₂)² / ((s₁²/n₁)²/(n₁−1) + (s₂²/n₂)²/(n₂−1))`.
///
/// For [`TwoSampleKind::Pooled`], the pooled variance is
/// `s_p² = ((n₁−1)s₁² + (n₂−1)s₂²) / (n₁+n₂−2)` and
/// `df = n₁+n₂−2`.
pub fn unpaired_t(xs: &[f64], ys: &[f64], kind: TwoSampleKind) -> Result<TwoSampleT, NumericError> {
    let d1 = Descriptives::of(xs)?;
    let d2 = Descriptives::of(ys)?;
    if d1.n < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: d1.n,
        });
    }
    if d2.n < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: d2.n,
        });
    }
    let v1 = d1.variance.ok_or(NumericError::TooFewValues {
        required: 2,
        got: d1.n,
    })?;
    let v2 = d2.variance.ok_or(NumericError::TooFewValues {
        required: 2,
        got: d2.n,
    })?;
    let n1 = d1.n as f64;
    let n2 = d2.n as f64;
    let mean_diff = d2.mean - d1.mean;

    let (se_diff, df) = match kind {
        TwoSampleKind::Welch => {
            let a = v1 / n1;
            let b = v2 / n2;
            let se = (a + b).sqrt();
            // Welch–Satterthwaite. Zero-variance groups make the
            // denominator vanish, which we reject rather than silently
            // returning ∞ df.
            let denom = a * a / (n1 - 1.0) + b * b / (n2 - 1.0);
            if denom == 0.0 {
                return Err(NumericError::DivisionByZero);
            }
            let df = (a + b).powi(2) / denom;
            (se, df)
        }
        TwoSampleKind::Pooled => {
            let df_total = n1 + n2 - 2.0;
            let sp2 = ((n1 - 1.0) * v1 + (n2 - 1.0) * v2) / df_total;
            let se = (sp2 * (1.0 / n1 + 1.0 / n2)).sqrt();
            (se, df_total)
        }
    };
    if !se_diff.is_finite() || se_diff == 0.0 {
        return Err(NumericError::DivisionByZero);
    }
    let t = mean_diff / se_diff;
    let p_two_sided = 2.0 * (1.0 - t_cdf(t.abs(), df)?);
    Ok(TwoSampleT {
        kind,
        n1: d1.n,
        n2: d2.n,
        mean1: d1.mean,
        mean2: d2.mean,
        mean_diff,
        se_diff,
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
        // Hand-worked: xs = [2, 4, 4, 4, 5, 5, 7, 9],
        // mean = 5, sample sd = sqrt(32/7), sem = sd/sqrt(8).
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

    #[test]
    fn paired_t_closed_form() {
        // xs = [1,2,3,4,5], ys = [2,4,6,8,10]. Diffs = [1,2,3,4,5],
        // mean = 3, sample var = 2.5, sem = sqrt(2.5)/sqrt(5) = sqrt(0.5).
        // t = 3 / sqrt(0.5) = sqrt(18); df = 4.
        let xs = [1.0, 2.0, 3.0, 4.0, 5.0];
        let ys = [2.0, 4.0, 6.0, 8.0, 10.0];
        let r = paired_t(&xs, &ys).unwrap();
        assert_eq!(r.n, 5);
        assert_eq!(r.df, 4.0);
        assert!((r.mean_diff - 3.0).abs() < 1e-15);
        assert!((r.t - 18.0_f64.sqrt()).abs() < 1e-13);
        // t≈4.24 on df=4 → two-sided p ≈ 0.013.
        assert!(r.p_two_sided > 0.0 && r.p_two_sided < 0.05);
    }

    #[test]
    fn paired_t_length_mismatch_rejected() {
        let err = paired_t(&[1.0, 2.0], &[1.0]).unwrap_err();
        assert!(matches!(err, NumericError::TooFewValues { .. }));
    }

    #[test]
    fn welch_closed_form() {
        // xs = [1,2,3,4,5]  (n1=5, mean=3, s² = 2.5)
        // ys = [10,12,14]   (n2=3, mean=12, s² = 4)
        // Welch: se² = 2.5/5 + 4/3 = 11/6; t = 9 / sqrt(11/6).
        // df   = (11/6)² / ((0.5)²/4 + (4/3)²/2) = 484/137.
        let xs = [1.0, 2.0, 3.0, 4.0, 5.0];
        let ys = [10.0, 12.0, 14.0];
        let r = unpaired_t(&xs, &ys, TwoSampleKind::Welch).unwrap();
        assert_eq!(r.n1, 5);
        assert_eq!(r.n2, 3);
        assert!((r.mean_diff - 9.0).abs() < 1e-14);
        let expected_t = 9.0 / (11.0_f64 / 6.0).sqrt();
        assert!(
            (r.t - expected_t).abs() < 1e-13,
            "t={} vs {}",
            r.t,
            expected_t
        );
        let expected_df = 484.0 / 137.0;
        assert!(
            (r.df - expected_df).abs() < 1e-13,
            "df={} vs {}",
            r.df,
            expected_df
        );
    }

    #[test]
    fn pooled_closed_form() {
        // Same xs, ys. Pooled: sp² = (4·2.5 + 2·4)/6 = 3;
        // se² = 3·(1/5 + 1/3) = 8/5; t = 9 / sqrt(8/5); df = 6.
        let xs = [1.0, 2.0, 3.0, 4.0, 5.0];
        let ys = [10.0, 12.0, 14.0];
        let r = unpaired_t(&xs, &ys, TwoSampleKind::Pooled).unwrap();
        assert_eq!(r.df, 6.0);
        let expected_t = 9.0 / (8.0_f64 / 5.0).sqrt();
        assert!(
            (r.t - expected_t).abs() < 1e-13,
            "t={} vs {}",
            r.t,
            expected_t
        );
    }

    #[test]
    fn welch_equal_variance_matches_pooled() {
        // When n1 = n2 and s1 = s2, Welch and Pooled give the same t.
        let xs = [10.0, 12.0, 14.0];
        let ys = [20.0, 22.0, 24.0];
        let w = unpaired_t(&xs, &ys, TwoSampleKind::Welch).unwrap();
        let p = unpaired_t(&xs, &ys, TwoSampleKind::Pooled).unwrap();
        assert!((w.t - p.t).abs() < 1e-14);
    }

    #[test]
    fn unpaired_both_zero_variance_rejected() {
        // Both groups constant ⇒ se_diff = 0, Welch df undefined.
        let err = unpaired_t(&[3.0, 3.0], &[1.0, 1.0], TwoSampleKind::Welch).unwrap_err();
        assert_eq!(err, NumericError::DivisionByZero);
        let err = unpaired_t(&[3.0, 3.0], &[1.0, 1.0], TwoSampleKind::Pooled).unwrap_err();
        assert_eq!(err, NumericError::DivisionByZero);
    }
}
