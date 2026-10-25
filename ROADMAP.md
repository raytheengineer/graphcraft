# GraphCraft Roadmap

Status headline (2026-10-25): **M3 + M4 complete, M5.1 shipped.**
Statistics, regression (OLS + LM + model library), and the first
scene builder + SVG renderer. The visual half of the project begins.
46 numeric parity rows live; graphs tested by structural invariants.

## Honest assessment (2026-10-25)

**Where we stand.** Statistics layer up through M3 is working: 29 parity
rows cover descriptives, t-tests, ANOVA + four post-hocs, and
non-parametric rank tests. Still ahead: regression (M4), graphs and
export (M5), the rest of the analysis catalogue (M6), and ship polish
(M7). No file format beyond a round-trippable stub.

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
- [x] M3.3a — F-distribution (`f_cdf`/`f_sf`/`f_inv_cdf`); one-way
      ANOVA (`one_way_anova`, `column_one_way_anova`) with the full
      ANOVA table; Bonferroni and Šidák p-value adjustments
      (`bonferroni`, `sidak`, `*_family`). Five new closed-form parity
      rows at 1e-13 or tighter.
- [x] M3.3b.1 — Normal distribution (`erf`/`phi`/`big_phi`/`phi_inv`
      via A&S 7.1.26); 32-point Gauss-Legendre quadrature primitive;
      studentized-range distribution via the standard integral form;
      Tukey HSD (Tukey-Kramer form) with `tukey_hsd` /
      `column_tukey_hsd`. Five new parity rows (two normal, one
      closed-form q, two Harter 1960 tabled)
- [x] M3.3b.2 — Dunnett distribution for balanced designs
      (equi-correlated multivariate t with ρ=1/2, one- and
      two-sided max). `dunnett_cdf` / `dunnett_inv_cdf`; `dunnett`
      post-hoc on OneWayAnova; `column_dunnett` wrapper. Four new
      parity rows (two closed form, two Dunnett 1964 tabled)
- [x] M3.4 — Non-parametric: `graphcraft_numeric::ranks_with_ties`;
      `inc_gamma_reg_p/q`; chi-squared distribution; Mann-Whitney U
      (`mann_whitney`), Wilcoxon signed-rank (`wilcoxon_signed_rank`),
      Kruskal-Wallis H (`kruskal_wallis`), Dunn's post-hoc
      (`dunns_test`); analysis-layer wrappers. Six new closed-form
      parity rows (chi-squared, Mann-Whitney, Wilcoxon, KW H and p).
      M3 complete.
### M4 — Linear + nonlinear regression (LM + published model library)
- [x] M4.1 — Linear OLS via Cholesky-normal-equations. Dense-matrix
      primitives (`DenseMatrix`, `cholesky`, `solve_spd`, `invert_spd`).
      Full OlsFit output (coefficients, SEs, t, p, R², adj R², residual
      SE, overall F). `simple_linear_regression` and `ols_fit`.
      Analysis wrappers `xy_simple_regression`, `column_simple_regression`.
      NIST StRD *Norris* parity: 6 rows at 1e-11 or tighter.
- [x] M4.2 — Levenberg-Marquardt nonlinear fitting via Marquardt's
      `diag(JᵀJ)` damping and central-difference Jacobian.
      `levenberg_marquardt(f, xs, ys, initial, opts)` →
      `LmFit { params, se, rss, residual_se, R², adj R², F, n_iter,
      converged, final_lambda, … }`. NIST StRD *Misra1a* parity:
      4 rows at 1e-5 or tighter (b1, b2, RSS, residual SD).
- [x] M4.3 — Published model library: `NonlinearModel` trait +
      `fit_model` dispatcher + six models (`OneSiteBinding`,
      `HillBinding`, `ExpDecayOnePhase`, `ExpGrowth`,
      `LogDoseResponse`, `Line`), each with a data-driven
      initial-estimate heuristic from the published literature.
      Early-step convergence check added to LM. 7 new closed-form
      synthetic parity rows at 1e-6 or tighter. M4 complete.
### M5 — Core graphs (XY, Column, Grouped, Survival) + SVG/PDF export
- [x] M5.1 — Scene model (Axis / Series / MarkerStyle / Color /
      Legend) at L4; nice-number tick layout via Heckbert (1990);
      XY scatter builder (`xy_scene`, `xy_scene_from_points`); SVG
      renderer at L5 (`render_svg`) emitting a standalone document
      with inline attributes only (passes the WASM gate). Structural
      tests verify ticks, titles, series elements, XML escaping.
- [x] M5.2 — Column (`column_scene`, SD/SEM error bars), Grouped
      (`grouped_scene`, subcolumn-per-series), and Survival
      (`survival_scene` with Kaplan–Meier via `km_estimate` and
      censor-tick overlays). Scene gets `Bars`, `GroupedBars`,
      `CensorTicks` series variants; renderer draws them.
- [ ] M5.3 — PDF export
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
| f-cdf | `F(1; 2, 2) = 0.5`; `F(1; 1, 1) = 0.5` | passing | 1e-13 |
| one-way-anova | Closed form on `[1,2,3]/[4,5,6]/[7,8,9]`: `F=27`, `SS=(54,6)` | passing | 1e-13 |
| bonferroni | Closed form `p·m` | passing | 1e-15 |
| sidak | Closed form `1−(1−p)^m` | passing | 1e-15 |
| normal-cdf | Φ(0)=0.5, Φ(1.959964)=0.975 | passing | 2e-7 (A&S bound) |
| studentized-range-cdf | `F(q;2,∞) = 2·Φ(q/√2)−1`; Harter 1960 crit values at (3,∞) and (3,10) | passing | 1e-5 closed / 2e-3 tabled |
| dunnett-one-sided-cdf | `m=1`: Φ(z); Dunnett 1964 crit value at (m=2, ν=∞) | passing | 1e-5 closed / 2e-3 tabled |
| dunnett-two-sided-cdf | `m=1`: 2Φ−1; Dunnett 1964 crit value at (m=2, ν=∞) | passing | 1e-5 closed / 2e-3 tabled |
| chi2-cdf / chi2-inv | `P(χ²₂≤2) = 1−e⁻¹`; `χ²₂, p=0.95 → 2·ln 20` | passing | 1e-13 / 1e-10 |
| mann-whitney | Closed form `U=0`, `Z=−4/√5.25` on `[1,2,3]` vs `[4,5,6]` | passing | 1e-13 |
| wilcoxon-signed-rank | Closed form `Z = 7/√13.75` on diffs `[1..5]` | passing | 1e-13 |
| kruskal-wallis | Closed form `H=7.2`, `p=e⁻³·⁶` on arithmetic groups | passing | 1e-12 |
| simple-linear-regression | NIST StRD *Norris* (B0, B1, SE B0, SE B1, R², residual SD) | passing | 1e-11 |
| misra1a (LM) | NIST StRD *Misra1a* (b1, b2, RSS, residual SD) | passing | 1e-5 |
| library-fit:* | Six models (one-site, Hill, exp-decay, log-dose-response, line) on synthetic exact data | passing | 1e-6 to 1e-12 |

## Known numerical gaps
_(none logged yet)_
