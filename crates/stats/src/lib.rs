//! L2 statistics: distributions, hypothesis tests, p-value adjustment.
//!
//! Behaviour reference is public documentation only; see `AGENTS.md`.

pub mod anova;
pub mod descriptive;
pub mod distributions;
pub mod mcomp;
pub mod nonpar;
pub mod quadrature;
pub mod special;
pub mod ttest;
pub use anova::{dunnett, one_way_anova, tukey_hsd, OneWayAnova, PairwiseComparison};
pub use descriptive::Descriptives;
pub use distributions::dunnett::DunnettSide;
pub use mcomp::{bonferroni, bonferroni_family, sidak, sidak_family};
pub use nonpar::{
    dunns_test, kruskal_wallis, mann_whitney, wilcoxon_signed_rank, DunnPair, KruskalWallis,
    MannWhitney, WilcoxonSignedRank,
};
pub use ttest::{
    one_sample_t, paired_t, unpaired_t, OneSampleT, PairedT, TwoSampleKind, TwoSampleT,
};
