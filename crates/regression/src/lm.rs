#![allow(clippy::needless_range_loop)]
//! Levenberg–Marquardt nonlinear least-squares.
//!
//! Minimises `S(β) = Σ (y_i − f(x_i; β))²` by the standard
//! Marquardt step
//!   `(JᵀJ + λ · diag(JᵀJ)) · δ = Jᵀr`
//! with multiplicative λ updates: a successful step scales λ down
//! by `nu_down` (default 0.1), a failed step scales it up by
//! `nu_up` (default 10). The initial λ is `1e-3`.
//!
//! The Jacobian is computed by central differences in parameter
//! space with a scale-aware step `h_j = sqrt(eps) · max(|β_j|, 1)`.
//! An analytic Jacobian route is scheduled for M4.2+ once a
//! parity row demands tighter precision than central differences
//! can deliver.
//!
//! Convergence declares when either:
//! - `|ΔS| ≤ ftol · (S + ftol)`, or
//! - every component of δ satisfies `|δ_j| ≤ ptol · (|β_j| + ptol)`.
//!
//! Reference (behaviour): Marquardt, D. W. (1963) "An algorithm
//! for least-squares estimation of nonlinear parameters", J. SIAM
//! 11, 431–441; Moré, J. J. (1978) in *Numerical Analysis*
//! (G. A. Watson, ed.), Lecture Notes in Mathematics 630, 105–116.
//! Classical algorithm; no third-party or GraphPad code.

use graphcraft_numeric::NumericError;
use graphcraft_stats::distributions::f::f_sf;

use crate::matrix::{invert_spd, solve_spd, DenseMatrix};

/// Options for [`levenberg_marquardt`].
#[derive(Debug, Clone, Copy)]
pub struct LmOptions {
    pub max_iter: usize,
    pub ftol: f64,
    pub ptol: f64,
    pub lambda_init: f64,
    pub lambda_up: f64,
    pub lambda_down: f64,
}

impl Default for LmOptions {
    fn default() -> Self {
        Self {
            max_iter: 400,
            ftol: 1e-12,
            ptol: 1e-12,
            lambda_init: 1e-3,
            lambda_up: 10.0,
            lambda_down: 0.1,
        }
    }
}

/// Result of a Levenberg–Marquardt fit.
#[derive(Debug, Clone, PartialEq)]
pub struct LmFit {
    pub params: Vec<f64>,
    pub fitted: Vec<f64>,
    pub residuals: Vec<f64>,
    pub rss: f64,
    /// Residual standard deviation `σ̂ = √(RSS / (n − p))`.
    pub residual_se: f64,
    /// Standard errors from `σ̂² · (JᵀJ)⁻¹` evaluated at the solution.
    pub se: Vec<f64>,
    /// `R² = 1 − RSS/TSS` with TSS about `ȳ`.
    pub r_squared: f64,
    /// `1 − (1 − R²)·(n − 1)/(n − p)`.
    pub adj_r_squared: f64,
    /// Right-tailed F p-value for `H₀: f(x; β) = ȳ` (model vs mean),
    /// mirroring the overall-F convention used by `OlsFit`.
    pub f_stat: f64,
    pub f_df_num: f64,
    pub f_df_den: f64,
    pub f_p: f64,
    pub n: usize,
    pub p_count: usize,
    pub n_iter: usize,
    pub converged: bool,
    pub final_lambda: f64,
}

