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

/// A Column-style data table: an overall title, optional per-row
/// labels, and a vector of columns. Each column is one group; its
/// cells are the group's replicate observations top-down.
///
/// Columns need not be the same length (ragged tables are allowed, as
/// in Prism's Column tables — one group may have more replicates than
/// another). `row_titles` is parallel to the longest column; an empty
/// `row_titles` means "no user labels, address rows by 1-based index".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnTable {
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_titles: Vec<String>,
    pub columns: Vec<Column>,
}

impl ColumnTable {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            row_titles: Vec::new(),
            columns: Vec::new(),
        }
    }

    pub fn kind(&self) -> TableKind {
        TableKind::Column
    }

    /// Number of columns (groups).
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

    /// Iterate the non-missing replicate values for one column (one
    /// group). Returns an empty iterator if the index is out of range.
    pub fn group_values(&self, col: usize) -> impl Iterator<Item = f64> + '_ {
        self.columns
            .get(col)
            .into_iter()
            .flat_map(|c| c.values.iter().copied().flatten())
    }

    /// Replicate count (non-missing) for one column.
    pub fn group_n(&self, col: usize) -> usize {
        self.group_values(col).count()
    }

    /// `true` if `row_titles` is either empty or has one entry per row
    /// of the longest column.
    pub fn row_titles_valid(&self) -> bool {
        self.row_titles.is_empty() || self.row_titles.len() == self.height()
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

// -------- Grouped tables ---------------------------------------------
//
// Prism's Grouped layout: rows × side-by-side groups × replicates
// within each group. One row corresponds to one condition (e.g.
// "Day 1"), one group to one factor level (e.g. "Control"), and the
// subcolumns within a group are the replicate observations at that
// row for that group. Powers two-way and three-way ANOVA, and the
// Grouped bar/scatter plots.
//
// Reference (public manual, "Grouped data tables"):
//   https://www.graphpad.com/guides/prism/latest/user-guide/

/// One group inside a [`GroupedTable`]: a header plus a vector of
/// replicate subcolumns. All subcolumns of one group must have the
/// same length — a missing observation at a row is encoded as
/// `None` in that subcolumn, not as a shorter column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupColumn {
    pub title: String,
    pub subcolumns: Vec<Column>,
}

impl GroupColumn {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subcolumns: Vec::new(),
        }
    }

    /// Replicate count for this group (number of subcolumns).
    pub fn replicates(&self) -> usize {
        self.subcolumns.len()
    }

    /// Row count — the common length of every subcolumn, or 0 if the
    /// group has no subcolumns or they disagree.
    pub fn rows(&self) -> usize {
        let mut iter = self.subcolumns.iter();
        let Some(first) = iter.next() else {
            return 0;
        };
        let n = first.values.len();
        for sub in iter {
            if sub.values.len() != n {
                return 0;
            }
        }
        n
    }
}

/// Reason a [`GroupedTable`] fails [`GroupedTable::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupedError {
    /// Subcolumns inside one group have different lengths.
    RaggedGroup { group: usize },
    /// `row_titles` is non-empty but doesn't match the row count.
    RowTitlesMismatch { expected: usize, got: usize },
}

impl std::fmt::Display for GroupedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GroupedError::RaggedGroup { group } => {
                write!(f, "group {group}: subcolumns have different lengths")
            }
            GroupedError::RowTitlesMismatch { expected, got } => write!(
                f,
                "row_titles has {got} entries but table has {expected} rows"
            ),
        }
    }
}

impl std::error::Error for GroupedError {}

/// A Grouped data table: rows × side-by-side groups × replicates.
///
/// `height()` is the common row count across groups (groups that
/// disagree with the shared row count are a validation error, not a
/// silent extension). Different groups may hold different replicate
/// counts; that is the whole point of the layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupedTable {
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_titles: Vec<String>,
    pub groups: Vec<GroupColumn>,
}

