# GraphCraft Roadmap

Status headline (2026-10-25): **M1 scaffolding in progress.** Nothing is
usable yet. The workspace builds, `cargo xtask ci` runs, and one trivial
command exists end-to-end.

## Honest assessment (2026-10-25)

**Where we stand.** Pre-alpha scaffolding. No analyses, no plots, no file
format beyond a round-trippable stub.

**Where we're lacking (the gap list, prioritized).**
1. Numerical core: special functions (gamma, beta, erf), finite-float
   discipline helpers, deterministic RNG wiring. (M1→M2)
2. Table model: 8 Prism-style table types with subcolumns, missing values,
   row/column titles. (M2)
3. Descriptive stats + 1-sample / 2-sample location tests + 1-way ANOVA
   with post-hocs. (M3)
4. Linear regression + Levenberg-Marquardt nonlinear fit + published
   model library. (M4)
5. Core graph types: XY, Column, Grouped, Survival; SVG + PDF export. (M5)
6. Remaining analyses: 2-way/mixed/3-way ANOVA, contingency, ROC,
   Bland-Altman, survival stats, heatmaps. (M6)
7. UI, perf budgets, MCP, web build. (M7)

**Where we're going.** M1 next: parity harness that reads
`tests/parity-matrix.csv` and asserts one closed-form row, as the pattern
for everything after.

## Milestones

### M1 — Skeleton (in progress)
- [x] Workspace, licensing, docs, clippy no-panic lints
- [x] Crate skeletons (numeric / stats / regression / table / analysis /
      plot / render / format / engine / ui-egui), all compile
- [x] xtask: fmt, clippy, test, layers, cleanroom, assets, wasm, ci
- [x] `engine::guard` + one trivial command + command-registry test
- [x] `tests/parity-matrix.csv` seed row + harness
- [x] WASM gate wired in GitHub Actions CI (`.github/workflows/ci.yml`)
- [x] First CSV importer (`graphcraft_format::import_csv`,
      `graphcraft-cli import <csv> <out>`)
- [x] First real public-reference parity row (NIST StRD *Lew* mean,
      certified to 15 digits, passing at 1e-13)

### M2 — Data tables
- [x] M2.1 — XY table (`graphcraft_table::XyTable`, `YFormat`
      {SingleY, Replicates, MeanSd, MeanSem}, structural validation,
      JSON round-trip)
- [x] M2.2 — Column table proper: row-label metadata, group-value
      helpers, and a typed `AnyTable` container (`kind`-tagged serde
      enum) so `Document.tables` is `Vec<AnyTable>` instead of
      untyped JSON values.
- [x] M2.3 — Grouped (`GroupedTable`, `GroupColumn`, cell /
      replicates-at / replicate_n helpers, rectangular-within-group
      validation, `AnyTable::Grouped` round-trip)
- [x] M2.4 — Contingency (`ContingencyTable`, row-major u64 counts,
      row/col totals, validate, `AnyTable::Contingency` round-trip)
- [x] M2.5 — Survival (`SurvivalTable`, `SurvivalGroup`,
      `SurvivalRow { time, censored }`, event/censored counts,
      finite+non-negative time validation,
      `AnyTable::Survival` round-trip)
- [x] M2.6 — Parts-of-whole (`PartsOfWholeTable`),
      Multiple-variables (`MultipleVariablesTable`, typed numeric or
      categorical columns), and Nested (`NestedTable`,
      group → subject → replicates). All three with validation and
      `AnyTable` round-trip. M2 complete.
### M3 — Descriptive + t-tests + 1-way ANOVA + non-parametric
- [x] M3.0 — Descriptive statistics (`graphcraft_stats::Descriptives`
      with n/mean/sd/variance/sem/median/min/max, two-pass
      Kahan-compensated; `graphcraft_analysis::column_descriptives`
      wrapping it on a ColumnTable; NIST StRD *Lew* SD row added to
      the parity matrix, passing at 1e-13)
- [x] M3.1 — Student-t distribution (CDF/SF/InvCDF via regularized
      incomplete beta); `Descriptives::ci_mean(α)` 95% CI;
      `one_sample_t` + `column_one_sample_t`; Cauchy-closed-form
      parity row at df=1, p=0.975
- [x] M3.2 — Two-sample t: `paired_t`, `unpaired_t` with
      `TwoSampleKind::{Welch, Pooled}`; Welch-Satterthwaite df;
      `column_paired_t` / `column_unpaired_t` analysis wrappers;
      closed-form parity rows for all three variants
- [ ] M3.3 — One-way ANOVA + Tukey/Dunnett/Šidák/Bonferroni post-hocs
- [ ] M3.4 — Non-parametric: Mann-Whitney, Wilcoxon, Kruskal-Wallis,
      Dunn's post-hoc
### M4 — Linear + nonlinear regression (LM + published model library)
### M5 — Core graphs (XY, Column, Grouped, Survival) + SVG/PDF export
### M6 — Remaining analyses + graph types
### M7 — Polish, perf budgets, MCP, web build

## Parity table
To be filled in as analyses land. Each row grades our output against
published reference values, not against Prism.

| Analysis | Reference | Status | Max relative error |
| --- | --- | --- | --- |
| mean | NIST StRD *Lew* (univariate, n=200, certified ybar = -177.435000000000) | passing | 1e-13 |
| sd | NIST StRD *Lew* (certified s = 277.332168044316) | passing | 1e-13 |
| t-quantile | Cauchy (df=1) closed form: tan(π·0.475) = 12.706204736432095 | passing | 1e-10 |
| welch-t | Closed form on `[1..5]` vs `[10,12,14]`: `t = 9/√(11/6)`, `df = 484/137` | passing | 1e-13 |
| pooled-t | Closed form on same pair: `t = 9/√(8/5)`, `df = 6` | passing | 1e-13 |
| paired-t | Closed form on `[1..5]` vs `[2,4,...,10]`: `t = √18`, `df = 4` | passing | 1e-13 |

## Known numerical gaps
_(none logged yet)_