/// Fit `y_i = f(x_i; β) + ε` by Levenberg–Marquardt. `f` takes
/// `(params, x_i)` and returns the predicted `ŷ`.
pub fn levenberg_marquardt<F>(
    f: F,
    xs: &[f64],
    ys: &[f64],
    initial: &[f64],
    opts: LmOptions,
) -> Result<LmFit, NumericError>
where
    F: Fn(&[f64], f64) -> f64,
{
    if xs.len() != ys.len() {
        return Err(NumericError::TooFewValues {
            required: xs.len(),
            got: ys.len(),
        });
    }
    for v in xs.iter().chain(ys.iter()) {
        if !v.is_finite() {
            return Err(NumericError::NotFinite);
        }
    }
    let n = xs.len();
    let p = initial.len();
    if n <= p {
        return Err(NumericError::TooFewValues {
            required: p + 1,
            got: n,
        });
    }

    let mut beta: Vec<f64> = initial.to_vec();
    let mut lambda = opts.lambda_init;
    let mut residuals = vec![0.0_f64; n];
    let mut fitted = vec![0.0_f64; n];
    let mut rss = compute_residuals(&f, &beta, xs, ys, &mut fitted, &mut residuals)?;

    let mut j_mat = DenseMatrix::zeros(n, p);
    let mut converged = false;
    let mut n_iter = 0;

    for iter in 1..=opts.max_iter {
        n_iter = iter;
        central_diff_jacobian(&f, &beta, xs, &mut j_mat)?;
        let jtj = j_mat.xtx();
        let jtr = j_mat.xty(&residuals)?;

        // Marquardt damping: diag(JᵀJ) * λ added in-place.
        let mut damped = jtj.clone();
        for i in 0..p {
            let d = damped.get(i, i);
            damped.set(i, i, d + lambda * d);
        }
        let delta = match solve_spd(&mut damped, &jtr) {
            Ok(d) => d,
            Err(_) => {
                // Singular linear system — bump λ and try again.
                lambda *= opts.lambda_up;
                continue;
            }
        };

        // Early convergence: if the proposed step is already within
        // ptol of each component, we've found the fit. Checking
        // this *before* evaluating the trial avoids pathological
        // λ growth when float-noise rejections stall the solver.
        if (0..p).all(|j| delta[j].abs() <= opts.ptol * (beta[j].abs() + opts.ptol)) {
            converged = true;
            break;
        }

        let mut trial = vec![0.0_f64; p];
        for i in 0..p {
            trial[i] = beta[i] + delta[i];
        }
        let mut trial_fit = vec![0.0_f64; n];
        let mut trial_res = vec![0.0_f64; n];
        let trial_rss = match compute_residuals(&f, &trial, xs, ys, &mut trial_fit, &mut trial_res)
        {
            Ok(v) => v,
            Err(_) => {
                lambda *= opts.lambda_up;
                continue;
            }
        };

        if trial_rss < rss {
            // Accept.
            let ds = (rss - trial_rss).abs();
            let step_small =
                (0..p).all(|j| delta[j].abs() <= opts.ptol * (beta[j].abs() + opts.ptol));
            beta = trial;
            fitted = trial_fit;
            residuals = trial_res;
            rss = trial_rss;
            lambda *= opts.lambda_down;
            if ds <= opts.ftol * (rss + opts.ftol) || step_small {
                converged = true;
                break;
            }
        } else {
            // Reject.
            lambda *= opts.lambda_up;
            if lambda > 1.0e30 {
                break;
            }
        }
    }

    // Finalise statistics with the Jacobian at the solution.
    central_diff_jacobian(&f, &beta, xs, &mut j_mat)?;
    let jtj = j_mat.xtx();
    let df_resid = (n - p) as f64;
    let sigma2 = rss / df_resid;
    let residual_se = sigma2.sqrt();
    let cov = invert_spd(&jtj).unwrap_or_else(|_| DenseMatrix::zeros(p, p));
    let mut se = vec![0.0_f64; p];
    for j in 0..p {
        let v = sigma2 * cov.get(j, j);
        se[j] = if v > 0.0 { v.sqrt() } else { 0.0 };
    }

    let y_mean: f64 = ys.iter().sum::<f64>() / n as f64;
    let tss: f64 = ys.iter().map(|v| (v - y_mean).powi(2)).sum();
    let r_squared = if tss > 0.0 { 1.0 - rss / tss } else { 0.0 };
    let adj_r_squared = if n > p {
        1.0 - (1.0 - r_squared) * (n as f64 - 1.0) / df_resid
    } else {
        r_squared
    };
    let f_df_num = if p >= 2 { (p - 1) as f64 } else { 1.0 };
    let model_ss = (tss - rss).max(0.0);
    let f_stat = if sigma2 > 0.0 && f_df_num > 0.0 {
        (model_ss / f_df_num) / sigma2
    } else {
        0.0
    };
    let f_p = if f_stat > 0.0 {
        f_sf(f_stat, f_df_num, df_resid)?
    } else {
        1.0
    };

    Ok(LmFit {
        params: beta,
        fitted,
        residuals,
        rss,
        residual_se,
        se,
        r_squared,
        adj_r_squared,
        f_stat,
        f_df_num,
        f_df_den: df_resid,
        f_p,
        n,
        p_count: p,
        n_iter,
        converged,
        final_lambda: lambda,
    })
}

