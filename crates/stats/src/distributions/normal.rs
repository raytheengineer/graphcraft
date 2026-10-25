//! Standard normal distribution.
//!
//! `erf` uses the Abramowitz & Stegun 7.1.26 rational approximation
//! (maximum absolute error 1.5·10⁻⁷). That is sufficient for the
//! M3.3b post-hoc critical-value parity rows (tabled to 3 decimal
//! places) and for later use in large-sample non-parametric
//! approximations (M3.4). A tighter erf — Chebyshev or
//! Cody-rational, giving double precision — is a scheduled upgrade
//! once a parity row demands it; the file is self-contained so the
//! swap is isolated.
//!
//! Reference (behaviour): Abramowitz, M. & Stegun, I. A. (1964)
//! *Handbook of Mathematical Functions*, §7.1.26. Public domain
//! (NIST). No GraphPad material.

use graphcraft_numeric::NumericError;

const SQRT_2PI: f64 = 2.506_628_274_631_000_7;
const INV_SQRT_2: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// Error function `erf(x) = (2/√π) ∫₀ˣ e^{-t²} dt`.
///
/// Max absolute error ≈ 1.5·10⁻⁷ (A&S 7.1.26).
pub fn erf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x.is_infinite() {
        return x.signum();
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let ax = x.abs();
    // A&S 7.1.26 coefficients.
    const P: f64 = 0.327_591_1;
    const A1: f64 = 0.254_829_592;
    const A2: f64 = -0.284_496_736;
    const A3: f64 = 1.421_413_741;
    const A4: f64 = -1.453_152_027;
    const A5: f64 = 1.061_405_429;
    let t = 1.0 / (1.0 + P * ax);
    let poly = ((((A5 * t + A4) * t + A3) * t + A2) * t + A1) * t;
    let y = 1.0 - poly * (-ax * ax).exp();
    sign * y
}

/// Standard normal PDF.
pub fn phi(x: f64) -> f64 {
    (-(x * x) * 0.5).exp() / SQRT_2PI
}

/// Standard normal CDF `Φ(x)`.
pub fn big_phi(x: f64) -> f64 {
    0.5 * (1.0 + erf(x * INV_SQRT_2))
}

/// Standard normal inverse CDF `Φ⁻¹(p)` via bisection on `big_phi`.
/// Precision inherits from `erf` (≈ 1.5·10⁻⁷).
pub fn phi_inv(p: f64) -> Result<f64, NumericError> {
    if !p.is_finite() || !(0.0..=1.0).contains(&p) {
        return Err(NumericError::NotFinite);
    }
    if p == 0.0 {
        return Ok(f64::NEG_INFINITY);
    }
    if p == 1.0 {
        return Ok(f64::INFINITY);
    }
    if p == 0.5 {
        return Ok(0.0);
    }
    let (target, sign) = if p < 0.5 { (1.0 - p, -1.0) } else { (p, 1.0) };
    let mut lo = 0.0_f64;
    let mut hi = 10.0_f64;
    while big_phi(hi) < target && hi < 40.0 {
        hi += 5.0;
    }
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if hi - lo < 1.0e-13 {
            return Ok(sign * mid);
        }
        if big_phi(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(sign * 0.5 * (lo + hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erf_zero_and_sign() {
        // A&S 7.1.26 is not exact at x=0 (max abs error 1.5e-7).
        assert!(erf(0.0).abs() < 2e-7);
        assert!(erf(1.0) > 0.0);
        assert!(erf(-1.0) < 0.0);
        assert!((erf(1.0) + erf(-1.0)).abs() < 1e-15);
    }

    #[test]
    fn erf_known_points() {
        // A&S 7.1.26 is accurate to ~1.5e-7.
        assert!((erf(0.5) - 0.520_499_877_813_046_5).abs() < 2e-7);
        assert!((erf(1.0) - 0.842_700_792_949_714_9).abs() < 2e-7);
        assert!((erf(2.0) - 0.995_322_265_018_952_7).abs() < 2e-7);
    }

    #[test]
    fn big_phi_endpoints_and_symmetry() {
        assert!((big_phi(0.0) - 0.5).abs() < 2e-7);
        for &x in &[0.3, 0.9, 1.7, 2.5] {
            assert!((big_phi(x) + big_phi(-x) - 1.0).abs() < 2e-7);
        }
    }

    #[test]
    fn phi_inv_round_trip() {
        for p in [0.001_f64, 0.025, 0.1, 0.5, 0.9, 0.975, 0.999] {
            let x = phi_inv(p).unwrap();
            let back = big_phi(x);
            assert!((back - p).abs() < 1e-6, "p={p} x={x} back={back}");
        }
    }

    #[test]
    fn phi_pdf_integrates_approximately() {
        // Trapezoidal integral of φ over [-6, 6] with 10001 points.
        let n = 10_000;
        let a = -6.0_f64;
        let b = 6.0_f64;
        let h = (b - a) / n as f64;
        let mut s = 0.5 * (phi(a) + phi(b));
        for i in 1..n {
            s += phi(a + i as f64 * h);
        }
        let total = s * h;
        assert!((total - 1.0).abs() < 1e-6);
    }
}
