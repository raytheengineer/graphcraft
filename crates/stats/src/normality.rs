#![allow(clippy::needless_range_loop, clippy::excessive_precision)]
//! Shapiro–Wilk W-test for normality (Royston 1992 approximation).
//!
//! Implements Royston's polynomial approximation to the Shapiro–Wilk
//! coefficients and the log-W → z-score transform. Valid for
//! `4 ≤ n ≤ 2000` (Royston 1992's claimed range).
//!
//! Algorithm summary:
//! 1. Sort `xs` → `y_(1) ≤ … ≤ y_(n)`.
//! 2. Compute Weisberg–Bingham expected normal order statistics
//!    `m_i = Φ⁻¹((i − 3/8) / (n + 1/4))`.
//! 3. Build Shapiro–Wilk coefficients `a` via Royston's polynomial
//!    corrections to `c = m / √(mᵀm)` at the two endpoints, with
//!    the scaling `ε²` to force `Σa² = 1`.
//! 4. `W = (Σ a_i y_(i))² / Σ (y_i − ȳ)²`.
//! 5. Transform `ln(1 − W)` to a standard normal `z` via Royston's
//!    small-n (4 ≤ n ≤ 11) or large-n (12 ≤ n ≤ 2000) coefficients.
//!
//! Reference (public):
//! - Shapiro, S. S. & Wilk, M. B. (1965) "An analysis of variance
//!   test for normality (complete samples)", Biometrika 52, 591–611.
//! - Royston, P. (1992) "Approximating the Shapiro–Wilk W-test for
//!   non-normality", Statistics and Computing 2, 117–119.
//! - Royston, P. (1995) "Remark AS R94: A remark on Algorithm AS 181:
//!   The W-test for normality", Applied Statistics 44, 547–551.
//!
//! Precision floor: the normal-inverse used for `m_i` and the normal
//! CDF used for the final p-value inherit the ≈ 1.5·10⁻⁷ error of
//! the A&S 7.1.26 erf we currently use. A tighter erf will tighten
//! this test.

use graphcraft_numeric::NumericError;

use crate::distributions::normal::{big_phi, phi_inv};

/// Result of a Shapiro–Wilk W-test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapiroWilk {
    pub n: usize,
    pub w: f64,
    /// Standardised z (approximate normal under H₀ of normality).
    pub z: f64,
    /// Upper-tail p-value `P(Z ≥ z_obs)` — small p rejects
    /// normality.
    pub p: f64,
}

/// Shapiro–Wilk normality test on `xs` (Royston 1992).
pub fn shapiro_wilk(xs: &[f64]) -> Result<ShapiroWilk, NumericError> {
    let n = xs.len();
    if !(4..=2000).contains(&n) {
        return Err(NumericError::TooFewValues {
            required: 4,
            got: n,
        });
    }
    for &x in xs {
        if !x.is_finite() {
            return Err(NumericError::NotFinite);
        }
    }

    // Sorted copy.
    let mut y = xs.to_vec();
    y.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Weisberg–Bingham m_i and normalised c_i.
    let nf = n as f64;
    let mut m = vec![0.0_f64; n];
    for i in 0..n {
        let p_i = (i as f64 + 1.0 - 0.375) / (nf + 0.25);
        m[i] = phi_inv(p_i)?;
    }
    let mtm: f64 = m.iter().map(|v| v * v).sum();
    if mtm <= 0.0 {
        return Err(NumericError::DivisionByZero);
    }
    let mtm_sqrt = mtm.sqrt();
    let c: Vec<f64> = m.iter().map(|v| v / mtm_sqrt).collect();

    // Royston's polynomial corrections at the two endpoints.
    // Coefficients from Royston (1992) eqs. (1) and (2).
    let u = 1.0 / nf.sqrt();
    let a_n = c[n - 1]
        + horner(
            u,
            &[0.221_157, -0.147_981, -2.071_190, 4.434_685, -2.706_056],
        ) * u;
    let a_nm1 = c[n - 2]
        + horner(
            u,
            &[0.042_981, -0.293_762, -1.752_460, 5.682_633, -3.582_633],
        ) * u;

    // Scaling ε to satisfy Σ a_i² = 1 after replacing a_n, a_{n-1}.
    // For n ≥ 6 both endpoints get the polynomial correction; for
    // n = 4 or 5 only the outermost pair is corrected and the
    // inner coefficients are taken directly from c with scaling.
    let (eps_sq, a) = if n > 5 {
        let eps_sq = (mtm - 2.0 * m[n - 1] * m[n - 1] - 2.0 * m[n - 2] * m[n - 2])
            / (1.0 - 2.0 * a_n * a_n - 2.0 * a_nm1 * a_nm1);
        if eps_sq <= 0.0 {
            return Err(NumericError::DivisionByZero);
        }
        let eps = eps_sq.sqrt();
        let mut a = vec![0.0_f64; n];
        a[n - 1] = a_n;
        a[n - 2] = a_nm1;
        a[0] = -a_n;
        a[1] = -a_nm1;
        for i in 2..(n - 2) {
            a[i] = m[i] / eps;
        }
        (eps_sq, a)
    } else {
        // n = 4 or 5: only correct the outermost pair.
        let eps_sq = (mtm - 2.0 * m[n - 1] * m[n - 1]) / (1.0 - 2.0 * a_n * a_n);
        if eps_sq <= 0.0 {
            return Err(NumericError::DivisionByZero);
        }
        let eps = eps_sq.sqrt();
        let mut a = vec![0.0_f64; n];
        a[n - 1] = a_n;
        a[0] = -a_n;
        for i in 1..(n - 1) {
            a[i] = m[i] / eps;
        }
        (eps_sq, a)
    };
    let _ = eps_sq;

    // W = (Σ a_i y_(i))² / Σ (y_i − ȳ)².
    let mean = y.iter().sum::<f64>() / nf;
    let ssd: f64 = y.iter().map(|v| (v - mean).powi(2)).sum();
    if ssd <= 0.0 {
        return Err(NumericError::DivisionByZero);
    }
    let num: f64 = a.iter().zip(y.iter()).map(|(ai, yi)| ai * yi).sum();
    let w = (num * num / ssd).clamp(0.0, 1.0);
    // Keep `a` from being optimised out by future linters.
    let _ = a.len();

    // z-transform (Royston 1992 Table 1).
    let (mu_z, sigma_z, log_term) = if n <= 11 {
        let n_f = nf;
        let gamma = -2.273 + 0.459 * n_f;
        let mu_z = 0.5440 - 0.39978 * n_f + 0.025054 * n_f * n_f - 0.0006714 * n_f * n_f * n_f;
        let sigma_z =
            (1.3822 - 0.77857 * n_f + 0.062767 * n_f * n_f - 0.0020322 * n_f * n_f * n_f).exp();
        let one_minus_w = (1.0 - w).max(1e-300);
        let arg = gamma - one_minus_w.ln();
        // Royston's small-n transform: y = −ln(γ − ln(1 − W)).
        let y_val = -(arg.ln());
        (mu_z, sigma_z, y_val)
    } else {
        let ln_n = nf.ln();
        let mu_z =
            -1.5861 - 0.31082 * ln_n - 0.083751 * ln_n * ln_n + 0.0038915 * ln_n * ln_n * ln_n;
        let sigma_z = (-0.4803 - 0.082676 * ln_n + 0.0030302 * ln_n * ln_n).exp();
        let one_minus_w = (1.0 - w).max(1e-300);
        (mu_z, sigma_z, one_minus_w.ln())
    };
    if sigma_z <= 0.0 || !sigma_z.is_finite() {
        return Err(NumericError::DivisionByZero);
    }
    let z = (log_term - mu_z) / sigma_z;
    // Small W (far from 1) → large positive z under the transform
    // → small upper-tail p (reject normality).
    let p = (1.0 - big_phi(z)).clamp(0.0, 1.0);

    Ok(ShapiroWilk { n, w, z, p })
}

