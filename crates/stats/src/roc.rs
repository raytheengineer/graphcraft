#![allow(clippy::needless_range_loop)]
//! Receiver Operating Characteristic analysis.
//!
//! Given per-subject (score, label) pairs with binary labels, build
//! the ROC curve and compute the AUC with its Hanley–McNeil standard
//! error. Higher scores are treated as more positive — negate your
//! predictor if the convention runs the other way.
//!
//! The AUC equals the Mann–Whitney U statistic normalised:
//!   `AUC = (U + 0.5·ties) / (n_pos · n_neg)`.
//! `U` here counts how often a positive outranks a negative,
//! with ties contributing 0.5 (Mason & Graham 2002).
//!
//! Behaviour reference (public):
//! - Hanley, J. A. & McNeil, B. J. (1982) "The meaning and use of
//!   the area under a receiver operating characteristic (ROC)
//!   curve", Radiology 143, 29–36.
//! - Mason, S. J. & Graham, N. E. (2002) "Areas beneath the
//!   relative operating characteristics (ROC) and relative
//!   operating levels (ROL) curves", QJRMS 128, 2145–2166.

use graphcraft_numeric::NumericError;

use crate::distributions::normal::big_phi;

/// Result of an ROC analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct RocAnalysis {
    pub n_pos: usize,
    pub n_neg: usize,
    pub auc: f64,
    /// Hanley–McNeil standard error of the AUC.
    pub se_auc: f64,
    /// `AUC ± 1.96 · SE(AUC)` on the raw AUC scale.
    pub ci95: (f64, f64),
    /// `(fpr, tpr)` points on the ROC curve, ordered by threshold
    /// from most restrictive to most permissive. The first point
    /// is `(0, 0)` and the last `(1, 1)`.
    pub curve: Vec<(f64, f64)>,
    /// Two-sided p-value under `H₀: AUC = 0.5`, from the Hanley
    /// z-approximation.
    pub p_two_sided: f64,
}

/// Build an ROC curve from `(score, label)` pairs. Labels are
/// `true` for positive, `false` for negative.
pub fn roc(scored: &[(f64, bool)]) -> Result<RocAnalysis, NumericError> {
    for &(s, _) in scored {
        if !s.is_finite() {
            return Err(NumericError::NotFinite);
        }
    }
    let n_pos = scored.iter().filter(|(_, y)| *y).count();
    let n_neg = scored.len() - n_pos;
    if n_pos == 0 || n_neg == 0 {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: n_pos.min(n_neg),
        });
    }

    // AUC via tie-aware Mann–Whitney U count.
    let mut u = 0.0_f64;
    for &(xs, xl) in scored {
        if !xl {
            continue;
        }
        for &(ys, yl) in scored {
            if yl {
                continue;
            }
            if xs > ys {
                u += 1.0;
            } else if xs == ys {
                u += 0.5;
            }
        }
    }
    let auc = u / (n_pos as f64 * n_neg as f64);

    // Hanley–McNeil SE.
    let q1 = auc / (2.0 - auc);
    let q2 = 2.0 * auc * auc / (1.0 + auc);
    let num = auc * (1.0 - auc)
        + (n_pos as f64 - 1.0) * (q1 - auc * auc)
        + (n_neg as f64 - 1.0) * (q2 - auc * auc);
    let se_auc = (num / (n_pos as f64 * n_neg as f64)).sqrt();

    let ci95 = (
        (auc - 1.959_963_984_540_054_f64 * se_auc).max(0.0),
        (auc + 1.959_963_984_540_054_f64 * se_auc).min(1.0),
    );

    // Two-sided p against AUC = 0.5 under the Hanley SE at 0.5
    // (approximate; the SE at the null, not at AUC_obs, is
    // conventional for the Mann–Whitney equivalence).
    let se0 = ((n_pos as f64 + n_neg as f64 + 1.0) / (12.0 * n_pos as f64 * n_neg as f64)).sqrt();
    let z = (auc - 0.5) / se0;
    let p_two_sided = (2.0 * (1.0 - big_phi(z.abs()))).clamp(0.0, 1.0);

    // Build the step curve by sorting on score descending and
    // walking: each positive at threshold moves TPR up by 1/n_pos,
    // each negative moves FPR right by 1/n_neg. Ties collapse into
    // a single "step" that moves diagonally.
    let mut sorted: Vec<&(f64, bool)> = scored.iter().collect();
    sorted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut curve = vec![(0.0_f64, 0.0_f64)];
    let mut tp = 0.0_f64;
    let mut fp = 0.0_f64;
    let mut i = 0;
    while i < sorted.len() {
        let s = sorted[i].0;
        let mut pos = 0.0_f64;
        let mut neg = 0.0_f64;
        let mut j = i;
        while j < sorted.len() && sorted[j].0 == s {
            if sorted[j].1 {
                pos += 1.0;
            } else {
                neg += 1.0;
            }
            j += 1;
        }
        tp += pos;
        fp += neg;
        curve.push((fp / n_neg as f64, tp / n_pos as f64));
        i = j;
    }
    // Guarantee the final point lands exactly at (1, 1).
    if let Some(last) = curve.last_mut() {
        *last = (1.0, 1.0);
    }

    Ok(RocAnalysis {
        n_pos,
        n_neg,
        auc,
        se_auc,
        ci95,
        curve,
        p_two_sided,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_separation_auc_one() {
        let data = vec![
            (0.1, false),
            (0.2, false),
            (0.3, false),
            (0.7, true),
            (0.8, true),
            (0.9, true),
        ];
        let r = roc(&data).unwrap();
        assert!((r.auc - 1.0).abs() < 1e-14);
        assert!(r.p_two_sided < 0.05);
    }

    #[test]
    fn complete_inversion_auc_zero() {
        let data = vec![(0.9, false), (0.8, false), (0.1, true), (0.2, true)];
        let r = roc(&data).unwrap();
        assert!((r.auc - 0.0).abs() < 1e-14);
    }

    #[test]
    fn ties_contribute_half() {
        // All pairs tie: AUC = 0.5.
        let data = vec![(1.0, true), (1.0, false), (1.0, true), (1.0, false)];
        let r = roc(&data).unwrap();
        assert!((r.auc - 0.5).abs() < 1e-14);
    }

    #[test]
    fn single_group_rejected() {
        assert!(roc(&[(1.0, true), (2.0, true)]).is_err());
    }

    #[test]
    fn roc_hand_counted_small_case() {
        // 2 positives, 3 negatives. Scores:
        //   pos: 0.8, 0.6
        //   neg: 0.9, 0.7, 0.5
        // Pairs (pos, neg):
        //   (0.8, 0.9) → neg wins
        //   (0.8, 0.7) → pos wins
        //   (0.8, 0.5) → pos wins
        //   (0.6, 0.9) → neg wins
        //   (0.6, 0.7) → neg wins
        //   (0.6, 0.5) → pos wins
        // U = 3; AUC = 3 / (2·3) = 0.5.
        let data = vec![
            (0.8, true),
            (0.6, true),
            (0.9, false),
            (0.7, false),
            (0.5, false),
        ];
        let r = roc(&data).unwrap();
        assert!((r.auc - 0.5).abs() < 1e-14);
    }
}
