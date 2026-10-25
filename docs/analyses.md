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
| Paired t-test                 | `paired_t`           | `column_paired_t`      | Closed form: `sqrt(18)` |
| Welch's unpaired t-test       | `unpaired_t(Welch)`  | `column_unpaired_t`    | Closed form: `9/sqrt(11/6)`, `df = 484/137` |
| Pooled unpaired t-test        | `unpaired_t(Pooled)` | `column_unpaired_t`    | Closed form: `9/sqrt(8/5)`, `df = 6` |
| F-distribution                | `distributions::f::{f_cdf, f_sf, f_inv_cdf}` | — | `F(1;2,2) = 0.5` and `F(1;1,1) = 0.5` |
| One-way ANOVA                 | `one_way_anova`      | `column_one_way_anova` | Closed form: `F = 27, SS = (54, 6)` |
| Bonferroni adjustment         | `bonferroni` / `bonferroni_family` | — | Closed form: `p·m` |
| Šidák adjustment              | `sidak` / `sidak_family` | — | Closed form: `1 − (1 − p)^m` |
| Normal distribution           | `distributions::normal::{phi, big_phi, phi_inv, erf}` | — | A&S 7.1.26 (≈1.5·10⁻⁷) |
| Studentized-range distribution | `distributions::studentized_range::{q_cdf, q_sf, q_inv_cdf}` | — | `F(q;2,∞) = 2·Φ(q/√2)−1`; Harter (1960) tabled |
| Tukey HSD post-hoc            | `tukey_hsd`          | `column_tukey_hsd`     | Verified via q-dist parity |
| Dunnett distribution (ρ=1/2) | `distributions::dunnett::{dunnett_cdf, dunnett_inv_cdf}` | — | Dunnett (1964) tabled crit values |
| Dunnett many-to-one post-hoc  | `dunnett`            | `column_dunnett`       | Verified via Dunnett-dist parity |

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
- F-distribution — same incomplete-beta backbone — lands with ANOVA
  in M3.3.

## Two-sample t-tests (M3.2)

`graphcraft_stats::ttest` now ships three two-sample flavours.

- `paired_t(xs, ys)` — pairs by index, then runs the one-sample t on
  the differences `ys[i] − xs[i]` against `μ = 0`. `Err` on length
  mismatch, `n < 2`, zero variance of differences, or non-finite
  values.
- `unpaired_t(xs, ys, TwoSampleKind::Welch)` — **Prism's documented
  default** for unpaired comparisons. Standard error of the mean
  difference is `√(s₁²/n₁ + s₂²/n₂)`; df uses the Welch–Satterthwaite
  approximation
  `df = (s₁²/n₁ + s₂²/n₂)² / ((s₁²/n₁)²/(n₁−1) + (s₂²/n₂)²/(n₂−1))`,
  giving a real-valued df. The Student-t CDF already accepts real
  `df`, so no extra plumbing was needed.
- `unpaired_t(xs, ys, TwoSampleKind::Pooled)` — classic equal-variance
  Student form; pooled variance
  `s_p² = ((n₁−1)s₁² + (n₂−1)s₂²) / (n₁+n₂−2)` and
  `df = n₁+n₂−2`.

All three set `mean_diff = mean(ys) − mean(xs)` so a positive sign
means "second group is larger".

Analysis-layer wrappers on `ColumnTable`:
- `column_unpaired_t(table, col_a, col_b, kind)` — picks the two
  columns by index, drops missings per column.
- `column_paired_t(table, col_a, col_b)` — pairs by row index, drops
  rows where either column is missing.

Behaviour references (public):
- Welch, B. L. (1947) *Biometrika* 34, 28–35.
- Satterthwaite, F. E. (1946) *Biometrics* 2, 110–114.
- Student (1908) *Biometrika* 6, 1–25.

Parity rows (closed-form, 1e-13):
- `welch-t` on `[1,2,3,4,5]` vs `[10,12,14]` → `t = 9/√(11/6)`,
  `df = 484/137`.
- `pooled-t` on the same pair → `t = 9/√(8/5)`, `df = 6`.
- `paired-t` on `[1,2,3,4,5]` vs `[2,4,6,8,10]` → `t = √18`, `df = 4`.

