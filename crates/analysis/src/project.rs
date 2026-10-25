//! Project-level analysis dispatch.
//!
//! Takes an [`AnalysisSpec`] + a source [`AnyTable`] and runs the
//! corresponding L2/L3 primitive, returning a JSON `Value` whose
//! shape matches the CLI and MCP outputs for the same analysis
//! (so clients can treat all three surfaces identically).
//!
//! The companion [`add_analysis_sheet`] appends an Analysis sheet
//! to a [`Project`], binding it to a Data sheet by id. On compute
//! failure the sheet is still created, but its `result` carries
//! `{ "error": "..." }` instead of a value payload.
//!
//! Source-hash based staleness is scheduled for M8.4; this slice
//! leaves `source_hash` as `None`.

use graphcraft_format::{AnalysisSpec, Project, Sheet, SheetBody, SheetId};
use graphcraft_numeric::NumericError;
use graphcraft_table::{AnyTable, ColumnTable};
use serde_json::{json, Value};

use crate::{
    column_descriptives, column_one_way_anova, column_pearson, column_simple_regression,
    column_spearman, column_unpaired_t,
};

/// Error path for the analysis dispatcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisError {
    /// Referenced sheet id does not exist in the project.
    SourceMissing(SheetId),
    /// Source sheet is not a Data sheet.
    SourceNotData(SheetId),
    /// Source table is not a layout this analysis understands yet
    /// (today, every analysis consumes a Column table).
    WrongTableKind {
        got: &'static str,
        want: &'static str,
    },
    /// Column argument did not resolve to a real column.
    UnknownColumn(String),
    /// The underlying primitive returned a numeric error.
    Numeric(NumericError),
}

impl std::fmt::Display for AnalysisError {
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
        }
    }
}

impl std::error::Error for AnalysisError {}

impl From<NumericError> for AnalysisError {
    fn from(value: NumericError) -> Self {
        AnalysisError::Numeric(value)
    }
}

/// Run `spec` against a source `AnyTable`. Returns the result as a
/// JSON value in the same shape the CLI and MCP already emit.
pub fn run_analysis(table: &AnyTable, spec: &AnalysisSpec) -> Result<Value, AnalysisError> {
    let col_table = require_column_table(table)?;
    match spec {
        AnalysisSpec::Descriptives => run_descriptives(col_table),
        AnalysisSpec::OneWayAnova => run_one_way_anova(col_table),
        AnalysisSpec::WelchT { a, b } => run_welch_t(col_table, a, b),
        AnalysisSpec::Pearson { a, b } => run_pearson(col_table, a, b),
        AnalysisSpec::OlsRegression { x, y } => run_ols(col_table, x, y),
        AnalysisSpec::ShapiroWilk { col } => run_shapiro_wilk(col_table, col),
    }
}

/// Append an Analysis sheet to `project` referencing `source`. The
/// analysis is run eagerly and its result is cached on the sheet.
/// Errors still produce a sheet, with `result` set to
/// `{ "error": "..." }`, so the UI always has something to show.
pub fn add_analysis_sheet(
    project: &mut Project,
    source: SheetId,
    spec: AnalysisSpec,
) -> Result<SheetId, AnalysisError> {
    let table = project
        .get(source)
        .ok_or(AnalysisError::SourceMissing(source))?;
    let table = match &table.body {
        SheetBody::Data { table } => table.clone(),
        _ => return Err(AnalysisError::SourceNotData(source)),
    };
    let title = spec.default_title();
    let result = match run_analysis(&table, &spec) {
        Ok(v) => v,
        Err(e) => json!({ "error": format!("{e}") }),
    };
    let id = project.next_sheet_id();
    project.sheets.push(Sheet {
        id,
        title,
        body: SheetBody::Analysis {
            source,
            spec,
            result,
            source_hash: None,
        },
    });
    Ok(id)
}

// ---------------------------------------------------------------
// Per-spec implementations.
// ---------------------------------------------------------------

