#![allow(clippy::needless_range_loop)]
//! Heatmap scene builder.
//!
//! Takes a row-major 2D grid of values and produces a scene of
//! coloured cells, with optional row/column labels and a title.
//! The colour scale is one of the built-in colormaps in
//! [`Colormap`].

use graphcraft_numeric::NumericError;
use serde::{Deserialize, Serialize};

use crate::scene::{Axis, AxisScale, Color, HeatmapCell, Scene, Series};

/// A row-major 2D grid of f64 values (`values[row][col]`).
///
/// `None` cells are drawn as a neutral background in the renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct HeatmapData<'a> {
    pub values: &'a [Vec<Option<f64>>],
    pub row_titles: &'a [String],
    pub col_titles: &'a [String],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Colormap {
    /// Blue → white → red diverging palette, centred on the midpoint
    /// of the data range. Standard for correlation heatmaps.
    DivergingBlueRed,
    /// White → black. Useful for strictly non-negative data.
    Grayscale,
    /// Three-stop sequential blue palette.
    SequentialBlue,
}

#[derive(Debug, Clone)]
pub struct HeatmapOptions {
    pub title: Option<String>,
    pub width_pt: f64,
    pub height_pt: f64,
    pub colormap: Colormap,
    /// Fixed scale `(vmin, vmax)` if provided; otherwise computed
    /// from the data.
    pub value_range: Option<(f64, f64)>,
    pub show_row_titles: bool,
    pub show_col_titles: bool,
}

impl Default for HeatmapOptions {
    fn default() -> Self {
        Self {
            title: None,
            width_pt: 432.0,
            height_pt: 324.0,
            colormap: Colormap::DivergingBlueRed,
            value_range: None,
            show_row_titles: true,
            show_col_titles: true,
        }
    }
}

