#![allow(clippy::needless_range_loop)]
//! Published nonlinear model library.
//!
//! Each model implements [`NonlinearModel`], which carries:
//! - a stable `id` string for dispatch and persistence;
//! - the parameter names in the published literature's own notation;
//! - the model equation as `predict(params, x)`;
//! - a data-driven [`NonlinearModel::initial_estimates`] so the
//!   caller does not need to guess starting values. These
//!   heuristics are from the published literature on each model
//!   (e.g. half-max for Michaelis–Menten, Scatchard-style for
//!   binding) and are documented inline.
//!
//! Behaviour reference (list of models and their published
//! equations only): Motulsky, H. & Christopoulos, A. (2004)
//! *Fitting Models to Biological Data Using Linear and Nonlinear
//! Regression*, Oxford University Press. (Textbook; not from the
//! Prism manual.) The implementations are our own; no GraphPad
//! material.

use graphcraft_numeric::NumericError;

use crate::lm::{levenberg_marquardt, LmFit, LmOptions};

/// A named nonlinear model with a derivation of starting values
/// from the input data.
pub trait NonlinearModel {
    fn id(&self) -> &'static str;
    fn param_names(&self) -> &'static [&'static str];
    fn n_params(&self) -> usize {
        self.param_names().len()
    }
    fn predict(&self, params: &[f64], x: f64) -> f64;
    /// Pick a reasonable starting point from the data. Each model
    /// documents its own heuristic.
    fn initial_estimates(&self, xs: &[f64], ys: &[f64]) -> Vec<f64>;
}

/// Fit a published model to `(xs, ys)` by Levenberg–Marquardt from
/// the model's own data-driven initial estimates.
pub fn fit_model<M: NonlinearModel>(
    model: &M,
    xs: &[f64],
    ys: &[f64],
    opts: LmOptions,
) -> Result<LmFit, NumericError> {
    let initial = model.initial_estimates(xs, ys);
    if initial.len() != model.n_params() {
        return Err(NumericError::TooFewValues {
            required: model.n_params(),
            got: initial.len(),
        });
    }
    let f = |b: &[f64], x: f64| model.predict(b, x);
    levenberg_marquardt(f, xs, ys, &initial, opts)
}

// ---------------------------------------------------------------------
// One-site binding (Michaelis–Menten form)
//   y = Bmax · x / (Kd + x)
// ---------------------------------------------------------------------

/// One-site specific binding: `y = Bmax · x / (Kd + x)`.
pub struct OneSiteBinding;
impl NonlinearModel for OneSiteBinding {
    fn id(&self) -> &'static str {
        "one-site-binding"
    }
    fn param_names(&self) -> &'static [&'static str] {
        &["Bmax", "Kd"]
    }
    fn predict(&self, b: &[f64], x: f64) -> f64 {
        b[0] * x / (b[1] + x)
    }
    fn initial_estimates(&self, xs: &[f64], ys: &[f64]) -> Vec<f64> {
        // Bmax: slightly above max(y) so the saturation level is
        // not pinned at the data ceiling. Kd: the x that produces
        // the response closest to half the (observed) ceiling.
        let ymax = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let bmax = if ymax.is_finite() { ymax * 1.1 } else { 1.0 };
        let half = 0.5 * ymax;
        let mut kd = xs.iter().cloned().fold(0.0_f64, f64::max) * 0.5;
        let mut best = f64::INFINITY;
        for (&x, &y) in xs.iter().zip(ys.iter()) {
            let d = (y - half).abs();
            if d < best {
                best = d;
                kd = x;
            }
        }
        if kd <= 0.0 {
            kd = 1.0;
        }
        vec![bmax, kd]
    }
}

// ---------------------------------------------------------------------
// Hill saturation (one-site with Hill slope)
//   y = Bmax · x^h / (Kd^h + x^h)
// ---------------------------------------------------------------------

/// One-site binding with a Hill slope:
/// `y = Bmax · x^h / (Kd^h + x^h)`.
pub struct HillBinding;
impl NonlinearModel for HillBinding {
    fn id(&self) -> &'static str {
        "hill-binding"
    }
    fn param_names(&self) -> &'static [&'static str] {
        &["Bmax", "Kd", "h"]
    }
    fn predict(&self, b: &[f64], x: f64) -> f64 {
        let xh = x.powf(b[2]);
        let kh = b[1].powf(b[2]);
        b[0] * xh / (kh + xh)
    }
    fn initial_estimates(&self, xs: &[f64], ys: &[f64]) -> Vec<f64> {
        let mut mm = OneSiteBinding.initial_estimates(xs, ys);
        mm.push(1.0); // h ≈ 1 is the published default start.
        mm
    }
}

