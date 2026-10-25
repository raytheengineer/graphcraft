//! Kaplan–Meier survival-curve scene builder for `SurvivalTable`.
//!
//! Produces one stepwise `Series::Line` per group (the KM
//! estimator) plus a companion `Series::CensorTicks` series marking
//! censored observations at their time points.
//!
//! Reference (behaviour): Kaplan, E. L. & Meier, P. (1958)
//! "Nonparametric estimation from incomplete observations", JASA
//! 53, 457–481. Classical algorithm; no third-party or GraphPad
//! code.

use graphcraft_numeric::NumericError;
use graphcraft_table::SurvivalTable;

use crate::scene::{Axis, AxisScale, Color, Legend, LegendPosition, LineStyle, Scene, Series};
use crate::ticks::nice_linear;

#[derive(Debug, Clone)]
pub struct SurvivalOptions {
    pub title: Option<String>,
    pub x_title: Option<String>,
    pub y_title: Option<String>,
    pub width_pt: f64,
    pub height_pt: f64,
    pub x_tick_target: usize,
    pub y_tick_target: usize,
    pub legend: Option<LegendPosition>,
    pub show_censor_ticks: bool,
}

impl Default for SurvivalOptions {
    fn default() -> Self {
        Self {
            title: None,
            x_title: Some("Time".into()),
            y_title: Some("Survival".into()),
            width_pt: 432.0,
            height_pt: 324.0,
            x_tick_target: 6,
            y_tick_target: 6,
            legend: Some(LegendPosition::TopRight),
            show_censor_ticks: true,
        }
    }
}

/// Compute Kaplan–Meier step points for one group's rows: a
/// sequence of `(t, S(t))` pairs starting at `(0, 1)` and
/// decreasing at each event time. Censored rows are returned
/// separately so a renderer can tick them.
/// Returned from [`km_estimate`]: step-function points for the
/// Kaplan–Meier curve and the censoring tick positions.
pub type KmPoints = (Vec<(f64, f64)>, Vec<(f64, f64)>);

pub fn km_estimate(rows: &[graphcraft_table::SurvivalRow]) -> Result<KmPoints, NumericError> {
    for r in rows {
        if !r.time.is_finite() || r.time < 0.0 {
            return Err(NumericError::NotFinite);
        }
    }
    let mut rows: Vec<&graphcraft_table::SurvivalRow> = rows.iter().collect();
    rows.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let n_total = rows.len();
    if n_total == 0 {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }

    let mut steps = vec![(0.0_f64, 1.0_f64)];
    let mut censor = Vec::new();
    let mut at_risk = n_total;
    let mut s = 1.0_f64;
    let mut i = 0;
    while i < rows.len() {
        // Group rows at this exact time.
        let t = rows[i].time;
        let mut events = 0usize;
        let mut censored = 0usize;
        let mut j = i;
        while j < rows.len() && rows[j].time == t {
            if rows[j].censored {
                censored += 1;
            } else {
                events += 1;
            }
            j += 1;
        }
        if events > 0 {
            // Horizontal step up to time t.
            steps.push((t, s));
            s *= (at_risk as f64 - events as f64) / at_risk as f64;
            steps.push((t, s));
        }
        if censored > 0 {
            for _ in 0..censored {
                censor.push((t, s));
            }
        }
        at_risk -= events + censored;
        i = j;
    }
    Ok((steps, censor))
}

