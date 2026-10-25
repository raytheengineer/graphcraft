#![allow(clippy::needless_range_loop)]
//! Chi-squared independence test and Fisher's exact (2×2) test.
//!
//! The chi-squared statistic for an `r × c` table is
//!   `X² = Σ (O_ij − E_ij)² / E_ij`
//! with `E_ij = row_i · col_j / n`, and p-value `P(χ²_{(r−1)(c−1)} ≥ X²)`.
//!
//! Fisher's exact test for a 2×2 table conditions on both margins
//! and uses the hypergeometric distribution. We compute the two-
//! sided p-value by Prism's documented convention — "sum of
//! probabilities of all tables (with the given margins) at least
//! as extreme as the observed" — which for 2×2 is unambiguous.
//!
//! Reference (behaviour):
//! - Pearson, K. (1900) *Philosophical Magazine* 50, 157–175.
//! - Fisher, R. A. (1922) *JRSS* 85, 87–94.
//! - Agresti, A. (2013) *Categorical Data Analysis*, 3rd ed.,
//!   Wiley, §3.5 (chi-squared) and §3.5.5 (Fisher's exact).

use graphcraft_numeric::NumericError;

use crate::distributions::chi_squared::chi2_sf;
use crate::special::ln_gamma;

/// Result of a chi-squared independence test on an `r × c` table.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiSquaredTest {
    pub n_rows: usize,
    pub n_cols: usize,
    pub n: u64,
    pub observed: Vec<Vec<u64>>,
    pub expected: Vec<Vec<f64>>,
    pub chi2: f64,
    pub df: f64,
    /// Right-tailed `P(X² ≥ chi2)`.
    pub p: f64,
}

/// Chi-squared test of independence on a row-major `r × c` count
/// matrix.
pub fn chi_squared_independence(observed: &[Vec<u64>]) -> Result<ChiSquaredTest, NumericError> {
    let r = observed.len();
    if r < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: r,
        });
    }
    let c = observed[0].len();
    if c < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: c,
        });
    }
    for row in observed {
        if row.len() != c {
            return Err(NumericError::TooFewValues {
                required: c,
                got: row.len(),
            });
        }
    }
    let mut row_sums = vec![0_u64; r];
    let mut col_sums = vec![0_u64; c];
    let mut total: u64 = 0;
    for i in 0..r {
        for j in 0..c {
            row_sums[i] = row_sums[i].saturating_add(observed[i][j]);
            col_sums[j] = col_sums[j].saturating_add(observed[i][j]);
            total = total.saturating_add(observed[i][j]);
        }
    }
    if total == 0 {
        return Err(NumericError::DivisionByZero);
    }
    let total_f = total as f64;
    let mut expected = vec![vec![0.0_f64; c]; r];
    let mut chi2 = 0.0_f64;
    for i in 0..r {
        for j in 0..c {
            let e = row_sums[i] as f64 * col_sums[j] as f64 / total_f;
            expected[i][j] = e;
            if e > 0.0 {
                let d = observed[i][j] as f64 - e;
                chi2 += d * d / e;
            } else if observed[i][j] > 0 {
                // Impossible under the null (row or column is empty
                // but a cell has data) — nothing sensible to add.
                return Err(NumericError::DivisionByZero);
            }
        }
    }
    let df = (r as f64 - 1.0) * (c as f64 - 1.0);
    let p = chi2_sf(chi2, df)?.clamp(0.0, 1.0);
    Ok(ChiSquaredTest {
        n_rows: r,
        n_cols: c,
        n: total,
        observed: observed.to_vec(),
        expected,
        chi2,
        df,
        p,
    })
}

/// Result of Fisher's exact test on a 2×2 table
/// `[[a, b], [c, d]]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FisherExact {
    pub a: u64,
    pub b: u64,
    pub c: u64,
    pub d: u64,
    /// Point probability of the observed table.
    pub p_observed: f64,
    /// Two-sided p (sum of all tables at least as extreme as
    /// observed, by point-probability, under the hypergeometric
    /// distribution with fixed margins).
    pub p_two_sided: f64,
    /// One-sided p (`P(X ≥ a)` for upper tail, `P(X ≤ a)` for lower,
    /// whichever is smaller).
    pub p_one_sided: f64,
    /// Odds ratio `(a · d) / (b · c)`, or `NaN` when the denominator
    /// is zero.
    pub odds_ratio: f64,
}

