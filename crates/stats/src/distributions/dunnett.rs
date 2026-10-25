//! Dunnett's maximum-|T| and maximum-T distributions for a
//! balanced one-way design comparing `k − 1` treatment groups to a
//! common control.
//!
//! In that balanced case the `k − 1` t-statistics
//!   `T_i = (ȳ_i − ȳ_0) / (S · √(2/n))`
//! have equi-correlated multivariate-t joint distribution with
//! correlation `ρ = 1/2` and `ν = k·(n − 1)` degrees of freedom.
//! The equi-correlated structure lets the joint CDF collapse to a
//! single conditioning integral:
//!   `P(max_i T_i < d) = ∫₀^∞ f_W(w) · ∫ φ(z) · Φ(√2·d·w − z)^{k−1}
//!                        dz dw`
//! for the one-sided case, where `W = √(χ²_ν/ν)` is the pooled-SD
//! scale factor; the two-sided case uses `[Φ(a) − Φ(−a)]` in the
//! inner integrand.
//!
//! Reference (behaviour): Dunnett, C. W. (1955) "A multiple
//! comparison procedure for comparing several treatments with a
//! control", JASA 50, 1096–1121; Dunnett, C. W. (1964) "New tables
//! for multiple comparisons with a control", Biometrics 20, 482–491.
//! Public-domain critical-value tables; no third-party or GraphPad
//! code is used.

use graphcraft_numeric::NumericError;

use crate::distributions::normal::{big_phi, phi};
use crate::quadrature::gl32_integrate;
use crate::special::ln_gamma;

/// Which side of the Dunnett statistic to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DunnettSide {
    /// `P(max_i T_i < d)` — upper-tailed treatments-vs-control.
    OneSided,
    /// `P(max_i |T_i| < d)` — two-sided treatments-vs-control.
    TwoSided,
}

