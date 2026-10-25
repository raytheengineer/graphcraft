//! Studentized-range distribution `Q = (max − min) / S`.
//!
//! Needed for Tukey's HSD post-hoc. The CDF has no closed form in
//! general and is computed from its standard integral representation
//! using Gauss–Legendre quadrature.
//!
//! For `ν = ∞` (asymptotic case):
//!   `F(q; k, ∞) = k · ∫ φ(z) · [Φ(z+q) − Φ(z)]^{k−1} dz`.
//!
//! For finite `ν`:
//!   `F(q; k, ν) = ∫₀^∞ f_W(w) · F(q·w; k, ∞) dw`
//! where `W = √(χ²_ν/ν)` has density
//!   `f_W(w) = (ν^{ν/2} / (2^{ν/2 − 1} · Γ(ν/2))) · w^{ν−1} ·
//!             exp(−ν · w²/2)`.
//!
//! Reference (behaviour): Lund, R. E. & Lund, J. R. (1983)
//! Algorithm AS 190 (studentized range probabilities and quantiles),
//! Applied Statistics 32, 204–210; and Harter, H. L. (1960)
//! "Tables of range and studentized range", Annals of Mathematical
//! Statistics 31, 1122–1147 (public-domain tables used for parity).
//! Numerical approach (Gauss–Legendre quadrature on the published
//! integral representation) is standard; no GraphPad material.

use graphcraft_numeric::NumericError;

use crate::distributions::normal::{big_phi, phi};
use crate::quadrature::gl32_integrate;
use crate::special::ln_gamma;

/// Studentized-range CDF at `(q, k, df)`. `k ≥ 2` is the number of
/// means; `df > 0` is the within-group degrees of freedom, with
/// `df = f64::INFINITY` giving the asymptotic case.
pub fn q_cdf(q: f64, k: usize, df: f64) -> Result<f64, NumericError> {
    if !q.is_finite() {
        if q <= 0.0 {
            return Ok(0.0);
        }
        return Ok(1.0);
    }
    if q <= 0.0 {
        return Ok(0.0);
    }
    if k < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: k,
        });
    }
    if !df.is_finite() {
        return Ok(q_asymptotic(q, k));
    }
    if df <= 0.0 {
        return Err(NumericError::NotFinite);
    }
    Ok(q_finite(q, k, df))
}

/// Studentized-range survival function.
pub fn q_sf(q: f64, k: usize, df: f64) -> Result<f64, NumericError> {
    Ok(1.0 - q_cdf(q, k, df)?)
}

/// Studentized-range quantile (inverse CDF) via bisection. `p ∈ (0, 1)`.
pub fn q_inv_cdf(p: f64, k: usize, df: f64) -> Result<f64, NumericError> {
    if !p.is_finite() || !(0.0..=1.0).contains(&p) {
        return Err(NumericError::NotFinite);
    }
    if p == 0.0 {
        return Ok(0.0);
    }
    if p == 1.0 {
        return Ok(f64::INFINITY);
    }
    let mut lo = 0.0_f64;
    let mut hi = 20.0_f64;
    while q_cdf(hi, k, df)? < p && hi < 1.0e6 {
        hi *= 2.0;
    }
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if hi - lo < 1.0e-6 {
            return Ok(mid);
        }
        if q_cdf(mid, k, df)? < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(0.5 * (lo + hi))
}

/// `F(q; k, ∞)` — the inner quadrature over `z`.
fn q_asymptotic(q: f64, k: usize) -> f64 {
    // Integrate k · φ(z) · [Φ(z+q) − Φ(z)]^{k−1} over z ∈ [-8, 8].
    // φ(z) is ~1e-15 at |z|=8, so this truncation is safe.
    let km1 = (k - 1) as f64;
    let kf = k as f64;
    let integrand = |z: f64| {
        let w = big_phi(z + q) - big_phi(z);
        if w <= 0.0 {
            0.0
        } else {
            kf * phi(z) * w.powf(km1)
        }
    };
    gl32_integrate(-8.0, 8.0, integrand)
}

