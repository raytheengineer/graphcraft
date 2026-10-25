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
| Grouped               | planned     | —                             |
| Contingency           | planned     | —                             |
| Survival              | planned     | —                             |
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

## Typed container (`AnyTable`)

Documents store tables as `Vec<AnyTable>` with a serde `"kind"`
discriminator (`"column"`, `"xy"`, …). Round-trip tested at the
document level in `graphcraft_format`.
