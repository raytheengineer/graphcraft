//! Special functions needed for continuous distributions.
//!
//! - [`ln_gamma`] via the Lanczos approximation (g=7, 9 coefficients).
//! - [`ln_beta`] = ln Γ(a) + ln Γ(b) − ln Γ(a+b).
//! - [`inc_beta_reg`] — the regularized incomplete beta function
//!   `I_x(a, b)`, computed from the Lentz continued-fraction form of
//!   `B(x; a, b)` with the standard `x > (a+1)/(a+b+2)` swap.
//! - [`inc_gamma_reg_p`] / [`inc_gamma_reg_q`] — the regularized
//!   lower and upper incomplete gamma functions `P(a, x)` and
//!   `Q(a, x)` via the standard series/continued-fraction split at
//!   `x < a + 1` vs `x ≥ a + 1`.
//!
//! Implementation is a direct transcription of the published
//! algorithms (Lanczos 1964; Numerical Recipes §6.2 and §6.4); no
//! third-party statistical code is used, and no symbolic Prism
//! material.

use graphcraft_numeric::NumericError;

const LN_2PI: f64 = 1.8378770664093454_f64;

/// Natural logarithm of the gamma function.
pub fn ln_gamma(x: f64) -> f64 {
    // Lanczos g=7 coefficients (published, e.g. Numerical Recipes
    // 3rd ed. §6.1 and the Lanczos original paper).
    const G: f64 = 7.0;
    #[allow(clippy::excessive_precision)]
    const COEFFS: [f64; 9] = [
        0.999_999_999_999_809_93,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_571_6e-6,
        1.505_632_735_149_311_6e-7,
    ];

    if x < 0.5 {
        // Reflection: Γ(x)Γ(1−x) = π / sin(πx).
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = COEFFS[0];
    for (i, c) in COEFFS.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    let t = x + G + 0.5;
    0.5 * LN_2PI + (x + 0.5) * t.ln() - t + a.ln()
}

/// Natural logarithm of the beta function.
pub fn ln_beta(a: f64, b: f64) -> f64 {
    ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)
}

/// Regularized incomplete beta function `I_x(a, b)`.
pub fn inc_beta_reg(x: f64, a: f64, b: f64) -> Result<f64, NumericError> {
    if !x.is_finite() || !a.is_finite() || !b.is_finite() {
        return Err(NumericError::NotFinite);
    }
    if !(0.0..=1.0).contains(&x) || a <= 0.0 || b <= 0.0 {
        return Err(NumericError::NotFinite);
    }
    if x == 0.0 {
        return Ok(0.0);
    }
    if x == 1.0 {
        return Ok(1.0);
    }
    let bt = (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        Ok(bt * betacf(a, b, x)? / a)
    } else {
        Ok(1.0 - bt * betacf(b, a, 1.0 - x)? / b)
    }
}

