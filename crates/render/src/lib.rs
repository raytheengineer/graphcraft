//! L5 render: SVG and PDF writers for `graphcraft_plot::Scene`.
//!
//! The renderer is deterministic and I/O-free: `render_svg(&Scene)`
//! returns a `String`, `render_pdf(&Scene)` returns a `Vec<u8>`.
//! Callers write them to a file themselves.

pub mod pdf;
pub mod svg;

pub use pdf::render_pdf;
pub use svg::render_svg;
