//! Descriptive statistics on `&[f64]`.
//!
//! All inputs are required to be finite. Variance uses the n-1
//! (sample) denominator, matching the public Prism manual's default
//! for "Analyze → Column statistics". The sum of squared deviations
//! is computed via a two-pass algorithm with Neumaier (Kahan)
//! compensation to tame cancellation on data far from zero.
//!
//! References (behaviour, not source):
//!   - NIST StRD "Lew (Beam Deflection Data)" certified values —
//!     https://itl.nist.gov/div898/strd/univ/lew.html
//!   - GraphPad Prism public manual, "Column statistics".

use graphcraft_numeric::{kahan_sum, require_finite, NumericError};

/// A one-pass bundle of descriptive statistics on a non-empty slice
/// of finite `f64`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Descriptives {
    pub n: usize,
    pub mean: f64,
    /// Sample standard deviation (denominator = n - 1). `None` when
    /// `n < 2`.
    pub sd: Option<f64>,
    /// Sample variance (denominator = n - 1). `None` when `n < 2`.
    pub variance: Option<f64>,
    /// Standard error of the mean, `sd / sqrt(n)`. `None` when
    /// `n < 2`.
    pub sem: Option<f64>,
    /// Median (linear interpolation between the two middle values on
    /// even-length samples — this is the Prism-manual default).
    pub median: f64,
    pub min: f64,
    pub max: f64,
}

impl Descriptives {
    /// Compute descriptives on `xs`. Returns `Err` if the slice is
    /// empty or contains any non-finite value.
    pub fn of(xs: &[f64]) -> Result<Self, NumericError> {
        if xs.is_empty() {
            return Err(NumericError::TooFewValues {
                required: 1,
                got: 0,
            });
        }
        for &x in xs {
            require_finite(x)?;
        }
        let n = xs.len();
        let s = kahan_sum(xs)?;
        let mean = s / (n as f64);

        let (variance, sd, sem) = if n >= 2 {
            // Sum of squared deviations with compensation.
            let deviations: Vec<f64> = xs.iter().map(|&x| (x - mean).powi(2)).collect();
            let ss = kahan_sum(&deviations)?;
            let var = ss / ((n - 1) as f64);
            let sd = var.sqrt();
            let sem = sd / (n as f64).sqrt();
            (Some(var), Some(sd), Some(sem))
        } else {
            (None, None, None)
        };

        let mut sorted: Vec<f64> = xs.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = median_of_sorted(&sorted);
        let min = sorted.first().copied().unwrap_or(f64::NAN);
        let max = sorted.last().copied().unwrap_or(f64::NAN);

        Ok(Self {
            n,
            mean,
            sd,
            variance,
            sem,
            median,
            min,
            max,
        })
    }

    pub fn range(&self) -> f64 {
        self.max - self.min
    }
}

fn median_of_sorted(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    #[allow(clippy::integer_division)]
    let mid = n / 2;
    if n % 2 == 0 {
        let a = sorted.get(mid - 1).copied().unwrap_or(f64::NAN);
        let b = sorted.get(mid).copied().unwrap_or(f64::NAN);
        0.5 * (a + b)
    } else {
        sorted.get(mid).copied().unwrap_or(f64::NAN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textbook_small_sample() {
        // xs = 2, 4, 4, 4, 5, 5, 7, 9  — n=8, mean=5. Squared
        // deviations sum to 32 ⇒ sample variance 32/7 ≈ 4.5714286,
        // sample sd = sqrt(32/7) ≈ 2.1380899.
        let d = Descriptives::of(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]).unwrap();
        assert_eq!(d.n, 8);
        assert!((d.mean - 5.0).abs() < 1e-15);
        assert!((d.variance.unwrap() - 32.0 / 7.0).abs() < 1e-14);
        assert!((d.sd.unwrap() - (32.0_f64 / 7.0).sqrt()).abs() < 1e-14);
        assert!((d.sem.unwrap() - (32.0_f64 / 7.0).sqrt() / 8.0_f64.sqrt()).abs() < 1e-14);
        assert!((d.median - 4.5).abs() < 1e-15);
        assert_eq!(d.min, 2.0);
        assert_eq!(d.max, 9.0);
        assert_eq!(d.range(), 7.0);
    }

    #[test]
    fn median_odd_sample() {
        let d = Descriptives::of(&[1.0, 3.0, 2.0]).unwrap();
        assert_eq!(d.median, 2.0);
    }

    #[test]
    fn single_value_has_no_dispersion() {
        let d = Descriptives::of(&[42.0]).unwrap();
        assert_eq!(d.n, 1);
        assert_eq!(d.mean, 42.0);
        assert_eq!(d.median, 42.0);
        assert_eq!(d.sd, None);
        assert_eq!(d.variance, None);
        assert_eq!(d.sem, None);
    }

    #[test]
    fn empty_rejected() {
        assert!(matches!(
            Descriptives::of(&[]),
            Err(NumericError::TooFewValues { .. })
        ));
    }

    #[test]
    fn nan_rejected() {
        assert!(matches!(
            Descriptives::of(&[1.0, f64::NAN, 3.0]),
            Err(NumericError::NotFinite)
        ));
    }
}
