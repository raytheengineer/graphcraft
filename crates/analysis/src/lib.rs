//! L4 analysis layer.
//!
//! Project-level dispatch (M8.2+) lives in [`project`]; the
//! column-table-based wrappers below sit beneath it. The parity
//! harness in `tests/parity.rs` consumes `tests/parity-matrix.csv`
//! to drive output-parity acceptance tests.

pub mod graph;
pub mod project;
pub use graph::{add_graph_sheet, build_graph_scene, GraphError};
pub use project::{add_analysis_sheet, run_analysis, AnalysisError};

use graphcraft_numeric::NumericError;
use graphcraft_regression::{simple_linear_regression, OlsFit};
use graphcraft_stats::{
    chi_squared_independence, dunnett, dunns_test, fisher_exact_2x2, kruskal_wallis, log_rank,
    mann_whitney, one_sample_t, one_way_anova, paired_t, pearson, spearman, tukey_hsd, unpaired_t,
    wilcoxon_signed_rank, ChiSquaredTest, Correlation, Descriptives, DunnPair, DunnettSide,
    FisherExact, KruskalWallis, LogRank, MannWhitney, OneSampleT, OneWayAnova, PairedT,
    PairwiseComparison, TwoSampleKind, TwoSampleT, WilcoxonSignedRank,
};
use graphcraft_table::{ColumnTable, ContingencyTable, SurvivalTable, XyTable};

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

/// Pearson correlation on two `ColumnTable` columns, pairing by
/// row index and dropping rows missing either value.
pub fn column_pearson(
    table: &ColumnTable,
    col_a: usize,
    col_b: usize,
) -> Result<Correlation, NumericError> {
    let (xs, ys) = pair_columns(table, col_a, col_b)?;
    pearson(&xs, &ys)
}

/// Spearman rank correlation on two `ColumnTable` columns.
pub fn column_spearman(
    table: &ColumnTable,
    col_a: usize,
    col_b: usize,
) -> Result<Correlation, NumericError> {
    let (xs, ys) = pair_columns(table, col_a, col_b)?;
    spearman(&xs, &ys)
}

fn pair_columns(
    table: &ColumnTable,
    col_a: usize,
    col_b: usize,
) -> Result<(Vec<f64>, Vec<f64>), NumericError> {
    if col_a >= table.columns.len() || col_b >= table.columns.len() {
        return Err(NumericError::TooFewValues {
            required: col_a.max(col_b) + 1,
            got: table.columns.len(),
        });
    }
    let a = &table.columns[col_a].values;
    let b = &table.columns[col_b].values;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (ai, bi) in a.iter().zip(b.iter()) {
        if let (Some(x), Some(y)) = (ai, bi) {
            xs.push(*x);
            ys.push(*y);
        }
    }
    Ok((xs, ys))
}

/// Two-group log-rank test on a `SurvivalTable`. The two groups
/// are picked by index into `table.groups`.
pub fn table_log_rank(
    table: &SurvivalTable,
    group_a: usize,
    group_b: usize,
) -> Result<LogRank, NumericError> {
    if group_a >= table.groups.len() || group_b >= table.groups.len() {
        return Err(NumericError::TooFewValues {
            required: group_a.max(group_b) + 1,
            got: table.groups.len(),
        });
    }
    let g1: Vec<(f64, bool)> = table.groups[group_a]
        .rows
        .iter()
        .map(|r| (r.time, r.censored))
        .collect();
    let g2: Vec<(f64, bool)> = table.groups[group_b]
        .rows
        .iter()
        .map(|r| (r.time, r.censored))
        .collect();
    log_rank(&g1, &g2)
}

/// Chi-squared independence test on a `ContingencyTable`.
pub fn table_chi_squared(table: &ContingencyTable) -> Result<ChiSquaredTest, NumericError> {
    chi_squared_independence(&table.counts)
}

