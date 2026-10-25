//! L3 regression: linear OLS, Levenberg-Marquardt nonlinear fit,
//! published model library.
//!
//! M4.1 ships linear OLS with full Prism-style output (coefficients,
//! SEs, t-stats, p-values, R², adjusted R², residual SE, overall-F).
//! M4.2 will add Levenberg-Marquardt; M4.3 the model library.

pub mod matrix;
pub mod ols;

pub use matrix::{
    back_solve_transposed, cholesky, forward_solve, invert_spd, solve_spd, DenseMatrix,
};
pub use ols::{ols_fit, simple_linear_regression, OlsFit};