fn require_column_table(table: &AnyTable) -> Result<&ColumnTable, AnalysisError> {
    match table {
        AnyTable::Column(c) => Ok(c),
        AnyTable::Xy(_) => Err(AnalysisError::WrongTableKind {
            got: "xy",
            want: "column",
        }),
        AnyTable::Grouped(_) => Err(AnalysisError::WrongTableKind {
            got: "grouped",
            want: "column",
        }),
        AnyTable::Contingency(_) => Err(AnalysisError::WrongTableKind {
            got: "contingency",
            want: "column",
        }),
        AnyTable::Survival(_) => Err(AnalysisError::WrongTableKind {
            got: "survival",
            want: "column",
        }),
        AnyTable::PartsOfWhole(_) => Err(AnalysisError::WrongTableKind {
            got: "parts_of_whole",
            want: "column",
        }),
        AnyTable::MultipleVariables(_) => Err(AnalysisError::WrongTableKind {
            got: "multiple_variables",
            want: "column",
        }),
        AnyTable::Nested(_) => Err(AnalysisError::WrongTableKind {
            got: "nested",
            want: "column",
        }),
    }
}

fn resolve_column(table: &ColumnTable, spec: &str) -> Result<usize, AnalysisError> {
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
    Err(AnalysisError::UnknownColumn(spec.to_string()))
}

fn run_descriptives(t: &ColumnTable) -> Result<Value, AnalysisError> {
    let rows = column_descriptives(t)?;
    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "group": r.group_title,
                "n": r.stats.n, "mean": r.stats.mean,
                "sd": r.stats.sd, "variance": r.stats.variance,
                "sem": r.stats.sem, "median": r.stats.median,
                "min": r.stats.min, "max": r.stats.max,
            })
        })
        .collect();
    Ok(json!({ "descriptives": out }))
}

fn run_one_way_anova(t: &ColumnTable) -> Result<Value, AnalysisError> {
    let a = column_one_way_anova(t)?;
    Ok(json!({
        "k": a.k, "n_total": a.n_total,
        "group_ns": a.group_ns, "group_means": a.group_means,
        "grand_mean": a.grand_mean,
        "ss_between": a.ss_between, "ss_within": a.ss_within,
        "df_between": a.df_between, "df_within": a.df_within,
        "f": a.f, "p": a.p,
    }))
}

fn run_welch_t(t: &ColumnTable, a: &str, b: &str) -> Result<Value, AnalysisError> {
    let ia = resolve_column(t, a)?;
    let ib = resolve_column(t, b)?;
    let r = column_unpaired_t(t, ia, ib, graphcraft_stats::TwoSampleKind::Welch)?;
    Ok(json!({
        "col_a": &t.columns[ia].title,
        "col_b": &t.columns[ib].title,
        "n1": r.n1, "n2": r.n2,
        "mean1": r.mean1, "mean2": r.mean2,
        "mean_diff": r.mean_diff, "se_diff": r.se_diff,
        "t": r.t, "df": r.df, "p_two_sided": r.p_two_sided,
    }))
}

fn run_pearson(t: &ColumnTable, a: &str, b: &str) -> Result<Value, AnalysisError> {
    let ia = resolve_column(t, a)?;
    let ib = resolve_column(t, b)?;
    let p = column_pearson(t, ia, ib)?;
    let s = column_spearman(t, ia, ib)?;
    Ok(json!({
        "pearson": {
            "n": p.n, "r": p.r, "t": p.t, "df": p.df,
            "p_two_sided": p.p_two_sided,
            "ci95": [p.ci95.0, p.ci95.1],
        },
        "spearman": {
            "n": s.n, "r": s.r, "t": s.t, "df": s.df,
            "p_two_sided": s.p_two_sided,
        }
    }))
}

fn run_ols(t: &ColumnTable, x: &str, y: &str) -> Result<Value, AnalysisError> {
    let ix = resolve_column(t, x)?;
    let iy = resolve_column(t, y)?;
    let r = column_simple_regression(t, ix, iy)?;
    Ok(json!({
        "intercept": r.coefficients[0],
        "slope": r.coefficients[1],
        "se_intercept": r.se[0],
        "se_slope": r.se[1],
        "t_intercept": r.t[0],
        "t_slope": r.t[1],
        "p_intercept": r.p[0],
        "p_slope": r.p[1],
        "rss": r.rss, "r_squared": r.r_squared,
        "adj_r_squared": r.adj_r_squared,
        "residual_se": r.residual_se,
        "f_stat": r.f_stat, "f_p": r.f_p, "n": r.n,
    }))
}

fn run_shapiro_wilk(t: &ColumnTable, col: &str) -> Result<Value, AnalysisError> {
    let i = resolve_column(t, col)?;
    let values: Vec<f64> = t.group_values(i).collect();
    let r = graphcraft_stats::shapiro_wilk(&values)?;
    Ok(json!({
        "col": &t.columns[i].title,
        "n": r.n, "w": r.w, "z": r.z, "p": r.p,
    }))
}

