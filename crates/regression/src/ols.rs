#![allow(clippy::needless_range_loop)]
//! Ordinary least-squares linear regression.
//!
//! Fit `y = X · β + ε`, solving the normal equations
//! `(XᵀX) β̂ = Xᵀy` with Cholesky. Returns coefficients, residuals,
//! standard errors, t-statistics with two-sided p-values, R², the
//! adjusted R², residual standard error, and the overall-F test
//! (vs the intercept-only model when `has_intercept = true`).
//!
//! Normal equations are numerically fine for the well-conditioned
//! parity targets (NIST StRD *Norris*, …). An ill-conditioned
//! dataset (e.g. NIST *Longley*) will eventually need a Householder
//! QR path; it is tracked as a scheduled upgrade when the first
//! such parity row demands it.
//!
//! Reference (behaviour): standard OLS theory, see Draper & Smith
//! (1998) *Applied Regression Analysis*, 3rd ed., Wiley. Classical
//! algorithm; no third-party or GraphPad code.

use graphcraft_numeric::NumericError;
use graphcraft_stats::distributions::f::f_sf;
use graphcraft_stats::distributions::t::t_sf;

use crate::matrix::{invert_spd, solve_spd, DenseMatrix};

/// Result of an ordinary least-squares fit.
#[derive(Debug, Clone, PartialEq)]
pub struct OlsFit {
    /// Coefficient estimates in the column order of `X`.
    pub coefficients: Vec<f64>,
    /// Fitted values `ŷ_i = x_iᵀ β̂`.
    pub fitted: Vec<f64>,
    /// Residuals `y_i − ŷ_i`.
    pub residuals: Vec<f64>,
    /// Standard error of each coefficient.
    pub se: Vec<f64>,
    /// `t = β̂ / se`.
    pub t: Vec<f64>,
    /// Two-sided p-value for each coefficient under the fit's
    /// residual degrees of freedom.
    pub p: Vec<f64>,
    /// Residual sum of squares `Σ e_i²`.
    pub rss: f64,
    /// Total sum of squares — about the mean when the model has an
    /// intercept, about zero otherwise.
    pub tss: f64,
    /// `1 − RSS/TSS`. The "about the mean" variant for intercept
    /// models, matching the Prism manual's default output.
    pub r_squared: f64,
    /// `1 − (1 − R²) · (n − 1) / (n − p)`.
    pub adj_r_squared: f64,
    /// `σ̂ = √(RSS / (n − p))`.
    pub residual_se: f64,
    /// Overall F statistic. For intercept models this compares the
    /// fit against the intercept-only null; otherwise against the
    /// zero model.
    pub f_stat: f64,
    pub f_df_num: f64,
    pub f_df_den: f64,
    pub f_p: f64,
    /// Number of observations.
    pub n: usize,
    /// Number of coefficients (including intercept if present).
    pub p_count: usize,
    pub has_intercept: bool,
}

/// Fit `y = X β + ε` by OLS. The caller supplies `X` directly, so
/// adding an intercept column is their responsibility; `has_intercept`
/// controls the convention used for TSS, R², and the overall-F test.
pub fn ols_fit(x: &DenseMatrix, y: &[f64], has_intercept: bool) -> Result<OlsFit, NumericError> {
    if y.len() != x.rows {
        return Err(NumericError::TooFewValues {
            required: x.rows,
            got: y.len(),
        });
    }
    for &v in y {
        if !v.is_finite() {
            return Err(NumericError::NotFinite);
        }
    }
    let n = x.rows;
    let p = x.cols;
    if n <= p {
        return Err(NumericError::TooFewValues {
            required: p + 1,
            got: n,
        });
    }

    let xtx = x.xtx();
    let xty = x.xty(y)?;
    let mut xtx_mut = xtx.clone();
    let coefficients = solve_spd(&mut xtx_mut, &xty)?;
    let xtx_inv = invert_spd(&xtx)?;

    let mut fitted = vec![0.0_f64; n];
    let mut residuals = vec![0.0_f64; n];
    for i in 0..n {
        let mut s = 0.0_f64;
        for j in 0..p {
            s += x.get(i, j) * coefficients[j];
        }
        fitted[i] = s;
        residuals[i] = y[i] - s;
    }
    let rss: f64 = residuals.iter().map(|r| r * r).sum();

    let y_mean: f64 = y.iter().sum::<f64>() / n as f64;
    let tss: f64 = if has_intercept {
        y.iter().map(|v| (v - y_mean).powi(2)).sum()
    } else {
        y.iter().map(|v| v * v).sum()
    };

    let df_resid = (n - p) as f64;
    let residual_se = (rss / df_resid).sqrt();
    let sigma2 = rss / df_resid;

    let mut se = vec![0.0_f64; p];
    let mut t = vec![0.0_f64; p];
    let mut pv = vec![0.0_f64; p];
    for j in 0..p {
        let v = sigma2 * xtx_inv.get(j, j);
        let s = if v > 0.0 { v.sqrt() } else { 0.0 };
        se[j] = s;
        t[j] = if s > 0.0 { coefficients[j] / s } else { 0.0 };
        pv[j] = 2.0 * t_sf(t[j].abs(), df_resid)?;
    }

    let r_squared = if tss > 0.0 { 1.0 - rss / tss } else { 0.0 };
    let adj_r_squared = if n > p {
        1.0 - (1.0 - r_squared) * (n as f64 - 1.0) / df_resid
    } else {
        r_squared
    };

    // Overall F: for intercept models, test "all non-intercept β = 0"
    // against the intercept-only model.
    let (f_df_num, f_stat) = if has_intercept {
        if p >= 2 {
            let df_num = (p - 1) as f64;
            let model_ss = (tss - rss).max(0.0);
            let f = (model_ss / df_num) / sigma2;
            (df_num, f)
        } else {
            (1.0, 0.0)
        }
    } else {
        let df_num = p as f64;
        let model_ss = (tss - rss).max(0.0);
        let f = (model_ss / df_num) / sigma2;
        (df_num, f)
    };
    let f_p = if f_stat > 0.0 {
        f_sf(f_stat, f_df_num, df_resid)?
    } else {
        1.0
    };

    Ok(OlsFit {
        coefficients,
        fitted,
        residuals,
        se,
        t,
        p: pv,
        rss,
        tss,
        r_squared,
        adj_r_squared,
        residual_se,
        f_stat,
        f_df_num,
        f_df_den: df_resid,
        f_p,
        n,
        p_count: p,
        has_intercept,
    })
}

