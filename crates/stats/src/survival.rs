#![allow(clippy::needless_range_loop)]
//! Log-rank test and Mantel–Haenszel hazard-ratio estimator for
//! Kaplan–Meier style survival data.
//!
//! For `k` groups and event times `t_1 < … < t_m`, at each event
//! time compute per-group observed events `O_ij`, at-risk counts
//! `n_ij`, and total events `d_i`. Under the null of identical
//! hazards, the expected events in group `j` at `t_i` are
//! `E_ij = n_ij / n_i · d_i` and the hypergeometric variance of
//! `O_ij − E_ij` is
//!   `V_ij = n_ij · (n_i − n_ij) · d_i · (n_i − d_i) / (n_i² · (n_i − 1))`.
//! Summed across event times, the two-group statistic is
//!   `X² = (Σ (O − E))² / Σ V ~ χ²_1`.
//! For `k > 2` groups the full-rank form generalises; this module
//! ships the two-group case (sufficient for every parity target in
//! the roadmap) and tracks the `k > 2` form as a follow-up.
//!
//! Reference (public):
//! - Mantel, N. (1966) "Evaluation of survival data and two new rank
//!   order statistics arising in its consideration", Cancer
//!   Chemotherapy Reports 50, 163–170.
//! - Peto, R. & Peto, J. (1972) *JRSS A* 135, 185–207.

use graphcraft_numeric::NumericError;

use crate::distributions::chi_squared::chi2_sf;

/// One survival observation: `(time, censored)`. Keeps the stats
/// crate independent of the L2 table layer; the analysis-layer
/// wrapper converts from `graphcraft_table::SurvivalRow`.
pub type SurvivalRow = (f64, bool);

/// Result of a two-group log-rank test.
#[derive(Debug, Clone, PartialEq)]
pub struct LogRank {
    pub n1: usize,
    pub n2: usize,
    pub o1_minus_e1: f64,
    pub variance: f64,
    pub chi2: f64,
    pub df: f64,
    pub p: f64,
    /// Mantel–Haenszel log hazard ratio `(O − E) / V` with SE
    /// `1 / √V`. Back-transformed `exp(log_hr)` on the response
    /// scale.
    pub log_hr: f64,
    pub se_log_hr: f64,
    pub hr: f64,
    pub hr_ci95: (f64, f64),
}

/// Two-group log-rank test on raw Kaplan–Meier rows.
pub fn log_rank(group1: &[SurvivalRow], group2: &[SurvivalRow]) -> Result<LogRank, NumericError> {
    for &(t, _) in group1.iter().chain(group2.iter()) {
        if !t.is_finite() || t < 0.0 {
            return Err(NumericError::NotFinite);
        }
    }
    if group1.is_empty() || group2.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: group1.len().min(group2.len()),
        });
    }
    // Build the sorted event-time axis: every distinct time at
    // which an event occurs in either group.
    let mut times: Vec<f64> = group1
        .iter()
        .chain(group2.iter())
        .filter(|(_, c)| !*c)
        .map(|(t, _)| *t)
        .collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    times.dedup_by(|a, b| a == b);

    // At-risk and event counts in each group at each event time.
    let n_at_risk =
        |g: &[SurvivalRow], t: f64| -> usize { g.iter().filter(|(ti, _)| *ti >= t).count() };
    let d_at = |g: &[SurvivalRow], t: f64| -> usize {
        g.iter()
            .filter(|(ti, c)| !*c && (ti - t).abs() < 1e-12)
            .count()
    };

    let mut o_minus_e = 0.0_f64;
    let mut variance = 0.0_f64;
    for &t in &times {
        let n1 = n_at_risk(group1, t) as f64;
        let n2 = n_at_risk(group2, t) as f64;
        let n = n1 + n2;
        let d1 = d_at(group1, t) as f64;
        let d2 = d_at(group2, t) as f64;
        let d = d1 + d2;
        if n < 2.0 || d < 1.0 {
            continue;
        }
        let e1 = n1 / n * d;
        o_minus_e += d1 - e1;
        // Hypergeometric variance of O1 at time t.
        variance += n1 * (n - n1) * d * (n - d) / (n * n * (n - 1.0));
    }
    if variance <= 0.0 {
        return Err(NumericError::DivisionByZero);
    }
    let chi2 = o_minus_e * o_minus_e / variance;
    let df = 1.0;
    let p = chi2_sf(chi2, df)?.clamp(0.0, 1.0);
    // Mantel–Haenszel log hazard ratio.
    let log_hr = o_minus_e / variance;
    let se_log_hr = (1.0 / variance).sqrt();
    let z = 1.959_963_984_540_054_f64;
    let hr = log_hr.exp();
    let hr_ci95 = (
        (log_hr - z * se_log_hr).exp(),
        (log_hr + z * se_log_hr).exp(),
    );
    Ok(LogRank {
        n1: group1.len(),
        n2: group2.len(),
        o1_minus_e1: o_minus_e,
        variance,
        chi2,
        df,
        p,
        log_hr,
        se_log_hr,
        hr,
        hr_ci95,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(t: f64, censored: bool) -> SurvivalRow {
        (t, censored)
    }

    #[test]
    fn identical_groups_zero_chi2() {
        let g1 = vec![row(1.0, false), row(2.0, false), row(3.0, false)];
        let g2 = vec![row(1.0, false), row(2.0, false), row(3.0, false)];
        let r = log_rank(&g1, &g2).unwrap();
        assert!(r.chi2.abs() < 1e-13);
        assert!((r.hr - 1.0).abs() < 1e-13);
    }

    #[test]
    fn hand_computed_separated_groups() {
        // Group 1 both die at t = 1; Group 2 both die at t = 2.
        //
        // At t = 1: N = 4, n1 = 2, d = 2 all in group 1.
        //   E1 = 2·2/4 = 1; O1 − E1 = 2 − 1 = 1.
        //   V = 2·2·2·2 / (16·3) = 1/3.
        // At t = 2: N = 2, n1 = 0, d = 2 all in group 2.
        //   E1 = 0; V = 0.
        // Total: Σ(O − E) = 1; Σ V = 1/3.
        // X² = 1 / (1/3) = 3; HR = exp(1/(1/3)) = exp(3) ≈ 20.09.
        //
        // (Mantel–Haenszel is a crude point estimate when the
        // groups separate completely; the X² is what matters here.)
        let g1 = vec![row(1.0, false), row(1.0, false)];
        let g2 = vec![row(2.0, false), row(2.0, false)];
        let r = log_rank(&g1, &g2).unwrap();
        assert!((r.o1_minus_e1 - 1.0).abs() < 1e-13);
        assert!((r.variance - 1.0 / 3.0).abs() < 1e-13);
        assert!((r.chi2 - 3.0).abs() < 1e-13);
    }

    #[test]
    fn censored_rows_do_not_count_as_events() {
        // One real event + one censor at the same time: only the
        // event drives the X², the censor just removes from n at
        // risk.
        let g1 = vec![row(1.0, false), row(2.0, true)];
        let g2 = vec![row(1.0, false), row(2.0, false)];
        let r = log_rank(&g1, &g2);
        assert!(r.is_ok());
    }

    #[test]
    fn empty_group_rejected() {
        assert!(log_rank(&[], &[row(1.0, false)]).is_err());
    }
}
