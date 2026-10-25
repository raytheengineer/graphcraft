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

## Deferred
- Multi-dataset scatter overlays, error bars (M5.2).
- Column, Grouped, Survival builders (M5.2).
- PDF export (M5.3).
- Visual-regression parity. Current parity is structural: tests
  assert the SVG contains the expected tick labels, axis titles,
  and series elements.