/// Simple linear regression `y = β₀ + β₁ · x`. Builds `X = [1 | x]`
/// and calls [`ols_fit`] with `has_intercept = true`.
pub fn simple_linear_regression(xs: &[f64], ys: &[f64]) -> Result<OlsFit, NumericError> {
    if xs.len() != ys.len() {
        return Err(NumericError::TooFewValues {
            required: xs.len(),
            got: ys.len(),
        });
    }
    let n = xs.len();
    if n < 3 {
        return Err(NumericError::TooFewValues {
            required: 3,
            got: n,
        });
    }
    let mut data = Vec::with_capacity(n * 2);
    for &x in xs {
        if !x.is_finite() {
            return Err(NumericError::NotFinite);
        }
        data.push(1.0);
        data.push(x);
    }
    let x_mat = DenseMatrix::from_row_major(n, 2, data)?;
    ols_fit(&x_mat, ys, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_line_simple() {
        // y = 2 + 3·x, exact.
        let xs = [1.0_f64, 2.0, 3.0, 4.0, 5.0];
        let ys = [5.0_f64, 8.0, 11.0, 14.0, 17.0];
        let r = simple_linear_regression(&xs, &ys).unwrap();
        assert!((r.coefficients[0] - 2.0).abs() < 1e-13);
        assert!((r.coefficients[1] - 3.0).abs() < 1e-13);
        assert!(r.rss < 1e-20);
        assert!((r.r_squared - 1.0).abs() < 1e-13);
    }

    #[test]
    fn textbook_small_sample() {
        // Classic 5-point example.
        // xs = 1..5, ys = [2, 4, 5, 4, 5].
        // ȳ = 4, x̄ = 3; Sxx = 10, Sxy = 6; β1 = 0.6, β0 = 2.2.
        // Fitted = [2.8, 3.4, 4.0, 4.6, 5.2];
        // residuals = [-0.8, 0.6, 1.0, -0.6, -0.2]; RSS = 2.4.
        // TSS = Σ(y − 4)² = 4 + 0 + 1 + 0 + 1 = 6; R² = 1 − 2.4/6 = 0.6.
        let xs = [1.0_f64, 2.0, 3.0, 4.0, 5.0];
        let ys = [2.0_f64, 4.0, 5.0, 4.0, 5.0];
        let r = simple_linear_regression(&xs, &ys).unwrap();
        assert!((r.coefficients[0] - 2.2).abs() < 1e-13);
        assert!((r.coefficients[1] - 0.6).abs() < 1e-13);
        assert!((r.rss - 2.4).abs() < 1e-13);
        assert!((r.tss - 6.0).abs() < 1e-13);
        assert!((r.r_squared - 0.6).abs() < 1e-13);
    }

    #[test]
    fn multiple_regression_closed_form() {
        // y = 1 + 2·x1 + 3·x2 at known points — exact fit.
        let x_data: Vec<f64> = vec![
            1.0, 0.0, 0.0, //
            1.0, 1.0, 0.0, //
            1.0, 0.0, 1.0, //
            1.0, 1.0, 1.0, //
            1.0, 2.0, 1.0, //
            1.0, 1.0, 2.0, //
        ];
        let x = DenseMatrix::from_row_major(6, 3, x_data).unwrap();
        // (1,2): 1+2+6 = 9, not 8.
        let ys = [1.0_f64, 3.0, 4.0, 6.0, 8.0, 9.0];
        let r = ols_fit(&x, &ys, true).unwrap();
        assert!((r.coefficients[0] - 1.0).abs() < 1e-12);
        assert!((r.coefficients[1] - 2.0).abs() < 1e-12);
        assert!((r.coefficients[2] - 3.0).abs() < 1e-12);
        assert!(r.rss < 1e-20);
    }
}
