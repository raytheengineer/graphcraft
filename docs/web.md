# Web build

The analysis + plot surface runs unmodified in the browser via
WebAssembly. `crates/web` is a thin wrapper around the native
crates; `apps/web/index.html` is a single-file page that loads the
WASM module, accepts a CSV in a textarea, and renders analysis
JSON or an SVG scatter inline.

## Build

Requires `wasm-pack`:

```
cargo install wasm-pack
apps/web/build.sh --release
```

The build drops `graphcraft_web.js`, `graphcraft_web_bg.wasm`, and
`graphcraft_web.d.ts` into `apps/web/pkg/`. The `pkg/` directory is
gitignored.

## Serve

```
cd apps/web && python3 -m http.server 8000
# then open http://localhost:8000/
```

Any static-file server works — the page is 100% static HTML + ES
modules + the WASM blob.

## Exported functions

| JS binding | Signature | Returns |
| --- | --- | --- |
| `graphcraft_version()` | `() -> string` | the version of the web wrapper crate |
| `describe(csv)` | `(string) -> string` | per-column descriptives, JSON |
| `anova(csv)` | `(string) -> string` | one-way ANOVA, JSON |
| `correlate(csv, a, b)` | `(string, string, string) -> string` | Pearson + Spearman, JSON |
| `ttest_welch(csv, a, b)` | `(string, string, string) -> string` | Welch's t, JSON |
| `regress(csv, x, y)` | `(string, string, string) -> string` | OLS, JSON |
| `plot_xy_svg(csv, x, y)` | `(string, string, string) -> string` | `{svg, n_points}` as JSON |

Every function returns a JSON string. On error, the shape is
`{ "error": "..." }` so the page can render it inline without a
try/catch dance.

## Where it fits in the layer

The web crate lives at **L5** alongside render and engine, so the
no-I/O rule for below-L5 crates still holds: the whole chain
numeric → stats → regression → analysis → plot → render → web
compiles for `wasm32-unknown-unknown` with no `std::fs`,
`std::net`, threads, or `std::time::Instant`. The engine's own
WASM gate (`cargo xtask wasm`) remains the cheaper daily check;
the web build is an on-demand release build.

## Clean-room

The web wrapper takes a hard dependency on `wasm-bindgen` (Apache-
2.0 OR MIT), listed in the workspace dependencies. No other
third-party web library — no `wasm-react`, no `web-sys` tree, no
JavaScript framework. The page is vanilla HTML + ES modules.
