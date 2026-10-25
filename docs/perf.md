# Perf budgets

`cargo xtask perf` runs the `graphcraft-bench` harness: it builds
in release mode and runs each workload a fixed number of times,
printing the median wall-time. The output is advisory — a human
reads it to spot regressions. CI does **not** enforce absolute
wall-clock budgets, because hosted runners are too variable for
tight thresholds; CI's role for perf is a smoke test (the bench
compiles and runs).

## Target budgets (developer laptop, release build)

These are the numbers GraphCraft is written to meet on a modern
laptop. On faster machines the real numbers will be lower; on
slower ones a bit higher. A regression of more than ~2× against
these is the signal to investigate.

| Workload | Target | What it measures |
| --- | --- | --- |
| `ols/norris` | < 1 ms | small-regression baseline |
| `lm/misra1a` | < 1 ms | small-nonlinear LM fit |
| `xy/svg_1k` | < 5 ms | 1 000-point scatter → SVG string |
| `xy/pdf_1k` | < 10 ms | 1 000-point scatter → PDF bytes |
| `nonpar/kw_3x100` | < 1 ms | Kruskal-Wallis on 3×100 |
| `contingency/chi2_10x10` | < 1 ms | Chi-squared on 10×10 |
| `library/one_site_binding_fit` | < 1 ms | model-library LM on 12 points |

## Baseline (2026-10-25, this contributor's laptop)

```
workload                        median wall-time
ols/norris                        0.002 ms
lm/misra1a                        0.007 ms
xy/svg_1k                         1.185 ms
xy/pdf_1k                         3.157 ms
nonpar/kw_3x100                   0.003 ms
contingency/chi2_10x10            0.001 ms
library/one_site_binding_fit      0.001 ms
```

## Adding a new workload

1. Add a `bench_...` function to `apps/graphcraft-bench/src/main.rs`.
2. Register it in `main()` with a short, kebab-case label and a
   sensible iteration count (small workloads: 50; medium: 20;
   long: 5).
3. Call `std::hint::black_box` on the result so the optimiser
   can't elide it.
4. Add a line to the target-budgets table above.