/// Fisher's exact test on a `ContingencyTable`. The table must be
/// 2×2.
pub fn table_fisher_exact(table: &ContingencyTable) -> Result<FisherExact, NumericError> {
    if table.rows() != 2 || table.cols() != 2 {
        return Err(NumericError::TooFewValues {
            required: 4,
            got: table.rows() * table.cols(),
        });
    }
    let a = table.cell(0, 0).unwrap_or(0);
    let b = table.cell(0, 1).unwrap_or(0);
    let c = table.cell(1, 0).unwrap_or(0);
    let d = table.cell(1, 1).unwrap_or(0);
    fisher_exact_2x2(a, b, c, d)
}

/// Simple linear regression on dataset `dataset_idx` of an
/// `XyTable`, using the first subcolumn of that dataset as y and the
/// shared x column. Rows where either x or y is missing are dropped.
pub fn xy_simple_regression(table: &XyTable, dataset_idx: usize) -> Result<OlsFit, NumericError> {
    if dataset_idx >= table.datasets.len() {
        return Err(NumericError::TooFewValues {
            required: dataset_idx + 1,
            got: table.datasets.len(),
        });
    }
    let ds = &table.datasets[dataset_idx];
    if ds.subcolumns.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    let ys_sub = &ds.subcolumns[0].values;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (xi, yi) in table.x.iter().zip(ys_sub.iter()) {
        if let (Some(x), Some(y)) = (xi, yi) {
            xs.push(*x);
            ys.push(*y);
        }
    }
    simple_linear_regression(&xs, &ys)
}

/// Simple linear regression on two columns of a `ColumnTable`,
/// pairing by row index. `x_col` is the predictor; `y_col` is the
/// response. Rows missing either value are dropped.
pub fn column_simple_regression(
    table: &ColumnTable,
    x_col: usize,
    y_col: usize,
) -> Result<OlsFit, NumericError> {
    if x_col >= table.columns.len() || y_col >= table.columns.len() {
        return Err(NumericError::TooFewValues {
            required: x_col.max(y_col) + 1,
            got: table.columns.len(),
        });
    }
    let a = &table.columns[x_col].values;
    let b = &table.columns[y_col].values;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (ai, bi) in a.iter().zip(b.iter()) {
        if let (Some(x), Some(y)) = (ai, bi) {
            xs.push(*x);
            ys.push(*y);
        }
    }
    simple_linear_regression(&xs, &ys)
}

/// Mann-Whitney U on two Column-table columns by index.
pub fn column_mann_whitney(
    table: &ColumnTable,
    col_a: usize,
    col_b: usize,
) -> Result<MannWhitney, NumericError> {
    if col_a >= table.columns.len() || col_b >= table.columns.len() {
        return Err(NumericError::TooFewValues {
            required: col_a.max(col_b) + 1,
            got: table.columns.len(),
        });
    }
    let xs: Vec<f64> = table.group_values(col_a).collect();
    let ys: Vec<f64> = table.group_values(col_b).collect();
    mann_whitney(&xs, &ys)
}

/// Wilcoxon signed-rank on two Column-table columns, pairing by row
/// index and dropping rows where either column is missing.
pub fn column_wilcoxon_signed_rank(
    table: &ColumnTable,
    col_a: usize,
    col_b: usize,
) -> Result<WilcoxonSignedRank, NumericError> {
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
    wilcoxon_signed_rank(&xs, &ys)
}

/// Kruskal-Wallis across all non-empty columns of a Column table.
pub fn column_kruskal_wallis(table: &ColumnTable) -> Result<KruskalWallis, NumericError> {
    let mut groups: Vec<Vec<f64>> = Vec::new();
    for (i, _) in table.columns.iter().enumerate() {
        let g: Vec<f64> = table.group_values(i).collect();
        if !g.is_empty() {
            groups.push(g);
        }
    }
    kruskal_wallis(&groups)
}

/// Dunn's post-hoc on top of a Kruskal-Wallis of a Column table.
pub fn column_dunns_test(table: &ColumnTable) -> Result<Vec<DunnPair>, NumericError> {
    let kw = column_kruskal_wallis(table)?;
    dunns_test(&kw)
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