impl GroupedTable {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            row_titles: Vec::new(),
            groups: Vec::new(),
        }
    }

    pub fn kind(&self) -> TableKind {
        TableKind::Grouped
    }

    pub fn width(&self) -> usize {
        self.groups.len()
    }

    /// Common row count across groups: the longest group's row count.
    /// Empty if there are no groups.
    pub fn height(&self) -> usize {
        self.groups.iter().map(|g| g.rows()).max().unwrap_or(0)
    }

    /// Fetch one cell: `(row, group, replicate)`. `None` if any index
    /// is out of range or the cell is missing.
    pub fn cell(&self, row: usize, group: usize, replicate: usize) -> Option<f64> {
        self.groups
            .get(group)?
            .subcolumns
            .get(replicate)?
            .values
            .get(row)
            .copied()
            .flatten()
    }

    /// Iterate the non-missing replicates for one (row, group) cell.
    pub fn replicates_at(&self, row: usize, group: usize) -> impl Iterator<Item = f64> + '_ {
        self.groups
            .get(group)
            .into_iter()
            .flat_map(move |g| g.subcolumns.iter())
            .filter_map(move |sub| sub.values.get(row).copied().flatten())
    }

    /// Replicate count (non-missing) for one `(row, group)`.
    pub fn replicate_n(&self, row: usize, group: usize) -> usize {
        self.replicates_at(row, group).count()
    }

    /// Validate: each group is internally rectangular (all subcolumns
    /// the same length) and `row_titles`, if non-empty, matches the
    /// row count.
    pub fn validate(&self) -> Result<(), GroupedError> {
        for (i, g) in self.groups.iter().enumerate() {
            if g.subcolumns.len() >= 2 {
                let first = g.subcolumns[0].values.len();
                for sub in &g.subcolumns[1..] {
                    if sub.values.len() != first {
                        return Err(GroupedError::RaggedGroup { group: i });
                    }
                }
            }
        }
        let h = self.height();
        if !self.row_titles.is_empty() && self.row_titles.len() != h {
            return Err(GroupedError::RowTitlesMismatch {
                expected: h,
                got: self.row_titles.len(),
            });
        }
        Ok(())
    }
}

// -------- Contingency tables -----------------------------------------
//
// Prism's Contingency layout: a small rectangular table of
// non-negative integer counts, with row and column titles. Powers
// chi-square (independence and goodness-of-fit), Fisher's exact test,
// and relative-risk / odds-ratio analyses.
//
// Reference (public manual, "Contingency tables"):
//   https://www.graphpad.com/guides/prism/latest/user-guide/

/// Reason a [`ContingencyTable`] fails [`ContingencyTable::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContingencyError {
    /// A row has a different column count from the first row.
    Ragged {
        row: usize,
        expected: usize,
        got: usize,
    },
    /// `row_titles` is non-empty and doesn't match row count.
    RowTitlesMismatch { expected: usize, got: usize },
    /// `column_titles` is non-empty and doesn't match column count.
    ColumnTitlesMismatch { expected: usize, got: usize },
    /// All counts are zero — chi-square and company are undefined.
    ZeroTotal,
}

impl std::fmt::Display for ContingencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContingencyError::Ragged { row, expected, got } => write!(
                f,
                "contingency row {row}: has {got} columns but table has {expected}"
            ),
            ContingencyError::RowTitlesMismatch { expected, got } => write!(
                f,
                "row_titles has {got} entries but table has {expected} rows"
            ),
            ContingencyError::ColumnTitlesMismatch { expected, got } => write!(
                f,
                "column_titles has {got} entries but table has {expected} columns"
            ),
            ContingencyError::ZeroTotal => f.write_str("contingency table total is zero"),
        }
    }
}

impl std::error::Error for ContingencyError {}

/// A contingency table: non-negative integer counts stored row-major.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContingencyTable {
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_titles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_titles: Vec<String>,
    /// `counts[row][col]`.
    pub counts: Vec<Vec<u64>>,
}

impl ContingencyTable {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            row_titles: Vec::new(),
            column_titles: Vec::new(),
            counts: Vec::new(),
        }
    }

    pub fn kind(&self) -> TableKind {
        TableKind::Contingency
    }

    pub fn rows(&self) -> usize {
        self.counts.len()
    }

    pub fn cols(&self) -> usize {
        self.counts.first().map_or(0, |r| r.len())
    }

    pub fn cell(&self, row: usize, col: usize) -> Option<u64> {
        self.counts.get(row)?.get(col).copied()
    }

    pub fn row_total(&self, row: usize) -> u64 {
        self.counts.get(row).map_or(0, |r| r.iter().copied().sum())
    }

    pub fn col_total(&self, col: usize) -> u64 {
        self.counts
            .iter()
            .map(|r| r.get(col).copied().unwrap_or(0))
            .sum()
    }

    pub fn total(&self) -> u64 {
        self.counts.iter().flatten().copied().sum()
    }

    pub fn validate(&self) -> Result<(), ContingencyError> {
        let cols = self.cols();
        for (i, row) in self.counts.iter().enumerate().skip(1) {
            if row.len() != cols {
                return Err(ContingencyError::Ragged {
                    row: i,
                    expected: cols,
                    got: row.len(),
                });
            }
        }
        let r = self.rows();
        if !self.row_titles.is_empty() && self.row_titles.len() != r {
            return Err(ContingencyError::RowTitlesMismatch {
                expected: r,
                got: self.row_titles.len(),
            });
        }
        if !self.column_titles.is_empty() && self.column_titles.len() != cols {
            return Err(ContingencyError::ColumnTitlesMismatch {
                expected: cols,
                got: self.column_titles.len(),
            });
        }
        if self.total() == 0 {
            return Err(ContingencyError::ZeroTotal);
        }
        Ok(())
    }
}

