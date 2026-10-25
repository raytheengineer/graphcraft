//! L2 tables: typed data tables for XY, Column, Grouped, Contingency,
//! Survival, Parts-of-whole, Multiple-variables and Nested layouts.
//!
//! Implemented so far: [`ColumnTable`], [`XyTable`]. The remaining
//! layouts land in later M2 steps.

use serde::{Deserialize, Serialize};

/// Enumeration of Prism-style table layouts GraphCraft will eventually
/// support. The variant payloads land as each layout is implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableKind {
    Xy,
    Column,
    Grouped,
    Contingency,
    Survival,
    PartsOfWhole,
    MultipleVariables,
    Nested,
}

/// A single column of a [`ColumnTable`]: a header plus a flat series of
/// optional numeric cells. `None` represents a missing value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub title: String,
    pub values: Vec<Option<f64>>,
}

impl Column {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            values: Vec::new(),
        }
    }
}

/// A Column-style data table: an overall title and a vector of columns,
/// each with its own header. Rows are implicit; columns need not be the
/// same length (ragged columns are allowed, as in Prism's Column tables).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnTable {
    pub title: String,
    pub columns: Vec<Column>,
}

impl ColumnTable {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            columns: Vec::new(),
        }
    }

    pub fn kind(&self) -> TableKind {
        TableKind::Column
    }

    /// Number of columns.
    pub fn width(&self) -> usize {
        self.columns.len()
    }

    /// Longest column length (the implicit row count).
    pub fn height(&self) -> usize {
        self.columns
            .iter()
            .map(|c| c.values.len())
            .max()
            .unwrap_or(0)
    }
}

// -------- XY tables ---------------------------------------------------
//
// Prism's XY tables share a single X column across one or more Y
// datasets. Each Y dataset can hold the data in several formats:
// a single Y per row, replicate observations side-by-side, or a mean
// plus an entered error value (SD, SEM, 95% CI, range). This first
// pass covers the four most common formats; the error-value variants
// (CI, range) land as later M2 steps.
//
// Reference (GraphPad Prism public manual, "XY tables"):
//   https://www.graphpad.com/guides/prism/latest/user-guide/

/// Shape of a single Y dataset inside an [`XyTable`].
///
/// The variant determines how many subcolumns the dataset has and how
/// they are interpreted by analyses and plots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum YFormat {
    /// One subcolumn: a single Y value per row.
    SingleY,
    /// Side-by-side raw observations; every subcolumn is an independent
    /// replicate of Y. The dataset's subcolumn count equals the maximum
    /// replicate count across groups and may be ragged by row (missing
    /// cells represent fewer replicates at that X).
    Replicates,
    /// Mean + sample standard deviation. Always two subcolumns, in that
    /// order; a third optional N subcolumn is present when `with_n` is
    /// true.
    MeanSd { with_n: bool },
    /// Mean + standard error of the mean. Same subcolumn layout as
    /// `MeanSd`.
    MeanSem { with_n: bool },
}

impl YFormat {
    /// Required number of subcolumns for `SingleY`, `MeanSd`, `MeanSem`.
    /// `Replicates` is variable; this returns `None` for that case.
    pub fn fixed_subcolumn_count(&self) -> Option<usize> {
        match *self {
            YFormat::SingleY => Some(1),
            YFormat::MeanSd { with_n } | YFormat::MeanSem { with_n } => {
                Some(if with_n { 3 } else { 2 })
            }
            YFormat::Replicates => None,
        }
    }
}

/// One Y dataset inside an [`XyTable`]: a title, a format tag, and one
/// or more subcolumns. The subcolumn count must match
/// `YFormat::fixed_subcolumn_count` where it is `Some`; `Replicates`
/// accepts any `>= 1` subcolumn count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YDataset {
    pub title: String,
    pub format: YFormat,
    pub subcolumns: Vec<Column>,
}

impl YDataset {
    pub fn new(title: impl Into<String>, format: YFormat) -> Self {
        Self {
            title: title.into(),
            format,
            subcolumns: Vec::new(),
        }
    }
}

/// Reason an [`XyTable`] fails [`XyTable::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XyError {
    /// A Y dataset's subcolumn count does not match its format.
    WrongSubcolumnCount {
        dataset: usize,
        expected: usize,
        got: usize,
    },
    /// A Y dataset of format `Replicates` has zero subcolumns.
    NoReplicates { dataset: usize },
    /// A subcolumn is longer than the shared X column.
    SubcolumnTooLong {
        dataset: usize,
        subcolumn: usize,
        x_len: usize,
        got: usize,
    },
}

impl std::fmt::Display for XyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XyError::WrongSubcolumnCount {
                dataset,
                expected,
                got,
            } => write!(
                f,
                "y dataset {dataset}: expected {expected} subcolumns, got {got}"
            ),
            XyError::NoReplicates { dataset } => {
                write!(
                    f,
                    "y dataset {dataset}: replicates format needs >= 1 subcolumn"
                )
            }
            XyError::SubcolumnTooLong {
                dataset,
                subcolumn,
                x_len,
                got,
            } => write!(
                f,
                "y dataset {dataset} subcolumn {subcolumn}: has {got} rows but x has {x_len}"
            ),
        }
    }
}

impl std::error::Error for XyError {}

/// An XY data table: a shared X column plus one or more Y datasets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XyTable {
    pub title: String,
    pub x_title: String,
    pub x: Vec<Option<f64>>,
    pub datasets: Vec<YDataset>,
}

