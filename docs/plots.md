# Plots

GraphCraft's plot layer is two crates:

- `graphcraft_plot` (L4): builds device-independent `Scene`
  descriptions from data tables. Units are points (1 pt = 1/72 in).
- `graphcraft_render` (L5): writes a scene to an output format.
  M5.1 ships SVG; M5.3 adds PDF.

Behaviour references are public — the plot vocabulary (title, axes,
ticks, series, legend, marker shapes, colour wheel) is standard
across every modern scientific plotting library.

## Scene model (M5.1)

```rust
Scene {
    width_pt, height_pt,       // canvas size (default 432×324, = 6×4.5 in)
    margins_pt,                // (top, right, bottom, left)
    title,                     // Option<String>
    x_axis, y_axis,            // Axis { title, min, max, ticks, scale }
    series,                    // Vec<Series::{Scatter, Line}>
    legend,                    // Option<Legend { position }>
}
```

Markers (`Circle`, `Square`, `Triangle`, `None`) carry fill, stroke,
and size. Colours ship a six-colour wheel from the public-domain
d3 category10 palette; `Color::pick(index)` cycles.

### Nice-number tick layout

`graphcraft_plot::nice_linear(lo, hi, target)` picks a tick step
from `{1, 2, 5} × 10^k` so the data range is covered by roughly
`target` ticks, and expands the min/max out to the nearest tick
(the "loose" variant). Based on Heckbert (1990), *Graphics Gems*
pp. 61–63.

## XY scatter builder (M5.1)

`xy_scene(table, dataset_idx, &XyOptions)` builds a `Scene` from
an `XyTable`'s dataset. Rows missing either x or y are dropped.
`xy_scene_from_points(&[(x,y)], &XyOptions, label)` is the slice-
based entry point.

`XyOptions` carries:
- `title`, `x_title`, `y_title`;
- `width_pt`, `height_pt`;
- `x_tick_target`, `y_tick_target`;
- `marker` style;
- `legend` (optional, with position).

## SVG renderer (M5.1)

`graphcraft_render::render_svg(&Scene) -> String` emits a
stand-alone SVG document using only `<rect>`, `<line>`, `<circle>`,
`<polygon>`, `<path>`, `<text>`, with inline attributes. No CSS,
no external resources. XML special characters in titles and labels
are escaped. The output passes the WASM gate — no file I/O below
L5.

## Column, Grouped, Survival builders (M5.2)

The scene `Series` enum now carries four more variants:
`Bars`, `GroupedBars`, `CensorTicks`, in addition to the M5.1
`Scatter` and `Line`. The renderer draws all five.

### Column — `column_scene(table, &ColumnOptions)`

One rectangular bar per non-empty column of a `ColumnTable`, with
height = sample mean, error bar = SD (default) or SEM. Column
titles label a categorical x-axis at integer positions
`1..=k`. Bars with `n < 2` are drawn with zero error.

### Grouped — `grouped_scene(table, &GroupedOptions)`

A grouped bar chart from a `GroupedTable`. Each subcolumn across
all groups becomes one `Series::GroupedBars`, so a 3-group /
2-subcolumn table produces 2 series with 3 bars each. Subcolumn
label becomes the legend entry; colour cycles through the public
colour wheel.

### Survival — `survival_scene(table, &SurvivalOptions)`

Kaplan–Meier curves (one `Series::Line` per group), with censored
observations rendered as separate `Series::CensorTicks`. The KM
estimator is in `km_estimate`: inputs are sorted by time, events
drive the step down, censored rows reduce `at_risk` but do not
step. Y-axis is pinned to `[0, 1]` with ticks at
`{0, 0.25, 0.5, 0.75, 1}`.

Reference (behaviour): Kaplan, E. L. & Meier, P. (1958) *JASA*
53, 457–481. Classical algorithm.

## PDF export (M5.3)

`graphcraft_render::render_pdf(&Scene) -> Vec<u8>` writes a PDF 1.4
single-page document from scratch — no third-party PDF crate.

The page uses one built-in font (Helvetica, one of the 14
"standard 14" fonts every PDF viewer carries by spec), so there is
nothing to embed. The same scene variants the SVG renderer draws
are supported in PDF: scatter markers (circles, squares, triangles
— circles via four cubic-Bezier arcs with κ = 0.5522847), lines,
bars, grouped bars, censor ticks, legend, axis titles, tick labels.

The PDF coordinate system has its origin at the bottom-left with
y increasing upward; the renderer flips y during projection so the
scene's data-space y still runs upward on the page.

Only ASCII characters appear in the generated text; higher
code-points are replaced with `?` for WinAnsiEncoding safety.
Characters `( )` and `\` in strings are escaped as `\( \) \\`.

Reference (behaviour): Adobe Systems (2006), *PDF Reference, sixth
edition, version 1.7*, §3 and §8. Classical page-description
model.

## Heatmap (M6.5)

`graphcraft_plot::heatmap_scene(data, &HeatmapOptions)` renders a
2D grid of coloured cells from a `HeatmapData { values, row_titles,
col_titles }`. `values` is a row-major `&[Vec<Option<f64>>]`;
`None` cells are drawn in neutral grey.

The `Series::Heatmap` scene variant carries the cells and the
labels along with the computed `(vmin, vmax)`. Three colormaps
ship, all with sampling via `colormap_sample(cm, v, vmin, vmax)`:

| Name | Low → High |
| --- | --- |
| `DivergingBlueRed` | blue → white → red (correlation heatmaps) |
| `Grayscale` | white → black |
| `SequentialBlue` | light-blue → mid-blue → dark-blue |

Both SVG and PDF renderers now draw `Series::Heatmap`.

## Deferred
- Multi-dataset XY overlays, mean±SD error bars, fit-line
  overlays.
- Log-scale axes.
- Visual-regression parity. Current parity is structural: tests
  assert the SVG / PDF contains the expected tick labels, axis
  titles, series elements, header, trailer, `%%EOF`, and that
  string escaping is correct.
- Non-ASCII text (would need an embedded TrueType font subset).
