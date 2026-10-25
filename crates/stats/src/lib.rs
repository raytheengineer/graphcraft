//! L2 statistics: distributions, hypothesis tests, p-value adjustment.
//!
//! Behaviour reference is public documentation only; see `AGENTS.md`.

pub mod anova;
pub mod anova2;
pub mod bland_altman;
pub mod contingency;
pub mod correlation;
pub mod descriptive;
pub mod distributions;
pub mod mcomp;
pub mod nonpar;
pub mod normality;
pub mod quadrature;
pub mod roc;
pub mod special;
pub mod survival;
pub mod ttest;
pub use anova::{dunnett, one_way_anova, tukey_hsd, OneWayAnova, PairwiseComparison};
pub use anova2::{two_way_anova, AnovaEffect, TwoWayAnova};
pub use bland_altman::{bland_altman, BlandAltman};
pub use contingency::{chi_squared_independence, fisher_exact_2x2, ChiSquaredTest, FisherExact};
pub use correlation::{pearson, spearman, Correlation};
pub use descriptive::Descriptives;
pub use distributions::dunnett::DunnettSide;
pub use mcomp::{bonferroni, bonferroni_family, sidak, sidak_family};
pub use nonpar::{
    dunns_test, kruskal_wallis, mann_whitney, wilcoxon_signed_rank, DunnPair, KruskalWallis,
    MannWhitney, WilcoxonSignedRank,
};
pub use normality::{shapiro_wilk, ShapiroWilk};
pub use roc::{roc, RocAnalysis};
pub use survival::{log_rank, LogRank};
pub use ttest::{
    one_sample_t, paired_t, unpaired_t, OneSampleT, PairedT, TwoSampleKind, TwoSampleT,
};
