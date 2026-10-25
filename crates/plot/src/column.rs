//! Column (bar) scene builder for `ColumnTable`.
//!
//! One bar per column, with its mean as the bar height and
//! (optional) SD or SEM error bars. Categorical x-axis.

use graphcraft_numeric::NumericError;
use graphcraft_table::ColumnTable;

use crate::scene::{
    Axis, AxisScale, Color, ColumnBar, Legend, LegendPosition, Marker, MarkerStyle, Scene, Series,
};
use crate::ticks::nice_linear;

/// How to summarise each column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnErrorBars {
    None,
    Sd,
    Sem,
}

#[derive(Debug, Clone)]
pub struct ColumnOptions {
    pub title: Option<String>,
    pub x_title: Option<String>,
    pub y_title: Option<String>,
    pub width_pt: f64,
    pub height_pt: f64,
    pub y_tick_target: usize,
    pub error_bars: ColumnErrorBars,
    pub bar_fill: Color,
    pub bar_stroke: Color,
    pub legend: Option<LegendPosition>,
}

impl Default for ColumnOptions {
    fn default() -> Self {
        Self {
            title: None,
            x_title: None,
            y_title: None,
            width_pt: 432.0,
            height_pt: 324.0,
            y_tick_target: 6,
            error_bars: ColumnErrorBars::Sd,
            bar_fill: Color::BLUE,
            bar_stroke: Color::BLACK,
            legend: None,
        }
    }
}

/// Build a column scene from a `ColumnTable`. Each non-empty column
/// contributes one bar.
pub fn column_scene(table: &ColumnTable, opts: &ColumnOptions) -> Result<Scene, NumericError> {
    let mut bars: Vec<ColumnBar> = Vec::new();
    for (i, col) in table.columns.iter().enumerate() {
        let xs: Vec<f64> = table.group_values(i).collect();
        if xs.is_empty() {
            continue;
        }
        let n = xs.len() as f64;
        let mean = xs.iter().sum::<f64>() / n;
        let error = if xs.len() >= 2 {
            let var = xs.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
            let sd = var.sqrt();
            match opts.error_bars {
                ColumnErrorBars::None => 0.0,
                ColumnErrorBars::Sd => sd,
                ColumnErrorBars::Sem => sd / n.sqrt(),
            }
        } else {
            0.0
        };
        bars.push(ColumnBar {
            label: col.title.clone(),
            mean,
            error,
        });
    }
    if bars.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    column_scene_from_bars(&bars, opts)
}

/// Build a column scene directly from pre-computed bars.
pub fn column_scene_from_bars(
    bars: &[ColumnBar],
    opts: &ColumnOptions,
) -> Result<Scene, NumericError> {
    if bars.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    for b in bars {
        if !b.mean.is_finite() || !b.error.is_finite() {
            return Err(NumericError::NotFinite);
        }
    }

    // Y range covers 0 to max(bar + error), with symmetric handling
    // for negative bars via min(bar - error).
    let mut y_lo = 0.0_f64;
    let mut y_hi = 0.0_f64;
    for b in bars {
        y_lo = y_lo.min(b.mean - b.error);
        y_hi = y_hi.max(b.mean + b.error);
    }
    if y_lo == y_hi {
        y_hi = y_lo + 1.0;
    }
    let y_nice = nice_linear(y_lo, y_hi, opts.y_tick_target);

    // Categorical x-axis: evenly-spaced centres at x = 1..=k on an
    // axis that runs from 0.5 to k+0.5. Ticks are the category
    // positions themselves.
    let k = bars.len();
    let x_min = 0.5;
    let x_max = k as f64 + 0.5;
    let x_ticks: Vec<f64> = (1..=k).map(|i| i as f64).collect();

    let marker = MarkerStyle {
        shape: Marker::None,
        size_pt: 0.0,
        fill: opts.bar_fill,
        stroke: opts.bar_stroke,
        stroke_width_pt: 1.0,
    };
    let series = Series::Bars {
        label: "mean".into(),
        bars: bars.to_vec(),
        marker,
    };

    let legend = opts.legend.map(|position| Legend { position });
    Ok(Scene {
        width_pt: opts.width_pt,
        height_pt: opts.height_pt,
        margins_pt: Scene::DEFAULT_MARGINS_PT,
        title: opts.title.clone(),
        x_axis: Axis {
            title: opts.x_title.clone(),
            min: x_min,
            max: x_max,
            ticks: x_ticks,
            scale: AxisScale::Linear,
        },
        y_axis: Axis {
            title: opts.y_title.clone(),
            min: y_nice.min,
            max: y_nice.max,
            ticks: y_nice.ticks,
            scale: AxisScale::Linear,
        },
        series: vec![series],
        legend,
    })
}

#[cfg(test)]
mod tests {
    use graphcraft_table::{Column, ColumnTable};

    use super::*;

    #[test]
    fn bars_from_two_groups() {
        let t = ColumnTable {
            title: "cols".into(),
            row_titles: Vec::new(),
            columns: vec![
                Column {
                    title: "ctrl".into(),
                    values: vec![Some(1.0), Some(2.0), Some(3.0)],
                },
                Column {
                    title: "drug".into(),
                    values: vec![Some(5.0), Some(9.0), Some(10.0)],
                },
            ],
        };
        let scene = column_scene(&t, &ColumnOptions::default()).unwrap();
        if let Series::Bars { bars, .. } = &scene.series[0] {
            assert_eq!(bars.len(), 2);
            assert!((bars[0].mean - 2.0).abs() < 1e-15);
            assert!((bars[1].mean - 8.0).abs() < 1e-15);
            // ctrl SD = sqrt(((1-2)²+(2-2)²+(3-2)²)/2) = 1
            assert!((bars[0].error - 1.0).abs() < 1e-15);
        } else {
            panic!("expected Bars series");
        }
        assert_eq!(scene.x_axis.ticks, vec![1.0, 2.0]);
    }

    #[test]
    fn empty_table_rejected() {
        let t = ColumnTable {
            title: "empty".into(),
            row_titles: Vec::new(),
            columns: vec![Column {
                title: "x".into(),
                values: vec![None, None],
            }],
        };
        assert!(column_scene(&t, &ColumnOptions::default()).is_err());
    }
}
