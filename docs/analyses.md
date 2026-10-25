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
| 95% CI of the mean            | `Descriptives::ci_mean(α)` | (via descriptives) | — |
| Student-t distribution        | `distributions::t::{t_cdf, t_sf, t_inv_cdf}` | — | Cauchy quantile at df=1, p=0.975 |
| One-sample t-test             | `one_sample_t`       | `column_one_sample_t`  | — |

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
- Skewness and kurtosis.
- Geometric mean and coefficient of variation.
- Shapiro-Wilk / D'Agostino / KS normality tests — M3 later.

## Student-t distribution and one-sample t-test (M3.1)

`graphcraft_stats::special` ships three building blocks:

- `ln_gamma` via the Lanczos approximation (g = 7, 9 coefficients).
- `ln_beta(a, b) = ln Γ(a) + ln Γ(b) − ln Γ(a+b)`.
- `inc_beta_reg(x, a, b)` — the regularized incomplete beta
  `I_x(a, b)`, computed from Lentz's continued-fraction form of
  `B(x; a, b)` with the standard `x > (a+1)/(a+b+2)` reflection
  for faster convergence.

`graphcraft_stats::distributions::t` then provides:

- `t_cdf(t, df)` via `F(t; ν) = 1 − 0.5 · I_{ν/(ν+t²)}(ν/2, 1/2)`
  for `t ≥ 0` (symmetry for `t < 0`). ν is real-valued.
- `t_sf(t, df) = 1 − F(t)`.
- `t_inv_cdf(p, df)` via bisection on the monotone CDF; converges to
  double precision in ≤ 100 iterations over the sensible range.

`Descriptives::ci_mean(α)` returns a two-sided `(1 − α)` CI using the
Student-t quantile with `n − 1` df. `None` when `n < 2` or `α` is not
in `(0, 1)`.

`graphcraft_stats::one_sample_t(xs, μ₀)` runs the standard one-sample
test: `t = (mean − μ₀) / sem`, `df = n − 1`, two-sided
`p = 2 · (1 − F(|t|; df))`. `Err` on `n < 2`, zero sample variance,
or non-finite input.
`graphcraft_analysis::column_one_sample_t(&ColumnTable, μ₀)` applies
it per non-empty column.

Parity rows:

- `t-quantile` with `df=1, p=0.975` → `12.706204736432095`,
  passing at 1e-10. The expected value is the Cauchy-distribution
  identity: at ν = 1 Student's t is the standard Cauchy, so
  `F⁻¹(p) = tan(π(p − 1/2))`. Cited to the Student's t-distribution
  Wikipedia article.

Deferred to M3.1+:
- Welch and paired two-sample t (M3.2).
- F-distribution — same incomplete-beta backbone — lands with ANOVA
  in M3.3.
