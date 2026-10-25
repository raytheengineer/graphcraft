//! L4 plot: scene graph for GraphCraft graphs.
//!
//! Scenes are device-independent descriptions (points at 72/inch)
//! that the L5 render crate converts to SVG (and, from M5.3, PDF).
//! Each scene builder takes a data table plus an options struct and
//! returns a `Scene` with its axes already laid out via the
//! nice-numbers tick algorithm.
//!
//! M5.1 ships the XY scatter builder. Column, Grouped, and Survival
//! builders follow in M5.2.

pub mod scene;
pub mod ticks;
pub mod xy;

pub use scene::{
    Axis, AxisScale, Color, Legend, LegendPosition, LineStyle, Marker, MarkerStyle, Scene, Series,
};
pub use ticks::{nice_linear, NiceAxis};
pub use xy::{xy_scene, xy_scene_from_points, XyOptions};
