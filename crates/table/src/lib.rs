//! L2 tables: typed data tables for XY, Column, Grouped, Contingency,
//! Survival, Parts-of-whole, Multiple-variables and Nested layouts.
//!
//! Minimal skeleton for M1 — just the enum of table kinds.

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