/// Horner evaluation of `poly[0] + poly[1]·u + poly[2]·u² + …`.
fn horner(u: f64, poly: &[f64]) -> f64 {
    let mut acc = 0.0_f64;
    for &c in poly.iter().rev() {
        acc = acc * u + c;
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_too_few() {
        let r = shapiro_wilk(&[1.0, 2.0, 3.0]);
        assert!(r.is_err());
    }

    #[test]
    fn rejects_zero_variance() {
        let r = shapiro_wilk(&[1.0, 1.0, 1.0, 1.0, 1.0]);
        assert!(r.is_err());
    }

    #[test]
    fn evenly_spaced_data_w_near_one() {
        // Nearly-perfect normal order statistics → W close to 1,
        // large p. Not an exact match because Royston approximates
        // the Shapiro coefficients.
        let n = 20;
        let xs: Vec<f64> = (0..n)
            .map(|i| phi_inv((i as f64 + 1.0 - 0.375) / (n as f64 + 0.25)).unwrap())
            .collect();
        let r = shapiro_wilk(&xs).unwrap();
        assert!(r.w > 0.99, "W={}", r.w);
        assert!(r.p > 0.5, "p={}", r.p);
    }

    #[test]
    fn skewed_data_rejects_normality() {
        // Exponential-looking tail: W small, p small.
        let xs = [
            0.01, 0.02, 0.03, 0.05, 0.08, 0.13, 0.21, 0.34, 0.55, 0.89, 1.44, 2.33,
        ];
        let r = shapiro_wilk(&xs).unwrap();
        assert!(r.w < 0.9, "W={}", r.w);
        assert!(r.p < 0.05, "p={}", r.p);
    }

    #[test]
    fn w_at_most_one() {
        let xs = [1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let r = shapiro_wilk(&xs).unwrap();
        assert!(r.w <= 1.0 + 1e-12);
    }

    #[test]
    fn shapiro_wilk_1965_table_2_example() {
        // Shapiro & Wilk (1965) Table 2, n = 11 body weights.
        // Published W to 3 significant figures is 0.79.
        let xs = [
            148.0_f64, 154.0, 158.0, 160.0, 161.0, 162.0, 166.0, 170.0, 182.0, 195.0, 236.0,
        ];
        let r = shapiro_wilk(&xs).unwrap();
        assert!(
            (r.w - 0.79).abs() < 1e-2,
            "W = {} (expected ≈ 0.79 per Shapiro-Wilk 1965)",
            r.w
        );
        // The 1965 paper rejects normality at α = 0.05 for these
        // data, so our p must be below 0.05.
        assert!(r.p < 0.05, "p = {}", r.p);
    }

    #[test]
    fn w_unaffected_by_affine_rescale() {
        // W is location-and-scale invariant.
        let xs: Vec<f64> = (0..15).map(|i| (i as f64 - 7.0).sin()).collect();
        let r1 = shapiro_wilk(&xs).unwrap();
        let xs2: Vec<f64> = xs.iter().map(|v| 2.5 * v + 10.0).collect();
        let r2 = shapiro_wilk(&xs2).unwrap();
        assert!((r1.w - r2.w).abs() < 1e-12);
    }
}
