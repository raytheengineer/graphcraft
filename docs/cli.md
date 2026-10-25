# graphcraft-cli

Headless entry point. Analyses read a CSV and print JSON on stdout;
plot commands write SVG or PDF based on the output file's extension.

## Build

```
cargo build --release -p graphcraft-cli
./target/release/graphcraft-cli --help
```

## Engine / file-io

| Command | Purpose |
| --- | --- |
| `commands` | List registered engine commands |
| `run <id>` | Run one command on an empty session |
| `new <path>` | Write an empty `.graphcraft` document |
| `import <csv> <graphcraft>` | Import a CSV into a `.graphcraft` document |

## Analyses

Each prints a pretty-printed JSON object on stdout.

| Command | Reports |
| --- | --- |
| `describe <csv>` | per-column `n`, `mean`, `sd`, `variance`, `sem`, `median`, `min`, `max` |
| `anova <csv>` | one-way `k`, SS, df, F, p across all non-empty columns |
| `ttest-unpaired <csv> <a> <b>` | Welch's two-sample t: means, `mean_diff`, SE, `t`, `df`, `p_two_sided` |
| `ttest-paired <csv> <a> <b>` | paired t: `n`, `mean_diff`, `sem_diff`, `t`, `df`, `p_two_sided` |
| `correlate <csv> <a> <b>` | Pearson and Spearman `r`, `t`, `df`, `p_two_sided`, Pearson CI95 |
| `regress <csv> <x> <y>` | OLS: `intercept`, `slope`, their SE/t/p, `rss`, R², adj R², residual SE, overall F |
| `chi2 <csv>` | Chi-squared independence (CSV is treated as a non-negative-integer count table) |

Column references are either a 0-based index or the exact header
name. Example:

```
graphcraft-cli ttest-unpaired data.csv ctrl drug
graphcraft-cli regress data.csv 0 1
```

## Plots

The output file's extension picks the format: `.svg` → inline
SVG, `.pdf` → the hand-written PDF 1.4 writer. No external
rendering library is involved.

| Command | Draws |
| --- | --- |
| `plot xy <csv> <x-col> <y-col> <out.svg|pdf>` | XY scatter |
| `plot column <csv> <out.svg|pdf>` | one bar per column with SD error bars |

## Example session

```
$ cat > data.csv <<EOF
ctrl,drug
1,5
2,9
3,10
EOF

$ graphcraft-cli describe data.csv
{
  "descriptives": [
    { "group": "ctrl", "n": 3, "mean": 2.0, "sd": 1.0, ... },
    { "group": "drug", "n": 3, "mean": 8.0, "sd": 2.646..., ... }
  ]
}

$ graphcraft-cli ttest-unpaired data.csv ctrl drug
{
  "ttest_welch": {
    "mean_diff": 6.0, "t": 3.674..., "df": 2.56,
    "p_two_sided": 0.0453...
  }
}

$ graphcraft-cli plot column data.csv out.pdf
wrote out.pdf
```
