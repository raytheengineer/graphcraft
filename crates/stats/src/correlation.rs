//! Pearson and Spearman rank correlations.
//!
//! Both return a signed `r`, its two-sided p-value under the null
//! `ρ = 0` using the Student-t transform
//!   `t = r · √((n − 2) / (1 − r²))` on `n − 2` df,
//! and the Fisher-z 95% CI (symmetric interval on `atanh r` back-
//! transformed by `tanh`). Spearman differs from Pearson only in
//! that it runs on tied-rank-transformed data.
//!
//! Behaviour reference (public):
//! - Fisher, R. A. (1915) "Frequency distribution of the values of
//!   the correlation coefficient in samples from an indefinitely
//!   large population", Biometrika 10, 507–521.
//! - Spearman, C. (1904) "The proof and measurement of association
//!   between two things", American Journal of Psychology 15, 72–101.

use graphcraft_numeric::{ranks_with_ties, NumericError};

use crate::distributions::t::t_sf;

/// Result of a Pearson (or Spearman, after rank-transform) correlation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Correlation {
    pub n: usize,
    pub r: f64,
    pub t: f64,
    pub df: f64,
    pub p_two_sided: f64,
    /// Fisher-z 95% CI on `r`.
    pub ci95: (f64, f64),
}

/// Pearson's product-moment correlation.
pub fn pearson(xs: &[f64], ys: &[f64]) -> Result<Correlation, NumericError> {
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
    for v in xs.iter().chain(ys.iter()) {
        if !v.is_finite() {
            return Err(NumericError::NotFinite);
        }
    }
    let nf = n as f64;
    let mx = xs.iter().sum::<f64>() / nf;
    let my = ys.iter().sum::<f64>() / nf;
    let mut sxx = 0.0_f64;
    let mut syy = 0.0_f64;
    let mut sxy = 0.0_f64;
    for (x, y) in xs.iter().zip(ys.iter()) {
        let dx = x - mx;
        let dy = y - my;
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    if sxx == 0.0 || syy == 0.0 {
        return Err(NumericError::DivisionByZero);
    }
    let r = sxy / (sxx * syy).sqrt();
    let r = r.clamp(-1.0, 1.0);
    let df = (n - 2) as f64;
    let t = if r.abs() < 1.0 {
        r * (df / (1.0 - r * r)).sqrt()
    } else {
        // Perfect correlation: finite r but infinite t; call p = 0.
        f64::INFINITY.copysign(r)
    };
    let p_two_sided = if t.is_finite() {
        (2.0 * t_sf(t.abs(), df)?).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let ci95 = fisher_z_ci(r, n, 0.95);
    Ok(Correlation {
        n,
        r,
        t,
        df,
        p_two_sided,
        ci95,
    })
}

/// Spearman's rank correlation. Equivalent to `pearson` on the
/// tie-averaged ranks of both inputs.
pub fn spearman(xs: &[f64], ys: &[f64]) -> Result<Correlation, NumericError> {
    if xs.len() != ys.len() {
        return Err(NumericError::TooFewValues {
            required: xs.len(),
            got: ys.len(),
        });
    }
    let (rx, _) = ranks_with_ties(xs)?;
    let (ry, _) = ranks_with_ties(ys)?;
    pearson(&rx, &ry)
}

fn fisher_z_ci(r: f64, n: usize, confidence: f64) -> (f64, f64) {
    if n < 4 || !(0.0..1.0).contains(&confidence) {
        return (f64::NAN, f64::NAN);
    }
    // z-quantile at (1 + confidence)/2 — hard-coded 1.96 for 95% to
    // avoid cross-crate chatter here; precision floor from erf
    // dominates the CI anyway.
    #[allow(clippy::excessive_precision)]
    let z = match confidence {
        c if (c - 0.95).abs() < 1e-9 => 1.959_963_984_540_054_f64,
        c if (c - 0.99).abs() < 1e-9 => 2.575_829_303_548_901_f64,
        c if (c - 0.90).abs() < 1e-9 => 1.644_853_626_951_472_f64,
        _ => 1.959_963_984_540_054_f64,
    };
    let zr = r.atanh();
    let se = (1.0 / (n as f64 - 3.0)).sqrt();
    let lo = (zr - z * se).tanh();
    let hi = (zr + z * se).tanh();
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pearson_exact_line_is_one() {
        let xs = [1.0_f64, 2.0, 3.0, 4.0, 5.0];
        let ys = [2.0_f64, 4.0, 6.0, 8.0, 10.0];
        let r = pearson(&xs, &ys).unwrap();
        assert!((r.r - 1.0).abs() < 1e-14);
        assert!(r.p_two_sided < 1e-6);
    }

    #[test]
    fn pearson_anti_line_is_minus_one() {
        let xs = [1.0_f64, 2.0, 3.0, 4.0, 5.0];
        let ys = [10.0_f64, 8.0, 6.0, 4.0, 2.0];
        let r = pearson(&xs, &ys).unwrap();
        assert!((r.r + 1.0).abs() < 1e-14);
    }

    #[test]
    fn pearson_textbook_small_sample() {
        // xs = 1..5, ys = [2, 4, 5, 4, 5]; Sxx=10, Sxy=6, Syy=6.
        // r = 6 / sqrt(60) = 6 / 7.74597 ≈ 0.774597.
        let xs = [1.0_f64, 2.0, 3.0, 4.0, 5.0];
        let ys = [2.0_f64, 4.0, 5.0, 4.0, 5.0];
        let r = pearson(&xs, &ys).unwrap();
        let expected = 6.0 / 60.0_f64.sqrt();
        assert!((r.r - expected).abs() < 1e-14);
    }

    #[test]
    fn spearman_on_monotone_input_is_one() {
        let xs = [10.0_f64, 20.0, 30.0, 40.0, 50.0];
        let ys = [1.0_f64, 100.0, 1000.0, 10_000.0, 100_000.0];
        let r = spearman(&xs, &ys).unwrap();
        assert!((r.r - 1.0).abs() < 1e-14);
    }

    #[test]
    fn pearson_zero_variance_rejected() {
        let err = pearson(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0]).unwrap_err();
        assert_eq!(err, NumericError::DivisionByZero);
    }

    #[test]
    fn ci95_bounds_contain_r_on_strong_data() {
        let xs = [1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let ys = [1.1_f64, 2.0, 2.9, 4.1, 4.9, 6.1, 6.9, 8.0];
        let c = pearson(&xs, &ys).unwrap();
        assert!(c.ci95.0 < c.r && c.r < c.ci95.1);
    }
}
