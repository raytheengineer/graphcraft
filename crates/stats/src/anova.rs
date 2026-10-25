//! Analysis of variance.
//!
//! M3.3 ships the one-way (fixed-effects) ANOVA with real-valued F
//! degrees of freedom. The F p-value uses the regularized
//! incomplete-beta backbone already in `special`.
//!
//! Reference (behaviour): Fisher, R. A. (1925) *Statistical Methods
//! for Research Workers*, Oliver and Boyd; and any modern biostats
//! textbook (e.g. Sokal & Rohlf, *Biometry*, 4th ed., Ch. 8).
//! No GraphPad material is used.

use graphcraft_numeric::NumericError;

use crate::descriptive::Descriptives;
use crate::distributions::f::f_sf;

/// Result of a one-way ANOVA.
#[derive(Debug, Clone, PartialEq)]
pub struct OneWayAnova {
    pub k: usize,
    pub n_total: usize,
    pub group_ns: Vec<usize>,
    pub group_means: Vec<f64>,
    pub grand_mean: f64,
    pub ss_between: f64,
    pub ss_within: f64,
    pub ss_total: f64,
    pub df_between: f64,
    pub df_within: f64,
    pub ms_between: f64,
    pub ms_within: f64,
    pub f: f64,
    /// Right-tailed F p-value `P(F ≥ f_obs)`.
    pub p: f64,
}

/// One-way (fixed-effects) ANOVA on `k ≥ 2` groups.
///
/// Each group must be non-empty. The total sample size must exceed
/// `k`. Returns `Err(DivisionByZero)` when every group is a single
/// constant (within-group sum of squares is zero and F is undefined).
pub fn one_way_anova(groups: &[Vec<f64>]) -> Result<OneWayAnova, NumericError> {
    if groups.len() < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: groups.len(),
        });
    }
    let descs: Vec<Descriptives> = groups
        .iter()
        .map(|g| Descriptives::of(g))
        .collect::<Result<_, _>>()?;
    let k = groups.len();
    let n_total: usize = descs.iter().map(|d| d.n).sum();
    if n_total <= k {
        return Err(NumericError::TooFewValues {
            required: k + 1,
            got: n_total,
        });
    }

    let grand_sum: f64 = descs.iter().map(|d| d.mean * d.n as f64).sum();
    let grand_mean = grand_sum / n_total as f64;

    let ss_between: f64 = descs
        .iter()
        .map(|d| (d.n as f64) * (d.mean - grand_mean).powi(2))
        .sum();
    // SS_within = Σ (n_i − 1) · s_i². Groups with n = 1 contribute 0.
    let ss_within: f64 = descs
        .iter()
        .filter_map(|d| d.variance.map(|v| (d.n as f64 - 1.0) * v))
        .sum();
    let ss_total = ss_between + ss_within;

    let df_between = (k - 1) as f64;
    let df_within = (n_total - k) as f64;
    let ms_between = ss_between / df_between;
    let ms_within = ss_within / df_within;
    if !ms_within.is_finite() || ms_within == 0.0 {
        return Err(NumericError::DivisionByZero);
    }
    let f = ms_between / ms_within;
    let p = f_sf(f, df_between, df_within)?;

    Ok(OneWayAnova {
        k,
        n_total,
        group_ns: descs.iter().map(|d| d.n).collect(),
        group_means: descs.iter().map(|d| d.mean).collect(),
        grand_mean,
        ss_between,
        ss_within,
        ss_total,
        df_between,
        df_within,
        ms_between,
        ms_within,
        f,
        p,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_way_closed_form_three_arithmetic_groups() {
        // A = [1,2,3], B = [4,5,6], C = [7,8,9]
        // Means 2, 5, 8; grand mean 5.
        // SS_between = 3·9 + 0 + 3·9 = 54.
        // SS_within  = 3·2 = 6 (each group s² = 1, (n-1)s² = 2).
        // df = (2, 6). MS_between = 27, MS_within = 1. F = 27.
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let c = vec![7.0, 8.0, 9.0];
        let r = one_way_anova(&[a, b, c]).unwrap();
        assert_eq!(r.k, 3);
        assert_eq!(r.n_total, 9);
        assert_eq!(r.df_between, 2.0);
        assert_eq!(r.df_within, 6.0);
        assert!((r.ss_between - 54.0).abs() < 1e-13);
        assert!((r.ss_within - 6.0).abs() < 1e-13);
        assert!((r.f - 27.0).abs() < 1e-13, "F={}", r.f);
        // F=27 on (2,6) is a very extreme result; p ≈ 1e-3.
        assert!(r.p > 0.0 && r.p < 0.01);
    }

    #[test]
    fn one_way_rejects_single_group() {
        let err = one_way_anova(&[vec![1.0, 2.0, 3.0]]).unwrap_err();
        assert!(matches!(err, NumericError::TooFewValues { .. }));
    }

    #[test]
    fn one_way_rejects_all_constant() {
        let err = one_way_anova(&[vec![1.0, 1.0], vec![1.0, 1.0]]).unwrap_err();
        assert_eq!(err, NumericError::DivisionByZero);
    }
}