Deferred to M3.2+:
- One-way ANOVA and the F-distribution (M3.3).

## F-distribution, one-way ANOVA, and p-value adjustments (M3.3)

`graphcraft_stats::distributions::f` adds the Fisher-Snedecor F
distribution on real-valued df, computed from the same
`inc_beta_reg` backbone:

- `f_cdf(x, d1, d2) = I_u(d1/2, d2/2)` with `u = d1·x / (d1·x + d2)`.
- `f_sf` is the complement; `f_inv_cdf` uses the same bisection
  pattern as the Student-t quantile.

`graphcraft_stats::anova::one_way_anova(groups)` produces the
textbook one-way ANOVA table:

- Grand mean weighted by group `n`.
- `SS_between = Σ n_i · (ȳ_i − ȳ)²`.
- `SS_within  = Σ (n_i − 1) · s_i²` (groups with `n = 1` contribute
  zero).
- `df = (k − 1, N − k)`, `MS = SS/df`, `F = MS_between/MS_within`,
  right-tailed p from `f_sf`.
- `Err(DivisionByZero)` when every group is a single constant
  (within-group variance is zero and F is undefined).

Analysis API: `column_one_way_anova(&ColumnTable)` runs ANOVA across
all non-empty columns, preserving column order in `group_ns` /
`group_means`.

`graphcraft_stats::mcomp` adds two closed-form family-wise error
adjustments:

- `bonferroni(p, m) = min(1, p·m)`.
- `sidak(p, m) = 1 − (1 − p)^m`.
- `bonferroni_family(&[f64])` / `sidak_family(&[f64])` run the
  adjustment across a slice of raw p-values with `m = ps.len()`.

Parity rows:

- `f-cdf` at `(d1=2, d2=2, x=1)` → `0.5` (closed form via
  `I_{0.5}(1, 1) = 0.5`), passing at 1e-14.
- `f-cdf` at `(d1=1, d2=1, x=1)` → `0.5` (arcsine-distribution
  symmetry), passing at 1e-13.
- `one-way-anova` on `[1,2,3] / [4,5,6] / [7,8,9]`: `F = 27`,
  `SS_between = 54`, `SS_within = 6` — all closed form at 1e-13.
- `bonferroni(0.01, 5) = 0.05` at 1e-15.
- `sidak(0.05, 2) = 0.0975` at 1e-15.

Deferred to M3.3+:
- Dunnett's test — needs the multivariate t-distribution. Shipping
  in M3.3b.2 (next slice).
- Two-way and mixed ANOVA — scheduled with M6 remaining analyses.

## Normal distribution, studentized-range, and Tukey HSD (M3.3b.1)

`graphcraft_stats::distributions::normal` ships `erf`, `phi`,
`big_phi` (Φ), and `phi_inv` (Φ⁻¹). `erf` uses the Abramowitz &
Stegun 7.1.26 rational approximation with max absolute error
≈ 1.5·10⁻⁷; that is sufficient for the post-hoc critical-value
tables (3 dp) and for M3.4 large-sample approximations, and will
be upgraded to full double precision when a parity row needs it.
Φ⁻¹ bisects on Φ.

`graphcraft_stats::quadrature` ships the 32-point Gauss–Legendre
rule on `[−1, 1]`, with nodes and weights from the public A&S §25.4
tables, and a convenience `gl32_integrate(a, b, f)`.

`graphcraft_stats::distributions::studentized_range` ships
`q_cdf(q, k, df)`, `q_sf`, and `q_inv_cdf` for the Tukey q
distribution. For `df = ∞` the CDF uses the standard identity
`F(q; k, ∞) = k · ∫ φ(z) · [Φ(z+q) − Φ(z)]^{k−1} dz` evaluated by
Gauss–Legendre over `z ∈ [−8, 8]`. For finite df the finite-ν
integral weights the asymptotic form by the chi density of
`W = √(χ²_ν/ν)`.

`graphcraft_stats::anova::tukey_hsd(&OneWayAnova)` produces all
`k·(k−1)/2` pairwise comparisons with the Tukey–Kramer harmonic-mean
adjustment for unequal `n_i`:
- `n_h = 2 / (1/n_i + 1/n_j)`
- `se = √(MS_within / n_h)`
- `q = |mean_diff| / se`
- `p_adj = P(Q ≥ q; k, df_within)`.

