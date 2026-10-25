# GraphCraft — instructions for agents

GraphCraft is a clean-room, open-source, Rust-native scientific graphing and
analysis app targeting output parity with GraphPad Prism (same input data +
same documented analysis = matching statistics, to published numerical
tolerance). Native on macOS, Windows, Linux; WASM build is a CI gate for
architectural drift.

GraphCraft is an independent project, not affiliated with GraphPad / Dotmatics.
It takes design cues from the ArtCraft family of apps (VectorCraft,
PhotoCraft, PDFCraft) but is not currently part of the `storytold`
organisation on GitHub.

## Start every session here
1. Read `ROADMAP.md`: current milestone, gap list, direction. Unless the user
   gives you a task, pick work from there.
2. Read `plan/STATUS.md` (gitignored, local notes) and the active section of
   `plan/architecture.md`. Behaviour reference is **public documentation
   only** (see Clean-room below); contributor-authored notes from the public
   Prism manual live in `plan/prism/*.md`.
3. Orient → plan → implement + test → verify → record → commit. Don't stop
   to ask unless the decision is genuinely the user's.

`plan/` is gitignored.

## Non-negotiables

### Clean-room — never use GraphPad software (absolute rule)
GraphPad's EULA for Prism forbids reverse engineering (observing a program's
inputs and outputs to recreate it), benchmarking against GraphPad products,
and distribution of GraphPad-authored content. So, in this project:

- Never launch, run, install, script, automate, screenshot, time or measure
  Prism (or any other GraphPad / Dotmatics product), and never compare our
  output against output we produced with it. Don't produce test fixtures
  with it, not even "synthetic" ones.
- Never read, disassemble or copy anything inside a Prism install (binaries,
  resources, example `.pzfx` files that shipped with Prism, presets, help
  files, fonts, icons).
