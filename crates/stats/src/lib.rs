//! L2 statistics: distributions, hypothesis tests, p-value adjustment.
//!
//! Behaviour reference is public documentation only; see `AGENTS.md`.

pub mod descriptive;
pub mod distributions;
pub mod special;
pub mod ttest;
pub use descriptive::Descriptives;
pub use ttest::{one_sample_t, OneSampleT};
