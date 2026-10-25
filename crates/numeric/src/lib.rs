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

/// Tie-aware fractional ranks ("midrank" or "average rank" method)
/// for the input values. Returns `ranks[i]` as the rank of `xs[i]`
/// in ascending order, with each tied group assigned the average
/// of the ranks it occupies.
///
/// Also returns `tie_correction`, defined as `Σ (t³ − t)` over each
/// tied group of size `t`, which is the standard correction factor
/// used by Mann–Whitney, Kruskal–Wallis and related rank statistics.
///
/// `Err(NotFinite)` if any value is NaN or infinite.
pub fn ranks_with_ties(xs: &[f64]) -> Result<(Vec<f64>, f64), NumericError> {
    for &x in xs {
        require_finite(x)?;
    }
    let n = xs.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        xs[a]
            .partial_cmp(&xs[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ranks = vec![0.0_f64; n];
    let mut tie_corr = 0.0_f64;
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && xs[order[j]] == xs[order[i]] {
            j += 1;
        }
        // Positions i..j (0-based) occupy ranks i+1..=j, average
        // rank = (i+1 + j) / 2 = i + (j - i + 1) / 2 ... simpler:
        let avg = ((i + 1 + j) as f64) * 0.5;
        for k in i..j {
            ranks[order[k]] = avg;
        }
        let t = (j - i) as f64;
        if t > 1.0 {
            tie_corr += t * t * t - t;
        }
        i = j;
    }
    Ok((ranks, tie_corr))
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
    fn ranks_no_ties() {
        let (r, c) = ranks_with_ties(&[3.0, 1.0, 2.0]).unwrap();
        assert_eq!(r, vec![3.0, 1.0, 2.0]);
        assert_eq!(c, 0.0);
    }

    #[test]
    fn ranks_with_ties_averaged() {
        // Values 1, 2, 2, 3, 3, 3.
        // Rank positions 1, 2, 3, 4, 5, 6.
        // The two 2's: avg rank (2+3)/2 = 2.5.
        // The three 3's: avg rank (4+5+6)/3 = 5.
        // Tie correction = (2³ − 2) + (3³ − 3) = 6 + 24 = 30.
        let (r, c) = ranks_with_ties(&[1.0, 2.0, 2.0, 3.0, 3.0, 3.0]).unwrap();
        assert_eq!(r, vec![1.0, 2.5, 2.5, 5.0, 5.0, 5.0]);
        assert_eq!(c, 30.0);
    }

    #[test]
    fn ranks_rejects_nan() {
        assert_eq!(
            ranks_with_ties(&[1.0, f64::NAN]),
            Err(NumericError::NotFinite)
        );
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
