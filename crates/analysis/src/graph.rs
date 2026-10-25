//! Project-level graph dispatch.
//!
//! Takes a [`GraphSpec`] + a source [`AnyTable`] and builds a
//! [`Scene`]. The scene is *not* cached in the file — it's
//! derivable from `(source, spec)`, so storing it duplicates data
//! that would rot the moment the source changes. The analysis-side
//! [`add_graph_sheet`] just records the binding.

use graphcraft_format::{
    source_hash, GraphColormap, GraphErrorBars, GraphSpec, LegendMarker, LegendRegistry, Project,
    Sheet, SheetBody, SheetId,
};
use graphcraft_numeric::NumericError;
use graphcraft_plot::{
    column_scene, heatmap_scene, xy_scene_from_points, Color, Colormap, ColumnErrorBars,
    ColumnOptions, HeatmapData, HeatmapOptions, Marker, Scene, Series, XyOptions,
};
use graphcraft_table::{AnyTable, ColumnTable};

/// Error path for the graph dispatcher.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphError {
    SourceMissing(SheetId),
    SourceNotData(SheetId),
    WrongTableKind {
        got: &'static str,
        want: &'static str,
    },
    UnknownColumn(String),
    Numeric(NumericError),
    /// A spec kind the dispatcher does not yet build for this
    /// source-table kind.
    Unsupported(String),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceMissing(id) => write!(f, "source sheet {} not found", id.0),
            Self::SourceNotData(id) => {
                write!(f, "source sheet {} is not a data sheet", id.0)
            }
            Self::WrongTableKind { got, want } => {
                write!(f, "wrong source table kind: got {got}, need {want}")
            }
            Self::UnknownColumn(c) => write!(f, "unknown column {c:?}"),
            Self::Numeric(e) => write!(f, "{e}"),
            Self::Unsupported(msg) => write!(f, "unsupported: {msg}"),
        }
    }
}

impl std::error::Error for GraphError {}

impl From<NumericError> for GraphError {
    fn from(value: NumericError) -> Self {
        GraphError::Numeric(value)
    }
}

/// Build a scene from `(table, spec)` and apply the project's
/// legend registry so series inherit the user-chosen colour for
/// each key. Call this (instead of the bare `build_graph_scene`)
/// from any UI that respects shared styling.
pub fn build_graph_scene_in_project(
    project: &Project,
    source: SheetId,
    spec: &GraphSpec,
) -> Result<Scene, GraphError> {
    let table = project
        .data_sheet_table(source)
        .ok_or(GraphError::SourceMissing(source))?;
    let mut scene = build_graph_scene(table, spec)?;
    apply_legend(&mut scene, &project.legend_registry);
    Ok(scene)
}

/// Overwrite scene series colours from the registry. Series
/// without a matching key keep whatever colour the plot layer
/// gave them (the "allow, not force" default).
pub fn apply_legend(scene: &mut Scene, registry: &LegendRegistry) {
    for series in &mut scene.series {
        match series {
            Series::Scatter { label, marker, .. } => {
                if let Some(e) = registry.get(label) {
                    marker.fill = Color(e.color.0, e.color.1, e.color.2);
                    marker.shape = marker_from_legend(e.marker);
                    marker.size_pt = e.marker_size_pt;
                }
            }
            Series::Line { label, style, .. } => {
                if let Some(e) = registry.get(label) {
                    style.stroke = Color(e.color.0, e.color.1, e.color.2);
                    style.width_pt = e.line_width_pt;
                }
            }
            Series::Bars { bars, .. } => {
                // Each bar carries its own column title in `label`
                // and its own optional `color`. Look each one up
                // in the registry and override the per-bar fill;
                // a miss leaves `None` so the shared series fill
                // continues to apply. Marker shape / size doesn't
                // apply to rectangular bars.
                for b in bars.iter_mut() {
                    if let Some(e) = registry.get(&b.label) {
                        b.color = Some(Color(e.color.0, e.color.1, e.color.2));
                    }
                }
            }
            Series::GroupedBars { label, marker, .. } => {
                if let Some(e) = registry.get(label) {
                    marker.fill = Color(e.color.0, e.color.1, e.color.2);
                }
            }
            Series::CensorTicks { label, style, .. } => {
                if let Some(e) = registry.get(label) {
                    style.stroke = Color(e.color.0, e.color.1, e.color.2);
                    style.width_pt = e.line_width_pt;
                }
            }
            Series::Heatmap { .. } => {
                // Heatmaps use a colormap; no per-series legend
                // override (M10+).
            }
        }
    }
}

fn marker_from_legend(m: LegendMarker) -> Marker {
    match m {
        LegendMarker::Circle => Marker::Circle,
        LegendMarker::Square => Marker::Square,
        LegendMarker::Triangle => Marker::Triangle,
        LegendMarker::None => Marker::None,
    }
}