// ---------------------------------------------------------------------
// One-phase exponential decay
//   y = (Y0 − Plateau) · exp(−k · x) + Plateau
// ---------------------------------------------------------------------

/// One-phase exponential decay:
/// `y = (Y0 − Plateau) · exp(−k · x) + Plateau`.
pub struct ExpDecayOnePhase;
impl NonlinearModel for ExpDecayOnePhase {
    fn id(&self) -> &'static str {
        "exp-decay-one-phase"
    }
    fn param_names(&self) -> &'static [&'static str] {
        &["Y0", "Plateau", "k"]
    }
    fn predict(&self, b: &[f64], x: f64) -> f64 {
        (b[0] - b[1]) * (-b[2] * x).exp() + b[1]
    }
    fn initial_estimates(&self, xs: &[f64], ys: &[f64]) -> Vec<f64> {
        // Y0 ≈ first y, Plateau ≈ last y, k ≈ ln(2) / x_half where
        // x_half is the x at which y reaches halfway between Y0
        // and Plateau — the textbook starting rule.
        let y0 = ys.first().copied().unwrap_or(0.0);
        let plateau = ys.last().copied().unwrap_or(y0);
        let halfway = 0.5 * (y0 + plateau);
        let mut best = f64::INFINITY;
        let mut x_half = 1.0_f64;
        for (&x, &y) in xs.iter().zip(ys.iter()) {
            let d = (y - halfway).abs();
            if d < best {
                best = d;
                x_half = x;
            }
        }
        let k = if x_half > 0.0 {
            std::f64::consts::LN_2 / x_half
        } else {
            1.0
        };
        vec![y0, plateau, k]
    }
}

// ---------------------------------------------------------------------
// Exponential growth
//   y = Y0 · exp(k · x)
// ---------------------------------------------------------------------

/// Exponential growth: `y = Y0 · exp(k · x)`.
pub struct ExpGrowth;
impl NonlinearModel for ExpGrowth {
    fn id(&self) -> &'static str {
        "exp-growth"
    }
    fn param_names(&self) -> &'static [&'static str] {
        &["Y0", "k"]
    }
    fn predict(&self, b: &[f64], x: f64) -> f64 {
        b[0] * (b[1] * x).exp()
    }
    fn initial_estimates(&self, xs: &[f64], ys: &[f64]) -> Vec<f64> {
        let y0 = ys.first().copied().unwrap_or(1.0).max(1e-12);
        // Simple log-linear slope between the first and last point.
        if let (Some(&x1), Some(&y1), Some(&x2), Some(&y2)) =
            (xs.first(), ys.first(), xs.last(), ys.last())
        {
            if y1 > 0.0 && y2 > 0.0 && x2 != x1 {
                let k = (y2.ln() - y1.ln()) / (x2 - x1);
                return vec![y0, k];
            }
        }
        vec![y0, 0.1]
    }
}

// ---------------------------------------------------------------------
// Log-dose-response (variable Hill slope)
//   y = Bottom + (Top − Bottom) / (1 + 10^((logEC50 − x) · HillSlope))
// ---------------------------------------------------------------------

/// Log-dose-response (variable Hill slope):
/// `y = Bottom + (Top − Bottom) / (1 + 10^((logEC50 − x) · HillSlope))`.
pub struct LogDoseResponse;
impl NonlinearModel for LogDoseResponse {
    fn id(&self) -> &'static str {
        "log-dose-response"
    }
    fn param_names(&self) -> &'static [&'static str] {
        &["Bottom", "Top", "logEC50", "HillSlope"]
    }
    fn predict(&self, b: &[f64], x: f64) -> f64 {
        let bottom = b[0];
        let top = b[1];
        let log_ec50 = b[2];
        let hill = b[3];
        bottom + (top - bottom) / (1.0 + 10f64.powf((log_ec50 - x) * hill))
    }
    fn initial_estimates(&self, xs: &[f64], ys: &[f64]) -> Vec<f64> {
        let ymin = ys.iter().cloned().fold(f64::INFINITY, f64::min);
        let ymax = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let bottom = if ymin.is_finite() { ymin } else { 0.0 };
        let top = if ymax.is_finite() { ymax } else { bottom + 1.0 };
        let half = 0.5 * (bottom + top);
        let mut log_ec50 = xs.iter().cloned().sum::<f64>() / xs.len().max(1) as f64;
        let mut best = f64::INFINITY;
        for (&x, &y) in xs.iter().zip(ys.iter()) {
            let d = (y - half).abs();
            if d < best {
                best = d;
                log_ec50 = x;
            }
        }
        vec![bottom, top, log_ec50, 1.0]
    }
}

// ---------------------------------------------------------------------
// Straight line (also useful for library-API consistency)
//   y = Intercept + Slope · x
// ---------------------------------------------------------------------

