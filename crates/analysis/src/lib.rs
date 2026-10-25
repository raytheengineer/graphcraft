//! L4 analysis layer.
//!
//! The parity harness in `tests/parity.rs` consumes
//! `tests/parity-matrix.csv` to drive output-parity acceptance tests.
//!
//! First analysis (M3.0): per-group descriptive statistics on a
//! Column table.

use graphcraft_numeric::NumericError;
use graphcraft_stats::{one_sample_t, Descriptives, OneSampleT};
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
