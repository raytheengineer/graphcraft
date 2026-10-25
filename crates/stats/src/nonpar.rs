//! Non-parametric rank-based tests.
//!
//! M3.4 ships the large-sample (normal / χ²) approximations with
//! standard tie corrections:
//! - [`mann_whitney`] — two-sample Wilcoxon–Mann–Whitney rank-sum.
//! - [`wilcoxon_signed_rank`] — paired/one-sample signed-rank test
//!   with zero-differences dropped (Pratt vs Wilcoxon convention is
//!   documented in `docs/analyses.md`).
//! - [`kruskal_wallis`] — `k`-sample analogue of one-way ANOVA.
//! - [`dunns_test`] — post-hoc pairwise mean-rank comparisons after
//!   Kruskal–Wallis, with Bonferroni family-wise adjustment.
//!
//! Exact small-sample permutation enumerations are tracked as a
//! follow-up.
//!
//! Behaviour references (public):
//! - Mann, H. B. & Whitney, D. R. (1947) *Annals of Mathematical
//!   Statistics* 18, 50–60.
//! - Wilcoxon, F. (1945) *Biometrics Bulletin* 1, 80–83.
//! - Kruskal, W. H. & Wallis, W. A. (1952) *JASA* 47, 583–621.
//! - Dunn, O. J. (1964) *Technometrics* 6, 241–252.

use graphcraft_numeric::{ranks_with_ties, NumericError};

use crate::distributions::chi_squared::chi2_sf;
use crate::distributions::normal::big_phi;
use crate::mcomp::bonferroni;

/// Result of a Mann–Whitney U (Wilcoxon rank-sum) test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MannWhitney {
    pub n1: usize,
    pub n2: usize,
    /// Rank sum of group 1 (`xs`) in the combined ranking.
    pub r1: f64,
    /// Mann–Whitney `U = R1 − n1·(n1+1)/2`.
    pub u: f64,
    pub mean_u: f64,
    pub sd_u: f64,
    /// Standardised statistic with a continuity correction of ±0.5
    /// toward `mean_u`.
    pub z: f64,
    /// Two-sided large-sample p-value.
    pub p_two_sided: f64,
}

/// Mann–Whitney U with tie correction. Uses the large-sample normal
/// approximation with a 0.5 continuity correction toward the null
/// mean.
pub fn mann_whitney(xs: &[f64], ys: &[f64]) -> Result<MannWhitney, NumericError> {
    if xs.is_empty() || ys.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: xs.len().min(ys.len()),
        });
    }
    let n1 = xs.len();
    let n2 = ys.len();
    let n = n1 + n2;
    let mut combined = Vec::with_capacity(n);
    combined.extend_from_slice(xs);
    combined.extend_from_slice(ys);
    let (ranks, tie_corr) = ranks_with_ties(&combined)?;
    let r1: f64 = ranks[..n1].iter().sum();
    let n1_f = n1 as f64;
    let n2_f = n2 as f64;
    let n_f = n as f64;
    let u = r1 - n1_f * (n1_f + 1.0) * 0.5;
    let mean_u = n1_f * n2_f * 0.5;
    // Variance with tie correction (standard form):
    //   V = n1·n2·(n³ − n − T) / (12·n·(n−1)),
    //   where T = Σ (t³ − t).
    let var_u = n1_f * n2_f / 12.0 * ((n_f + 1.0) - tie_corr / (n_f * (n_f - 1.0)));
    if !var_u.is_finite() || var_u <= 0.0 {
        return Err(NumericError::DivisionByZero);
    }
    let sd_u = var_u.sqrt();
    // Continuity correction toward the null mean.
    let diff = u - mean_u;
    let z = if diff.abs() < 0.5 {
        0.0
    } else {
        (diff - 0.5_f64.copysign(diff)) / sd_u
    };
    let p_two_sided = 2.0 * (1.0 - big_phi(z.abs()));
    Ok(MannWhitney {
        n1,
        n2,
        r1,
        u,
        mean_u,
        sd_u,
        z,
        p_two_sided: p_two_sided.clamp(0.0, 1.0),
    })
}

/// Result of a Wilcoxon signed-rank test on paired or one-sample
/// differences.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WilcoxonSignedRank {
    /// Number of non-zero differences used.
    pub n_eff: usize,
    pub w_plus: f64,
    pub w_minus: f64,
    pub mean_w: f64,
    pub sd_w: f64,
    pub z: f64,
    pub p_two_sided: f64,
}