/// `F(q; k, ν)` — outer quadrature over the `W = √(χ²_ν/ν)` scale.
fn q_finite(q: f64, k: usize, df: f64) -> f64 {
    // Density of W: ln f_W(w) = (ν/2)·ln(ν) − (ν/2 − 1)·ln(2)
    //                           − ln Γ(ν/2) + (ν−1)·ln(w) − ν·w²/2.
    let nu = df;
    let ln_coef =
        0.5 * nu * nu.ln() - (0.5 * nu - 1.0) * std::f64::consts::LN_2 - ln_gamma(0.5 * nu);
    let f_w = |w: f64| -> f64 {
        if w <= 0.0 {
            return 0.0;
        }
        let ln_fw = ln_coef + (nu - 1.0) * w.ln() - 0.5 * nu * w * w;
        if ln_fw < -700.0 {
            0.0
        } else {
            ln_fw.exp()
        }
    };
    // Truncate W-integration. W concentrates near 1 for large ν; for
    // small ν the right tail stretches but exp(-ν·w²/2) kills it by
    // w ≈ 6. Lower bound just above 0 to avoid w^{ν-1} issues for
    // very small ν.
    let w_lo = 1.0e-6_f64;
    let w_hi = 8.0_f64;
    let integrand = |w: f64| f_w(w) * q_asymptotic(q * w, k);
    // Two-piece split improves accuracy around the mode (w≈1).
    gl32_integrate(w_lo, 1.0, integrand) + gl32_integrate(1.0, w_hi, integrand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn k_eq_2_asymptotic_closed_form() {
        // F(q; 2, ∞) = 2·Φ(q/√2) − 1.
        // At q = √2 · 1.959963984540054 ≈ 2.7718619194388, F = 0.95.
        let q = 2.0_f64.sqrt() * 1.959_963_984_540_054;
        let got = q_cdf(q, 2, f64::INFINITY).unwrap();
        // A&S 7.1.26 erf precision ~1.5e-7 flows into Φ, so expect
        // the asymptotic quadrature to match the identity to ~1e-6.
        assert!((got - 0.95).abs() < 1e-5, "got={got}");
    }

    #[test]
    fn q_cdf_monotone_in_q() {
        let a = q_cdf(2.0, 3, 10.0).unwrap();
        let b = q_cdf(3.0, 3, 10.0).unwrap();
        let c = q_cdf(5.0, 3, 10.0).unwrap();
        assert!(a < b && b < c);
    }

    #[test]
    fn harter_published_crit_values_approx() {
        // Harter (1960) critical values at α = 0.05 (CDF = 0.95):
        //   k = 3, ν = ∞ → q = 3.3145
        //   k = 3, ν = 10 → q = 3.877
        //   k = 4, ν = 20 → q = 3.958
        for &(q, k, nu, expected) in &[
            (3.3145_f64, 3usize, f64::INFINITY, 0.95_f64),
            (3.877, 3, 10.0, 0.95),
            (3.958, 4, 20.0, 0.95),
        ] {
            let got = q_cdf(q, k, nu).unwrap();
            assert!(
                (got - expected).abs() < 2e-3,
                "k={k} nu={nu} q={q} got={got} expected={expected}"
            );
        }
    }

    #[test]
    fn q_inv_round_trip() {
        for p in [0.5_f64, 0.9, 0.95, 0.99] {
            for &(k, nu) in &[(3usize, f64::INFINITY), (4, 10.0), (5, 30.0)] {
                let q = q_inv_cdf(p, k, nu).unwrap();
                let back = q_cdf(q, k, nu).unwrap();
                assert!(
                    (back - p).abs() < 1e-3,
                    "p={p} k={k} nu={nu} q={q} back={back}"
                );
            }
        }
    }
}
