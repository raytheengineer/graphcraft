//! Multiple-comparison p-value adjustments.
//!
//! M3.3 ships the two closed-form family-wise error adjustments. The
//! studentized-range (Tukey HSD) and Dunnett's multivariate-t
//! adjustments ride on new distributions and are tracked as separate
//! follow-ups in `ROADMAP.md`.
//!
//! Reference (behaviour):
//! - Bonferroni, C. E. (1936). "Teoria statistica delle classi e
//!   calcolo delle probabilità." Pubblicazioni del R. Istituto
//!   Superiore di Scienze Economiche e Commerciali di Firenze 8:3–62.
//! - Šidák, Z. (1967). "Rectangular confidence regions for the means
//!   of multivariate normal distributions." JASA 62(318): 626–633.

use graphcraft_numeric::NumericError;

/// Bonferroni-adjusted p-value for a single comparison in a family of
/// `m` tests. Clamped to `[0, 1]`.
pub fn bonferroni(p: f64, m: usize) -> Result<f64, NumericError> {
    if !p.is_finite() || !(0.0..=1.0).contains(&p) || m == 0 {
        return Err(NumericError::NotFinite);
    }
    Ok((p * m as f64).min(1.0))
}

/// Šidák-adjusted p-value: `1 − (1 − p)^m`.
pub fn sidak(p: f64, m: usize) -> Result<f64, NumericError> {
    if !p.is_finite() || !(0.0..=1.0).contains(&p) || m == 0 {
        return Err(NumericError::NotFinite);
    }
    // Guard against pow underflow when p is near 1.
    let adj = 1.0 - (1.0 - p).powi(m as i32);
    Ok(adj.clamp(0.0, 1.0))
}

/// Apply Bonferroni across a slice of raw p-values. The number of
/// tests `m` is the slice length.
pub fn bonferroni_family(ps: &[f64]) -> Result<Vec<f64>, NumericError> {
    let m = ps.len();
    ps.iter().map(|&p| bonferroni(p, m)).collect()
}

/// Apply Šidák across a slice of raw p-values. The number of tests
/// `m` is the slice length.
pub fn sidak_family(ps: &[f64]) -> Result<Vec<f64>, NumericError> {
    let m = ps.len();
    ps.iter().map(|&p| sidak(p, m)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bonferroni_scales_and_clamps() {
        assert!((bonferroni(0.01, 5).unwrap() - 0.05).abs() < 1e-15);
        assert_eq!(bonferroni(0.3, 10).unwrap(), 1.0); // clamped
        assert_eq!(bonferroni(0.0, 10).unwrap(), 0.0);
        assert_eq!(bonferroni(1.0, 10).unwrap(), 1.0);
    }

    #[test]
    fn sidak_closed_form() {
        // m = 2: 1 - (1 - p)² = 2p - p².
        let p = 0.05;
        let expected = 2.0 * p - p * p;
        assert!((sidak(p, 2).unwrap() - expected).abs() < 1e-15);
    }

    #[test]
    fn sidak_dominates_bonferroni() {
        // Šidák is uniformly ≤ Bonferroni (both ≥ p for m ≥ 1).
        for &p in &[0.001_f64, 0.01, 0.1, 0.5] {
            for m in 1..=10 {
                let s = sidak(p, m).unwrap();
                let b = bonferroni(p, m).unwrap();
                assert!(s <= b + 1e-15, "p={p} m={m} s={s} b={b}");
                assert!(s >= p - 1e-15);
            }
        }
    }

    #[test]
    fn family_wrappers() {
        let ps = [0.01_f64, 0.02, 0.03];
        let b = bonferroni_family(&ps).unwrap();
        assert_eq!(b.len(), 3);
        assert!((b[0] - 0.03).abs() < 1e-15);
        let s = sidak_family(&ps).unwrap();
        assert!(s[0] < b[0]);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(bonferroni(-0.1, 3).is_err());
        assert!(bonferroni(0.5, 0).is_err());
        assert!(sidak(f64::NAN, 3).is_err());
    }
}
