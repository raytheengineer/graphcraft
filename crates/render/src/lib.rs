//! L5 render: SVG (M5.1) and PDF (M5.3) writers for
//! `graphcraft_plot::Scene`.
//!
//! The renderer is deterministic and I/O-free: `render_svg(&Scene)`
//! returns a `String`. Callers write it to a file themselves.

pub mod svg;

pub use svg::render_svg;