/// Wilcoxon signed-rank test on `diffs = ys − xs` against `0`. Zero
/// differences are dropped (Wilcoxon's own convention; Pratt's
/// zero-splitting variant is a scheduled option).
pub fn wilcoxon_signed_rank(xs: &[f64], ys: &[f64]) -> Result<WilcoxonSignedRank, NumericError> {
    if xs.len() != ys.len() {
        return Err(NumericError::TooFewValues {
            required: xs.len(),
            got: ys.len(),
        });
    }
    if xs.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    let mut diffs = Vec::with_capacity(xs.len());
    for (a, b) in xs.iter().zip(ys.iter()) {
        let d = b - a;
        if !d.is_finite() {
            return Err(NumericError::NotFinite);
        }
        if d != 0.0 {
            diffs.push(d);
        }
    }
    if diffs.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    let abs: Vec<f64> = diffs.iter().map(|d| d.abs()).collect();
    let (ranks, tie_corr) = ranks_with_ties(&abs)?;
    let mut w_plus = 0.0_f64;
    let mut w_minus = 0.0_f64;
    for (d, r) in diffs.iter().zip(ranks.iter()) {
        if *d > 0.0 {
            w_plus += r;
        } else {
            w_minus += r;
        }
    }
    let n = diffs.len() as f64;
    let mean_w = n * (n + 1.0) * 0.25;
    // Variance with tie correction: V = n(n+1)(2n+1)/24 − T/48,
    // where T = Σ (t³ − t) across |d| ties (Lehmann/Hollander form).
    let var_w = n * (n + 1.0) * (2.0 * n + 1.0) / 24.0 - tie_corr / 48.0;
    if !var_w.is_finite() || var_w <= 0.0 {
        return Err(NumericError::DivisionByZero);
    }
    let sd_w = var_w.sqrt();
    let diff = w_plus - mean_w;
    let z = if diff.abs() < 0.5 {
        0.0
    } else {
        (diff - 0.5_f64.copysign(diff)) / sd_w
    };
    let p_two_sided = 2.0 * (1.0 - big_phi(z.abs()));
    Ok(WilcoxonSignedRank {
        n_eff: diffs.len(),
        w_plus,
        w_minus,
        mean_w,
        sd_w,
        z,
        p_two_sided: p_two_sided.clamp(0.0, 1.0),
    })
}

/// Result of a Kruskal–Wallis H test.
#[derive(Debug, Clone, PartialEq)]
pub struct KruskalWallis {
    pub k: usize,
    pub n_total: usize,
    pub group_ns: Vec<usize>,
    pub rank_sums: Vec<f64>,
    pub mean_ranks: Vec<f64>,
    pub h: f64,
    pub h_corrected: f64,
    pub df: f64,
    pub p: f64,
}

/// Kruskal–Wallis H on `k ≥ 2` groups. Tie correction divides H by
/// `1 − Σ(t³−t) / (N³−N)`.
pub fn kruskal_wallis(groups: &[Vec<f64>]) -> Result<KruskalWallis, NumericError> {
    if groups.len() < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: groups.len(),
        });
    }
    let mut combined: Vec<f64> = Vec::new();
    let mut group_ns: Vec<usize> = Vec::with_capacity(groups.len());
    for g in groups {
        if g.is_empty() {
            return Err(NumericError::TooFewValues {
                required: 1,
                got: 0,
            });
        }
        group_ns.push(g.len());
        combined.extend_from_slice(g);
    }
    let n_total = combined.len();
    let (ranks, tie_corr) = ranks_with_ties(&combined)?;
    let n_f = n_total as f64;
    let mut rank_sums: Vec<f64> = Vec::with_capacity(groups.len());
    let mut mean_ranks: Vec<f64> = Vec::with_capacity(groups.len());
    let mut cursor = 0;
    let mut h_core = 0.0_f64;
    for &ni in &group_ns {
        let slice = &ranks[cursor..cursor + ni];
        let rsum: f64 = slice.iter().sum();
        rank_sums.push(rsum);
        mean_ranks.push(rsum / ni as f64);
        h_core += rsum * rsum / ni as f64;
        cursor += ni;
    }
    let h = 12.0 / (n_f * (n_f + 1.0)) * h_core - 3.0 * (n_f + 1.0);
    let tie_factor = 1.0 - tie_corr / (n_f * n_f * n_f - n_f);
    let h_corrected = if tie_factor > 0.0 { h / tie_factor } else { h };
    let df = (groups.len() - 1) as f64;
    let p = chi2_sf(h_corrected, df)?;
    Ok(KruskalWallis {
        k: groups.len(),
        n_total,
        group_ns,
        rank_sums,
        mean_ranks,
        h,
        h_corrected,
        df,
        p,
    })
}

/// A single Dunn's test pairwise comparison on Kruskal–Wallis ranks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DunnPair {
    pub i: usize,
    pub j: usize,
    pub mean_rank_diff: f64,
    pub z: f64,
    /// Raw two-sided p-value for this comparison.
    pub p_raw: f64,
    /// Bonferroni-adjusted p-value across all `k(k−1)/2` pairs.
    pub p_adj: f64,
}

