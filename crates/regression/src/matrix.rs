#![allow(clippy::needless_range_loop)]
//! Minimal dense-matrix primitives used by the OLS fit.
//!
//! `DenseMatrix` is row-major `Vec<f64>` with explicit `rows`/`cols`.
//! Operations are those we actually need for M4.1: matmul for small
//! shapes, a transposed-self product `X'·X`, Cholesky factorisation
//! for symmetric positive-definite matrices, and triangular solves.
//!
//! No third-party linear algebra crate is used.
//!
//! Reference (behaviour): Golub, G. H. & Van Loan, C. F. (2013)
//! *Matrix Computations*, 4th ed., Johns Hopkins Univ. Press,
//! §4.2 (Cholesky). Classical algorithm; no third-party or
//! GraphPad code.

use graphcraft_numeric::NumericError;

/// Row-major dense matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct DenseMatrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f64>,
}

impl DenseMatrix {
    /// Zero matrix of the given shape.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
    }

    /// Build from a row-major Vec. Fails if the length does not
    /// match `rows · cols`.
    pub fn from_row_major(rows: usize, cols: usize, data: Vec<f64>) -> Result<Self, NumericError> {
        if data.len() != rows * cols {
            return Err(NumericError::TooFewValues {
                required: rows * cols,
                got: data.len(),
            });
        }
        for &x in &data {
            if !x.is_finite() {
                return Err(NumericError::NotFinite);
            }
        }
        Ok(Self { rows, cols, data })
    }

    #[inline]
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.data[i * self.cols + j]
    }

    #[inline]
    pub fn set(&mut self, i: usize, j: usize, v: f64) {
        self.data[i * self.cols + j] = v;
    }

    /// `X' · X` where `self = X`. Result is symmetric `cols × cols`.
    pub fn xtx(&self) -> DenseMatrix {
        let p = self.cols;
        let n = self.rows;
        let mut out = DenseMatrix::zeros(p, p);
        for j in 0..p {
            for k in j..p {
                let mut s = 0.0_f64;
                for i in 0..n {
                    s += self.get(i, j) * self.get(i, k);
                }
                out.set(j, k, s);
                out.set(k, j, s);
            }
        }
        out
    }

    /// `X' · y` where `self = X`, `y.len() == self.rows`.
    pub fn xty(&self, y: &[f64]) -> Result<Vec<f64>, NumericError> {
        if y.len() != self.rows {
            return Err(NumericError::TooFewValues {
                required: self.rows,
                got: y.len(),
            });
        }
        let p = self.cols;
        let mut out = vec![0.0_f64; p];
        for j in 0..p {
            let mut s = 0.0_f64;
            for i in 0..self.rows {
                s += self.get(i, j) * y[i];
            }
            out[j] = s;
        }
        Ok(out)
    }
}

/// In-place Cholesky factorisation `A = L · Lᵀ` for a symmetric
/// positive-definite matrix. The lower triangle of `a` is
/// overwritten with `L`; the strict upper triangle is zeroed.
pub fn cholesky(a: &mut DenseMatrix) -> Result<(), NumericError> {
    if a.rows != a.cols {
        return Err(NumericError::TooFewValues {
            required: a.cols,
            got: a.rows,
        });
    }
    let n = a.rows;
    for i in 0..n {
        for j in 0..=i {
            let mut sum = a.get(i, j);
            for k in 0..j {
                sum -= a.get(i, k) * a.get(j, k);
            }
            if i == j {
                if sum <= 0.0 || !sum.is_finite() {
                    return Err(NumericError::DivisionByZero);
                }
                a.set(i, j, sum.sqrt());
            } else {
                let ljj = a.get(j, j);
                if ljj == 0.0 {
                    return Err(NumericError::DivisionByZero);
                }
                a.set(i, j, sum / ljj);
            }
        }
    }
    // Zero the strict upper triangle for cleanliness.
    for i in 0..n {
        for j in (i + 1)..n {
            a.set(i, j, 0.0);
        }
    }
    Ok(())
}

/// Solve `L · z = b` for lower-triangular `L` (in-place on `b`).
pub fn forward_solve(l: &DenseMatrix, b: &mut [f64]) -> Result<(), NumericError> {
    let n = l.rows;
    if l.cols != n || b.len() != n {
        return Err(NumericError::TooFewValues {
            required: n,
            got: b.len(),
        });
    }
    for i in 0..n {
        let mut sum = b[i];
        for k in 0..i {
            sum -= l.get(i, k) * b[k];
        }
        let diag = l.get(i, i);
        if diag == 0.0 {
            return Err(NumericError::DivisionByZero);
        }
        b[i] = sum / diag;
    }
    Ok(())
}