/// Build a scene from `(table, spec)`.
pub fn build_graph_scene(table: &AnyTable, spec: &GraphSpec) -> Result<Scene, GraphError> {
    match spec {
        GraphSpec::Placeholder => Err(GraphError::Unsupported(
            "graph not configured yet — pick a kind in the detail pane".into(),
        )),
        GraphSpec::Xy { x, y, title } => build_xy(table, x, y, title.as_deref()),
        GraphSpec::Column {
            title,
            error_bars,
            y_title,
        } => build_column(table, title.as_deref(), *error_bars, y_title.as_deref()),
        GraphSpec::Grouped { .. } => Err(GraphError::Unsupported(
            "Grouped graphs require a Grouped data sheet (not yet produced by import)".into(),
        )),
        GraphSpec::Survival { .. } => Err(GraphError::Unsupported(
            "Survival graphs require a Survival data sheet (not yet produced by import)".into(),
        )),
        GraphSpec::Heatmap { title, colormap } => build_heatmap(table, title.as_deref(), *colormap),
    }
}

/// Append a Graph sheet to `project` referencing `source`. The
/// scene is not computed here — the caller or UI calls
/// [`build_graph_scene`] on demand.
pub fn add_graph_sheet(
    project: &mut Project,
    source: SheetId,
    spec: GraphSpec,
) -> Result<SheetId, GraphError> {
    let sheet = project
        .get(source)
        .ok_or(GraphError::SourceMissing(source))?;
    let hash = match &sheet.body {
        SheetBody::Data { table } => source_hash(table),
        _ => return Err(GraphError::SourceNotData(source)),
    };
    let title = spec.default_title();
    let id = project.next_sheet_id();
    project.sheets.push(Sheet {
        id,
        title,
        body: SheetBody::Graph {
            source,
            spec,
            source_hash: Some(hash),
        },
    });
    Ok(id)
}

