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

### M2 — Data tables
### M3 — Descriptive + t-tests + 1-way ANOVA + non-parametric
### M4 — Linear + nonlinear regression (LM + published model library)
### M5 — Core graphs (XY, Column, Grouped, Survival) + SVG/PDF export
### M6 — Remaining analyses + graph types
### M7 — Polish, perf budgets, MCP, web build

## Parity table
To be filled in as analyses land. Each row grades our output against
published reference values, not against Prism.

| Analysis | Reference | Status | Max relative error |
| --- | --- | --- | --- |
| _(none yet)_ | | | |

## Known numerical gaps
_(none logged yet)_
