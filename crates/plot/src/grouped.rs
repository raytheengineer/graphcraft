//! Grouped-bar scene builder for `GroupedTable`.
//!
//! Produces one `Series::GroupedBars` per subcolumn (e.g. by
//! treatment within a `(dose × treatment)` factorial), with group
//! labels on a categorical x-axis.

use graphcraft_numeric::NumericError;
use graphcraft_table::GroupedTable;

use crate::scene::{
    Axis, AxisScale, Color, GroupedBar, Legend, LegendPosition, Marker, MarkerStyle, Scene, Series,
};
use crate::ticks::nice_linear;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupedErrorBars {
    None,
    Sd,
    Sem,
}

#[derive(Debug, Clone)]
pub struct GroupedOptions {
    pub title: Option<String>,
    pub x_title: Option<String>,
    pub y_title: Option<String>,
    pub width_pt: f64,
    pub height_pt: f64,
    pub y_tick_target: usize,
    pub error_bars: GroupedErrorBars,
    pub legend: Option<LegendPosition>,
}

impl Default for GroupedOptions {
    fn default() -> Self {
        Self {
            title: None,
            x_title: None,
            y_title: None,
            width_pt: 486.0,
            height_pt: 324.0,
            y_tick_target: 6,
            error_bars: GroupedErrorBars::Sd,
            legend: Some(LegendPosition::TopRight),
        }
    }
}

/// Build a grouped-bar scene from a `GroupedTable`. Each
/// subcolumn across all groups becomes one `Series::GroupedBars`
/// entry; rows missing a value are dropped per-cell.
pub fn grouped_scene(table: &GroupedTable, opts: &GroupedOptions) -> Result<Scene, NumericError> {
    let n_groups = table.groups.len();
    if n_groups == 0 {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    let n_sub = table
        .groups
        .first()
        .map(|g| g.subcolumns.len())
        .unwrap_or(0);
    if n_sub == 0 {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }

    let mut y_lo = 0.0_f64;
    let mut y_hi = 0.0_f64;
    let mut series = Vec::with_capacity(n_sub);
    for s in 0..n_sub {
        let label = table
            .groups
            .first()
            .and_then(|g| g.subcolumns.get(s))
            .map(|sc| sc.title.clone())
            .unwrap_or_else(|| format!("sub-{s}"));
        let mut bars = Vec::with_capacity(n_groups);
        for (gi, g) in table.groups.iter().enumerate() {
            let sub = match g.subcolumns.get(s) {
                Some(sc) => sc,
                None => continue,
            };
            let vals: Vec<f64> = sub.values.iter().filter_map(|v| *v).collect();
            if vals.is_empty() {
                continue;
            }
            let n = vals.len() as f64;
            let mean = vals.iter().sum::<f64>() / n;
            let error = if vals.len() >= 2 {
                let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
                let sd = var.sqrt();
                match opts.error_bars {
                    GroupedErrorBars::None => 0.0,
                    GroupedErrorBars::Sd => sd,
                    GroupedErrorBars::Sem => sd / n.sqrt(),
                }
            } else {
                0.0
            };
            y_lo = y_lo.min(mean - error);
            y_hi = y_hi.max(mean + error);
            bars.push(GroupedBar {
                group_index: gi,
                mean,
                error,
            });
        }
        if bars.is_empty() {
            continue;
        }
        let color = Color::pick(s);
        let marker = MarkerStyle {
            shape: Marker::None,
            size_pt: 0.0,
            fill: color,
            stroke: Color::BLACK,
            stroke_width_pt: 1.0,
        };
        series.push(Series::GroupedBars {
            label,
            subcolumn: s,
            subcolumns_per_group: n_sub,
            n_groups,
            bars,
            marker,
        });
    }
    if series.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    if y_lo == y_hi {
        y_hi = y_lo + 1.0;
    }
    let y_nice = nice_linear(y_lo, y_hi, opts.y_tick_target);
    let x_min = 0.5;
    let x_max = n_groups as f64 + 0.5;
    let x_ticks: Vec<f64> = (1..=n_groups).map(|i| i as f64).collect();
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
        series,
        legend,
    })
}

#[cfg(test)]
mod tests {
    use graphcraft_table::{Column, GroupColumn, GroupedTable};

    use super::*;

    #[test]
    fn two_groups_two_subcolumns() {
        let mk_sub = |title: &str, vs: &[f64]| Column {
            title: title.into(),
            values: vs.iter().map(|v| Some(*v)).collect(),
        };
        let t = GroupedTable {
            title: "g".into(),
            row_titles: Vec::new(),
            groups: vec![
                GroupColumn {
                    title: "A".into(),
                    subcolumns: vec![
                        mk_sub("Pre", &[1.0, 2.0, 3.0]),
                        mk_sub("Post", &[4.0, 5.0, 6.0]),
                    ],
                },
                GroupColumn {
                    title: "B".into(),
                    subcolumns: vec![
                        mk_sub("Pre", &[7.0, 8.0, 9.0]),
                        mk_sub("Post", &[10.0, 11.0, 12.0]),
                    ],
                },
            ],
        };
        let scene = grouped_scene(&t, &GroupedOptions::default()).unwrap();
        assert_eq!(scene.series.len(), 2); // one per subcolumn
        if let Series::GroupedBars { bars, .. } = &scene.series[0] {
            assert_eq!(bars.len(), 2);
            assert!((bars[0].mean - 2.0).abs() < 1e-15);
            assert!((bars[1].mean - 8.0).abs() < 1e-15);
        }
        assert_eq!(scene.x_axis.ticks, vec![1.0, 2.0]);
    }
}
