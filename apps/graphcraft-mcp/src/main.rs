#![allow(clippy::needless_range_loop)]
//! Minimal MCP (Model Context Protocol) stdio server for
//! GraphCraft.
//!
//! Speaks line-delimited JSON-RPC 2.0 on stdin/stdout. Supports:
//! - `initialize` (returns server info and protocol version)
//! - `tools/list` (returns the catalogue of GraphCraft tools)
//! - `tools/call` (dispatches to one tool and returns its JSON
//!   result as a `TextContent` entry)
//!
//! Each tool takes a CSV string in `inputs.csv` and the usual
//! per-analysis arguments; it returns a JSON object describing the
//! result. Clean-room: this binary is Rust-only and uses no
//! third-party MCP library.
//!
//! Reference (public): the Model Context Protocol specification at
//! <https://modelcontextprotocol.io/>, which defines the JSON-RPC
//! message shapes used here. No GraphPad material.

use std::io::{BufRead, Write};

use graphcraft_format::SheetBody;
use graphcraft_table::{AnyTable, ColumnTable};
use serde_json::{json, Value};

const SERVER_NAME: &str = "graphcraft-mcp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const PROTOCOL_VERSION: &str = "2024-11-05";

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines().map_while(Result::ok) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(line) {
            Ok(req) => handle(&req),
            Err(e) => error_response(&Value::Null, -32700, &format!("parse error: {e}")),
        };
        if let Ok(s) = serde_json::to_string(&response) {
            let _ = writeln!(out, "{s}");
            let _ = out.flush();
        }
    }
}

fn handle(req: &Value) -> Value {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let method = match req.get("method").and_then(Value::as_str) {
        Some(m) => m,
        None => return error_response(&id, -32600, "missing method"),
    };
    let params = req.get("params").cloned().unwrap_or(Value::Null);
    match method {
        "initialize" => success_response(&id, &initialize_result()),
        "tools/list" => success_response(&id, &tools_list_result()),
        "tools/call" => match tools_call(&params) {
            Ok(v) => success_response(&id, &v),
            Err((code, msg)) => error_response(&id, code, &msg),
        },
        // Clients sometimes send a `notifications/initialized`
        // notification; it has no id and expects no reply.
        "notifications/initialized" => Value::Null,
        _ => error_response(&id, -32601, &format!("unknown method {method:?}")),
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
    })
}

fn tools_list_result() -> Value {
    let csv_param = json!({
        "type": "string",
        "description": "Full CSV text to analyse. First row is header names, following rows are numeric cells. Blank cells become missing values."
    });
    let col_param = |description: &str| {
        json!({
            "type": "string",
            "description": description,
        })
    };
    let input = |extra: Value| {
        let mut props = json!({ "csv": csv_param });
        for (k, v) in extra.as_object().unwrap_or(&serde_json::Map::new()) {
            props[k] = v.clone();
        }
        let mut required = vec![json!("csv")];
        for key in extra.as_object().into_iter().flatten().map(|(k, _)| k) {
            required.push(json!(key));
        }
        json!({
            "type": "object",
            "properties": props,
            "required": required,
        })
    };
    json!({
        "tools": [
            {
                "name": "describe",
                "description": "Descriptive statistics per column.",
                "inputSchema": input(json!({})),
            },
            {
                "name": "anova",
                "description": "One-way ANOVA across non-empty columns.",
                "inputSchema": input(json!({})),
            },
            {
                "name": "ttest_unpaired",
                "description": "Welch's two-sample t-test on columns a vs b.",
                "inputSchema": input(json!({
                    "a": col_param("First column (0-based index or header name)."),
                    "b": col_param("Second column (0-based index or header name)."),
                })),
            },
            {
                "name": "ttest_paired",
                "description": "Paired t-test on columns a vs b.",
                "inputSchema": input(json!({
                    "a": col_param("First column (0-based index or header name)."),
                    "b": col_param("Second column (0-based index or header name)."),
                })),
            },
            {
                "name": "correlate",
                "description": "Pearson and Spearman correlation of columns a and b.",
                "inputSchema": input(json!({
                    "a": col_param("First column (0-based index or header name)."),
                    "b": col_param("Second column (0-based index or header name)."),
                })),
            },
            {
                "name": "regress",
                "description": "Simple linear regression y ~ x.",
                "inputSchema": input(json!({
                    "x": col_param("Predictor column."),
                    "y": col_param("Response column."),
                })),
            },
            {
                "name": "chi2_independence",
                "description": "Chi-squared independence on a count-table CSV.",
                "inputSchema": input(json!({})),
            },
            {
                "name": "plot_xy_svg",
                "description": "Build an XY scatter and return the SVG as text.",
                "inputSchema": input(json!({
                    "x": col_param("Predictor column."),
                    "y": col_param("Response column."),
                })),
            },
        ]
    })
}