/// Build a heatmap scene from a row-major `Vec<Vec<Option<f64>>>`
/// matrix. Rows and columns may be labelled.
pub fn heatmap_scene(data: HeatmapData<'_>, opts: &HeatmapOptions) -> Result<Scene, NumericError> {
    let n_rows = data.values.len();
    if n_rows == 0 {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    let n_cols = data.values[0].len();
    if n_cols == 0 {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    for row in data.values {
        if row.len() != n_cols {
            return Err(NumericError::TooFewValues {
                required: n_cols,
                got: row.len(),
            });
        }
        for x in row.iter().flatten() {
            if !x.is_finite() {
                return Err(NumericError::NotFinite);
            }
        }
    }

    let (vmin, vmax) = match opts.value_range {
        Some((lo, hi)) => (lo, hi),
        None => {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for row in data.values {
                for x in row.iter().flatten() {
                    if *x < lo {
                        lo = *x;
                    }
                    if *x > hi {
                        hi = *x;
                    }
                }
            }
            if !lo.is_finite() {
                (0.0, 1.0)
            } else if lo == hi {
                (lo - 0.5, hi + 0.5)
            } else {
                (lo, hi)
            }
        }
    };

    let mut cells = Vec::with_capacity(n_rows * n_cols);
    for i in 0..n_rows {
        for j in 0..n_cols {
            let v = data.values[i][j];
            let color = match v {
                Some(x) => colormap_sample(opts.colormap, x, vmin, vmax),
                None => Color(230, 230, 230),
            };
            cells.push(HeatmapCell {
                row: i,
                col: j,
                value: v,
                color,
            });
        }
    }

    // Row/col titles: default to indices if the slice is empty.
    let row_labels: Vec<String> = if data.row_titles.is_empty() {
        (0..n_rows).map(|i| format!("{i}")).collect()
    } else {
        data.row_titles.to_vec()
    };
    let col_labels: Vec<String> = if data.col_titles.is_empty() {
        (0..n_cols).map(|j| format!("{j}")).collect()
    } else {
        data.col_titles.to_vec()
    };

    Ok(Scene {
        width_pt: opts.width_pt,
        height_pt: opts.height_pt,
        margins_pt: Scene::DEFAULT_MARGINS_PT,
        title: opts.title.clone(),
        x_axis: Axis {
            title: None,
            min: 0.5,
            max: n_cols as f64 + 0.5,
            ticks: (1..=n_cols).map(|j| j as f64).collect(),
            scale: AxisScale::Linear,
        },
        y_axis: Axis {
            title: None,
            min: 0.5,
            max: n_rows as f64 + 0.5,
            ticks: (1..=n_rows).map(|i| i as f64).collect(),
            scale: AxisScale::Linear,
        },
        series: vec![Series::Heatmap {
            label: "heatmap".into(),
            n_rows,
            n_cols,
            cells,
            row_labels: if opts.show_row_titles {
                row_labels
            } else {
                Vec::new()
            },
            col_labels: if opts.show_col_titles {
                col_labels
            } else {
                Vec::new()
            },
            vmin,
            vmax,
        }],
        legend: None,
    })
}

/// Sample a colormap at `v / (vmax − vmin)`, clamped.
pub fn colormap_sample(cm: Colormap, v: f64, vmin: f64, vmax: f64) -> Color {
    let t = if vmax > vmin {
        ((v - vmin) / (vmax - vmin)).clamp(0.0, 1.0)
    } else {
        0.5
    };
    match cm {
        Colormap::DivergingBlueRed => blend3(
            t,
            Color(33, 102, 172),
            Color(247, 247, 247),
            Color(178, 24, 43),
        ),
        Colormap::Grayscale => blend2(t, Color(255, 255, 255), Color(0, 0, 0)),
        Colormap::SequentialBlue => blend3(
            t,
            Color(247, 251, 255),
            Color(107, 174, 214),
            Color(8, 48, 107),
        ),
    }
}

fn blend2(t: f64, a: Color, b: Color) -> Color {
    Color(lerp(t, a.0, b.0), lerp(t, a.1, b.1), lerp(t, a.2, b.2))
}

fn blend3(t: f64, a: Color, mid: Color, b: Color) -> Color {
    if t < 0.5 {
        blend2(t * 2.0, a, mid)
    } else {
        blend2((t - 0.5) * 2.0, mid, b)
    }
}

fn lerp(t: f64, a: u8, b: u8) -> u8 {
    let af = a as f64;
    let bf = b as f64;
    (af + t * (bf - af)).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heatmap_from_simple_matrix() {
        let v: Vec<Vec<Option<f64>>> = vec![
            vec![Some(1.0), Some(2.0), Some(3.0)],
            vec![Some(4.0), Some(5.0), Some(6.0)],
        ];
        let data = HeatmapData {
            values: &v,
            row_titles: &[],
            col_titles: &[],
        };
        let scene = heatmap_scene(data, &HeatmapOptions::default()).unwrap();
        if let Series::Heatmap {
            n_rows,
            n_cols,
            cells,
            vmin,
            vmax,
            ..
        } = &scene.series[0]
        {
            assert_eq!(*n_rows, 2);
            assert_eq!(*n_cols, 3);
            assert_eq!(cells.len(), 6);
            assert!((*vmin - 1.0).abs() < 1e-15);
            assert!((*vmax - 6.0).abs() < 1e-15);
        } else {
            panic!("expected Heatmap series");
        }
    }

    #[test]
    fn colormap_endpoints_lerp_correctly() {
        let lo = colormap_sample(Colormap::Grayscale, 0.0, 0.0, 1.0);
        let hi = colormap_sample(Colormap::Grayscale, 1.0, 0.0, 1.0);
        assert_eq!(lo, Color(255, 255, 255));
        assert_eq!(hi, Color(0, 0, 0));
    }

    #[test]
    fn missing_value_rendered_as_gray_stub() {
        let v: Vec<Vec<Option<f64>>> = vec![vec![Some(1.0), None], vec![Some(2.0), Some(3.0)]];
        let data = HeatmapData {
            values: &v,
            row_titles: &[],
            col_titles: &[],
        };
        let scene = heatmap_scene(data, &HeatmapOptions::default()).unwrap();
        if let Series::Heatmap { cells, .. } = &scene.series[0] {
            let missing = cells.iter().find(|c| c.value.is_none()).unwrap();
            assert_eq!(missing.color, Color(230, 230, 230));
        }
    }

    #[test]
    fn empty_matrix_rejected() {
        let v: Vec<Vec<Option<f64>>> = vec![];
        let data = HeatmapData {
            values: &v,
            row_titles: &[],
            col_titles: &[],
        };
        assert!(heatmap_scene(data, &HeatmapOptions::default()).is_err());
    }
}