- Never copy GraphPad-authored code, text, images, equations as rendered by
  them, example datasets, or wording beyond unavoidable feature names
  ("one-way ANOVA", "Dunnett's test" — fine; "GraphPad's recommendation for
  this design is…" — not fine, that is their text).
- Behaviour comes only from **public documentation**, cited by URL in
  `plan/prism/*.md`:
  - The Prism manual at `graphpad.com/guides/prism/latest/...`
  - The GraphPad Statistics Guide and Curve Fitting Guide (same site)
  - Public forum posts and KB articles on `graphpad.com`
  - Peer-reviewed papers and textbooks (cited by DOI/ISBN/page)
- `.pzfx` files that users send us are used **locally only**, never
  committed, never used as a behaviour oracle beyond what the public
  documentation already states. The `.pzfx` format is not reverse-engineered
  by us; `.pzfx` import/export is deliberately out of scope at least through
  M6 and may stay out of scope indefinitely.
- If a task seems to need any of the above, stop and say so instead.
- `cargo xtask cleanroom` (run by `cargo xtask ci`) scans the repo for
  banned-provenance markers and fails the build if any appear.

### Output parity — the goal
We don't match Prism by watching Prism; we match it by implementing the
same **published** formulas and verifying against the same **public**
reference values.

- Every analysis command has an acceptance test backed by one or both of:
  1. A worked example from a published textbook/paper, cited by DOI/ISBN
     and page, with hand-computed expected values; or
  2. A public reference dataset with certified published answers — primarily
     NIST StRD (`itl.nist.gov/div898/strd/`, public domain, certified to
     15 digits), with cross-checks against R and SciPy reference values
     *from their published documentation* (numeric facts, not their code).
- `tests/parity-matrix.csv` is the project spine: one row per
  `(analysis, dataset, statistic, expected_value, source_url)`. The parity
  harness in `crates/analysis/tests/parity.rs` consumes it.
- Tolerance policy: closed-form expectations use
  `assert_relative_eq!(got, expected, max_relative = 1e-9)`; iterative
  expectations use `1e-6`. If we can't hit that, we file a known-gap row
  in `ROADMAP.md` rather than relax the tolerance silently.
- Prism-specific defaults that are *documented in the public manual* (e.g.
  Welch's correction as the default unpaired t-test, Geisser-Greenhouse
  epsilon as the default for RM ANOVA, ROUT Q=1% for outliers, recommended
  multiple-comparison test per design) are encoded as an opt-in
  `PrismDefaults` option set in `crates/analysis`, each cited by URL.

### Assets — permissive licences only
Every image, icon, font or other binary asset must be one of: original work
created for GraphCraft by a contributor (licensed MIT OR Apache-2.0), OSI
open source, public domain / CC0, or Creative Commons with redistribution
allowed. Every asset file has a row in `ASSETS.md` (path, author, source
URL, licence, notes). `cargo xtask assets` fails on any unattributed asset.

Never add GraphPad icons, screenshots, or example figures.

### Never crash — no panics in shipped code
Scientific input is untrusted. A crash loses the user's unsaved analysis.

- Fallible work returns `Result` (or `Option`); callers handle it or
  propagate with `?`. Failures reach the user as an error message, never as
  an abort.
- Workspace clippy lints ban `unwrap()`, `expect()`, `panic!`,
  `unreachable!`, `todo!`, `unimplemented!` outside tests. `cargo xtask ci`
  fails on them.
- No implicit panics either: `.get(i)` not `v[i]` on data-indexed access,
  check floats finite before casting or looping on them, cap sizes read
  from input, don't divide by zero.
- `engine::guard` catches any panic at every entry point (commands, control
  requests, UI frame, MCP), rolls the document back, reports an internal
  error. WASM has no safety net — the no-panic lints exist for the WASM
  build in particular.
- Importers (CSV, Excel), the native format reader, the expression
  evaluator, and the command sweep are fuzzed.

### Everything is a command
User-visible behaviour = a command in `crates/engine/src/cmd/*` (id, label,
menu path, shortcut, params doc, `enabled`, `run`) + tests. Analyses and
plots are commands too. The control channel and MCP reach all of them.

### Layering — enforced
`cargo xtask layers` enforces the L0→L6 crate dependency order:

```
L1 numeric
L2 stats, table
L3 regression
L4 analysis, plot, format
L5 render, engine
L6 ui-egui, apps/*
```

Nothing below L6 depends on egui / eframe / winit / rfd. Below L5 nothing
depends on `std::fs`, `std::net`, threading, or anything else that breaks
the WASM build.

### WASM is a CI gate
`cargo xtask wasm` builds the engine for `wasm32-unknown-unknown` and runs
its unit tests under `wasm-bindgen-test` in headless Chrome. If a
disallowed dep sneaks in, the build breaks. We do not currently ship a web
UI; the gate exists to keep the engine deterministic, I/O-free below L5,
and panic-free.

Native-only deps (`rfd`, `calamine`, OS printing) go behind a Cargo feature
and a `#[cfg(not(target_arch = "wasm32"))]` guard. Use the `web-time` crate
instead of `std::time::Instant` in engine code.

### Rust only
No handwritten JS/TS.

### Quality gates before every commit
`cargo xtask ci` runs: `fmt --check`, `clippy -D warnings` (incl. the
no-panic lints), `test`, `layers`, `cleanroom`, `assets`, `wasm`. One
task id per commit (`M1.3: engine::guard`).

## Running

- `cargo run --release -p graphcraft -- [file.graphcraft|file.csv]` once
  the app exists (M1 ships the CLI first).
- `cargo run --release -p graphcraft-cli -- --help` for headless use.
- `cargo xtask ci` for all gates.

## Contributor credits
When you prepare a PR, if your human's GitHub username doesn't appear in
`contributors/people.toml` (file introduced later; may not exist yet), ask
them once whether they want to be credited by more than their username.
Never add, edit, guess or copy anyone else's entry.
