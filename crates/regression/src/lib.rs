//! L3 regression: linear OLS, Levenberg-Marquardt nonlinear fit,
//! published model library.
//!
//! M4.1 ships linear OLS; M4.2 adds Levenberg-Marquardt nonlinear
//! fitting via central-difference Jacobian and Marquardt's
//! diag(JᵀJ) damping. M4.3 will add the published model library.

pub mod lm;
pub mod matrix;
pub mod models;
pub mod ols;

pub use lm::{levenberg_marquardt, LmFit, LmOptions};
pub use matrix::{
    back_solve_transposed, cholesky, forward_solve, invert_spd, solve_spd, DenseMatrix,
};
pub use models::{
    fit_model, ExpDecayOnePhase, ExpGrowth, HillBinding, Line, LogDoseResponse, NonlinearModel,
    OneSiteBinding,
};
pub use ols::{ols_fit, simple_linear_regression, OlsFit};