// -------- Survival tables --------------------------------------------
//
// Prism's Survival layout: one group per column (treatment arm, say),
// one row per subject, each row recording a time and a censor flag.
// `censored = false` means the event (death, failure, …) occurred at
// that time; `censored = true` means the subject exited observation
// without the event (lost to follow-up, still alive at study end).
// Powers Kaplan-Meier estimation, log-rank and Gehan-Breslow-Wilcoxon.
//
// Reference (public manual, "Survival analysis"):
//   https://www.graphpad.com/guides/prism/latest/user-guide/

/// One subject's record in a [`SurvivalGroup`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SurvivalRow {
    pub time: f64,
    /// `false` = event occurred at `time`; `true` = censored at `time`.
    pub censored: bool,
}

/// One group (treatment arm) in a [`SurvivalTable`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurvivalGroup {
    pub title: String,
    pub rows: Vec<SurvivalRow>,
}

impl SurvivalGroup {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            rows: Vec::new(),
        }
    }

    /// Count of recorded events (uncensored rows).
    pub fn events(&self) -> usize {
        self.rows.iter().filter(|r| !r.censored).count()
    }

    /// Count of censored rows.
    pub fn censored(&self) -> usize {
        self.rows.iter().filter(|r| r.censored).count()
    }
}

/// Reason a [`SurvivalTable`] fails [`SurvivalTable::validate`].
#[derive(Debug, Clone, PartialEq)]
pub enum SurvivalError {
    /// A row has a non-finite time.
    TimeNotFinite { group: usize, row: usize },
    /// A row has a negative time (times are durations).
    NegativeTime { group: usize, row: usize, time: f64 },
}

impl std::fmt::Display for SurvivalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SurvivalError::TimeNotFinite { group, row } => {
                write!(f, "survival group {group} row {row}: time is not finite")
            }
            SurvivalError::NegativeTime { group, row, time } => write!(
                f,
                "survival group {group} row {row}: time {time} is negative"
            ),
        }
    }
}

impl std::error::Error for SurvivalError {}

/// A survival-analysis data table: one group per treatment arm, one
/// row per subject.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurvivalTable {
    pub title: String,
    pub time_title: String,
    pub groups: Vec<SurvivalGroup>,
}

impl SurvivalTable {
    pub fn new(title: impl Into<String>, time_title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            time_title: time_title.into(),
            groups: Vec::new(),
        }
    }

    pub fn kind(&self) -> TableKind {
        TableKind::Survival
    }

    /// Number of groups (treatment arms).
    pub fn width(&self) -> usize {
        self.groups.len()
    }

    /// Total subjects across all groups.
    pub fn subjects(&self) -> usize {
        self.groups.iter().map(|g| g.rows.len()).sum()
    }

    pub fn validate(&self) -> Result<(), SurvivalError> {
        for (gi, g) in self.groups.iter().enumerate() {
            for (ri, r) in g.rows.iter().enumerate() {
                if !r.time.is_finite() {
                    return Err(SurvivalError::TimeNotFinite { group: gi, row: ri });
                }
                if r.time < 0.0 {
                    return Err(SurvivalError::NegativeTime {
                        group: gi,
                        row: ri,
                        time: r.time,
                    });
                }
            }
        }
        Ok(())
    }
}

// -------- Parts-of-whole tables --------------------------------------
//
// Prism's Parts-of-whole layout: a list of (label, value) rows that
// a pie/donut chart renders as slices of a total. Values must be
// finite and non-negative; the grand total must be positive.
//
// Reference (public manual, "Parts of whole data tables"):
//   https://www.graphpad.com/guides/prism/latest/user-guide/

/// One slice of a [`PartsOfWholeTable`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartSlice {
    pub label: String,
    pub value: f64,
}

/// Reason a [`PartsOfWholeTable`] fails [`PartsOfWholeTable::validate`].
#[derive(Debug, Clone, PartialEq)]
pub enum PartsOfWholeError {
    SliceNotFinite { slice: usize },
    SliceNegative { slice: usize, value: f64 },
    ZeroTotal,
}