/// Straight line: `y = Intercept + Slope · x`.
pub struct Line;
impl NonlinearModel for Line {
    fn id(&self) -> &'static str {
        "line"
    }
    fn param_names(&self) -> &'static [&'static str] {
        &["Intercept", "Slope"]
    }
    fn predict(&self, b: &[f64], x: f64) -> f64 {
        b[0] + b[1] * x
    }
    fn initial_estimates(&self, xs: &[f64], ys: &[f64]) -> Vec<f64> {
        if let (Some(&x1), Some(&y1), Some(&x2), Some(&y2)) =
            (xs.first(), ys.first(), xs.last(), ys.last())
        {
            if x1 != x2 {
                let slope = (y2 - y1) / (x2 - x1);
                let intercept = y1 - slope * x1;
                return vec![intercept, slope];
            }
        }
        vec![0.0, 0.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_exact<M: NonlinearModel>(m: &M, b: &[f64], xs: &[f64]) -> Vec<f64> {
        xs.iter().map(|&x| m.predict(b, x)).collect()
    }

    #[test]
    fn one_site_binding_round_trip() {
        let model = OneSiteBinding;
        let b_true = [100.0_f64, 10.0];
        let xs: Vec<f64> = (1..=12).map(|k| k as f64 * 5.0).collect();
        let ys = make_exact(&model, &b_true, &xs);
        let fit = fit_model(&model, &xs, &ys, LmOptions::default()).unwrap();
        assert!(fit.converged);
        assert!((fit.params[0] - 100.0).abs() < 1e-6);
        assert!((fit.params[1] - 10.0).abs() < 1e-6);
    }

    #[test]
    fn exp_decay_one_phase_round_trip() {
        let model = ExpDecayOnePhase;
        let b_true = [100.0_f64, 10.0, 0.3];
        let xs: Vec<f64> = (0..15).map(|k| k as f64 * 0.5).collect();
        let ys = make_exact(&model, &b_true, &xs);
        let fit = fit_model(&model, &xs, &ys, LmOptions::default()).unwrap();
        assert!(fit.converged, "did not converge: {fit:?}");
        assert!((fit.params[0] - 100.0).abs() < 1e-5);
        assert!((fit.params[1] - 10.0).abs() < 1e-5);
        assert!((fit.params[2] - 0.3).abs() < 1e-7);
    }

    #[test]
    fn exp_growth_round_trip() {
        let model = ExpGrowth;
        let b_true = [2.0_f64, 0.4];
        let xs: Vec<f64> = (0..10).map(|k| k as f64).collect();
        let ys = make_exact(&model, &b_true, &xs);
        let fit = fit_model(&model, &xs, &ys, LmOptions::default()).unwrap();
        assert!(fit.converged);
        assert!((fit.params[0] - 2.0).abs() < 1e-9);
        assert!((fit.params[1] - 0.4).abs() < 1e-9);
    }

    #[test]
    fn log_dose_response_round_trip() {
        let model = LogDoseResponse;
        // Bottom=0, Top=100, logEC50=-6, HillSlope=1 — classic.
        let b_true = [0.0_f64, 100.0, -6.0, 1.0];
        let xs: Vec<f64> = (-9..=-3).map(|k| k as f64).collect();
        let ys = make_exact(&model, &b_true, &xs);
        let fit = fit_model(&model, &xs, &ys, LmOptions::default()).unwrap();
        assert!(fit.converged, "did not converge: {fit:?}");
        assert!((fit.params[0] - 0.0).abs() < 1e-4);
        assert!((fit.params[1] - 100.0).abs() < 1e-4);
        assert!((fit.params[2] + 6.0).abs() < 1e-6);
        assert!((fit.params[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hill_binding_round_trip() {
        let model = HillBinding;
        let b_true = [100.0_f64, 10.0, 2.0];
        let xs: Vec<f64> = (1..=15).map(|k| k as f64 * 2.0).collect();
        let ys = make_exact(&model, &b_true, &xs);
        let fit = fit_model(&model, &xs, &ys, LmOptions::default()).unwrap();
        assert!(fit.converged);
        assert!((fit.params[0] - 100.0).abs() < 1e-5);
        assert!((fit.params[1] - 10.0).abs() < 1e-5);
        assert!((fit.params[2] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn line_round_trip() {
        let model = Line;
        let xs = [1.0_f64, 2.0, 3.0, 4.0, 5.0];
        let ys = [3.0_f64, 5.0, 7.0, 9.0, 11.0];
        let fit = fit_model(&model, &xs, &ys, LmOptions::default()).unwrap();
        assert!(fit.converged);
        assert!((fit.params[0] - 1.0).abs() < 1e-12);
        assert!((fit.params[1] - 2.0).abs() < 1e-12);
    }
}
