//! L2 statistics: distributions, hypothesis tests, p-value adjustment.
//!
//! Behaviour reference is public documentation only; see `AGENTS.md`.

pub mod anova;
pub mod descriptive;
pub mod distributions;
pub mod mcomp;
pub mod quadrature;
pub mod special;
pub mod ttest;
pub use anova::{one_way_anova, tukey_hsd, OneWayAnova, PairwiseComparison};
pub use descriptive::Descriptives;
pub use mcomp::{bonferroni, bonferroni_family, sidak, sidak_family};
pub use ttest::{
    one_sample_t, paired_t, unpaired_t, OneSampleT, PairedT, TwoSampleKind, TwoSampleT,
};