/// Build a Kaplan–Meier scene from a `SurvivalTable`. One curve per
/// group; curve colour cycles through the public colour wheel.
pub fn survival_scene(
    table: &SurvivalTable,
    opts: &SurvivalOptions,
) -> Result<Scene, NumericError> {
    if table.groups.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    let mut series = Vec::new();
    let mut x_hi = 0.0_f64;
    for (i, g) in table.groups.iter().enumerate() {
        if g.rows.is_empty() {
            continue;
        }
        let (steps, censor) = km_estimate(&g.rows)?;
        if let Some(&(t, _)) = steps
            .iter()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        {
            x_hi = x_hi.max(t);
        }
        let color = Color::pick(i);
        series.push(Series::Line {
            label: g.title.clone(),
            points: steps,
            style: LineStyle {
                stroke: color,
                width_pt: 1.5,
            },
        });
        if opts.show_censor_ticks && !censor.is_empty() {
            series.push(Series::CensorTicks {
                label: format!("{} (censored)", g.title),
                points: censor,
                style: LineStyle {
                    stroke: color,
                    width_pt: 1.5,
                },
            });
        }
    }
    if series.is_empty() {
        return Err(NumericError::TooFewValues {
            required: 1,
            got: 0,
        });
    }
    let x_nice = nice_linear(0.0, x_hi.max(1.0), opts.x_tick_target);
    // Y always [0, 1] with nice ticks at 0, 0.25, …, 1.
    let y_ticks = vec![0.0_f64, 0.25, 0.5, 0.75, 1.0];
    let legend = opts.legend.map(|position| Legend { position });
    Ok(Scene {
        width_pt: opts.width_pt,
        height_pt: opts.height_pt,
        margins_pt: Scene::DEFAULT_MARGINS_PT,
        title: opts.title.clone(),
        x_axis: Axis {
            title: opts.x_title.clone(),
            min: 0.0,
            max: x_nice.max,
            ticks: x_nice.ticks,
            scale: AxisScale::Linear,
        },
        y_axis: Axis {
            title: opts.y_title.clone(),
            min: 0.0,
            max: 1.0,
            ticks: y_ticks,
            scale: AxisScale::Linear,
        },
        series,
        legend,
    })
}

#[cfg(test)]
mod tests {
    use graphcraft_table::{SurvivalGroup, SurvivalRow, SurvivalTable};

    use super::*;

    #[test]
    fn km_simple_all_events_no_ties() {
        let rows = vec![
            SurvivalRow {
                time: 1.0,
                censored: false,
            },
            SurvivalRow {
                time: 2.0,
                censored: false,
            },
            SurvivalRow {
                time: 3.0,
                censored: false,
            },
            SurvivalRow {
                time: 4.0,
                censored: false,
            },
        ];
        let (steps, censor) = km_estimate(&rows).unwrap();
        assert!(censor.is_empty());
        // Starts at (0, 1); each event drops by 1/n:
        //   S(1) = 3/4, S(2) = 2/4, S(3) = 1/4, S(4) = 0.
        assert_eq!(steps[0], (0.0, 1.0));
        // Final step should hit 0.
        let last = steps.last().copied().unwrap();
        assert!((last.1 - 0.0).abs() < 1e-15);
    }

    #[test]
    fn km_with_censored_row() {
        let rows = vec![
            SurvivalRow {
                time: 1.0,
                censored: false,
            },
            SurvivalRow {
                time: 2.0,
                censored: true,
            }, // censored
            SurvivalRow {
                time: 3.0,
                censored: false,
            },
            SurvivalRow {
                time: 4.0,
                censored: false,
            },
        ];
        let (steps, censor) = km_estimate(&rows).unwrap();
        assert_eq!(censor.len(), 1);
        // After event at t=1: S = 3/4. Censor at t=2 (no step).
        // Event at t=3: at_risk was 2 (one censored lost), S *= 1/2
        //   → S = 3/8. Event at t=4: at_risk was 1 → S = 0.
        let last = steps.last().copied().unwrap();
        assert!((last.1 - 0.0).abs() < 1e-15);
    }

    #[test]
    fn survival_scene_two_groups() {
        let g1 = SurvivalGroup {
            title: "A".into(),
            rows: vec![
                SurvivalRow {
                    time: 1.0,
                    censored: false,
                },
                SurvivalRow {
                    time: 2.0,
                    censored: true,
                },
                SurvivalRow {
                    time: 4.0,
                    censored: false,
                },
            ],
        };
        let g2 = SurvivalGroup {
            title: "B".into(),
            rows: vec![
                SurvivalRow {
                    time: 1.5,
                    censored: false,
                },
                SurvivalRow {
                    time: 3.0,
                    censored: false,
                },
            ],
        };
        let t = SurvivalTable {
            title: "surv".into(),
            time_title: "days".into(),
            groups: vec![g1, g2],
        };
        let scene = survival_scene(&t, &SurvivalOptions::default()).unwrap();
        assert!(scene.series.len() >= 2);
        assert_eq!(scene.y_axis.ticks, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
    }
}