impl std::fmt::Display for PartsOfWholeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PartsOfWholeError::SliceNotFinite { slice } => {
                write!(f, "parts-of-whole slice {slice}: value is not finite")
            }
            PartsOfWholeError::SliceNegative { slice, value } => {
                write!(f, "parts-of-whole slice {slice}: value {value} is negative")
            }
            PartsOfWholeError::ZeroTotal => f.write_str("parts-of-whole total is zero"),
        }
    }
}

impl std::error::Error for PartsOfWholeError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartsOfWholeTable {
    pub title: String,
    pub value_title: String,
    pub slices: Vec<PartSlice>,
}

impl PartsOfWholeTable {
    pub fn new(title: impl Into<String>, value_title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            value_title: value_title.into(),
            slices: Vec::new(),
        }
    }

    pub fn kind(&self) -> TableKind {
        TableKind::PartsOfWhole
    }

    pub fn total(&self) -> f64 {
        self.slices.iter().map(|s| s.value).sum()
    }

    pub fn validate(&self) -> Result<(), PartsOfWholeError> {
        for (i, s) in self.slices.iter().enumerate() {
            if !s.value.is_finite() {
                return Err(PartsOfWholeError::SliceNotFinite { slice: i });
            }
            if s.value < 0.0 {
                return Err(PartsOfWholeError::SliceNegative {
                    slice: i,
                    value: s.value,
                });
            }
        }
        if self.total() <= 0.0 {
            return Err(PartsOfWholeError::ZeroTotal);
        }
        Ok(())
    }
}

// -------- Multiple-variables tables ----------------------------------
//
// Prism's Multiple-variables layout: a flat spreadsheet of typed
// columns. Each column is either numeric (f64, missing = None) or
// categorical (String, missing = None). All columns share one row
// count. Powers correlation matrices, multiple regression, PCA.
//
// Reference (public manual, "Multiple variables data tables"):
//   https://www.graphpad.com/guides/prism/latest/user-guide/

/// One column of a [`MultipleVariablesTable`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "values", rename_all = "snake_case")]
pub enum MvColumnData {
    Numeric(Vec<Option<f64>>),
    Categorical(Vec<Option<String>>),
}

impl MvColumnData {
    pub fn len(&self) -> usize {
        match self {
            MvColumnData::Numeric(v) => v.len(),
            MvColumnData::Categorical(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MvColumn {
    pub title: String,
    pub data: MvColumnData,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MultipleVariablesError {
    /// Columns have different lengths.
    LengthMismatch {
        column: usize,
        expected: usize,
        got: usize,
    },
}

impl std::fmt::Display for MultipleVariablesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultipleVariablesError::LengthMismatch {
                column,
                expected,
                got,
            } => write!(
                f,
                "multiple-variables column {column}: has {got} rows but table has {expected}"
            ),
        }
    }
}

impl std::error::Error for MultipleVariablesError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultipleVariablesTable {
    pub title: String,
    pub columns: Vec<MvColumn>,
}

impl MultipleVariablesTable {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            columns: Vec::new(),
        }
    }

    pub fn kind(&self) -> TableKind {
        TableKind::MultipleVariables
    }

    pub fn width(&self) -> usize {
        self.columns.len()
    }

    pub fn height(&self) -> usize {
        self.columns.first().map_or(0, |c| c.data.len())
    }

    pub fn validate(&self) -> Result<(), MultipleVariablesError> {
        let h = self.height();
        for (i, c) in self.columns.iter().enumerate().skip(1) {
            if c.data.len() != h {
                return Err(MultipleVariablesError::LengthMismatch {
                    column: i,
                    expected: h,
                    got: c.data.len(),
                });
            }
        }
        Ok(())
    }
}

// -------- Nested tables ----------------------------------------------
//
// Prism's Nested layout: one column per treatment group; inside each
// group one block per nested unit ("subject"); inside each subject
// one or more replicate observations. Replicate counts may differ
// across subjects (realistic experimental data). Powers nested
// t-test, nested one-way ANOVA and hierarchical models.
//
// Reference (public manual, "Nested data tables"):
//   https://www.graphpad.com/guides/prism/latest/user-guide/

/// One nested subject inside a [`NestedGroup`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NestedSubject {
    pub title: String,
    pub values: Vec<Option<f64>>,
}

/// One treatment group inside a [`NestedTable`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NestedGroup {
    pub title: String,
    pub subjects: Vec<NestedSubject>,
}

impl NestedGroup {
    /// Total non-missing observations across all subjects in this group.
    pub fn observations(&self) -> usize {
        self.subjects
            .iter()
            .map(|s| s.values.iter().filter(|v| v.is_some()).count())
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NestedTable {
    pub title: String,
    pub groups: Vec<NestedGroup>,
}

impl NestedTable {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            groups: Vec::new(),
        }
    }

