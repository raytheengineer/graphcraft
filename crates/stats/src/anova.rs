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
use crate::distributions::dunnett::{dunnett_cdf, DunnettSide};
use crate::distributions::f::f_sf;
use crate::distributions::studentized_range::q_sf;

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

/// A single pairwise comparison from a post-hoc test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairwiseComparison {
    pub i: usize,
    pub j: usize,
    /// `mean_j − mean_i`.
    pub mean_diff: f64,
    /// Standard error of the difference under the model that produced
    /// the comparison (post-hoc specific).
    pub se_diff: f64,
    /// Test statistic (e.g. studentized `q` for Tukey HSD).
    pub stat: f64,
    /// Adjusted (family-wise) p-value under the post-hoc.
    pub p_adj: f64,
}

/// Tukey's HSD post-hoc for all `k·(k−1)/2` pairs of groups from a
/// one-way ANOVA. The pooled within-group variance from the ANOVA
/// (`ms_within`, `df_within`) is used, with Tukey's harmonic-mean
/// adjustment for unequal `n_i` (Tukey–Kramer form).
///
/// `stat` is the studentized `q = |mean_diff| / √(MS_within / n_h)`
/// where `n_h = 2 / (1/n_i + 1/n_j)` is the harmonic mean of the two
/// group sizes. `p_adj` is `P(Q ≥ q; k, ν)` from the studentized-range
/// distribution.
pub fn tukey_hsd(a: &OneWayAnova) -> Result<Vec<PairwiseComparison>, NumericError> {
    let k = a.k;
    if k < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: k,
        });
    }
    if !a.ms_within.is_finite() || a.ms_within <= 0.0 {
        return Err(NumericError::DivisionByZero);
    }
    #[allow(clippy::integer_division)]
    let mut out = Vec::with_capacity(k * (k - 1) / 2);
    for i in 0..k {
        for j in (i + 1)..k {
            let ni = a.group_ns[i] as f64;
            let nj = a.group_ns[j] as f64;
            if ni < 1.0 || nj < 1.0 {
                return Err(NumericError::TooFewValues {
                    required: 1,
                    got: 0,
                });
            }
            let n_h = 2.0 / (1.0 / ni + 1.0 / nj);
            let se = (a.ms_within / n_h).sqrt();
            let mean_diff = a.group_means[j] - a.group_means[i];
            let q_stat = mean_diff.abs() / se;
            let p_adj = q_sf(q_stat, k, a.df_within)?;
            out.push(PairwiseComparison {
                i,
                j,
                mean_diff,
                se_diff: se,
                stat: q_stat,
                p_adj,
            });
        }
    }
    Ok(out)
}

/// Dunnett's many-to-one post-hoc on top of a one-way ANOVA.
/// `control_idx` is the group index of the control (0-based in the
/// ANOVA order). Returns one comparison per non-control group.
///
/// For each treatment `i ≠ control`:
/// - `t = (ȳ_i − ȳ_c) / √(MS_within · (1/n_i + 1/n_c))`
/// - `p_adj = 1 − F_D(|t|; m, df_within, side)` for `TwoSided`;
///   `1 − F_D(t; m, df_within, OneSided)` for `OneSided`, where
///   `m` is the number of treatment groups (`k − 1`) and `F_D` is
///   the Dunnett equi-correlated max-T CDF.
///
/// The equi-correlated form assumes balanced per-group sample size;
/// for unbalanced designs the correlations drift from `1/2` and the
/// p-values are conservative in the common direction. A full
/// unbalanced-Dunnett implementation is tracked for a later slice.
pub fn dunnett(
    a: &OneWayAnova,
    control_idx: usize,
    side: DunnettSide,
) -> Result<Vec<PairwiseComparison>, NumericError> {
    if a.k < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: a.k,
        });
    }
    if control_idx >= a.k {
        return Err(NumericError::TooFewValues {
            required: control_idx + 1,
            got: a.k,
        });
    }
    if !a.ms_within.is_finite() || a.ms_within <= 0.0 {
        return Err(NumericError::DivisionByZero);
    }
    let m = a.k - 1;
    let n_c = a.group_ns[control_idx] as f64;
    let mean_c = a.group_means[control_idx];
    let mut out = Vec::with_capacity(m);
    for i in 0..a.k {
        if i == control_idx {
            continue;
        }
        let n_i = a.group_ns[i] as f64;
        let se = (a.ms_within * (1.0 / n_i + 1.0 / n_c)).sqrt();
        let mean_diff = a.group_means[i] - mean_c;
        let t_stat = mean_diff / se;
        let p_adj = match side {
            DunnettSide::OneSided => 1.0 - dunnett_cdf(t_stat, m, a.df_within, side)?,
            DunnettSide::TwoSided => 1.0 - dunnett_cdf(t_stat.abs(), m, a.df_within, side)?,
        };
        out.push(PairwiseComparison {
            i: control_idx,
            j: i,
            mean_diff,
            se_diff: se,
            stat: t_stat,
            p_adj,
        });
    }
    Ok(out)
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
    fn tukey_hsd_three_groups() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let c = vec![7.0, 8.0, 9.0];
        let anova = one_way_anova(&[a, b, c]).unwrap();
        let pairs = tukey_hsd(&anova).unwrap();
        assert_eq!(pairs.len(), 3);
        // Each pair has mean_diff ∈ {3, 3, 6}; se = √(1/3) with n_h=3,
        // MS_within = 1, so q(ab) = 3/√(1/3) = 3√3 ≈ 5.196, q(ac) = 6√3 ≈ 10.39.
        // All three should be strongly significant.
        for p in &pairs {
            assert!(p.p_adj < 0.05, "pair {:?}: p={}", (p.i, p.j), p.p_adj);
        }
        let p_ac = pairs.iter().find(|p| p.i == 0 && p.j == 2).unwrap();
        assert!((p_ac.mean_diff - 6.0).abs() < 1e-13);
        // q = 6/sqrt(1/3) = 6*sqrt(3) ≈ 10.3923.
        assert!((p_ac.stat - 6.0 * 3.0_f64.sqrt()).abs() < 1e-13);
    }

    #[test]
    fn dunnett_three_groups_matches_tukey_sign() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let c = vec![7.0, 8.0, 9.0];
        let anova = one_way_anova(&[a, b, c]).unwrap();
        let out = dunnett(&anova, 0, DunnettSide::TwoSided).unwrap();
        assert_eq!(out.len(), 2);
        for p in &out {
            assert_eq!(p.i, 0);
            assert!(p.p_adj < 0.05, "p={}", p.p_adj);
        }
        // mean_diff(A→B) = 3, mean_diff(A→C) = 6.
        let diff_ac = out.iter().find(|p| p.j == 2).unwrap().mean_diff;
        assert!((diff_ac - 6.0).abs() < 1e-13);
    }

    #[test]
    fn one_way_rejects_all_constant() {
        let err = one_way_anova(&[vec![1.0, 1.0], vec![1.0, 1.0]]).unwrap_err();
        assert_eq!(err, NumericError::DivisionByZero);
    }
}
