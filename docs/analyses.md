# Analyses

GraphCraft's analysis layer lives in `graphcraft_analysis` (L4) and
rides on primitives in `graphcraft_stats` (L2) and
`graphcraft_regression` (L3). Each analysis has:

- A `graphcraft_stats` or `graphcraft_regression` primitive that
  takes `&[f64]` (or similar) and returns a `Result`;
- A `graphcraft_analysis` wrapper that takes a table and returns a
  structured result;
- One or more rows in `tests/parity-matrix.csv` backed by a public
  reference dataset (NIST StRD, textbook worked example, …);
- A docs entry here with the behaviour decisions and manual URL.

Behaviour reference is public documentation only — see `AGENTS.md`.

## Status

| Analysis                      | Primitive            | Analysis API           | Parity row |
| ----------------------------- | -------------------- | ---------------------- | ---------- |
| Descriptive statistics        | `Descriptives::of`   | `column_descriptives`  | NIST StRD *Lew* mean + SD |

## Descriptive statistics (M3.0)

`Descriptives::of(&[f64]) -> Result<Descriptives, NumericError>`
returns `n`, `mean`, `sd`, `variance`, `sem`, `median`, `min`, `max`.
Dispersion fields are `None` when `n < 2`; dispersion uses the
sample denominator `n - 1`, matching the Prism manual's default for
"Column statistics".

Implementation notes:

- Mean is Kahan-compensated (`graphcraft_numeric::kahan_sum`).
- Variance is a two-pass sum of squared deviations, also
  Kahan-compensated. Two-pass is numerically fine for well-scaled
  data; a Welford switch is on the table if a future parity row
  demands it.
- Median linearly interpolates on even-length samples (the Prism
  default; textbook convention).

Analysis API: `column_descriptives(&ColumnTable)` returns one row
per group with a non-missing observation; empty groups are skipped.
Non-finite values in any group return `Err` for the whole call.

Parity rows:

- `mean` on NIST StRD *Lew* — certified `ybar = -177.435000000000`,
  passing at 1e-13 (ships since M1.3).
- `sd` on NIST StRD *Lew* — certified
  `s = 277.332168044316`, passing at 1e-13.

Deferred to M3.0+:
- 95% confidence interval of the mean — requires the Student-t
  quantile, which lands with the t-tests in M3.1.
- Skewness and kurtosis.
- Geometric mean and coefficient of variation.
- Shapiro-Wilk / D'Agostino / KS normality tests — M3 later.
