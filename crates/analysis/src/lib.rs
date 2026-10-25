//! L4 analysis layer.
//!
//! The parity harness in `tests/parity.rs` consumes
//! `tests/parity-matrix.csv` to drive output-parity acceptance tests.
//!
//! First analysis (M3.0): per-group descriptive statistics on a
//! Column table.

use graphcraft_numeric::NumericError;
use graphcraft_stats::{
    dunnett, one_sample_t, one_way_anova, paired_t, tukey_hsd, unpaired_t, Descriptives,
    DunnettSide, OneSampleT, OneWayAnova, PairedT, PairwiseComparison, TwoSampleKind, TwoSampleT,
};
use graphcraft_table::ColumnTable;

/// Per-group one-sample t-test result on a Column table.
#[derive(Debug, Clone)]
pub struct ColumnOneSampleT {
    pub group_title: String,
    pub test: OneSampleT,
}

/// Run a one-sample t-test on every non-empty group of a Column
/// table against the same null `μ₀`.
pub fn column_one_sample_t(
    table: &ColumnTable,
    mu0: f64,
) -> Result<Vec<ColumnOneSampleT>, NumericError> {
    let mut out = Vec::new();
    for (i, col) in table.columns.iter().enumerate() {
        let xs: Vec<f64> = table.group_values(i).collect();
        if xs.is_empty() {
            continue;
        }
        let test = one_sample_t(&xs, mu0)?;
        out.push(ColumnOneSampleT {
            group_title: col.title.clone(),
            test,
        });
    }
    Ok(out)
}

/// Run an unpaired two-sample t-test on two columns of a Column table
/// by index. The direction is `col_b − col_a`.
pub fn column_unpaired_t(
    table: &ColumnTable,
    col_a: usize,
    col_b: usize,
    kind: TwoSampleKind,
) -> Result<TwoSampleT, NumericError> {
    if col_a >= table.columns.len() || col_b >= table.columns.len() {
        return Err(NumericError::TooFewValues {
            required: col_a.max(col_b) + 1,
            got: table.columns.len(),
        });
    }
    let xs: Vec<f64> = table.group_values(col_a).collect();
    let ys: Vec<f64> = table.group_values(col_b).collect();
    unpaired_t(&xs, &ys, kind)
}

/// Run a paired t-test on two columns of a Column table, pairing by
/// row index. Rows where either column is missing are dropped.
/// Direction is `col_b − col_a`.
pub fn column_paired_t(
    table: &ColumnTable,
    col_a: usize,
    col_b: usize,
) -> Result<PairedT, NumericError> {
    if col_a >= table.columns.len() || col_b >= table.columns.len() {
        return Err(NumericError::TooFewValues {
            required: col_a.max(col_b) + 1,
            got: table.columns.len(),
        });
    }
    let a = &table.columns[col_a].values;
    let b = &table.columns[col_b].values;
    let rows = a.len().min(b.len());
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for i in 0..rows {
        if let (Some(x), Some(y)) = (a[i], b[i]) {
            xs.push(x);
            ys.push(y);
        }
    }
    paired_t(&xs, &ys)
}

/// One-way ANOVA across all non-empty columns of a Column table.
/// Column order is preserved in `group_ns` / `group_means`.
pub fn column_one_way_anova(table: &ColumnTable) -> Result<OneWayAnova, NumericError> {
    let mut groups: Vec<Vec<f64>> = Vec::new();
    for (i, _) in table.columns.iter().enumerate() {
        let g: Vec<f64> = table.group_values(i).collect();
        if !g.is_empty() {
            groups.push(g);
        }
    }
    one_way_anova(&groups)
}

/// Dunnett many-to-one post-hoc on top of a one-way ANOVA of a
/// Column table. `control_col` indexes the control column in the
/// original table order; empty columns are skipped before ANOVA so
/// the control must itself be non-empty.
pub fn column_dunnett(
    table: &ColumnTable,
    control_col: usize,
    side: DunnettSide,
) -> Result<Vec<PairwiseComparison>, NumericError> {
    // Preserve column order; count non-empty columns up to the
    // chosen control so the ANOVA index matches it.
    let mut anova_index_for_col = Vec::new();
    for (i, _) in table.columns.iter().enumerate() {
        let g: Vec<f64> = table.group_values(i).collect();
        if !g.is_empty() {
            anova_index_for_col.push(Some(i));
        } else {
            anova_index_for_col.push(None);
        }
    }
    let mapped_control = anova_index_for_col
        .iter()
        .take(control_col + 1)
        .filter(|m| m.is_some())
        .count()
        .checked_sub(1)
        .ok_or(NumericError::TooFewValues {
            required: 1,
            got: 0,
        })?;
    let anova = column_one_way_anova(table)?;
    dunnett(&anova, mapped_control, side)
}

/// Tukey HSD post-hoc on top of a one-way ANOVA of a Column table.
/// The returned indices (`i`, `j`) refer to the ANOVA group order,
/// which preserves the original column order after dropping empty
/// columns.
pub fn column_tukey_hsd(table: &ColumnTable) -> Result<Vec<PairwiseComparison>, NumericError> {
    let anova = column_one_way_anova(table)?;
    tukey_hsd(&anova)
}

/// Descriptive statistics for one group (one column) of a Column
/// table.
#[derive(Debug, Clone)]
pub struct ColumnDescriptives {
    pub group_title: String,
    pub stats: Descriptives,
}

/// Compute descriptive statistics for each column of a Column table.
/// A column with zero non-missing observations is skipped (yields no
/// row). A column whose non-missing values include a non-finite
/// number returns `Err` for the whole table.
pub fn column_descriptives(table: &ColumnTable) -> Result<Vec<ColumnDescriptives>, NumericError> {
    let mut out = Vec::new();
    for (i, col) in table.columns.iter().enumerate() {
        let values: Vec<f64> = table.group_values(i).collect();
        if values.is_empty() {
            continue;
        }
        let stats = Descriptives::of(&values)?;
        out.push(ColumnDescriptives {
            group_title: col.title.clone(),
            stats,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use graphcraft_table::{Column, ColumnTable};

    use super::*;

    #[test]
    fn two_groups_descriptives() {
        let t = ColumnTable {
            title: "two groups".into(),
            row_titles: Vec::new(),
            columns: vec![
                Column {
                    title: "ctrl".into(),
                    values: vec![Some(1.0), Some(2.0), Some(3.0)],
                },
                Column {
                    title: "drug".into(),
                    values: vec![Some(5.0), None, Some(9.0), Some(10.0)],
                },
            ],
        };
        let out = column_descriptives(&t).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].group_title, "ctrl");
        assert_eq!(out[0].stats.n, 3);
        assert!((out[0].stats.mean - 2.0).abs() < 1e-15);
        assert_eq!(out[1].stats.n, 3); // one None skipped
        assert!((out[1].stats.mean - 8.0).abs() < 1e-15);
    }

    #[test]
    fn empty_group_is_skipped() {
        let t = ColumnTable {
            title: "one empty one not".into(),
            row_titles: Vec::new(),
            columns: vec![
                Column {
                    title: "empty".into(),
                    values: vec![None, None],
                },
                Column {
                    title: "data".into(),
                    values: vec![Some(1.0)],
                },
            ],
        };
        let out = column_descriptives(&t).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].group_title, "data");
    }
}