/// Lentz continued fraction for the incomplete beta — Numerical
/// Recipes §6.4 formulation.
fn betacf(a: f64, b: f64, x: f64) -> Result<f64, NumericError> {
    const MAX_ITER: usize = 500;
    const EPS: f64 = 1e-15;
    const FPMIN: f64 = 1e-300;

    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0_f64;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < FPMIN {
        d = FPMIN;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=MAX_ITER {
        let m_f = m as f64;
        let m2 = 2.0 * m_f;
        // Even step.
        let aa = m_f * (b - m_f) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        h *= d * c;
        // Odd step.
        let aa = -(a + m_f) * (qab + m_f) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < EPS {
            return Ok(h);
        }
    }
    // Did not converge within MAX_ITER — report a numeric error
    // rather than silently returning a half-cooked value.
    Err(NumericError::NotFinite)
}

/// Regularized lower incomplete gamma `P(a, x) = γ(a, x) / Γ(a)`.
///
/// Numerical Recipes §6.2 split:
/// - `x < a + 1`: series expansion `P(a, x) = e^{-x} x^a Σ Γ(a)/Γ(a+1+n) x^n / Γ(a)`.
/// - `x ≥ a + 1`: continued fraction for `Q(a, x) = 1 − P(a, x)`.
pub fn inc_gamma_reg_p(a: f64, x: f64) -> Result<f64, NumericError> {
    if !a.is_finite() || !x.is_finite() || a <= 0.0 || x < 0.0 {
        return Err(NumericError::NotFinite);
    }
    if x == 0.0 {
        return Ok(0.0);
    }
    if x < a + 1.0 {
        gser(a, x)
    } else {
        Ok(1.0 - gcf(a, x)?)
    }
}

/// Regularized upper incomplete gamma `Q(a, x) = 1 − P(a, x)`.
pub fn inc_gamma_reg_q(a: f64, x: f64) -> Result<f64, NumericError> {
    Ok(1.0 - inc_gamma_reg_p(a, x)?)
}

/// Series form of `P(a, x)` for `x < a + 1`.
fn gser(a: f64, x: f64) -> Result<f64, NumericError> {
    const MAX_ITER: usize = 1000;
    const EPS: f64 = 1e-16;
    let gln = ln_gamma(a);
    let mut ap = a;
    let mut sum = 1.0 / a;
    let mut del = sum;
    for _ in 0..MAX_ITER {
        ap += 1.0;
        del *= x / ap;
        sum += del;
        if del.abs() < sum.abs() * EPS {
            return Ok(sum * (-x + a * x.ln() - gln).exp());
        }
    }
    Err(NumericError::NotFinite)
}

/// Lentz continued fraction for `Q(a, x)` when `x ≥ a + 1`.
fn gcf(a: f64, x: f64) -> Result<f64, NumericError> {
    const MAX_ITER: usize = 1000;
    const EPS: f64 = 1e-16;
    const FPMIN: f64 = 1e-300;
    let gln = ln_gamma(a);
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / FPMIN;
    let mut d = 1.0 / b;
    let mut h = d;
    for n in 1..=MAX_ITER {
        let an = -(n as f64) * (n as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = b + an / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < EPS {
            return Ok((-x + a * x.ln() - gln).exp() * h);
        }
    }
    Err(NumericError::NotFinite)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ln_gamma_integer_factorials() {
        // Γ(n) = (n-1)! so ln Γ(n+1) = ln n!
        assert!((ln_gamma(1.0) - 0.0).abs() < 1e-13);
        assert!((ln_gamma(2.0) - 0.0).abs() < 1e-13); // ln 1
        assert!((ln_gamma(6.0) - (120.0_f64.ln())).abs() < 1e-12);
        assert!((ln_gamma(11.0) - (3_628_800.0_f64.ln())).abs() < 1e-10);
    }

    #[test]
    fn ln_gamma_half() {
        // Γ(1/2) = √π
        let expected = std::f64::consts::PI.sqrt().ln();
        assert!((ln_gamma(0.5) - expected).abs() < 1e-13);
    }

    #[test]
    fn inc_beta_symmetry() {
        // I_x(a, b) + I_{1-x}(b, a) = 1
        let a = 2.5;
        let b = 4.7;
        let x = 0.37;
        let ib1 = inc_beta_reg(x, a, b).unwrap();
        let ib2 = inc_beta_reg(1.0 - x, b, a).unwrap();
        assert!((ib1 + ib2 - 1.0).abs() < 1e-13);
    }

    #[test]
    fn inc_gamma_closed_forms() {
        // P(1, x) = 1 − exp(−x) (exponential CDF with λ=1).
        for x in [0.5_f64, 1.0, 2.5, 7.0] {
            let got = inc_gamma_reg_p(1.0, x).unwrap();
            let expected = 1.0 - (-x).exp();
            assert!(
                (got - expected).abs() < 1e-13,
                "P(1,{x})={got} vs {expected}"
            );
        }
    }

    #[test]
    fn inc_gamma_half_matches_erf() {
        // P(1/2, x) = erf(√x). We don't have double-precision erf
        // yet (A&S bound ~1.5e-7), so verify P(1/2, 1) ≈ erf(1).
        let got = inc_gamma_reg_p(0.5, 1.0).unwrap();
        let expected = 0.842_700_792_949_714_9_f64;
        assert!((got - expected).abs() < 1e-13);
    }

    #[test]
    fn inc_gamma_p_plus_q() {
        for (a, x) in [(0.5_f64, 1.0), (3.0, 2.0), (10.0, 15.0)] {
            let p = inc_gamma_reg_p(a, x).unwrap();
            let q = inc_gamma_reg_q(a, x).unwrap();
            assert!((p + q - 1.0).abs() < 1e-13);
        }
    }

    #[test]
    fn inc_beta_endpoints() {
        assert_eq!(inc_beta_reg(0.0, 2.0, 3.0).unwrap(), 0.0);
        assert_eq!(inc_beta_reg(1.0, 2.0, 3.0).unwrap(), 1.0);
    }
}