/// Solve `Lᵀ · x = z` for lower-triangular `L` (in-place on `z`).
pub fn back_solve_transposed(l: &DenseMatrix, z: &mut [f64]) -> Result<(), NumericError> {
    let n = l.rows;
    if l.cols != n || z.len() != n {
        return Err(NumericError::TooFewValues {
            required: n,
            got: z.len(),
        });
    }
    for ii in 0..n {
        let i = n - 1 - ii;
        let mut sum = z[i];
        for k in (i + 1)..n {
            sum -= l.get(k, i) * z[k];
        }
        let diag = l.get(i, i);
        if diag == 0.0 {
            return Err(NumericError::DivisionByZero);
        }
        z[i] = sum / diag;
    }
    Ok(())
}

/// Solve `A · x = b` for symmetric positive-definite `A` using
/// Cholesky. `a` is overwritten with its L factor; returns `x`.
pub fn solve_spd(a: &mut DenseMatrix, b: &[f64]) -> Result<Vec<f64>, NumericError> {
    cholesky(a)?;
    let mut x = b.to_vec();
    forward_solve(a, &mut x)?;
    back_solve_transposed(a, &mut x)?;
    Ok(x)
}

/// Invert a symmetric positive-definite matrix via its Cholesky
/// factor. Returns the dense inverse `A⁻¹`.
pub fn invert_spd(a: &DenseMatrix) -> Result<DenseMatrix, NumericError> {
    let n = a.rows;
    if a.cols != n {
        return Err(NumericError::TooFewValues {
            required: n,
            got: a.cols,
        });
    }
    let mut l = a.clone();
    cholesky(&mut l)?;
    let mut inv = DenseMatrix::zeros(n, n);
    let mut e = vec![0.0_f64; n];
    for j in 0..n {
        for v in e.iter_mut() {
            *v = 0.0;
        }
        e[j] = 1.0;
        forward_solve(&l, &mut e)?;
        back_solve_transposed(&l, &mut e)?;
        for i in 0..n {
            inv.set(i, j, e[i]);
        }
    }
    Ok(inv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cholesky_of_identity_is_identity() {
        let mut a =
            DenseMatrix::from_row_major(3, 3, vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
                .unwrap();
        cholesky(&mut a).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((a.get(i, j) - expected).abs() < 1e-15);
            }
        }
    }

    #[test]
    fn solve_spd_recovers_x() {
        // A = [[4, 12, -16], [12, 37, -43], [-16, -43, 98]] is spd
        // (Golub & Van Loan canonical example). Solve A·x = b for a
        // known b.
        let a_data = vec![4.0, 12.0, -16.0, 12.0, 37.0, -43.0, -16.0, -43.0, 98.0];
        let a = DenseMatrix::from_row_major(3, 3, a_data).unwrap();
        let x_true = [1.0_f64, 2.0, 3.0];
        // b = A·x
        let mut b = vec![0.0_f64; 3];
        for i in 0..3 {
            for j in 0..3 {
                b[i] += a.get(i, j) * x_true[j];
            }
        }
        let mut a_mut = a.clone();
        let x = solve_spd(&mut a_mut, &b).unwrap();
        for i in 0..3 {
            assert!((x[i] - x_true[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn invert_spd_round_trip() {
        let a = DenseMatrix::from_row_major(
            3,
            3,
            vec![4.0, 12.0, -16.0, 12.0, 37.0, -43.0, -16.0, -43.0, 98.0],
        )
        .unwrap();
        let inv = invert_spd(&a).unwrap();
        // A · A⁻¹ = I
        for i in 0..3 {
            for j in 0..3 {
                let mut s = 0.0_f64;
                for k in 0..3 {
                    s += a.get(i, k) * inv.get(k, j);
                }
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((s - expected).abs() < 1e-11, "({i},{j}) = {s}");
            }
        }
    }

    #[test]
    fn cholesky_rejects_non_pd() {
        let mut a = DenseMatrix::from_row_major(2, 2, vec![1.0, 2.0, 2.0, 1.0]).unwrap();
        let err = cholesky(&mut a).unwrap_err();
        assert_eq!(err, NumericError::DivisionByZero);
    }
}