impl XyTable {
    pub fn new(title: impl Into<String>, x_title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            x_title: x_title.into(),
            x: Vec::new(),
            datasets: Vec::new(),
        }
    }

    pub fn kind(&self) -> TableKind {
        TableKind::Xy
    }

    pub fn row_count(&self) -> usize {
        self.x.len()
    }

    /// Validate structural invariants: subcolumn counts match the
    /// format tag, and no subcolumn is longer than the X column.
    /// Shorter subcolumns are allowed and represent trailing missing
    /// cells.
    pub fn validate(&self) -> Result<(), XyError> {
        for (i, ds) in self.datasets.iter().enumerate() {
            match ds.format.fixed_subcolumn_count() {
                Some(expected) if ds.subcolumns.len() != expected => {
                    return Err(XyError::WrongSubcolumnCount {
                        dataset: i,
                        expected,
                        got: ds.subcolumns.len(),
                    });
                }
                _ => {}
            }
            if matches!(ds.format, YFormat::Replicates) && ds.subcolumns.is_empty() {
                return Err(XyError::NoReplicates { dataset: i });
            }
            for (j, sub) in ds.subcolumns.iter().enumerate() {
                if sub.values.len() > self.x.len() {
                    return Err(XyError::SubcolumnTooLong {
                        dataset: i,
                        subcolumn: j,
                        x_len: self.x.len(),
                        got: sub.values.len(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_is_longest_column() {
        let t = ColumnTable {
            title: "t".into(),
            columns: vec![
                Column {
                    title: "a".into(),
                    values: vec![Some(1.0), Some(2.0)],
                },
                Column {
                    title: "b".into(),
                    values: vec![Some(3.0), Some(4.0), Some(5.0)],
                },
            ],
        };
        assert_eq!(t.width(), 2);
        assert_eq!(t.height(), 3);
    }

    #[test]
    fn empty_table_has_zero_height() {
        let t = ColumnTable::new("empty");
        assert_eq!(t.width(), 0);
        assert_eq!(t.height(), 0);
    }

    fn sample_xy() -> XyTable {
        XyTable {
            title: "dose-response".into(),
            x_title: "dose (nM)".into(),
            x: vec![Some(0.1), Some(1.0), Some(10.0)],
            datasets: vec![
                YDataset {
                    title: "ctrl".into(),
                    format: YFormat::SingleY,
                    subcolumns: vec![Column {
                        title: "Y".into(),
                        values: vec![Some(2.3), Some(5.6), Some(8.9)],
                    }],
                },
                YDataset {
                    title: "treated".into(),
                    format: YFormat::MeanSd { with_n: false },
                    subcolumns: vec![
                        Column {
                            title: "mean".into(),
                            values: vec![Some(3.1), Some(6.4), Some(9.8)],
                        },
                        Column {
                            title: "sd".into(),
                            values: vec![Some(0.2), Some(0.3), Some(0.5)],
                        },
                    ],
                },
            ],
        }
    }

    #[test]
    fn xy_kind_is_xy_and_rows_match_x() {
        let t = sample_xy();
        assert_eq!(t.kind(), TableKind::Xy);
        assert_eq!(t.row_count(), 3);
        t.validate().unwrap();
    }

    #[test]
    fn xy_single_y_requires_exactly_one_subcolumn() {
        let mut t = sample_xy();
        t.datasets[0].subcolumns.push(Column::new("stray"));
        let err = t.validate().unwrap_err();
        assert!(matches!(
            err,
            XyError::WrongSubcolumnCount {
                dataset: 0,
                expected: 1,
                got: 2
            }
        ));
    }

    #[test]
    fn xy_mean_sd_with_n_wants_three_subcolumns() {
        let mut t = sample_xy();
        t.datasets[1].format = YFormat::MeanSd { with_n: true };
        let err = t.validate().unwrap_err();
        assert!(matches!(
            err,
            XyError::WrongSubcolumnCount {
                dataset: 1,
                expected: 3,
                got: 2
            }
        ));
    }

    #[test]
    fn xy_replicates_variable_width_ok() {
        let t = XyTable {
            title: "".into(),
            x_title: "x".into(),
            x: vec![Some(0.0), Some(1.0)],
            datasets: vec![YDataset {
                title: "treated".into(),
                format: YFormat::Replicates,
                subcolumns: vec![
                    Column {
                        title: "rep1".into(),
                        values: vec![Some(1.0), Some(2.0)],
                    },
                    Column {
                        title: "rep2".into(),
                        values: vec![Some(1.1), None],
                    },
                    Column {
                        title: "rep3".into(),
                        values: vec![Some(0.9), Some(2.1)],
                    },
                ],
            }],
        };
        t.validate().unwrap();
    }

    #[test]
    fn xy_replicates_zero_subcolumns_rejected() {
        let t = XyTable {
            title: "".into(),
            x_title: "x".into(),
            x: vec![Some(0.0)],
            datasets: vec![YDataset {
                title: "d".into(),
                format: YFormat::Replicates,
                subcolumns: vec![],
            }],
        };
        assert!(matches!(
            t.validate().unwrap_err(),
            XyError::NoReplicates { dataset: 0 }
        ));
    }

    #[test]
    fn xy_subcolumn_longer_than_x_rejected() {
        let mut t = sample_xy();
        t.datasets[0].subcolumns[0].values.push(Some(99.0));
        assert!(matches!(
            t.validate().unwrap_err(),
            XyError::SubcolumnTooLong { .. }
        ));
    }

    #[test]
    fn xy_round_trips_through_json() {
        let t = sample_xy();
        let bytes = serde_json::to_vec(&t).unwrap();
        let back: XyTable = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(t, back);
    }
}