/// Refresh the `source_hash` of a Graph sheet to the current
/// source table. (The scene itself is derived on demand, so there
/// is nothing else to update.) Fails if the sheet or its source
/// has disappeared.
pub fn recompute_graph_sheet(project: &mut Project, id: SheetId) -> Result<(), GraphError> {
    let source = match project.get(id) {
        Some(s) => match &s.body {
            SheetBody::Graph { source, .. } => *source,
            _ => return Err(GraphError::SourceNotData(id)),
        },
        None => return Err(GraphError::SourceMissing(id)),
    };
    let table = project
        .data_sheet_table(source)
        .ok_or(GraphError::SourceMissing(source))?
        .clone();
    let hash = source_hash(&table);
    if let Some(sheet) = project.get_mut(id) {
        if let SheetBody::Graph { source_hash: h, .. } = &mut sheet.body {
            *h = Some(hash);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------
// Per-spec implementations.
// ---------------------------------------------------------------

fn require_column_table(table: &AnyTable) -> Result<&ColumnTable, GraphError> {
    match table {
        AnyTable::Column(c) => Ok(c),
        other => Err(GraphError::WrongTableKind {
            got: table_kind(other),
            want: "column",
        }),
    }
}

fn table_kind(t: &AnyTable) -> &'static str {
    match t {
        AnyTable::Column(_) => "column",
        AnyTable::Xy(_) => "xy",
        AnyTable::Grouped(_) => "grouped",
        AnyTable::Contingency(_) => "contingency",
        AnyTable::Survival(_) => "survival",
        AnyTable::PartsOfWhole(_) => "parts_of_whole",
        AnyTable::MultipleVariables(_) => "multiple_variables",
        AnyTable::Nested(_) => "nested",
    }
}

fn resolve_column(table: &ColumnTable, spec: &str) -> Result<usize, GraphError> {
    if let Ok(i) = spec.parse::<usize>() {
        if i < table.columns.len() {
            return Ok(i);
        }
    }
    for (i, c) in table.columns.iter().enumerate() {
        if c.title == spec {
            return Ok(i);
        }
    }
    Err(GraphError::UnknownColumn(spec.to_string()))
}

fn build_xy(table: &AnyTable, x: &str, y: &str, title: Option<&str>) -> Result<Scene, GraphError> {
    let t = require_column_table(table)?;
    let ix = resolve_column(t, x)?;
    let iy = resolve_column(t, y)?;
    let xs = &t.columns[ix].values;
    let ys = &t.columns[iy].values;
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for (a, b) in xs.iter().zip(ys.iter()) {
        if let (Some(xa), Some(yb)) = (a, b) {
            if xa.is_finite() && yb.is_finite() {
                pts.push((*xa, *yb));
            }
        }
    }
    if pts.is_empty() {
        return Err(GraphError::Numeric(NumericError::TooFewValues {
            required: 1,
            got: 0,
        }));
    }
    let opts = XyOptions {
        title: title
            .map(|s| s.to_string())
            .or_else(|| Some(t.title.clone())),
        x_title: Some(t.columns[ix].title.clone()),
        y_title: Some(t.columns[iy].title.clone()),
        ..XyOptions::default()
    };
    // Use the y column's title as the series label so the legend
    // registry (keyed on column titles) finds it.
    Ok(xy_scene_from_points(
        &pts,
        &opts,
        t.columns[iy].title.clone(),
    )?)
}

fn build_column(
    table: &AnyTable,
    title: Option<&str>,
    error_bars: GraphErrorBars,
    y_title: Option<&str>,
) -> Result<Scene, GraphError> {
    let t = require_column_table(table)?;
    let opts = ColumnOptions {
        title: title
            .map(|s| s.to_string())
            .or_else(|| Some(t.title.clone())),
        y_title: y_title.map(|s| s.to_string()),
        error_bars: match error_bars {
            GraphErrorBars::None => ColumnErrorBars::None,
            GraphErrorBars::Sd => ColumnErrorBars::Sd,
            GraphErrorBars::Sem => ColumnErrorBars::Sem,
        },
        ..ColumnOptions::default()
    };
    Ok(column_scene(t, &opts)?)
}

fn build_heatmap(
    table: &AnyTable,
    title: Option<&str>,
    colormap: GraphColormap,
) -> Result<Scene, GraphError> {
    let t = require_column_table(table)?;
    if t.columns.is_empty() {
        return Err(GraphError::Numeric(NumericError::TooFewValues {
            required: 1,
            got: 0,
        }));
    }
    let n_rows = t.columns.iter().map(|c| c.values.len()).max().unwrap_or(0);
    if n_rows == 0 {
        return Err(GraphError::Numeric(NumericError::TooFewValues {
            required: 1,
            got: 0,
        }));
    }
    // Build a row-major matrix: values[row][col] = t.columns[col].values[row].
    let mut values: Vec<Vec<Option<f64>>> = vec![vec![None; t.columns.len()]; n_rows];
    for (ci, col) in t.columns.iter().enumerate() {
        for (ri, v) in col.values.iter().enumerate() {
            values[ri][ci] = *v;
        }
    }
    let col_titles: Vec<String> = t.columns.iter().map(|c| c.title.clone()).collect();
    let row_titles: Vec<String> = t.row_titles.clone();
    let data = HeatmapData {
        values: &values,
        row_titles: &row_titles,
        col_titles: &col_titles,
    };
    let opts = HeatmapOptions {
        title: title
            .map(|s| s.to_string())
            .or_else(|| Some(t.title.clone())),
        colormap: match colormap {
            GraphColormap::DivergingBlueRed => Colormap::DivergingBlueRed,
            GraphColormap::Grayscale => Colormap::Grayscale,
            GraphColormap::SequentialBlue => Colormap::SequentialBlue,
        },
        ..HeatmapOptions::default()
    };
    Ok(heatmap_scene(data, &opts)?)
}

#[cfg(test)]
mod tests {
    use graphcraft_format::{GraphSpec, Project, SheetBody};
    use graphcraft_plot::Series;
    use graphcraft_table::{Column, ColumnTable};

    use super::*;

    fn sample_project() -> (Project, SheetId) {
        let table = ColumnTable {
            title: "demo".into(),
            row_titles: Vec::new(),
            columns: vec![
                Column {
                    title: "x".into(),
                    values: vec![Some(1.0), Some(2.0), Some(3.0), Some(4.0), Some(5.0)],
                },
                Column {
                    title: "y".into(),
                    values: vec![Some(2.0), Some(4.0), Some(5.0), Some(4.0), Some(5.0)],
                },
            ],
        };
        let mut p = Project::new("demo");
        let id = p.add_data_sheet("samples", table.into());
        (p, id)
    }

    #[test]
    fn xy_graph_sheet_round_trips() {
        let (mut p, src) = sample_project();
        let id = add_graph_sheet(
            &mut p,
            src,
            GraphSpec::Xy {
                x: "x".into(),
                y: "y".into(),
                title: None,
            },
        )
        .unwrap();
        let sheet = p.get(id).unwrap();
        assert!(sheet.title.starts_with("Scatter"));
        assert!(matches!(sheet.body, SheetBody::Graph { .. }));
        // Scene builds on demand.
        if let SheetBody::Graph { spec, .. } = &sheet.body {
            let table = p.data_sheet_table(src).unwrap();
            let scene = build_graph_scene(table, spec).unwrap();
            assert_eq!(scene.series.len(), 1);
            matches!(scene.series[0], Series::Scatter { .. });
        }
    }

    #[test]
    fn column_graph_builds_bars() {
        let (mut p, src) = sample_project();
        let id = add_graph_sheet(
            &mut p,
            src,
            GraphSpec::Column {
                title: None,
                error_bars: GraphErrorBars::Sd,
                y_title: None,
            },
        )
        .unwrap();
        if let SheetBody::Graph { spec, .. } = &p.get(id).unwrap().body {
            let table = p.data_sheet_table(src).unwrap();
            let scene = build_graph_scene(table, spec).unwrap();
            assert_eq!(scene.series.len(), 1);
            match &scene.series[0] {
                Series::Bars { bars, .. } => assert_eq!(bars.len(), 2),
                _ => panic!("expected Bars"),
            }
        }
    }

    #[test]
    fn heatmap_graph_builds_cells() {
        let (mut p, src) = sample_project();
        let id = add_graph_sheet(
            &mut p,
            src,
            GraphSpec::Heatmap {
                title: None,
                colormap: GraphColormap::Grayscale,
            },
        )
        .unwrap();
        if let SheetBody::Graph { spec, .. } = &p.get(id).unwrap().body {
            let table = p.data_sheet_table(src).unwrap();
            let scene = build_graph_scene(table, spec).unwrap();
            match &scene.series[0] {
                Series::Heatmap {
                    n_rows,
                    n_cols,
                    cells,
                    ..
                } => {
                    assert_eq!(*n_rows, 5);
                    assert_eq!(*n_cols, 2);
                    assert_eq!(cells.len(), 10);
                }
                _ => panic!("expected Heatmap"),
            }
        }
    }

    #[test]
    fn grouped_on_column_source_errors() {
        let (mut p, src) = sample_project();
        let id = add_graph_sheet(
            &mut p,
            src,
            GraphSpec::Grouped {
                title: None,
                error_bars: GraphErrorBars::Sd,
            },
        )
        .unwrap();
        // The sheet is created, but build_graph_scene errors.
        if let SheetBody::Graph { spec, .. } = &p.get(id).unwrap().body {
            let table = p.data_sheet_table(src).unwrap();
            assert!(matches!(
                build_graph_scene(table, spec),
                Err(GraphError::Unsupported(_))
            ));
        }
    }

    #[test]
    fn unknown_column_rejected() {
        let (mut p, src) = sample_project();
        let id = add_graph_sheet(
            &mut p,
            src,
            GraphSpec::Xy {
                x: "nope".into(),
                y: "y".into(),
                title: None,
            },
        )
        .unwrap();
        if let SheetBody::Graph { spec, .. } = &p.get(id).unwrap().body {
            let table = p.data_sheet_table(src).unwrap();
            assert!(matches!(
                build_graph_scene(table, spec),
                Err(GraphError::UnknownColumn(_))
            ));
        }
    }

    #[test]
    fn mutating_source_marks_graph_stale() {
        use graphcraft_format::{source_hash, SheetStatus};
        let (mut p, src) = sample_project();
        let g = add_graph_sheet(
            &mut p,
            src,
            GraphSpec::Column {
                title: None,
                error_bars: GraphErrorBars::Sd,
                y_title: None,
            },
        )
        .unwrap();
        assert_eq!(p.sheet_status(g), SheetStatus::Fresh);

        if let SheetBody::Data {
            table: graphcraft_table::AnyTable::Column(t),
        } = &mut p.get_mut(src).unwrap().body
        {
            t.columns[0].values[0] = Some(999.0);
        }
        assert_eq!(p.sheet_status(g), SheetStatus::Stale);

        recompute_graph_sheet(&mut p, g).unwrap();
        assert_eq!(p.sheet_status(g), SheetStatus::Fresh);

        let current = source_hash(p.data_sheet_table(src).unwrap());
        if let SheetBody::Graph { source_hash: h, .. } = &p.get(g).unwrap().body {
            assert_eq!(h.as_deref(), Some(current.as_str()));
        }
    }

    #[test]
    fn graph_sheet_count_tracks_adds() {
        let (mut p, src) = sample_project();
        add_graph_sheet(
            &mut p,
            src,
            GraphSpec::Xy {
                x: "x".into(),
                y: "y".into(),
                title: None,
            },
        )
        .unwrap();
        add_graph_sheet(
            &mut p,
            src,
            GraphSpec::Column {
                title: None,
                error_bars: GraphErrorBars::Sem,
                y_title: None,
            },
        )
        .unwrap();
        assert_eq!(p.graph_sheet_count(), 2);
        assert_eq!(p.data_sheet_count(), 1);
    }
}
