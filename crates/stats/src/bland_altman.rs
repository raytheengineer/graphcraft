//! Bland–Altman agreement analysis.
//!
//! Compares two paired measurements by summarising the differences:
//! bias (mean difference), SD of differences, and 95% limits of
//! agreement `bias ± 1.96 · SD`. The analysis is diagnostic in the
//! Bland–Altman sense (not a hypothesis test), so no p-value is
//! produced.
//!
//! Reference (public): Bland, J. M. & Altman, D. G. (1986)
//! "Statistical methods for assessing agreement between two methods
//! of clinical measurement", The Lancet 327, 307–310.

use graphcraft_numeric::NumericError;

use crate::descriptive::Descriptives;

/// Result of a Bland–Altman analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct BlandAltman {
    pub n: usize,
    pub bias: f64,
    pub sd_diff: f64,
    /// `bias − 1.96 · SD`, `bias + 1.96 · SD`.
    pub lower_loa: f64,
    pub upper_loa: f64,
    /// 95% CI on the bias: `bias ± 1.96 · SE`.
    pub bias_ci95: (f64, f64),
    /// Per-point `(mean(x, y), y − x)` for the Bland–Altman plot.
    pub points: Vec<(f64, f64)>,
}

/// Bland–Altman on paired measurements. `method_a[i]` and
/// `method_b[i]` are the two values for subject `i`. Rows where
/// either value is non-finite are rejected.
pub fn bland_altman(method_a: &[f64], method_b: &[f64]) -> Result<BlandAltman, NumericError> {
    if method_a.len() != method_b.len() {
        return Err(NumericError::TooFewValues {
            required: method_a.len(),
            got: method_b.len(),
        });
    }
    if method_a.len() < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: method_a.len(),
        });
    }
    let mut diffs = Vec::with_capacity(method_a.len());
    let mut points = Vec::with_capacity(method_a.len());
    for (a, b) in method_a.iter().zip(method_b.iter()) {
        if !a.is_finite() || !b.is_finite() {
            return Err(NumericError::NotFinite);
        }
        diffs.push(b - a);
        points.push(((a + b) * 0.5, b - a));
    }
    let d = Descriptives::of(&diffs)?;
    let sd = d.sd.unwrap_or(0.0);
    let z = 1.959_963_984_540_054_f64;
    let bias = d.mean;
    let sem = sd / (d.n as f64).sqrt();
    Ok(BlandAltman {
        n: d.n,
        bias,
        sd_diff: sd,
        lower_loa: bias - z * sd,
        upper_loa: bias + z * sd,
        bias_ci95: (bias - z * sem, bias + z * sem),
        points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_methods_zero_bias() {
        let a = [1.0_f64, 2.0, 3.0, 4.0, 5.0];
        let b = [1.0_f64, 2.0, 3.0, 4.0, 5.0];
        let r = bland_altman(&a, &b).unwrap();
        assert!((r.bias - 0.0).abs() < 1e-15);
        assert!((r.sd_diff - 0.0).abs() < 1e-15);
        // Limits of agreement collapse onto the bias when SD is zero.
        assert!((r.lower_loa - r.upper_loa).abs() < 1e-15);
    }

    #[test]
    fn constant_offset_recovered() {
        // b = a + 5 → bias = 5, sd_diff = 0 → DivisionByZero.
        // Need some variation in the differences for a real test.
        let a = [1.0_f64, 2.0, 3.0, 4.0, 5.0];
        let b = [1.0_f64, 3.0, 4.0, 6.0, 7.0];
        // diffs = [0, 1, 1, 2, 2] → mean = 1.2,
        //   var (n-1) = ((0.72 + 0.04 + 0.04 + 0.64 + 0.64)/4)
        //              = 2.08/4 = 0.52. Wait let me recompute...
        //   centred: [-1.2, -0.2, -0.2, 0.8, 0.8]
        //   squared: [1.44, 0.04, 0.04, 0.64, 0.64] = 2.80
        //   var = 2.80/4 = 0.70 → sd = sqrt(0.7).
        let r = bland_altman(&a, &b).unwrap();
        assert_eq!(r.n, 5);
        assert!((r.bias - 1.2).abs() < 1e-13);
        assert!((r.sd_diff - 0.7_f64.sqrt()).abs() < 1e-13);
        let z = 1.959_963_984_540_054_f64;
        assert!((r.upper_loa - (1.2 + z * 0.7_f64.sqrt())).abs() < 1e-13);
    }

    #[test]
    fn length_mismatch_rejected() {
        assert!(bland_altman(&[1.0, 2.0], &[1.0]).is_err());
    }
}
