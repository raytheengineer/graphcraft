# GraphCraft architecture

## Crate layering (enforced by `cargo xtask layers`)

```
L1  numeric       — finite-float helpers, special functions, deterministic RNG
L2  stats         — distributions, hypothesis tests, p-value adjustment
    table         — typed tables (XY/Column/Grouped/...), subcolumns, missing
L3  regression    — linear OLS, Levenberg-Marquardt nonlinear fit, model library
L4  analysis      — analysis definitions, inputs, options, results tree
    plot          — scene graph for graphs (axes, series, legends, annotations)
    format        — .graphcraft read/write, CSV import (and later Excel, native-only)
L5  render        — tiny-skia rasterizer, SVG/PDF writer
    engine        — commands, undo, document, guard, control channel
L6  ui-egui       — panels, menus, theme tokens
    apps/*        — binaries
```

Rules:

- A crate at level `N` may depend only on crates at levels `<= N` (plus
  external deps). Cycles are prevented by Cargo itself; the layer rule
  prevents a lower-level crate from reaching up into a higher-level one.
  `cargo xtask layers` enforces this.
- No crate below L5 may depend on `std::fs`, `std::net`, threading, or
  anything else that breaks the WASM build (`cargo xtask wasm`).
- No crate below L6 may depend on `egui` / `eframe` / `winit` / `rfd`.
- `unsafe_code = "forbid"` for the whole workspace.

## Native-format sketch

The `.graphcraft` file is a versioned JSON document (CBOR variant later for
large tables). Top-level shape:

```json
{
  "format": "graphcraft",
  "version": 1,
  "tables": [],
  "analyses": [],
  "graphs": [],
  "layouts": []
}
```

Importers (`.pzfx` is **not** one of them; see `AGENTS.md` › Clean-room)
produce the same in-memory `Document`.

## Panic policy

`engine::guard` wraps every entry point (command dispatch, control request,
UI frame, MCP). It catches with `std::panic::catch_unwind`, rolls the
document back to the pre-call snapshot, and reports an `EngineError::Panic`
with a stable message. WASM aborts on panic, so the lints — not the guard —
are the real defence on the web build.