/// Fisher's exact test for a 2×2 table.
///
/// The table is `[[a, b], [c, d]]`, i.e. two rows and two columns of
/// counts. Row sums are `a+b` and `c+d`; column sums are `a+c` and
/// `b+d`; grand total is `n`.
pub fn fisher_exact_2x2(a: u64, b: u64, c: u64, d: u64) -> Result<FisherExact, NumericError> {
    let r1 = a + b;
    let r2 = c + d;
    let c1 = a + c;
    let c2 = b + d;
    let n = r1 + r2;
    if n == 0 {
        return Err(NumericError::DivisionByZero);
    }
    // The random variable `X = a` on a 2×2 with fixed margins has
    // hypergeometric distribution `Hypergeom(N = n, K = r1, n_s = c1)`,
    // with support `max(0, r1 + c1 − n) ≤ X ≤ min(r1, c1)`.
    let lo = r1.saturating_sub(c2);
    let hi = r1.min(c1);
    let p_obs = hyper_pmf(a, r1, c1, n);
    let p_lower: f64 = (lo..=a).map(|k| hyper_pmf(k, r1, c1, n)).sum();
    let p_upper: f64 = (a..=hi).map(|k| hyper_pmf(k, r1, c1, n)).sum();
    let p_one_sided = p_lower.min(p_upper).clamp(0.0, 1.0);
    // Two-sided by the "point-probability" convention: sum PMF for
    // every k whose PMF ≤ p_obs. Tolerant to float jitter at ties.
    let mut p_two: f64 = 0.0;
    let p_tol = p_obs * (1.0 + 1e-12);
    for k in lo..=hi {
        let p = hyper_pmf(k, r1, c1, n);
        if p <= p_tol {
            p_two += p;
        }
    }
    let odds_ratio = if b > 0 && c > 0 {
        (a as f64 * d as f64) / (b as f64 * c as f64)
    } else {
        f64::NAN
    };
    Ok(FisherExact {
        a,
        b,
        c,
        d,
        p_observed: p_obs,
        p_two_sided: p_two.clamp(0.0, 1.0),
        p_one_sided,
        odds_ratio,
    })
}

/// Hypergeometric PMF `P(X = k | N, K, n)` using log-factorials
/// (log Γ(n+1)) to stay numerically stable for large tables.
fn hyper_pmf(k: u64, big_k: u64, small_n: u64, big_n: u64) -> f64 {
    if k > big_k || k > small_n || big_k > big_n || small_n > big_n {
        return 0.0;
    }
    if (big_n - big_k) < small_n.saturating_sub(k) {
        return 0.0;
    }
    let lf = |n: u64| ln_gamma(n as f64 + 1.0);
    let num = lf(big_k) + lf(big_n - big_k) + lf(small_n) + lf(big_n - small_n);
    let den =
        lf(big_n) + lf(k) + lf(big_k - k) + lf(small_n - k) + lf(big_n - big_k - (small_n - k));
    (num - den).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chi2_perfectly_independent_table_is_zero() {
        // Rows [10, 20], [20, 40]: ratios identical, so expected == observed.
        let obs = vec![vec![10u64, 20], vec![20, 40]];
        let r = chi_squared_independence(&obs).unwrap();
        assert!((r.chi2 - 0.0).abs() < 1e-13);
        assert!((r.p - 1.0).abs() < 1e-13);
        assert_eq!(r.df, 1.0);
    }

    #[test]
    fn chi2_textbook_two_by_two() {
        // Classic diet-and-outcome 2×2 (Rosner, 7th ed., Table 10.1
        // style synthetic): observed [[20, 30], [40, 10]].
        // Row sums (50, 50); col sums (60, 40); n=100.
        // Expected [[30, 20], [30, 20]].
        // X² = (10²/30 + 10²/20) · 2 = (100/30 + 100/20)·2 =
        //    = (3.3333 + 5)·2 = 16.6667.
        let obs = vec![vec![20u64, 30], vec![40, 10]];
        let r = chi_squared_independence(&obs).unwrap();
        let expected_chi2 = 2.0 * (100.0 / 30.0 + 100.0 / 20.0);
        assert!((r.chi2 - expected_chi2).abs() < 1e-13);
        assert_eq!(r.df, 1.0);
        assert!(r.p < 1e-4);
    }

    #[test]
    fn chi2_rejects_wrong_width() {
        let obs = vec![vec![1u64, 2], vec![3]];
        assert!(chi_squared_independence(&obs).is_err());
    }

    #[test]
    fn fisher_exact_textbook_tea_tasting() {
        // Fisher's lady-tasting-tea: 4 cups milk-first and 4 cups
        // tea-first; the lady correctly classifies all 8. Observed
        // table [[4, 0], [0, 4]]. Two-sided p = 2/70 ≈ 0.02857.
        let r = fisher_exact_2x2(4, 0, 0, 4).unwrap();
        assert!((r.p_two_sided - 2.0 / 70.0).abs() < 1e-13);
        // One-sided (upper tail): P(X ≥ 4) = 1/70.
        assert!((r.p_one_sided - 1.0 / 70.0).abs() < 1e-13);
    }

    #[test]
    fn fisher_exact_odds_ratio_closed_form() {
        let r = fisher_exact_2x2(3, 1, 1, 3).unwrap();
        assert!((r.odds_ratio - 9.0).abs() < 1e-13);
    }

    #[test]
    fn fisher_exact_symmetric_in_rows() {
        // Flipping the two rows should swap the sign of the OR but
        // leave the two-sided p identical.
        let a = fisher_exact_2x2(2, 3, 4, 5).unwrap();
        let b = fisher_exact_2x2(4, 5, 2, 3).unwrap();
        assert!((a.p_two_sided - b.p_two_sided).abs() < 1e-13);
    }
}