    pub fn kind(&self) -> TableKind {
        TableKind::Nested
    }

    pub fn width(&self) -> usize {
        self.groups.len()
    }

    /// Total non-missing observations across the whole table.
    pub fn observations(&self) -> usize {
        self.groups.iter().map(NestedGroup::observations).sum()
    }

    /// Non-missing replicates for one `(group, subject)` cell.
    pub fn replicates(&self, group: usize, subject: usize) -> impl Iterator<Item = f64> + '_ {
        self.groups
            .get(group)
            .and_then(|g| g.subjects.get(subject))
            .into_iter()
            .flat_map(|s| s.values.iter().copied().flatten())
    }
}

// -------- AnyTable (typed container) ---------------------------------
//
// Documents hold a heterogenous list of tables (an experiment may mix
// Column tables for one-way comparisons with XY tables for a
// dose-response). `AnyTable` is the discriminator-tagged serde enum
// used in [`graphcraft_format::Document::tables`].

/// Any table layout GraphCraft currently understands. Serialized with
/// a `"kind"` discriminator so new variants can be added without
/// breaking existing documents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnyTable {
    Column(ColumnTable),
    Xy(XyTable),
    Grouped(GroupedTable),
    Contingency(ContingencyTable),
    Survival(SurvivalTable),
    PartsOfWhole(PartsOfWholeTable),
    MultipleVariables(MultipleVariablesTable),
    Nested(NestedTable),
}

impl AnyTable {
    pub fn kind(&self) -> TableKind {
        match self {
            AnyTable::Column(_) => TableKind::Column,
            AnyTable::Xy(_) => TableKind::Xy,
            AnyTable::Grouped(_) => TableKind::Grouped,
            AnyTable::Contingency(_) => TableKind::Contingency,
            AnyTable::Survival(_) => TableKind::Survival,
            AnyTable::PartsOfWhole(_) => TableKind::PartsOfWhole,
            AnyTable::MultipleVariables(_) => TableKind::MultipleVariables,
            AnyTable::Nested(_) => TableKind::Nested,
        }
    }
}

impl From<ColumnTable> for AnyTable {
    fn from(t: ColumnTable) -> Self {
        AnyTable::Column(t)
    }
}

impl From<XyTable> for AnyTable {
    fn from(t: XyTable) -> Self {
        AnyTable::Xy(t)
    }
}

impl From<GroupedTable> for AnyTable {
    fn from(t: GroupedTable) -> Self {
        AnyTable::Grouped(t)
    }
}

impl From<ContingencyTable> for AnyTable {
    fn from(t: ContingencyTable) -> Self {
        AnyTable::Contingency(t)
    }
}

impl From<SurvivalTable> for AnyTable {
    fn from(t: SurvivalTable) -> Self {
        AnyTable::Survival(t)
    }
}

impl From<PartsOfWholeTable> for AnyTable {
    fn from(t: PartsOfWholeTable) -> Self {
        AnyTable::PartsOfWhole(t)
    }
}

impl From<MultipleVariablesTable> for AnyTable {
    fn from(t: MultipleVariablesTable) -> Self {
        AnyTable::MultipleVariables(t)
    }
}

