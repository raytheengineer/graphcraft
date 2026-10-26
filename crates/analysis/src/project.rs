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

use graphcraft_format::{source_hash, AnalysisSpec, Project, Sheet, SheetBody, SheetId};
use graphcraft_numeric::NumericError;
use graphcraft_table::{AnyTable, ColumnTable, ContingencyTable, GroupedTable, SurvivalTable};
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
    match spec {
        AnalysisSpec::Descriptives => run_descriptives(require_column_table(table)?),
        AnalysisSpec::OneWayAnova => run_one_way_anova(require_column_table(table)?),
        AnalysisSpec::WelchT { a, b } => run_welch_t(require_column_table(table)?, a, b),
        AnalysisSpec::Pearson { a, b } => run_pearson(require_column_table(table)?, a, b),
        AnalysisSpec::OlsRegression { x, y } => run_ols(require_column_table(table)?, x, y),
        AnalysisSpec::ShapiroWilk { col } => run_shapiro_wilk(require_column_table(table)?, col),
        AnalysisSpec::GroupedOneWayAnova => {
            run_grouped_one_way_anova(require_grouped_table(table)?)
        }
        AnalysisSpec::LogRank { a, b } => run_log_rank(require_survival_table(table)?, a, b),
        AnalysisSpec::ChiSquare => run_chi_square(require_contingency_table(table)?),
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
    let sheet = project
        .get(source)
        .ok_or(AnalysisError::SourceMissing(source))?;
    let table = match &sheet.body {
        SheetBody::Data { table } => table.clone(),
        _ => return Err(AnalysisError::SourceNotData(source)),
    };
    let title = spec.default_title();
    let hash = source_hash(&table);
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
            source_hash: Some(hash),
        },
    });
    Ok(id)
}