/// Dunn's post-hoc test on top of a Kruskal–Wallis result. Uses the
/// standard pooled variance `(N(N+1)/12) · (1/n_i + 1/n_j)` without a
/// tie correction (the tie-corrected form is a scheduled option).
pub fn dunns_test(kw: &KruskalWallis) -> Result<Vec<DunnPair>, NumericError> {
    let k = kw.k;
    if k < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: k,
        });
    }
    #[allow(clippy::integer_division)]
    let m = k * (k - 1) / 2;
    let n_f = kw.n_total as f64;
    let base_var = n_f * (n_f + 1.0) / 12.0;
    let mut out = Vec::with_capacity(m);
    for i in 0..k {
        for j in (i + 1)..k {
            let ni = kw.group_ns[i] as f64;
            let nj = kw.group_ns[j] as f64;
            let se = (base_var * (1.0 / ni + 1.0 / nj)).sqrt();
            let diff = kw.mean_ranks[j] - kw.mean_ranks[i];
            let z = diff / se;
            let p_raw = 2.0 * (1.0 - big_phi(z.abs()));
            let p_raw = p_raw.clamp(0.0, 1.0);
            let p_adj = bonferroni(p_raw, m)?;
            out.push(DunnPair {
                i,
                j,
                mean_rank_diff: diff,
                z,
                p_raw,
                p_adj,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mann_whitney_textbook_disjoint() {
        // xs = [1,2,3], ys = [4,5,6]. R1 = 6, U = 0, mean = 4.5,
        // var = 9·7/12 = 5.25 (no ties), z = (0 − 4.5 + 0.5)/√5.25 ≈ −1.746.
        let r = mann_whitney(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]).unwrap();
        assert_eq!(r.n1, 3);
        assert_eq!(r.n2, 3);
        assert!((r.u - 0.0).abs() < 1e-13);
        assert!((r.mean_u - 4.5).abs() < 1e-13);
        assert!((r.sd_u - 5.25_f64.sqrt()).abs() < 1e-13);
        let expected_z = -4.0 / 5.25_f64.sqrt();
        assert!((r.z - expected_z).abs() < 1e-13, "z={}", r.z);
    }

    #[test]
    fn mann_whitney_with_ties() {
        // Deliberate ties: xs = [1,2,2], ys = [2,3,4].
        // Combined 1,2,2,2,3,4. Ranks for the three 2's: avg = 3.
        // Group 1 ranks: 1, 3, 3 → R1 = 7. U = 7 − 6 = 1.
        let r = mann_whitney(&[1.0, 2.0, 2.0], &[2.0, 3.0, 4.0]).unwrap();
        assert!((r.r1 - 7.0).abs() < 1e-13);
        assert!((r.u - 1.0).abs() < 1e-13);
    }

    #[test]
    fn wilcoxon_signed_rank_all_positive() {
        // diffs = [1,2,3,4,5]. w+ = 15, w− = 0.
        // mean = 7.5, var = 55/4 = 13.75, z = (15 − 7.5 − 0.5)/√13.75.
        let r =
            wilcoxon_signed_rank(&[0.0, 0.0, 0.0, 0.0, 0.0], &[1.0, 2.0, 3.0, 4.0, 5.0]).unwrap();
        assert_eq!(r.n_eff, 5);
        assert!((r.w_plus - 15.0).abs() < 1e-13);
        assert!((r.w_minus - 0.0).abs() < 1e-13);
        let expected_z = 7.0 / 13.75_f64.sqrt();
        assert!((r.z - expected_z).abs() < 1e-13, "z={}", r.z);
    }

    #[test]
    fn wilcoxon_drops_zero_differences() {
        let r = wilcoxon_signed_rank(&[1.0, 2.0, 3.0], &[1.0, 2.0, 5.0]).unwrap();
        assert_eq!(r.n_eff, 1);
        assert_eq!(r.w_plus, 1.0);
        assert_eq!(r.w_minus, 0.0);
    }

    #[test]
    fn kruskal_wallis_closed_form_three_arithmetic() {
        // [1,2,3],[4,5,6],[7,8,9]: no ties, H = 7.2.
        // p = exp(-3.6) (χ²₂ closed form).
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let c = vec![7.0, 8.0, 9.0];
        let r = kruskal_wallis(&[a, b, c]).unwrap();
        assert_eq!(r.k, 3);
        assert_eq!(r.n_total, 9);
        assert!((r.h - 7.2).abs() < 1e-13, "H={}", r.h);
        assert!((r.h_corrected - r.h).abs() < 1e-13); // no ties
        let expected_p = (-3.6_f64).exp();
        assert!((r.p - expected_p).abs() < 1e-12);
    }

    #[test]
    fn dunns_test_matches_kw_structure() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let c = vec![7.0, 8.0, 9.0];
        let kw = kruskal_wallis(&[a, b, c]).unwrap();
        let pairs = dunns_test(&kw).unwrap();
        assert_eq!(pairs.len(), 3);
        // All adjustments ≤ 1, all |z| > 0.
        for p in &pairs {
            assert!(p.p_adj >= p.p_raw - 1e-15);
            assert!(p.z.abs() > 0.0);
        }
    }
}
