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

pub mod column;
pub mod grouped;
pub mod heatmap;
pub mod scene;
pub mod survival;
pub mod ticks;
pub mod xy;

pub use column::{column_scene, column_scene_from_bars, ColumnErrorBars, ColumnOptions};
pub use grouped::{grouped_scene, GroupedErrorBars, GroupedOptions};
pub use heatmap::{colormap_sample, heatmap_scene, Colormap, HeatmapData, HeatmapOptions};
pub use scene::{
    Axis, AxisScale, Color, ColumnBar, GroupedBar, HeatmapCell, Legend, LegendPosition, LineStyle,
    Marker, MarkerStyle, Scene, Series,
};
pub use survival::{km_estimate, survival_scene, SurvivalOptions};
pub use ticks::{nice_linear, NiceAxis};
pub use xy::{xy_scene, xy_scene_from_points, XyOptions};