/// Dunnett CDF for `k − 1` treatments-vs-control at the balanced
/// design. `m = k − 1` is the number of comparisons, `df > 0` is
/// the pooled-variance degrees of freedom; `df = ∞` gives the
/// asymptotic case.
pub fn dunnett_cdf(d: f64, m: usize, df: f64, side: DunnettSide) -> Result<f64, NumericError> {
    if !d.is_finite() {
        return Ok(match (d > 0.0, side) {
            (true, _) => 1.0,
            (false, _) => 0.0,
        });
    }
    if m == 0 {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    if !df.is_finite() {
        return Ok(dunnett_asymptotic(d, m, side));
    }
    if df <= 0.0 {
        return Err(NumericError::NotFinite);
    }
    Ok(dunnett_finite(d, m, df, side))
}

/// Dunnett critical value — the inverse CDF at probability `p`.
pub fn dunnett_inv_cdf(p: f64, m: usize, df: f64, side: DunnettSide) -> Result<f64, NumericError> {
    if !p.is_finite() || !(0.0..=1.0).contains(&p) {
        return Err(NumericError::NotFinite);
    }
    if p == 0.0 {
        // The two-sided variable is strictly positive; the one-sided
        // variable has support on all of ℝ. Return 0 as a safe lower
        // bound for consumers; callers interested in true boundaries
        // should use `f64::NEG_INFINITY` themselves.
        return Ok(0.0);
    }
    if p == 1.0 {
        return Ok(f64::INFINITY);
    }
    let mut lo = match side {
        DunnettSide::OneSided => -10.0_f64,
        DunnettSide::TwoSided => 0.0_f64,
    };
    let mut hi = 20.0_f64;
    while dunnett_cdf(hi, m, df, side)? < p && hi < 1.0e6 {
        hi *= 2.0;
    }
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if hi - lo < 1.0e-6 {
            return Ok(mid);
        }
        if dunnett_cdf(mid, m, df, side)? < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(0.5 * (lo + hi))
}

/// Asymptotic (`ν = ∞`) Dunnett CDF — the inner z-integral alone.
///
/// For ρ = 1/2, the max-T CDF collapses to
///   `k · ∫ φ(z) · Φ(√2·d − z)^{k−1} dz`
/// — the `√2` factor comes from `1/√(1 − ρ) = √2` at ρ = 1/2.
fn dunnett_asymptotic(d: f64, m: usize, side: DunnettSide) -> f64 {
    let mf = m as f64;
    let integrand = |z: f64| {
        let inner = match side {
            DunnettSide::OneSided => big_phi(std::f64::consts::SQRT_2 * d - z),
            DunnettSide::TwoSided => {
                let a = std::f64::consts::SQRT_2 * d - z;
                let b = -std::f64::consts::SQRT_2 * d - z;
                big_phi(a) - big_phi(b)
            }
        };
        if inner <= 0.0 {
            0.0
        } else {
            phi(z) * inner.powf(mf)
        }
    };
    // z ∈ [-8, 8]: φ(z) < 10⁻¹⁵ at |z|=8, so truncation error is
    // dominated by the A&S erf precision (~1.5·10⁻⁷) anyway.
    gl32_integrate(-8.0, 8.0, integrand)
}

/// Finite-df Dunnett CDF — outer quadrature over the pooled-SD
/// scale `W = √(χ²_ν/ν)`.
fn dunnett_finite(d: f64, m: usize, df: f64, side: DunnettSide) -> f64 {
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
    // `d` enters the asymptotic form as a scale on the normal
    // argument. Scaling by `w` corresponds to replacing `d` with
    // `d·w`, i.e. integrating `f_W(w) · dunnett_asymptotic(d·w, …)`.
    let integrand = |w: f64| f_w(w) * dunnett_asymptotic(d * w, m, side);
    gl32_integrate(1.0e-6, 1.0, integrand) + gl32_integrate(1.0, 8.0, integrand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_treatment_matches_one_sided_normal() {
        // m = 1, ν = ∞: Dunnett one-sided reduces to a single
        // one-sided z-test, so P(T < d) = Φ(d).
        let d = 1.6448536269514722_f64; // Φ⁻¹(0.95)
        let got = dunnett_cdf(d, 1, f64::INFINITY, DunnettSide::OneSided).unwrap();
        assert!((got - 0.95).abs() < 1e-5, "got={got}");
    }

    #[test]
    fn one_treatment_two_sided_matches_normal() {
        // m = 1, two-sided, ν = ∞: P(|T| < d) = 2·Φ(d) − 1.
        let d = 1.959963984540054_f64; // Φ⁻¹(0.975)
        let got = dunnett_cdf(d, 1, f64::INFINITY, DunnettSide::TwoSided).unwrap();
        assert!((got - 0.95).abs() < 1e-5, "got={got}");
    }

    #[test]
    fn dunnett_published_crit_values_one_sided() {
        // Dunnett (1964) Table 1 — one-sided α = 0.05 critical values:
        //   m = 2, ν = ∞ → 1.916
        //   m = 3, ν = ∞ → 2.062
        //   m = 2, ν = 20 → 2.03
        for &(d, m, nu) in &[
            (1.916_f64, 2usize, f64::INFINITY),
            (2.062, 3, f64::INFINITY),
            (2.03, 2, 20.0),
        ] {
            let got = dunnett_cdf(d, m, nu, DunnettSide::OneSided).unwrap();
            assert!((got - 0.95).abs() < 2e-3, "m={m} nu={nu} d={d} got={got}");
        }
    }

    #[test]
    fn dunnett_published_crit_values_two_sided() {
        // Dunnett (1964) Table 2 — two-sided α = 0.05 critical values:
        //   m = 2, ν = ∞ → 2.212
        //   m = 3, ν = ∞ → 2.349
        for &(d, m, nu) in &[
            (2.212_f64, 2usize, f64::INFINITY),
            (2.349, 3, f64::INFINITY),
        ] {
            let got = dunnett_cdf(d, m, nu, DunnettSide::TwoSided).unwrap();
            assert!((got - 0.95).abs() < 2e-3, "m={m} nu={nu} d={d} got={got}");
        }
    }

    #[test]
    fn monotone_in_d() {
        let a = dunnett_cdf(1.0, 3, 20.0, DunnettSide::TwoSided).unwrap();
        let b = dunnett_cdf(2.0, 3, 20.0, DunnettSide::TwoSided).unwrap();
        let c = dunnett_cdf(3.0, 3, 20.0, DunnettSide::TwoSided).unwrap();
        assert!(a < b && b < c);
    }
}
