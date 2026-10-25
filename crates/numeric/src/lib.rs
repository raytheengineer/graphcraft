//! L1 numeric utilities: finite-float discipline, special functions, RNG.
//!
//! No panics in shipped code; every fallible routine returns `Result` or
//! `Option`. Callers above the engine layer surface failures as
//! `EngineError`.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use std::fmt;

/// Error produced by numeric routines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericError {
    /// A float that must be finite was NaN or infinite.
    NotFinite,
    /// A sample size or dimension was below the required minimum.
    TooFewValues { required: usize, got: usize },
    /// A denominator was zero or sub-normal.
    DivisionByZero,
}

impl fmt::Display for NumericError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NumericError::NotFinite => f.write_str("value is NaN or infinite"),
            NumericError::TooFewValues { required, got } => {
                write!(f, "need at least {required} values, got {got}")
            }
            NumericError::DivisionByZero => f.write_str("division by zero"),
        }
    }
}

impl std::error::Error for NumericError {}

/// Return `Err(NotFinite)` if `x` is NaN or infinite.
#[inline]
pub fn require_finite(x: f64) -> Result<f64, NumericError> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(NumericError::NotFinite)
    }
}

/// Sum a slice using a Neumaier-corrected compensation to limit floating
/// point error accumulation. Returns `Err` if any element is non-finite.
pub fn kahan_sum(xs: &[f64]) -> Result<f64, NumericError> {
    let mut sum = 0.0_f64;
    let mut c = 0.0_f64;
    for &x in xs {
        require_finite(x)?;
        let t = sum + x;
        if sum.abs() >= x.abs() {
            c += (sum - t) + x;
        } else {
            c += (x - t) + sum;
        }
        sum = t;
    }
    Ok(sum + c)
}

/// Arithmetic mean. Errors if the slice is empty or non-finite.
pub fn mean(xs: &[f64]) -> Result<f64, NumericError> {
    if xs.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    let s = kahan_sum(xs)?;
    Ok(s / (xs.len() as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_of_three() {
        let m = mean(&[1.0, 2.0, 3.0]).unwrap();
        assert!((m - 2.0).abs() < 1e-15);
    }

    #[test]
    fn mean_empty_errs() {
        assert_eq!(
            mean(&[]),
            Err(NumericError::TooFewValues {
                required: 1,
                got: 0
            })
        );
    }

    #[test]
    fn nan_rejected() {
        assert_eq!(mean(&[1.0, f64::NAN]), Err(NumericError::NotFinite));
    }

    #[test]
    fn kahan_beats_naive_on_cancellation() {
        // 1e16 + 1 + ... + 1 - 1e16 : naive f64 sum loses the ones.
        let mut xs = vec![1.0e16];
        xs.extend(std::iter::repeat(1.0).take(1000));
        xs.push(-1.0e16);
        let got = kahan_sum(&xs).unwrap();
        assert!((got - 1000.0).abs() < 1e-6);
    }
}