fn tools_call(params: &Value) -> Result<Value, (i32, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing tool name".into()))?;
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);
    let csv = args
        .get("csv")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing `csv`".into()))?;

    let result = match name {
        "describe" => tool_describe(csv)?,
        "anova" => tool_anova(csv)?,
        "ttest_unpaired" => tool_ttest(csv, &args, false)?,
        "ttest_paired" => tool_ttest(csv, &args, true)?,
        "correlate" => tool_correlate(csv, &args)?,
        "regress" => tool_regress(csv, &args)?,
        "chi2_independence" => tool_chi2(csv)?,
        "plot_xy_svg" => tool_plot_xy_svg(csv, &args)?,
        other => return Err((-32601, format!("unknown tool {other:?}"))),
    };

    // Wrap the JSON result as a single text block per MCP's
    // `tools/call` content convention.
    let text = serde_json::to_string_pretty(&result).unwrap_or_default();
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
    }))
}

fn load_csv(csv: &str) -> Result<ColumnTable, (i32, String)> {
    let doc = graphcraft_format::import_csv(csv.as_bytes())
        .map_err(|e| (-32603, format!("import_csv failed: {e}")))?;
    match doc.project.sheets.into_iter().next().map(|s| s.body) {
        Some(SheetBody::Data {
            table: AnyTable::Column(t),
        }) => Ok(t),
        Some(SheetBody::Data { .. }) => Err((-32603, "expected a Column data sheet".into())),
        None => Err((-32603, "empty project".into())),
    }
}

fn resolve_col(table: &ColumnTable, spec: &str) -> Result<usize, (i32, String)> {
    if let Ok(i) = spec.parse::<usize>() {
        if i < table.columns.len() {
            return Ok(i);
        }
        return Err((-32602, format!("column index {i} out of range")));
    }
    for (i, c) in table.columns.iter().enumerate() {
        if c.title == spec {
            return Ok(i);
        }
    }
    Err((-32602, format!("no column named {spec:?}")))
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, (i32, String)> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or((-32602, format!("missing `{key}`")))
}

// ---------------------------------------------------------------
// Per-tool implementations.
// ---------------------------------------------------------------