impl From<NestedTable> for AnyTable {
    fn from(t: NestedTable) -> Self {
        AnyTable::Nested(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_is_longest_column() {
        let t = ColumnTable {
            title: "t".into(),
            row_titles: Vec::new(),
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

    #[test]
    fn column_group_values_skip_missing() {
        let t = ColumnTable {
            title: "t".into(),
            row_titles: Vec::new(),
            columns: vec![Column {
                title: "ctrl".into(),
                values: vec![Some(1.0), None, Some(2.0), Some(3.5), None],
            }],
        };
        let xs: Vec<f64> = t.group_values(0).collect();
        assert_eq!(xs, vec![1.0, 2.0, 3.5]);
        assert_eq!(t.group_n(0), 3);
    }

    #[test]
    fn column_group_values_out_of_range_is_empty() {
        let t = ColumnTable::new("empty");
        assert_eq!(t.group_values(42).count(), 0);
        assert_eq!(t.group_n(42), 0);
    }

    #[test]
    fn column_row_titles_valid_rules() {
        let mut t = ColumnTable {
            title: "t".into(),
            row_titles: Vec::new(),
            columns: vec![Column {
                title: "a".into(),
                values: vec![Some(1.0), Some(2.0), Some(3.0)],
            }],
        };
        assert!(t.row_titles_valid());
        t.row_titles = vec!["r1".into(), "r2".into(), "r3".into()];
        assert!(t.row_titles_valid());
        t.row_titles.push("r4".into());
        assert!(!t.row_titles_valid());
    }

    #[test]
    fn any_table_round_trips_with_kind_tag() {
        let column: AnyTable = ColumnTable {
            title: "cells".into(),
            row_titles: vec!["baseline".into()],
            columns: vec![Column {
                title: "ctrl".into(),
                values: vec![Some(1.0)],
            }],
        }
        .into();
        let xy: AnyTable = sample_xy().into();
        for t in [column, xy] {
            let json = serde_json::to_string(&t).unwrap();
            let back: AnyTable = serde_json::from_str(&json).unwrap();
            assert_eq!(t, back);
        }
    }

    #[test]
    fn any_table_json_carries_kind_discriminator() {
        let t: AnyTable = ColumnTable::new("x").into();
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"kind\":\"column\""), "got {json}");
    }

    fn sample_grouped() -> GroupedTable {
        // Two rows (Day 1 / Day 2), two groups (Ctrl / Drug), with
        // different replicate counts per group (3 vs 2).
        GroupedTable {
            title: "proliferation".into(),
            row_titles: vec!["Day 1".into(), "Day 2".into()],
            groups: vec![
                GroupColumn {
                    title: "Ctrl".into(),
                    subcolumns: vec![
                        Column {
                            title: "r1".into(),
                            values: vec![Some(10.0), Some(14.0)],
                        },
                        Column {
                            title: "r2".into(),
                            values: vec![Some(11.0), Some(13.5)],
                        },
                        Column {
                            title: "r3".into(),
                            values: vec![Some(10.5), None],
                        },
                    ],
                },
                GroupColumn {
                    title: "Drug".into(),
                    subcolumns: vec![
                        Column {
                            title: "r1".into(),
                            values: vec![Some(15.0), Some(22.0)],
                        },
                        Column {
                            title: "r2".into(),
                            values: vec![Some(14.5), Some(21.0)],
                        },
                    ],
                },
            ],
        }
    }

    #[test]
    fn grouped_shape() {
        let t = sample_grouped();
        assert_eq!(t.kind(), TableKind::Grouped);
        assert_eq!(t.width(), 2);
        assert_eq!(t.height(), 2);
        t.validate().unwrap();
    }

    #[test]
    fn grouped_cell_lookup() {
        let t = sample_grouped();
        assert_eq!(t.cell(0, 0, 0), Some(10.0));
        assert_eq!(t.cell(1, 0, 2), None); // missing replicate
        assert_eq!(t.cell(1, 1, 1), Some(21.0));
        assert_eq!(t.cell(5, 0, 0), None); // row OOR
        assert_eq!(t.cell(0, 9, 0), None); // group OOR
        assert_eq!(t.cell(0, 0, 9), None); // replicate OOR
    }

    #[test]
    fn grouped_replicates_at_skip_missing() {
        let t = sample_grouped();
        let reps_day2_ctrl: Vec<f64> = t.replicates_at(1, 0).collect();
        assert_eq!(reps_day2_ctrl, vec![14.0, 13.5]); // r3 at row 1 is None
        assert_eq!(t.replicate_n(1, 0), 2);
        assert_eq!(t.replicate_n(0, 0), 3);
        assert_eq!(t.replicate_n(0, 1), 2);
    }

    #[test]
    fn grouped_ragged_within_group_rejected() {
        let mut t = sample_grouped();
        t.groups[0].subcolumns[0].values.push(Some(99.0));
        assert!(matches!(
            t.validate().unwrap_err(),
            GroupedError::RaggedGroup { group: 0 }
        ));
    }

    #[test]
    fn grouped_row_titles_length_checked() {
        let mut t = sample_grouped();
        t.row_titles.push("Day 3".into());
        assert!(matches!(
            t.validate().unwrap_err(),
            GroupedError::RowTitlesMismatch {
                expected: 2,
                got: 3
            }
        ));
    }

    #[test]
    fn grouped_round_trip_as_any_table() {
        let t: AnyTable = sample_grouped().into();
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"kind\":\"grouped\""), "got {json}");
        let back: AnyTable = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    fn sample_contingency() -> ContingencyTable {
        // Classic 2x2: exposure x disease.
        ContingencyTable {
            title: "exposure vs disease".into(),
            row_titles: vec!["Exposed".into(), "Unexposed".into()],
            column_titles: vec!["Disease".into(), "No disease".into()],
            counts: vec![vec![30, 70], vec![10, 90]],
        }
    }

    #[test]
    fn contingency_shape_and_totals() {
        let t = sample_contingency();
        assert_eq!(t.kind(), TableKind::Contingency);
        assert_eq!(t.rows(), 2);
        assert_eq!(t.cols(), 2);
        assert_eq!(t.cell(0, 0), Some(30));
        assert_eq!(t.cell(1, 1), Some(90));
        assert_eq!(t.cell(5, 5), None);
        assert_eq!(t.row_total(0), 100);
        assert_eq!(t.row_total(1), 100);
        assert_eq!(t.col_total(0), 40);
        assert_eq!(t.col_total(1), 160);
        assert_eq!(t.total(), 200);
        t.validate().unwrap();
    }

    #[test]
    fn contingency_ragged_rejected() {
        let mut t = sample_contingency();
        t.counts[1].push(5);
        assert!(matches!(
            t.validate().unwrap_err(),
            ContingencyError::Ragged {
                row: 1,
                expected: 2,
                got: 3
            }
        ));
    }

    #[test]
    fn contingency_zero_total_rejected() {
        let t = ContingencyTable {
            title: "".into(),
            row_titles: Vec::new(),
            column_titles: Vec::new(),
            counts: vec![vec![0, 0], vec![0, 0]],
        };
        assert!(matches!(
            t.validate().unwrap_err(),
            ContingencyError::ZeroTotal
        ));
    }

    #[test]
    fn contingency_title_length_mismatches_rejected() {
        let mut t = sample_contingency();
        t.row_titles.pop();
        assert!(matches!(
            t.validate().unwrap_err(),
            ContingencyError::RowTitlesMismatch {
                expected: 2,
                got: 1
            }
        ));
        let mut t = sample_contingency();
        t.column_titles.push("extra".into());
        assert!(matches!(
            t.validate().unwrap_err(),
            ContingencyError::ColumnTitlesMismatch {
                expected: 2,
                got: 3
            }
        ));
    }

    #[test]
    fn contingency_round_trip_as_any_table() {
        let t: AnyTable = sample_contingency().into();
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"kind\":\"contingency\""), "got {json}");
        let back: AnyTable = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    fn sample_survival() -> SurvivalTable {
        SurvivalTable {
            title: "trial".into(),
            time_title: "months".into(),
            groups: vec![
                SurvivalGroup {
                    title: "Placebo".into(),
                    rows: vec![
                        SurvivalRow {
                            time: 2.0,
                            censored: false,
                        },
                        SurvivalRow {
                            time: 5.5,
                            censored: false,
                        },
                        SurvivalRow {
                            time: 12.0,
                            censored: true,
                        },
                    ],
                },
                SurvivalGroup {
                    title: "Drug".into(),
                    rows: vec![
                        SurvivalRow {
                            time: 8.0,
                            censored: false,
                        },
                        SurvivalRow {
                            time: 12.0,
                            censored: true,
                        },
                        SurvivalRow {
                            time: 12.0,
                            censored: true,
                        },
                    ],
                },
            ],
        }
    }