/// Re-run the analysis stored at `id`, replacing its cached result
/// and refreshing its `source_hash`. Fails only if the sheet or
/// its source has disappeared; a compute failure is persisted as
/// an error-shaped result, same as [`add_analysis_sheet`].
pub fn recompute_analysis_sheet(project: &mut Project, id: SheetId) -> Result<(), AnalysisError> {
    let (source, spec) = match project.get(id) {
        Some(s) => match &s.body {
            SheetBody::Analysis { source, spec, .. } => (*source, spec.clone()),
            _ => return Err(AnalysisError::SourceNotData(id)),
        },
        None => return Err(AnalysisError::SourceMissing(id)),
    };
    let table = project
        .data_sheet_table(source)
        .ok_or(AnalysisError::SourceMissing(source))?
        .clone();
    let hash = source_hash(&table);
    let result = match run_analysis(&table, &spec) {
        Ok(v) => v,
        Err(e) => json!({ "error": format!("{e}") }),
    };
    if let Some(sheet) = project.get_mut(id) {
        if let SheetBody::Analysis {
            result: r,
            source_hash: h,
            ..
        } = &mut sheet.body
        {
            *r = result;
            *h = Some(hash);
        }
    }
    Ok(())
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

fn require_grouped_table(table: &AnyTable) -> Result<&GroupedTable, AnalysisError> {
    match table {
        AnyTable::Grouped(g) => Ok(g),
        other => Err(AnalysisError::WrongTableKind {
            got: table_kind_str(other),
            want: "grouped",
        }),
    }
}

fn require_survival_table(table: &AnyTable) -> Result<&SurvivalTable, AnalysisError> {
    match table {
        AnyTable::Survival(s) => Ok(s),
        other => Err(AnalysisError::WrongTableKind {
            got: table_kind_str(other),
            want: "survival",
        }),
    }
}

fn require_contingency_table(table: &AnyTable) -> Result<&ContingencyTable, AnalysisError> {
    match table {
        AnyTable::Contingency(c) => Ok(c),
        other => Err(AnalysisError::WrongTableKind {
            got: table_kind_str(other),
            want: "contingency",
        }),
    }
}

fn table_kind_str(t: &AnyTable) -> &'static str {
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

fn run_grouped_one_way_anova(t: &GroupedTable) -> Result<Value, AnalysisError> {
    // Flatten each group's replicate subcolumns into a single
    // Vec<f64> of non-missing values; feed to the existing
    // one-way-anova primitive.
    let groups: Vec<Vec<f64>> = t
        .groups
        .iter()
        .map(|g| {
            let mut flat: Vec<f64> = Vec::new();
            for sub in &g.subcolumns {
                for x in sub.values.iter().flatten() {
                    if x.is_finite() {
                        flat.push(*x);
                    }
                }
            }
            flat
        })
        .collect();
    let labels: Vec<String> = t.groups.iter().map(|g| g.title.clone()).collect();
    let a = graphcraft_stats::one_way_anova(&groups)?;
    Ok(json!({
        "kind": "grouped-one-way-anova",
        "groups": labels,
        "k": a.k,
        "n_total": a.n_total,
        "group_ns": a.group_ns,
        "ss_between": a.ss_between,
        "ss_within": a.ss_within,
        "ss_total": a.ss_total,
        "df_between": a.df_between,
        "df_within": a.df_within,
        "ms_between": a.ms_between,
        "ms_within": a.ms_within,
        "f": a.f,
        "p": a.p,
    }))
}

fn run_log_rank(t: &SurvivalTable, a: &str, b: &str) -> Result<Value, AnalysisError> {
    let g1 = resolve_survival_group(t, a)?;
    let g2 = resolve_survival_group(t, b)?;
    let rows1: Vec<(f64, bool)> = t.groups[g1]
        .rows
        .iter()
        .map(|r| (r.time, r.censored))
        .collect();
    let rows2: Vec<(f64, bool)> = t.groups[g2]
        .rows
        .iter()
        .map(|r| (r.time, r.censored))
        .collect();
    let r = graphcraft_stats::log_rank(&rows1, &rows2)?;
    Ok(json!({
        "kind": "log-rank",
        "group_a": t.groups[g1].title.clone(),
        "group_b": t.groups[g2].title.clone(),
        "n1": r.n1,
        "n2": r.n2,
        "o1_minus_e1": r.o1_minus_e1,
        "variance": r.variance,
        "chi2": r.chi2,
        "df": r.df,
        "p": r.p,
        "log_hr": r.log_hr,
        "se_log_hr": r.se_log_hr,
        "hr": r.hr,
        "hr_ci95": [r.hr_ci95.0, r.hr_ci95.1],
    }))
}

fn resolve_survival_group(t: &SurvivalTable, s: &str) -> Result<usize, AnalysisError> {
    if let Ok(i) = s.parse::<usize>() {
        if i < t.groups.len() {
            return Ok(i);
        }
    }
    t.groups
        .iter()
        .position(|g| g.title == s)
        .ok_or_else(|| AnalysisError::UnknownColumn(s.to_string()))
}

fn run_chi_square(t: &ContingencyTable) -> Result<Value, AnalysisError> {
    let c = graphcraft_stats::chi_squared_independence(&t.counts)?;
    let mut out = json!({
        "kind": "chi-square",
        "n_rows": c.n_rows,
        "n_cols": c.n_cols,
        "n": c.n,
        "observed": c.observed,
        "expected": c.expected,
        "chi2": c.chi2,
        "df": c.df,
        "p": c.p,
    });
    // For a 2x2 table, add Fisher's exact alongside — the public
    // Prism manual recommends Fisher for small expected counts;
    // we always include it for 2x2 so callers can choose.
    if c.n_rows == 2 && c.n_cols == 2 {
        let a = t.counts[0][0];
        let b = t.counts[0][1];
        let cc = t.counts[1][0];
        let d = t.counts[1][1];
        if let Ok(f) = graphcraft_stats::fisher_exact_2x2(a, b, cc, d) {
            out["fisher_exact"] = json!({
                "a": f.a,
                "b": f.b,
                "c": f.c,
                "d": f.d,
                "p_observed": f.p_observed,
                "p_two_sided": f.p_two_sided,
                "p_one_sided": f.p_one_sided,
                "odds_ratio": f.odds_ratio,
            });
        }
    }
    Ok(out)
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
    fn mutating_source_marks_dependents_stale() {
        use graphcraft_format::{source_hash, SheetStatus};
        let (mut p, src) = sample_project();
        let ana = add_analysis_sheet(&mut p, src, AnalysisSpec::Descriptives).unwrap();
        assert_eq!(p.sheet_status(ana), SheetStatus::Fresh);

        // Reach into the source table and push a row.
        if let SheetBody::Data {
            table: graphcraft_table::AnyTable::Column(t),
        } = &mut p.get_mut(src).unwrap().body
        {
            t.columns[0].values.push(Some(99.0));
        }

        // Now the analysis is stale against the new source hash.
        assert_eq!(p.sheet_status(ana), SheetStatus::Stale);

        // Recompute refreshes the hash and (re-computes) the result.
        recompute_analysis_sheet(&mut p, ana).unwrap();
        assert_eq!(p.sheet_status(ana), SheetStatus::Fresh);

        // And the stored hash matches the current source.
        let current = source_hash(p.data_sheet_table(src).unwrap());
        if let SheetBody::Analysis { source_hash: h, .. } = &p.get(ana).unwrap().body {
            assert_eq!(h.as_deref(), Some(current.as_str()));
        }
    }

    #[test]
    fn missing_source_rejected() {
        let (mut p, _) = sample_project();
        let err = add_analysis_sheet(&mut p, SheetId(999), AnalysisSpec::Descriptives).unwrap_err();
        assert!(matches!(err, AnalysisError::SourceMissing(_)));
    }

    #[test]
    fn grouped_anova_flattens_replicates() {
        use graphcraft_table::{Column, GroupColumn, GroupedTable};
        let t = GroupedTable {
            title: "g".into(),
            row_titles: vec![],
            groups: vec![
                GroupColumn {
                    title: "A".into(),
                    subcolumns: vec![Column {
                        title: "".into(),
                        values: vec![Some(1.0), Some(2.0), Some(3.0)],
                    }],
                },
                GroupColumn {
                    title: "B".into(),
                    subcolumns: vec![Column {
                        title: "".into(),
                        values: vec![Some(10.0), Some(11.0), Some(12.0)],
                    }],
                },
                GroupColumn {
                    title: "C".into(),
                    subcolumns: vec![Column {
                        title: "".into(),
                        values: vec![Some(20.0), Some(21.0), Some(22.0)],
                    }],
                },
            ],
        };
        let v = run_analysis(&AnyTable::Grouped(t), &AnalysisSpec::GroupedOneWayAnova).unwrap();
        assert_eq!(v["k"], 3);
        assert_eq!(v["n_total"], 9);
        assert!(v["f"].as_f64().unwrap() > 0.0);
        assert!(v["p"].as_f64().unwrap() < 1e-4);
    }

    #[test]
    fn log_rank_resolves_group_names() {
        use graphcraft_table::{SurvivalGroup, SurvivalRow, SurvivalTable};
        let t = SurvivalTable {
            title: "s".into(),
            time_title: "t".into(),
            groups: vec![
                SurvivalGroup {
                    title: "Ctrl".into(),
                    rows: vec![
                        SurvivalRow {
                            time: 2.0,
                            censored: false,
                        },
                        SurvivalRow {
                            time: 4.0,
                            censored: false,
                        },
                        SurvivalRow {
                            time: 6.0,
                            censored: false,
                        },
                    ],
                },
                SurvivalGroup {
                    title: "Drug".into(),
                    rows: vec![
                        SurvivalRow {
                            time: 8.0,
                            censored: false,
                        },
                        SurvivalRow {
                            time: 10.0,
                            censored: false,
                        },
                        SurvivalRow {
                            time: 12.0,
                            censored: false,
                        },
                    ],
                },
            ],
        };
        let v = run_analysis(
            &AnyTable::Survival(t),
            &AnalysisSpec::LogRank {
                a: "Ctrl".into(),
                b: "Drug".into(),
            },
        )
        .unwrap();
        assert_eq!(v["group_a"], "Ctrl");
        assert_eq!(v["group_b"], "Drug");
        assert!(v["chi2"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn chi_square_2x2_adds_fisher() {
        use graphcraft_table::ContingencyTable;
        let t = ContingencyTable {
            title: "c".into(),
            row_titles: vec![],
            column_titles: vec!["placebo".into(), "treatment".into()],
            counts: vec![vec![12, 25], vec![30, 8]],
        };
        let v = run_analysis(&AnyTable::Contingency(t), &AnalysisSpec::ChiSquare).unwrap();
        assert_eq!(v["kind"], "chi-square");
        assert_eq!(v["n_rows"], 2);
        assert!(v["fisher_exact"].is_object());
        assert!(v["fisher_exact"]["p_two_sided"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn wrong_table_kind_rejected() {
        let (p, src) = sample_project();
        let _ = p; // use the usual column project to assert kind mismatch
        let column_as_grouped = AnyTable::Column(graphcraft_table::ColumnTable {
            title: "t".into(),
            row_titles: vec![],
            columns: vec![],
        });
        let err = run_analysis(&column_as_grouped, &AnalysisSpec::GroupedOneWayAnova).unwrap_err();
        assert!(matches!(
            err,
            AnalysisError::WrongTableKind {
                want: "grouped",
                ..
            }
        ));
        let _ = src;
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