fn tool_describe(csv: &str) -> Result<Value, (i32, String)> {
    let t = load_csv(csv)?;
    let rows = graphcraft_analysis::column_descriptives(&t)
        .map_err(|e| (-32603, format!("descriptives failed: {e}")))?;
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

fn tool_anova(csv: &str) -> Result<Value, (i32, String)> {
    let t = load_csv(csv)?;
    let a = graphcraft_analysis::column_one_way_anova(&t)
        .map_err(|e| (-32603, format!("anova failed: {e}")))?;
    Ok(json!({
        "k": a.k, "n_total": a.n_total,
        "group_ns": a.group_ns, "group_means": a.group_means,
        "ss_between": a.ss_between, "ss_within": a.ss_within,
        "df_between": a.df_between, "df_within": a.df_within,
        "f": a.f, "p": a.p,
    }))
}

fn tool_ttest(csv: &str, args: &Value, paired: bool) -> Result<Value, (i32, String)> {
    let t = load_csv(csv)?;
    let a_spec = arg_str(args, "a")?;
    let b_spec = arg_str(args, "b")?;
    let ia = resolve_col(&t, a_spec)?;
    let ib = resolve_col(&t, b_spec)?;
    if paired {
        let r = graphcraft_analysis::column_paired_t(&t, ia, ib)
            .map_err(|e| (-32603, format!("paired t failed: {e}")))?;
        Ok(json!({
            "n": r.n, "mean_diff": r.mean_diff, "sem_diff": r.sem_diff,
            "t": r.t, "df": r.df, "p_two_sided": r.p_two_sided,
        }))
    } else {
        let r = graphcraft_analysis::column_unpaired_t(
            &t,
            ia,
            ib,
            graphcraft_stats::TwoSampleKind::Welch,
        )
        .map_err(|e| (-32603, format!("Welch t failed: {e}")))?;
        Ok(json!({
            "n1": r.n1, "n2": r.n2,
            "mean1": r.mean1, "mean2": r.mean2,
            "mean_diff": r.mean_diff, "se_diff": r.se_diff,
            "t": r.t, "df": r.df, "p_two_sided": r.p_two_sided,
        }))
    }
}

fn tool_correlate(csv: &str, args: &Value) -> Result<Value, (i32, String)> {
    let t = load_csv(csv)?;
    let ia = resolve_col(&t, arg_str(args, "a")?)?;
    let ib = resolve_col(&t, arg_str(args, "b")?)?;
    let p = graphcraft_analysis::column_pearson(&t, ia, ib)
        .map_err(|e| (-32603, format!("pearson failed: {e}")))?;
    let s = graphcraft_analysis::column_spearman(&t, ia, ib)
        .map_err(|e| (-32603, format!("spearman failed: {e}")))?;
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

fn tool_regress(csv: &str, args: &Value) -> Result<Value, (i32, String)> {
    let t = load_csv(csv)?;
    let ix = resolve_col(&t, arg_str(args, "x")?)?;
    let iy = resolve_col(&t, arg_str(args, "y")?)?;
    let r = graphcraft_analysis::column_simple_regression(&t, ix, iy)
        .map_err(|e| (-32603, format!("regress failed: {e}")))?;
    Ok(json!({
        "intercept": r.coefficients[0],
        "slope": r.coefficients[1],
        "se_intercept": r.se[0],
        "se_slope": r.se[1],
        "rss": r.rss, "r_squared": r.r_squared,
        "residual_se": r.residual_se, "n": r.n,
    }))
}

fn tool_chi2(csv: &str) -> Result<Value, (i32, String)> {
    let t = load_csv(csv)?;
    let n_rows = t.columns.first().map(|c| c.values.len()).unwrap_or(0);
    let mut counts = Vec::with_capacity(n_rows);
    for r in 0..n_rows {
        let mut row = Vec::with_capacity(t.columns.len());
        for col in &t.columns {
            match col.values.get(r).copied().flatten() {
                Some(v) if v.is_finite() && v >= 0.0 => row.push(v.round() as u64),
                _ => return Err((-32602, "chi2 needs a non-negative integer table".into())),
            }
        }
        counts.push(row);
    }
    let r = graphcraft_stats::chi_squared_independence(&counts)
        .map_err(|e| (-32603, format!("chi2 failed: {e}")))?;
    Ok(json!({
        "n_rows": r.n_rows, "n_cols": r.n_cols, "n": r.n,
        "chi2": r.chi2, "df": r.df, "p": r.p,
        "expected": r.expected,
    }))
}

fn tool_plot_xy_svg(csv: &str, args: &Value) -> Result<Value, (i32, String)> {
    let t = load_csv(csv)?;
    let ix = resolve_col(&t, arg_str(args, "x")?)?;
    let iy = resolve_col(&t, arg_str(args, "y")?)?;
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
        return Err((-32603, "no finite (x, y) points to plot".into()));
    }
    let opts = graphcraft_plot::XyOptions {
        title: Some(t.title.clone()),
        x_title: Some(t.columns[ix].title.clone()),
        y_title: Some(t.columns[iy].title.clone()),
        ..graphcraft_plot::XyOptions::default()
    };
    let scene = graphcraft_plot::xy_scene_from_points(
        &pts,
        &opts,
        format!("{} vs {}", t.columns[iy].title, t.columns[ix].title),
    )
    .map_err(|e| (-32603, format!("scene failed: {e}")))?;
    let svg = graphcraft_render::render_svg(&scene);
    Ok(json!({ "svg": svg, "n_points": pts.len() }))
}

// ---------------------------------------------------------------
// JSON-RPC helpers.
// ---------------------------------------------------------------

fn success_response(id: &Value, result: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: &Value, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}
