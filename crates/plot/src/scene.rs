//! Device-independent scene graph for a single graph.
//!
//! Scenes are purely descriptive: they say what to draw (title,
//! axes, series, legend) in data space, in SI-neutral units
//! (points at 72/inch). The L5 render crate converts a scene into
//! an output format (SVG now; PDF in M5.3).

use serde::{Deserialize, Serialize};

/// One rendered graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    /// Overall canvas width, in points (1 pt = 1/72 inch).
    pub width_pt: f64,
    /// Overall canvas height, in points.
    pub height_pt: f64,
    /// Margins (points) around the plotting area, in CSS order:
    /// (top, right, bottom, left).
    pub margins_pt: (f64, f64, f64, f64),
    pub title: Option<String>,
    pub x_axis: Axis,
    pub y_axis: Axis,
    pub series: Vec<Series>,
    pub legend: Option<Legend>,
}

impl Scene {
    /// Default margins Prism-style publications use: a bit more on
    /// the left for the y-axis label and bottom for the x-axis.
    pub const DEFAULT_MARGINS_PT: (f64, f64, f64, f64) = (36.0, 24.0, 54.0, 60.0);

    /// Inner plot rectangle in canvas coordinates
    /// `(x, y, width, height)` with the origin at the top-left of
    /// the canvas.
    pub fn plot_rect(&self) -> (f64, f64, f64, f64) {
        let (t, r, b, l) = self.margins_pt;
        (l, t, self.width_pt - l - r, self.height_pt - t - b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AxisScale {
    Linear,
    Log10,
}

/// One axis of the scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Axis {
    pub title: Option<String>,
    /// Axis data-space extents.
    pub min: f64,
    pub max: f64,
    /// Explicit tick positions in data space.
    pub ticks: Vec<f64>,
    pub scale: AxisScale,
}

/// Hex RGB colour. Pure data; the renderer draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const BLACK: Color = Color(0, 0, 0);
    pub const WHITE: Color = Color(255, 255, 255);
    pub const BLUE: Color = Color(31, 119, 180);
    pub const ORANGE: Color = Color(255, 127, 14);
    pub const GREEN: Color = Color(44, 160, 44);
    pub const RED: Color = Color(214, 39, 40);

    /// A six-colour wheel from the public d3 category10 palette
    /// (public-domain colour sequence).
    pub const WHEEL: [Color; 6] = [
        Color::BLUE,
        Color::ORANGE,
        Color::GREEN,
        Color::RED,
        Color(148, 103, 189),
        Color(140, 86, 75),
    ];

    pub fn pick(index: usize) -> Color {
        Color::WHEEL[index % Color::WHEEL.len()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Marker {
    Circle,
    Square,
    Triangle,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MarkerStyle {
    pub shape: Marker,
    pub size_pt: f64,
    pub fill: Color,
    pub stroke: Color,
    pub stroke_width_pt: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LineStyle {
    pub stroke: Color,
    pub width_pt: f64,
}

/// One pre-computed bar: category label + mean (height) + error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnBar {
    pub label: String,
    pub mean: f64,
    pub error: f64,
}

/// One pre-computed grouped-bar entry: which group it belongs to
/// (0-based integer index into the grouped x-axis categories) and
/// its mean/error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupedBar {
    pub group_index: usize,
    pub mean: f64,
    pub error: f64,
}

/// One data series to draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Series {
    /// Scatter of `(x, y)` points.
    Scatter {
        label: String,
        points: Vec<(f64, f64)>,
        marker: MarkerStyle,
    },
    /// Connected polyline (used for fits and smoothed lines).
    Line {
        label: String,
        points: Vec<(f64, f64)>,
        style: LineStyle,
    },
    /// One rectangular bar per `ColumnBar`, placed at integer x
    /// positions `1..=bars.len()` on a categorical axis.
    Bars {
        label: String,
        bars: Vec<ColumnBar>,
        marker: MarkerStyle,
    },
    /// A single "subcolumn" of a grouped bar chart. The grouped
    /// axis has `n_groups` categories; this series sits in one
    /// subcolumn slot `subcolumn` out of `subcolumns_per_group`.
    GroupedBars {
        label: String,
        subcolumn: usize,
        subcolumns_per_group: usize,
        n_groups: usize,
        bars: Vec<GroupedBar>,
        marker: MarkerStyle,
    },
    /// Censor-tick marks: tiny vertical ticks drawn at data-space
    /// `(x, y)`. Used by the Kaplan–Meier builder.
    CensorTicks {
        label: String,
        points: Vec<(f64, f64)>,
        style: LineStyle,
    },
    /// A heatmap grid of coloured cells. Cells are addressed by
    /// `(row, col)` with row 0 at the top and column 0 on the left.
    Heatmap {
        label: String,
        n_rows: usize,
        n_cols: usize,
        cells: Vec<HeatmapCell>,
        row_labels: Vec<String>,
        col_labels: Vec<String>,
        vmin: f64,
        vmax: f64,
    },
}

/// One rendered heatmap cell.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeatmapCell {
    pub row: usize,
    pub col: usize,
    pub value: Option<f64>,
    pub color: Color,
}

impl Series {
    pub fn label(&self) -> &str {
        match self {
            Series::Scatter { label, .. }
            | Series::Line { label, .. }
            | Series::Bars { label, .. }
            | Series::GroupedBars { label, .. }
            | Series::CensorTicks { label, .. }
            | Series::Heatmap { label, .. } => label,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegendPosition {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Legend {
    pub position: LegendPosition,
}
