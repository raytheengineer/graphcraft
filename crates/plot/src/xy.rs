//! XY scatter scene builder.
//!
//! M5.1 ships the single-dataset scatter case. Multi-dataset
//! overlays, mean±SD error bars, and fit-line overlays follow once
//! the SVG renderer lands.

use graphcraft_numeric::NumericError;
use graphcraft_table::XyTable;

use crate::scene::{
    Axis, AxisScale, Color, Legend, LegendPosition, Marker, MarkerStyle, Scene, Series,
};
use crate::ticks::nice_linear;

/// Options for [`xy_scene`]. Fields are the knobs a Prism user
/// would reach for first: axis titles, canvas size, tick count,
/// marker style, legend.
#[derive(Debug, Clone)]
pub struct XyOptions {
    pub title: Option<String>,
    pub x_title: Option<String>,
    pub y_title: Option<String>,
    pub width_pt: f64,
    pub height_pt: f64,
    pub x_tick_target: usize,
    pub y_tick_target: usize,
    pub marker: MarkerStyle,
    pub legend: Option<LegendPosition>,
}

impl Default for XyOptions {
    fn default() -> Self {
        Self {
            title: None,
            x_title: None,
            y_title: None,
            width_pt: 432.0,  // 6 in
            height_pt: 324.0, // 4.5 in
            x_tick_target: 6,
            y_tick_target: 6,
            marker: MarkerStyle {
                shape: Marker::Circle,
                size_pt: 4.0,
                fill: Color::BLUE,
                stroke: Color::BLACK,
                stroke_width_pt: 0.5,
            },
            legend: None,
        }
    }
}

/// Build a scatter scene from an `XyTable`, one dataset.
///
/// Uses the first subcolumn of the given dataset against the shared
/// x column, dropping rows where either value is missing.
pub fn xy_scene(
    table: &XyTable,
    dataset_idx: usize,
    opts: &XyOptions,
) -> Result<Scene, NumericError> {
    if dataset_idx >= table.datasets.len() {
        return Err(NumericError::TooFewValues {
            required: dataset_idx + 1,
            got: table.datasets.len(),
        });
    }
    let ds = &table.datasets[dataset_idx];
    if ds.subcolumns.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    let ys_sub = &ds.subcolumns[0].values;
    let mut points: Vec<(f64, f64)> = Vec::new();
    for (xi, yi) in table.x.iter().zip(ys_sub.iter()) {
        if let (Some(x), Some(y)) = (xi, yi) {
            if x.is_finite() && y.is_finite() {
                points.push((*x, *y));
            }
        }
    }
    if points.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    xy_scene_from_points(&points, opts, ds.title.clone())
}

/// Build a scatter scene directly from `(x, y)` pairs.
pub fn xy_scene_from_points(
    points: &[(f64, f64)],
    opts: &XyOptions,
    series_label: String,
) -> Result<Scene, NumericError> {
    if points.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    for &(x, y) in points {
        if !x.is_finite() || !y.is_finite() {
            return Err(NumericError::NotFinite);
        }
    }
    let (xlo, xhi) = minmax(points.iter().map(|p| p.0));
    let (ylo, yhi) = minmax(points.iter().map(|p| p.1));
    let x_axis_nice = nice_linear(xlo, xhi, opts.x_tick_target);
    let y_axis_nice = nice_linear(ylo, yhi, opts.y_tick_target);
    let series = Series::Scatter {
        label: series_label,
        points: points.to_vec(),
        marker: opts.marker,
    };
    let legend = opts.legend.map(|position| Legend { position });
    Ok(Scene {
        width_pt: opts.width_pt,
        height_pt: opts.height_pt,
        margins_pt: Scene::DEFAULT_MARGINS_PT,
        title: opts.title.clone(),
        x_axis: Axis {
            title: opts.x_title.clone(),
            min: x_axis_nice.min,
            max: x_axis_nice.max,
            ticks: x_axis_nice.ticks,
            scale: AxisScale::Linear,
        },
        y_axis: Axis {
            title: opts.y_title.clone(),
            min: y_axis_nice.min,
            max: y_axis_nice.max,
            ticks: y_axis_nice.ticks,
            scale: AxisScale::Linear,
        },
        series: vec![series],
        legend,
    })
}

fn minmax<I: Iterator<Item = f64>>(it: I) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for v in it {
        if v < lo {
            lo = v;
        }
        if v > hi {
            hi = v;
        }
    }
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_axes_cover_data() {
        let pts = vec![(1.0, 2.0), (2.0, 4.0), (3.0, 6.0), (4.0, 8.0), (5.0, 10.0)];
        let scene = xy_scene_from_points(&pts, &XyOptions::default(), "A".into()).unwrap();
        assert!(scene.x_axis.min <= 1.0);
        assert!(scene.x_axis.max >= 5.0);
        assert!(scene.y_axis.min <= 2.0);
        assert!(scene.y_axis.max >= 10.0);
        assert_eq!(scene.series.len(), 1);
    }

    #[test]
    fn empty_rejected() {
        let scene = xy_scene_from_points(&[], &XyOptions::default(), "A".into());
        assert!(scene.is_err());
    }

    #[test]
    fn non_finite_rejected() {
        let pts = vec![(1.0, 2.0), (f64::NAN, 3.0)];
        let scene = xy_scene_from_points(&pts, &XyOptions::default(), "A".into());
        assert!(scene.is_err());
    }
}
