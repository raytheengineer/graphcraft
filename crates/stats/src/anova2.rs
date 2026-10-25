#![allow(clippy::needless_range_loop)]
//! Two-way (fixed-effects) ANOVA for a balanced design.
//!
//! Model: `y_ijk = μ + α_i + β_j + (αβ)_ij + ε_ijk` with `I` levels
//! of factor A, `J` levels of factor B, and `n` replicates per
//! cell (same `n` across all cells — "balanced").
//!
//! The sums of squares:
//! - `SS_A = n·J·Σ (ȳ_i.. − ȳ)²`
//! - `SS_B = n·I·Σ (ȳ_.j. − ȳ)²`
//! - `SS_AB = n·Σ (ȳ_ij. − ȳ_i.. − ȳ_.j. + ȳ)²`
//! - `SS_within = Σ (y_ijk − ȳ_ij.)²`
//!
//! Each with F statistic `MS_effect / MS_within` on its own df.
//!
//! Unbalanced designs — where cells have different `n` — need
//! Type I / II / III SS; those are scheduled follow-ups and are
//! tracked in `ROADMAP.md`.
//!
//! Reference (public): Scheffé, H. (1959) *The Analysis of
//! Variance*, Wiley, Ch. 4; and any modern biostatistics text.

use graphcraft_numeric::NumericError;

use crate::distributions::f::f_sf;

/// One effect's row in a two-way ANOVA table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnovaEffect {
    pub ss: f64,
    pub df: f64,
    pub ms: f64,
    pub f: f64,
    pub p: f64,
}

/// Result of a balanced two-way ANOVA.
#[derive(Debug, Clone, PartialEq)]
pub struct TwoWayAnova {
    pub n_rows: usize,
    pub n_cols: usize,
    pub n_per_cell: usize,
    pub grand_mean: f64,
    pub row_a: AnovaEffect,
    pub col_b: AnovaEffect,
    pub interaction: AnovaEffect,
    pub within: AnovaEffect,
    pub ss_total: f64,
    pub df_total: f64,
}

