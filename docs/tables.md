# Data tables

GraphCraft reimplements GraphPad Prism's eight table layouts in
`graphcraft_table` (L2). This page records the data-shape decisions
and points to the public documentation each layout is modelled on.

Behaviour is from public documentation only; see `AGENTS.md` ›
Clean-room. Contributor-authored notes from the manual live in the
gitignored `plan/prism/` directory and reference specific manual pages
by URL.

## Status

| Layout                | Implemented | Rust type                     |
| --------------------- | ----------- | ----------------------------- |
| Column                | ✅ (M1.2, M2.2 row labels) | `ColumnTable`     |
| XY                    | ✅ (M2.1)   | `XyTable`                     |
| Grouped               | ✅ (M2.3)   | `GroupedTable`                |
| Contingency           | ✅ (M2.4)   | `ContingencyTable`            |
| Survival              | ✅ (M2.5)   | `SurvivalTable`               |
| Parts-of-whole        | planned     | —                             |
| Multiple-variables    | planned     | —                             |
| Nested                | planned     | —                             |

Reference: GraphPad Prism public manual,
<https://www.graphpad.com/guides/prism/latest/user-guide/>.

## XY tables (`XyTable`)

An XY table pairs a single shared X column with one or more Y
datasets. Each Y dataset chooses how its subcolumns are interpreted
via a `YFormat` tag:

| `YFormat`                      | Subcolumns                | Meaning                                 |
| ------------------------------ | ------------------------- | --------------------------------------- |
| `SingleY`                      | 1                         | One Y value per row.                    |
| `Replicates`                   | ≥ 1 (variable)            | Raw observations, side-by-side.         |
| `MeanSd { with_n: false }`     | 2: mean, SD               | Mean + sample standard deviation.       |
| `MeanSd { with_n: true }`      | 3: mean, SD, N            | As above, with the replicate count.     |
| `MeanSem { with_n: false }`    | 2: mean, SEM              | Mean + standard error of the mean.      |
| `MeanSem { with_n: true }`     | 3: mean, SEM, N           | As above, with the replicate count.     |

Validation rules enforced by `XyTable::validate`:

- Subcolumn count must match the format tag where the tag is fixed.
  `Replicates` requires at least one subcolumn.
- No subcolumn may be longer than the X column. Shorter subcolumns are
  allowed and represent trailing missing cells.

Error-entered formats that aren't in this first pass (Mean + 95% CI as
two subcolumns, Mean + Range, geometric-mean variants) will land as
later M2 steps or as part of the specific analysis that consumes them,
with each format cited to its manual page.

## Column tables (`ColumnTable`)

Each column is one group; its cells are the group's replicate
observations top-down. Columns may be ragged (one group may have more
replicates than another). `None` cells represent missing values and
are skipped by analyses.

- `row_titles: Vec<String>` — optional per-row labels. Empty means
  "address rows by their 1-based index"; otherwise the length must
  match the longest column (`row_titles_valid()`).
- `group_values(col)` — iterator of non-missing `f64` for one group.
- `group_n(col)` — replicate count after missing-value filtering.

The subcolumn-per-group concept belongs to Grouped tables (M2.3), not
Column tables. One sample value per cell is correct for a Column
table.

## Grouped tables (`GroupedTable`)

Rows × side-by-side groups × replicates within each group. One row
is one condition (e.g. "Day 1"); one group is one factor level
(e.g. "Control"); the subcolumns within a group are the replicate
observations at that row for that group. Powers two-way and
three-way ANOVA, and the Grouped bar/scatter plots.

- Rectangular within each group — all subcolumns of one group must
  have the same length. Missing observations are `None` cells, not
  shorter subcolumns.
- Different groups may hold different replicate counts — that's the
  whole point of the layout.
- Helpers: `cell(row, group, replicate)` (optional f64),
  `replicates_at(row, group)` (iterator over non-missing f64),
  `replicate_n(row, group)`.
- Validation (`validate()`) rejects ragged groups and
  `row_titles`-length mismatches.

## Contingency tables (`ContingencyTable`)

A small rectangular matrix of non-negative integer counts
(`Vec<Vec<u64>>`, row-major), with optional row and column titles.
Powers chi-square (independence and goodness-of-fit), Fisher's exact
test, and relative-risk / odds-ratio analyses.

- `cell(row, col)`, `row_total(row)`, `col_total(col)`, `total()`.
- `validate()` rejects ragged rows, title-length mismatches, and
  the degenerate case of a zero grand total.
- Counts are `u64` (not `f64`) so a value error at import time is a
  structural rejection rather than a silent reinterpretation.

## Survival tables (`SurvivalTable`)

One group per treatment arm, one row per subject. Each row records
a non-negative `time` and a boolean `censored` flag:
`censored = false` means the event (death, failure, …) occurred at
`time`; `censored = true` means the subject exited observation
without the event (lost to follow-up, still alive at study end).
Rows are not required to be sorted — Kaplan-Meier estimation will
sort by time at analysis time.

- `width()` groups, `subjects()` total rows across groups.
- `SurvivalGroup::events()` and `.censored()` count by category.
- `validate()` rejects non-finite or negative times.

## Typed container (`AnyTable`)

Documents store tables as `Vec<AnyTable>` with a serde `"kind"`
discriminator (`"column"`, `"xy"`, …). Round-trip tested at the
document level in `graphcraft_format`.
