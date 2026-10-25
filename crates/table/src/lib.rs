//! L2 tables: typed data tables for XY, Column, Grouped, Contingency,
//! Survival, Parts-of-whole, Multiple-variables and Nested layouts.
//!
//! For M1 only [`ColumnTable`] is implemented (titled columns of
//! optional f64 values — a one-replicate-per-cell Column table in
//! Prism's taxonomy). The other table kinds land in M2.

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
}