/// Fit a balanced two-way ANOVA to `cells[i][j][k]` — `n_per_cell`
/// replicates per `(A, B)` combination. All cells must contain the
/// same number of replicates and `≥ 2 × I × J` total observations.
pub fn two_way_anova(cells: &[Vec<Vec<f64>>]) -> Result<TwoWayAnova, NumericError> {
    let n_rows = cells.len();
    if n_rows < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: n_rows,
        });
    }
    let n_cols = cells[0].len();
    if n_cols < 2 {
        return Err(NumericError::TooFewValues {
            required: 2,
            got: n_cols,
        });
    }
    let n_per_cell = cells[0][0].len();
    if n_per_cell == 0 {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    for row in cells {
        if row.len() != n_cols {
            return Err(NumericError::TooFewValues {
                required: n_cols,
                got: row.len(),
            });
        }
        for cell in row {
            if cell.len() != n_per_cell {
                return Err(NumericError::TooFewValues {
                    required: n_per_cell,
                    got: cell.len(),
                });
            }
            for &v in cell {
                if !v.is_finite() {
                    return Err(NumericError::NotFinite);
                }
            }
        }
    }

    // Cell means.
    let mut cell_mean = vec![vec![0.0_f64; n_cols]; n_rows];
    for i in 0..n_rows {
        for j in 0..n_cols {
            let mut s = 0.0_f64;
            for v in &cells[i][j] {
                s += v;
            }
            cell_mean[i][j] = s / n_per_cell as f64;
        }
    }
    // Row / column / grand means.
    let mut row_mean = vec![0.0_f64; n_rows];
    for i in 0..n_rows {
        let mut s = 0.0_f64;
        for j in 0..n_cols {
            s += cell_mean[i][j];
        }
        row_mean[i] = s / n_cols as f64;
    }
    let mut col_mean = vec![0.0_f64; n_cols];
    for j in 0..n_cols {
        let mut s = 0.0_f64;
        for i in 0..n_rows {
            s += cell_mean[i][j];
        }
        col_mean[j] = s / n_rows as f64;
    }
    let grand_mean = row_mean.iter().sum::<f64>() / n_rows as f64;

    let n_f = n_per_cell as f64;
    let i_f = n_rows as f64;
    let j_f = n_cols as f64;

    // SS_A: variability between row (factor A) means.
    let mut ss_a = 0.0_f64;
    for i in 0..n_rows {
        let d = row_mean[i] - grand_mean;
        ss_a += d * d;
    }
    ss_a *= n_f * j_f;
    // SS_B.
    let mut ss_b = 0.0_f64;
    for j in 0..n_cols {
        let d = col_mean[j] - grand_mean;
        ss_b += d * d;
    }
    ss_b *= n_f * i_f;
    // SS_AB.
    let mut ss_ab = 0.0_f64;
    for i in 0..n_rows {
        for j in 0..n_cols {
            let d = cell_mean[i][j] - row_mean[i] - col_mean[j] + grand_mean;
            ss_ab += d * d;
        }
    }
    ss_ab *= n_f;
    // SS_within.
    let mut ss_within = 0.0_f64;
    for i in 0..n_rows {
        for j in 0..n_cols {
            for &v in &cells[i][j] {
                let d = v - cell_mean[i][j];
                ss_within += d * d;
            }
        }
    }

    let df_a = i_f - 1.0;
    let df_b = j_f - 1.0;
    let df_ab = (i_f - 1.0) * (j_f - 1.0);
    let df_within = i_f * j_f * (n_f - 1.0);
    if df_within <= 0.0 {
        return Err(NumericError::TooFewValues {
            required: 2 * n_rows * n_cols,
            got: n_rows * n_cols * n_per_cell,
        });
    }

    let ms_within = ss_within / df_within;
    if !ms_within.is_finite() || ms_within <= 0.0 {
        return Err(NumericError::DivisionByZero);
    }
    let make_effect = |ss: f64, df: f64| -> Result<AnovaEffect, NumericError> {
        let ms = ss / df;
        let f = ms / ms_within;
        let p = if f > 0.0 {
            f_sf(f, df, df_within)?
        } else {
            1.0
        };
        Ok(AnovaEffect { ss, df, ms, f, p })
    };

    let ss_total = ss_a + ss_b + ss_ab + ss_within;
    let df_total = i_f * j_f * n_f - 1.0;

    Ok(TwoWayAnova {
        n_rows,
        n_cols,
        n_per_cell,
        grand_mean,
        row_a: make_effect(ss_a, df_a)?,
        col_b: make_effect(ss_b, df_b)?,
        interaction: make_effect(ss_ab, df_ab)?,
        within: AnovaEffect {
            ss: ss_within,
            df: df_within,
            ms: ms_within,
            f: f64::NAN,
            p: f64::NAN,
        },
        ss_total,
        df_total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_by_two_hand_computed() {
        // 2×2 factorial, n=2 replicates per cell.
        //   Cell(0,0): [1, 3] → mean 2
        //   Cell(0,1): [5, 7] → mean 6
        //   Cell(1,0): [2, 4] → mean 3
        //   Cell(1,1): [10,12] → mean 11
        // Row means: 4, 7; Col means: 2.5, 8.5; Grand mean: 5.5.
        // SS_A = n·J·((4-5.5)² + (7-5.5)²) = 4·4.5 = 18.
        // SS_B = n·I·((2.5-5.5)² + (8.5-5.5)²) = 4·18 = 72.
        // SS_AB = n·Σ (cell − row − col + grand)² = 2·4 = 8.
        // SS_within = each cell var = 2 → total 8.
        let cells = vec![
            vec![vec![1.0_f64, 3.0], vec![5.0, 7.0]],
            vec![vec![2.0, 4.0], vec![10.0, 12.0]],
        ];
        let a = two_way_anova(&cells).unwrap();
        assert!((a.row_a.ss - 18.0).abs() < 1e-13);
        assert!((a.col_b.ss - 72.0).abs() < 1e-13);
        assert!((a.interaction.ss - 8.0).abs() < 1e-13);
        assert!((a.within.ss - 8.0).abs() < 1e-13);
        assert_eq!(a.row_a.df, 1.0);
        assert_eq!(a.col_b.df, 1.0);
        assert_eq!(a.interaction.df, 1.0);
        assert_eq!(a.within.df, 4.0);
        assert!((a.row_a.f - 9.0).abs() < 1e-13);
        assert!((a.col_b.f - 36.0).abs() < 1e-13);
        assert!((a.interaction.f - 4.0).abs() < 1e-13);
    }

    #[test]
    fn rejects_jagged_cells() {
        let cells = vec![
            vec![vec![1.0_f64, 2.0], vec![3.0]],
            vec![vec![4.0, 5.0], vec![6.0, 7.0]],
        ];
        assert!(two_way_anova(&cells).is_err());
    }

    #[test]
    fn rejects_single_replicate() {
        let cells = vec![vec![vec![1.0_f64], vec![2.0]], vec![vec![3.0], vec![4.0]]];
        // n_per_cell=1 → df_within = 0.
        assert!(two_way_anova(&cells).is_err());
    }
}