fn compute_residuals<F>(
    f: &F,
    beta: &[f64],
    xs: &[f64],
    ys: &[f64],
    fitted: &mut [f64],
    residuals: &mut [f64],
) -> Result<f64, NumericError>
where
    F: Fn(&[f64], f64) -> f64,
{
    let mut rss = 0.0_f64;
    for i in 0..xs.len() {
        let yhat = f(beta, xs[i]);
        if !yhat.is_finite() {
            return Err(NumericError::NotFinite);
        }
        fitted[i] = yhat;
        let r = ys[i] - yhat;
        residuals[i] = r;
        rss += r * r;
    }
    Ok(rss)
}

fn central_diff_jacobian<F>(
    f: &F,
    beta: &[f64],
    xs: &[f64],
    j_mat: &mut DenseMatrix,
) -> Result<(), NumericError>
where
    F: Fn(&[f64], f64) -> f64,
{
    let p = beta.len();
    let n = xs.len();
    let mut local = beta.to_vec();
    // `h_j = sqrt(ulp) · max(|β_j|, 1)` — the usual central-diff
    // step for a scale-aware derivative.
    let eps = f64::EPSILON.sqrt();
    for j in 0..p {
        let h = eps * beta[j].abs().max(1.0);
        local[j] = beta[j] + h;
        let plus: Vec<f64> = xs.iter().map(|&x| f(&local, x)).collect();
        local[j] = beta[j] - h;
        let minus: Vec<f64> = xs.iter().map(|&x| f(&local, x)).collect();
        local[j] = beta[j];
        for i in 0..n {
            let a = plus[i];
            let b = minus[i];
            if !a.is_finite() || !b.is_finite() {
                return Err(NumericError::NotFinite);
            }
            // The residual `r = y − f`, so ∂r/∂β = −∂f/∂β. We want
            // the Jacobian of residuals wrt params (what LM solves
            // against), i.e. the negative of the model Jacobian.
            // Equivalently, we solve against `Jᵀ·r` with
            // `J = ∂f/∂β` because signs cancel in the normal
            // equations. Store `∂f/∂β` here.
            j_mat.set(i, j, (a - b) / (2.0 * h));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_model_matches_ols() {
        // f(β, x) = β0 + β1 · x on an exact line.
        let xs = [1.0_f64, 2.0, 3.0, 4.0, 5.0];
        let ys = [5.0_f64, 8.0, 11.0, 14.0, 17.0];
        let model = |b: &[f64], x: f64| b[0] + b[1] * x;
        let r =
            levenberg_marquardt(model, &xs, &ys, &[0.0_f64, 0.0], LmOptions::default()).unwrap();
        assert!(r.converged);
        assert!((r.params[0] - 2.0).abs() < 1e-9);
        assert!((r.params[1] - 3.0).abs() < 1e-9);
        assert!(r.rss < 1e-18);
    }

    #[test]
    fn exponential_recovers_params_from_rough_start() {
        // f(β, x) = β0 · (1 − exp(−β1 · x))
        let b_true = [2.0_f64, 0.5];
        let xs: Vec<f64> = (1..=10).map(|k| k as f64).collect();
        let ys: Vec<f64> = xs
            .iter()
            .map(|&x| b_true[0] * (1.0 - (-b_true[1] * x).exp()))
            .collect();
        let model = |b: &[f64], x: f64| b[0] * (1.0 - (-b[1] * x).exp());
        let r =
            levenberg_marquardt(model, &xs, &ys, &[1.0_f64, 0.1], LmOptions::default()).unwrap();
        assert!(r.converged);
        assert!((r.params[0] - b_true[0]).abs() < 1e-6, "b0={}", r.params[0]);
        assert!((r.params[1] - b_true[1]).abs() < 1e-6, "b1={}", r.params[1]);
    }
}
