# GraphCraft Roadmap

Status headline (2026-10-25): **M3–M6 complete, M7.1 shipped.**
Statistics catalogue, regression stack, five graph types with SVG
and PDF export, and now a usable CLI that drives the whole
pipeline. 71 numeric parity rows. Perf budgets, MCP, and the web
build remain for M7.2–M7.4.

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
- [x] M5.3 — PDF export. Hand-written PDF 1.4 writer at L5
      (`render_pdf -> Vec<u8>`), no third-party PDF crate. Uses
      the built-in Helvetica font (no embedding), four-Bezier
      circle approximation for scatter markers, string escaping
      for `( ) \`. Supports every scene variant the SVG renderer
      does. Structural tests: header, `xref` + `trailer` + `%%EOF`,
      parentheses escape. M5 complete.
### M6 — Remaining analyses + graph types
- [x] M6.1 — Pearson & Spearman correlation with t-transform p
      and Fisher-z CI; chi-squared independence test on an
      `r × c` table; Fisher's exact test for 2×2 via the
      hypergeometric distribution (two-sided by point-probability
      rule). Analysis wrappers `column_pearson`, `column_spearman`,
      `table_chi_squared`, `table_fisher_exact`. Eight new
      closed-form parity rows.
- [x] M6.2 — ROC analysis (AUC via normalised Mann-Whitney U,
      Hanley-McNeil SE, CI95, step curve, null-z p); Bland-Altman
      agreement (bias, SD of differences, limits of agreement, CI
      on bias, (mean, diff) points). Five new closed-form parity
      rows.
- [x] M6.3 — Two-way (balanced) fixed-effects ANOVA with the full
      table (SS_A, SS_B, SS_AB, SS_within; df; MS; F; p for each
      effect). Seven new closed-form parity rows on a 2×2
      factorial with n=2 per cell. Unbalanced Type-III SS and
      RM-ANOVA tracked as follow-ups.
- [x] M6.4 — Two-group log-rank test on `(time, censored)` tuples
      with the Mantel-Haenszel log-hazard-ratio point estimate and
      95% CI. `log_rank` in stats, `table_log_rank` wrapper over
      `SurvivalTable` in analysis. Four new closed-form parity
      rows. `k > 2` groups and stratified / weighted log-rank
      tracked as follow-ups.
- [x] M6.5 — Heatmap scene builder (`heatmap_scene` with three
      colormaps: DivergingBlueRed, Grayscale, SequentialBlue);
      `Series::Heatmap` variant drawn by both SVG and PDF
      renderers; `None` cells rendered neutral grey. M6 near-
      complete; the only remaining open item on M6 is a dedicated
      normality-tests slice (Shapiro-Wilk via Royston 1992).
- [x] M6.6 — Shapiro-Wilk normality via Royston (1992)
      polynomial approximation (4 ≤ n ≤ 2000). `shapiro_wilk(xs)`
      returns `W`, `z`, and upper-tail `p`. Six unit tests plus a
      parity row against Shapiro & Wilk (1965) Table 2
      (`W ≈ 0.79`). M6 complete.
### M7 — Polish, perf budgets, MCP, web build
- [x] M7.1 — CLI integration. `graphcraft-cli` grows a full
      analysis + export surface: `describe`, `anova`,
      `ttest-{unpaired,paired}`, `correlate`, `regress`, `chi2`
      (CSV → JSON), and `plot {xy,column} <csv> ... <out.svg|.pdf>`
      (format chosen by extension). Column args accept 0-based
      index or header-name match. Smoke-tested end-to-end:
      CSV → JSON, SVG, PDF.
- [ ] M7.2 — Perf budget harness
- [ ] M7.3 — MCP (Model Context Protocol) stdio wiring
- [ ] M7.4 — Web build (WASM engine in a real page)

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
| pearson / spearman | Closed form `r = 6/√60`, `r = 1` on exact / monotone | passing | 1e-13 |
| chi2-indep | Closed form 2×2 `X² = 50/3`, `df = 1` | passing | 1e-13 |
| fisher-exact | Fisher 1922 lady-tasting-tea `p = 2/70`; OR = 9 | passing | 1e-13 |
| roc | AUC = 1 / 0 / 0.5 on perfect / inverted / hand-counted case | passing | 1e-14 |
| bland-altman | Closed-form bias = 1.2, SD = √0.7 | passing | 1e-13 |
| two-way-anova | 2×2 factorial, n=2: `SS_A=18, SS_B=72, SS_AB=8, SS_w=8`; `F=9,36,4` | passing | 1e-13 |
| log-rank | `[1,1]` vs `[2,2]` → `χ²=3`, `O−E=1`, `V=1/3`; identical groups → `χ²=0` | passing | 1e-13 |
| shapiro-wilk | Shapiro & Wilk (1965) Table 2 → `W ≈ 0.79` | passing | 1e-2 (published 3-sf precision) |

## Known numerical gaps
_(none logged yet)_
