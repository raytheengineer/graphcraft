# GraphCraft

Clean-room, open-source, Rust-native scientific graphing and analysis app.
Targets output parity with GraphPad Prism — same input data + same documented
analysis = matching statistics, to published numerical tolerance.

GraphCraft is an independent project. It is not affiliated with, endorsed by,
or sponsored by GraphPad Software, Dotmatics, or any of their subsidiaries.
"GraphPad" and "Prism" are trademarks of their respective owners; see
[`NOTICE`](NOTICE).

## Status

Pre-alpha. See [`ROADMAP.md`](ROADMAP.md).

## Non-negotiables

- **Clean-room.** Behaviour is implemented from public documentation only
  (GraphPad's online manual, Statistics Guide, Curve Fitting Guide; published
  textbooks and papers). We never run, screenshot, time, or measure Prism;
  we never read Prism install contents; we never compare our output against
  output we produced with Prism. See [`AGENTS.md`](AGENTS.md).
- **Output parity oracle is public reference data,** primarily
  [NIST StRD](https://itl.nist.gov/div898/strd/) (public domain, certified
  values), with cross-checks against published R and SciPy reference values.
  The project spine is [`tests/parity-matrix.csv`](tests/parity-matrix.csv).
- **Never crash.** No panics in shipped code; the workspace clippy lints
  enforce it, and `engine::guard` is the last line of defence.
- **Permissive licenses only.** Code and original assets are
  `MIT OR Apache-2.0`. No GPL / AGPL dependencies.

## Quick start

```sh
cargo xtask ci           # fmt + clippy + test + layers + cleanroom + assets + wasm
cargo run -p graphcraft-cli -- --help
```

## License

`MIT OR Apache-2.0` at your option. See [`LICENSE-MIT`](LICENSE-MIT) and
[`LICENSE-APACHE`](LICENSE-APACHE).