#[cfg(test)]
mod tests {
    use graphcraft_format::{AnalysisSpec, Project, SheetBody};
    use graphcraft_table::{Column, ColumnTable};

    use super::*;

    fn sample_project() -> (Project, SheetId) {
        let table = ColumnTable {
            title: "demo".into(),
            row_titles: Vec::new(),
            columns: vec![
                Column {
                    title: "placebo".into(),
                    values: vec![
                        Some(2.1),
                        Some(3.4),
                        Some(2.9),
                        Some(3.1),
                        Some(2.4),
                        Some(3.8),
                        Some(2.7),
                        Some(3.2),
                        Some(2.5),
                        Some(3.6),
                    ],
                },
                Column {
                    title: "treatment".into(),
                    values: vec![
                        Some(4.3),
                        Some(5.1),
                        Some(6.2),
                        Some(4.8),
                        Some(5.9),
                        Some(6.4),
                        Some(5.5),
                        Some(7.1),
                        Some(4.9),
                        Some(5.8),
                    ],
                },
            ],
        };
        let mut p = Project::new("demo");
        let id = p.add_data_sheet("clinical", table.into());
        (p, id)
    }

    #[test]
    fn descriptives_sheet_round_trips() {
        let (mut p, src) = sample_project();
        let id = add_analysis_sheet(&mut p, src, AnalysisSpec::Descriptives).unwrap();
        let sheet = p.get(id).unwrap();
        assert_eq!(sheet.title, "Descriptives");
        match &sheet.body {
            SheetBody::Analysis {
                source,
                spec,
                result,
                ..
            } => {
                assert_eq!(*source, src);
                assert_eq!(spec, &AnalysisSpec::Descriptives);
                let rows = result["descriptives"].as_array().unwrap();
                assert_eq!(rows.len(), 2);
            }
            _ => panic!("expected Analysis body"),
        }
    }

    #[test]
    fn welch_t_sheet_pulls_mean_diff() {
        let (mut p, src) = sample_project();
        let id = add_analysis_sheet(
            &mut p,
            src,
            AnalysisSpec::WelchT {
                a: "placebo".into(),
                b: "treatment".into(),
            },
        )
        .unwrap();
        let sheet = p.get(id).unwrap();
        assert!(sheet.title.contains("Welch"));
        if let SheetBody::Analysis { result, .. } = &sheet.body {
            let diff = result["mean_diff"].as_f64().unwrap();
            assert!((diff - 2.63).abs() < 1e-6, "mean_diff = {diff}");
        } else {
            panic!("expected Analysis body");
        }
    }

    #[test]
    fn each_spec_produces_a_result_or_error_sheet() {
        let (mut p, src) = sample_project();
        let specs = [
            AnalysisSpec::Descriptives,
            AnalysisSpec::OneWayAnova,
            AnalysisSpec::WelchT {
                a: "0".into(),
                b: "1".into(),
            },
            AnalysisSpec::Pearson {
                a: "placebo".into(),
                b: "treatment".into(),
            },
            AnalysisSpec::OlsRegression {
                x: "placebo".into(),
                y: "treatment".into(),
            },
            AnalysisSpec::ShapiroWilk {
                col: "placebo".into(),
            },
        ];
        for spec in specs {
            let id = add_analysis_sheet(&mut p, src, spec).unwrap();
            let sheet = p.get(id).unwrap();
            assert!(matches!(sheet.body, SheetBody::Analysis { .. }));
        }
        // 1 Data + 6 Analysis.
        assert_eq!(p.data_sheet_count(), 1);
        assert_eq!(p.analysis_sheet_count(), 6);
    }

    #[test]
    fn missing_source_rejected() {
        let (mut p, _) = sample_project();
        let err = add_analysis_sheet(&mut p, SheetId(999), AnalysisSpec::Descriptives).unwrap_err();
        assert!(matches!(err, AnalysisError::SourceMissing(_)));
    }

    #[test]
    fn unknown_column_persists_as_error_sheet() {
        let (mut p, src) = sample_project();
        let id = add_analysis_sheet(
            &mut p,
            src,
            AnalysisSpec::WelchT {
                a: "nope".into(),
                b: "treatment".into(),
            },
        )
        .unwrap();
        if let SheetBody::Analysis { result, .. } = &p.get(id).unwrap().body {
            assert!(result["error"].as_str().unwrap().contains("nope"));
        } else {
            panic!("expected Analysis body");
        }
    }
}