Analysis API: `column_tukey_hsd(&ColumnTable)` runs ANOVA across the
non-empty columns and returns pairwise comparisons in column order.

Behaviour references (public):
- Lund, R. E. & Lund, J. R. (1983) Algorithm AS 190, Applied
  Statistics 32, 204–210.
- Harter, H. L. (1960) *Annals of Mathematical Statistics* 31,
  1122–1147 (critical-value tables, used only as public parity
  oracle, not for implementation).
- Tukey, J. W. (1953) "The problem of multiple comparisons",
  unpublished Princeton manuscript; see Kramer, C. Y. (1956)
  *Biometrics* 12, 307–310 for the harmonic-mean extension.

Parity rows (new):
- `normal-cdf` at `x = 0` → `0.5` at 2·10⁻⁷ (A&S bound).
- `normal-cdf` at `x = Φ⁻¹(0.975) ≈ 1.959964` → `0.975` at 2·10⁻⁷.
- `studentized-range-cdf` closed form at `(q = √2·Φ⁻¹(0.975),
  k = 2, df = ∞)` → `0.95` at 1·10⁻⁵.
- `studentized-range-cdf` at Harter's `q_{0.05}(3, ∞) = 3.3145` →
  `0.95` at 2·10⁻³.
- `studentized-range-cdf` at Harter's `q_{0.05}(3, 10) = 3.877` →
  `0.95` at 2·10⁻³.

Deferred to M3.3b+:
- A higher-precision `erf` to tighten the normal / q / Dunnett
  parity rows below ~10⁻⁷ when a downstream test needs it.

## Dunnett many-to-one post-hoc (M3.3b.2)

`graphcraft_stats::distributions::dunnett` ships Dunnett's
equi-correlated max-|T| and max-T distributions for a balanced
one-way design with correlation `ρ = 1/2`:

- `dunnett_cdf(d, m, df, side)` → `P(max_i T_i < d)` (one-sided) or
  `P(max_i |T_i| < d)` (two-sided), with `m = k − 1` treatment
  groups and real `df > 0` (or `df = ∞`).
- `dunnett_inv_cdf(p, m, df, side)` → critical value by bisection.

The ρ = 1/2 structure collapses the joint CDF to a single
conditioning integral over `W = √(χ²_ν/ν)` and an inner integral
over the shared normal factor. Both are 32-point Gauss–Legendre.

`graphcraft_stats::anova::dunnett(anova, control_idx, side)` runs
the many-to-one post-hoc on top of a one-way ANOVA, returning one
`PairwiseComparison` per non-control group. For the balanced design
the correlations are exactly `1/2`; for unbalanced designs the
reported p-values are in the usual conservative direction and a
full unbalanced-Dunnett is tracked as a follow-up.

Analysis API: `column_dunnett(table, control_col, side)` maps the
original column index to the ANOVA index (skipping empty columns)
and runs the test.

Behaviour references (public):
- Dunnett, C. W. (1955) "A multiple comparison procedure for
  comparing several treatments with a control", JASA 50, 1096–1121.
- Dunnett, C. W. (1964) "New tables for multiple comparisons with
  a control", Biometrics 20, 482–491 (tabled critical values used
  as parity oracle, not for implementation).

Parity rows (new):
- `dunnett-one-sided-cdf` closed form at `(d = Φ⁻¹(0.95), m = 1,
  df = ∞)` → `0.95` at 1·10⁻⁵.
- `dunnett-one-sided-cdf` at Dunnett's `d_{0.05}(2, ∞) = 1.916` →
  `0.95` at 2·10⁻³.
- `dunnett-two-sided-cdf` closed form at `(d = Φ⁻¹(0.975), m = 1,
  df = ∞)` → `0.95` at 1·10⁻⁵.
- `dunnett-two-sided-cdf` at Dunnett's `d_{0.05}(2, ∞) = 2.212` →
  `0.95` at 2·10⁻³.

Deferred to M3.3b+:
- Full unbalanced Dunnett (Hochberg or Hsu generalisation).
- Hsu's MCB (multiple comparisons with the best) and other
  Prism-documented post-hocs; scheduled with the remainder of M6.