    #[test]
    fn survival_shape_counts() {
        let t = sample_survival();
        assert_eq!(t.kind(), TableKind::Survival);
        assert_eq!(t.width(), 2);
        assert_eq!(t.subjects(), 6);
        assert_eq!(t.groups[0].events(), 2);
        assert_eq!(t.groups[0].censored(), 1);
        assert_eq!(t.groups[1].events(), 1);
        assert_eq!(t.groups[1].censored(), 2);
        t.validate().unwrap();
    }

    #[test]
    fn survival_nonfinite_time_rejected() {
        let mut t = sample_survival();
        t.groups[0].rows[0].time = f64::NAN;
        assert!(matches!(
            t.validate().unwrap_err(),
            SurvivalError::TimeNotFinite { group: 0, row: 0 }
        ));
    }

    #[test]
    fn survival_negative_time_rejected() {
        let mut t = sample_survival();
        t.groups[1].rows[1].time = -1.0;
        assert!(matches!(
            t.validate().unwrap_err(),
            SurvivalError::NegativeTime {
                group: 1,
                row: 1,
                ..
            }
        ));
    }

    #[test]
    fn survival_round_trip_as_any_table() {
        let t: AnyTable = sample_survival().into();
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"kind\":\"survival\""), "got {json}");
        let back: AnyTable = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    fn sample_pow() -> PartsOfWholeTable {
        PartsOfWholeTable {
            title: "market share".into(),
            value_title: "%".into(),
            slices: vec![
                PartSlice {
                    label: "A".into(),
                    value: 40.0,
                },
                PartSlice {
                    label: "B".into(),
                    value: 25.0,
                },
                PartSlice {
                    label: "C".into(),
                    value: 35.0,
                },
            ],
        }
    }

    #[test]
    fn pow_total_and_validate() {
        let t = sample_pow();
        assert_eq!(t.kind(), TableKind::PartsOfWhole);
        assert!((t.total() - 100.0).abs() < 1e-15);
        t.validate().unwrap();
    }

    #[test]
    fn pow_rejects_negative_and_nan_and_zero_total() {
        let mut t = sample_pow();
        t.slices[1].value = -1.0;
        assert!(matches!(
            t.validate().unwrap_err(),
            PartsOfWholeError::SliceNegative { slice: 1, .. }
        ));

        let mut t = sample_pow();
        t.slices[0].value = f64::NAN;
        assert!(matches!(
            t.validate().unwrap_err(),
            PartsOfWholeError::SliceNotFinite { slice: 0 }
        ));

        let t = PartsOfWholeTable {
            title: "".into(),
            value_title: "".into(),
            slices: vec![PartSlice {
                label: "zero".into(),
                value: 0.0,
            }],
        };
        assert!(matches!(
            t.validate().unwrap_err(),
            PartsOfWholeError::ZeroTotal
        ));
    }

    #[test]
    fn pow_round_trip_as_any_table() {
        let t: AnyTable = sample_pow().into();
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"kind\":\"parts_of_whole\""), "got {json}");
        let back: AnyTable = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    fn sample_mv() -> MultipleVariablesTable {
        MultipleVariablesTable {
            title: "cohort".into(),
            columns: vec![
                MvColumn {
                    title: "age".into(),
                    data: MvColumnData::Numeric(vec![Some(25.0), Some(37.0), None, Some(44.0)]),
                },
                MvColumnData::Categorical(vec![
                    Some("F".into()),
                    Some("M".into()),
                    Some("F".into()),
                    None,
                ])
                .into_column("sex"),
            ],
        }
    }

    // Convenience for terseness in the fixture.
    impl MvColumnData {
        fn into_column(self, title: &str) -> MvColumn {
            MvColumn {
                title: title.to_string(),
                data: self,
            }
        }
    }

    #[test]
    fn mv_shape_and_validate() {
        let t = sample_mv();
        assert_eq!(t.kind(), TableKind::MultipleVariables);
        assert_eq!(t.width(), 2);
        assert_eq!(t.height(), 4);
        t.validate().unwrap();
    }

    #[test]
    fn mv_rejects_length_mismatch() {
        let mut t = sample_mv();
        if let MvColumnData::Categorical(v) = &mut t.columns[1].data {
            v.push(None);
        }
        assert!(matches!(
            t.validate().unwrap_err(),
            MultipleVariablesError::LengthMismatch {
                column: 1,
                expected: 4,
                got: 5
            }
        ));
    }

    #[test]
    fn mv_round_trip_as_any_table() {
        let t: AnyTable = sample_mv().into();
        let json = serde_json::to_string(&t).unwrap();
        assert!(
            json.contains("\"kind\":\"multiple_variables\""),
            "got {json}"
        );
        let back: AnyTable = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    fn sample_nested() -> NestedTable {
        NestedTable {
            title: "slice counts".into(),
            groups: vec![
                NestedGroup {
                    title: "Ctrl".into(),
                    subjects: vec![
                        NestedSubject {
                            title: "mouse 1".into(),
                            values: vec![Some(10.0), Some(11.0), Some(12.0)],
                        },
                        NestedSubject {
                            title: "mouse 2".into(),
                            values: vec![Some(9.0), None, Some(13.0)],
                        },
                    ],
                },
                NestedGroup {
                    title: "Drug".into(),
                    subjects: vec![NestedSubject {
                        title: "mouse 3".into(),
                        values: vec![Some(20.0), Some(21.0)],
                    }],
                },
            ],
        }
    }

    #[test]
    fn nested_shape_and_counts() {
        let t = sample_nested();
        assert_eq!(t.kind(), TableKind::Nested);
        assert_eq!(t.width(), 2);
        assert_eq!(t.groups[0].observations(), 5); // 3 + 2 (one None)
        assert_eq!(t.groups[1].observations(), 2);
        assert_eq!(t.observations(), 7);
    }

    #[test]
    fn nested_replicates_skip_missing() {
        let t = sample_nested();
        let xs: Vec<f64> = t.replicates(0, 1).collect();
        assert_eq!(xs, vec![9.0, 13.0]);
        assert_eq!(t.replicates(9, 0).count(), 0);
    }

    #[test]
    fn nested_round_trip_as_any_table() {
        let t: AnyTable = sample_nested().into();
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"kind\":\"nested\""), "got {json}");
        let back: AnyTable = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }
}
