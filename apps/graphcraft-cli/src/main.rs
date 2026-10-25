//! GraphCraft CLI.
//!
//! Subcommands fall in three groups: engine management (`commands`,
//! `run`), file I/O (`new`, `import`), analyses on a CSV
//! (`describe`, `anova`, `ttest-*`, `correlate`, `regress`,
//! `chi2`), and plot export (`plot xy`, `plot column`). Analysis
//! commands print JSON on stdout; plot commands write SVG or PDF
//! chosen by output extension.
//!
//! All destructive operations are explicit (`new`, `import`, `plot`
//! and the renderer write to a file). The engine's `run <id>`
//! subcommand executes against an empty session.

use std::process::ExitCode;

use graphcraft_format::SheetBody;
use graphcraft_table::{AnyTable, ColumnTable};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let strs: Vec<&str> = args.iter().map(String::as_str).collect();
    match strs.as_slice() {
        [] | ["--help"] | ["-h"] => {
            print_usage();
            ExitCode::SUCCESS
        }
        ["commands"] => do_commands(),
        ["run", id] => do_run(id),
        ["new", path] => do_new(path),
        ["import", src, dst] => do_import(src, dst),
        ["describe", csv] => do_describe(csv),
        ["anova", csv] => do_anova(csv),
        ["ttest-unpaired", csv, a, b] => do_ttest_unpaired(csv, a, b),
        ["ttest-paired", csv, a, b] => do_ttest_paired(csv, a, b),
        ["correlate", csv, a, b] => do_correlate(csv, a, b),
        ["regress", csv, x, y] => do_regress(csv, x, y),
        ["chi2", csv] => do_chi2(csv),
        ["plot", "xy", csv, x, y, out] => do_plot_xy(csv, x, y, out),
        ["plot", "column", csv, out] => do_plot_column(csv, out),
        _ => {
            print_usage();
            ExitCode::from(2)
        }
    }
}

// ---------------------------------------------------------------
// Engine / file-io subcommands (unchanged from M1).
// ---------------------------------------------------------------

fn do_commands() -> ExitCode {
    let reg = graphcraft_engine::CommandRegistry::new();
    for id in reg.ids() {
        println!("{id}");
    }
    ExitCode::SUCCESS
}

fn do_run(id: &str) -> ExitCode {
    let reg = graphcraft_engine::CommandRegistry::new();
    let mut s = graphcraft_engine::Session::default();
    match reg.run(&mut s, id, &serde_json::Value::Null) {
        Ok(v) => {
            println!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => err_out(&format!("{e}")),
    }
}

fn do_new(path: &str) -> ExitCode {
    let doc = graphcraft_format::Document::empty();
    match graphcraft_format::serialize(&doc) {
        Ok(bytes) => match std::fs::write(path, bytes) {
            Ok(()) => {
                println!("wrote {path}");
                ExitCode::SUCCESS
            }
            Err(e) => err_out(&format!("write failed: {e}")),
        },
        Err(e) => err_out(&format!("serialize failed: {e}")),
    }
}

fn do_import(src: &str, dst: &str) -> ExitCode {
    let doc = match read_csv_doc(src) {
        Ok(d) => d,
        Err(code) => return code,
    };
    match graphcraft_format::serialize(&doc) {
        Ok(out) => match std::fs::write(dst, out) {
            Ok(()) => {
                println!("imported {src} -> {dst}");
                ExitCode::SUCCESS
            }
            Err(e) => err_out(&format!("write failed: {e}")),
        },
        Err(e) => err_out(&format!("serialize failed: {e}")),
    }
}

// ---------------------------------------------------------------
// Analyses — all read a CSV, print JSON to stdout.
// ---------------------------------------------------------------

fn do_describe(csv: &str) -> ExitCode {
    let table = match load_column_table(csv) {
        Ok(t) => t,
        Err(code) => return code,
    };
    match graphcraft_analysis::column_descriptives(&table) {
        Ok(rows) => {
            let out: Vec<_> = rows
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "group": r.group_title,
                        "n": r.stats.n,
                        "mean": r.stats.mean,
                        "sd": r.stats.sd,
                        "variance": r.stats.variance,
                        "sem": r.stats.sem,
                        "median": r.stats.median,
                        "min": r.stats.min,
                        "max": r.stats.max,
                    })
                })
                .collect();
            emit_json(&serde_json::json!({"descriptives": out}))
        }
        Err(e) => err_out(&format!("descriptives failed: {e}")),
    }
}

fn do_anova(csv: &str) -> ExitCode {
    let table = match load_column_table(csv) {
        Ok(t) => t,
        Err(code) => return code,
    };
    match graphcraft_analysis::column_one_way_anova(&table) {
        Ok(a) => emit_json(&serde_json::json!({
            "one_way_anova": {
                "k": a.k,
                "n_total": a.n_total,
                "group_ns": a.group_ns,
                "group_means": a.group_means,
                "grand_mean": a.grand_mean,
                "ss_between": a.ss_between,
                "ss_within": a.ss_within,
                "df_between": a.df_between,
                "df_within": a.df_within,
                "f": a.f,
                "p": a.p,
            }
        })),
        Err(e) => err_out(&format!("anova failed: {e}")),
    }
}

fn do_ttest_unpaired(csv: &str, a: &str, b: &str) -> ExitCode {
    let table = match load_column_table(csv) {
        Ok(t) => t,
        Err(code) => return code,
    };
    let (ia, ib) = match resolve_two_columns(&table, a, b) {
        Ok(ii) => ii,
        Err(code) => return code,
    };
    match graphcraft_analysis::column_unpaired_t(
        &table,
        ia,
        ib,
        graphcraft_stats::TwoSampleKind::Welch,
    ) {
        Ok(t) => emit_json(&serde_json::json!({
            "ttest_welch": {
                "col_a": &table.columns[ia].title,
                "col_b": &table.columns[ib].title,
                "n1": t.n1, "n2": t.n2,
                "mean1": t.mean1, "mean2": t.mean2,
                "mean_diff": t.mean_diff, "se_diff": t.se_diff,
                "t": t.t, "df": t.df, "p_two_sided": t.p_two_sided,
            }
        })),
        Err(e) => err_out(&format!("ttest failed: {e}")),
    }
}

fn do_ttest_paired(csv: &str, a: &str, b: &str) -> ExitCode {
    let table = match load_column_table(csv) {
        Ok(t) => t,
        Err(code) => return code,
    };
    let (ia, ib) = match resolve_two_columns(&table, a, b) {
        Ok(ii) => ii,
        Err(code) => return code,
    };
    match graphcraft_analysis::column_paired_t(&table, ia, ib) {
        Ok(t) => emit_json(&serde_json::json!({
            "ttest_paired": {
                "col_a": &table.columns[ia].title,
                "col_b": &table.columns[ib].title,
                "n": t.n, "mean_diff": t.mean_diff, "sem_diff": t.sem_diff,
                "t": t.t, "df": t.df, "p_two_sided": t.p_two_sided,
            }
        })),
        Err(e) => err_out(&format!("ttest-paired failed: {e}")),
    }
}

fn do_correlate(csv: &str, a: &str, b: &str) -> ExitCode {
    let table = match load_column_table(csv) {
        Ok(t) => t,
        Err(code) => return code,
    };
    let (ia, ib) = match resolve_two_columns(&table, a, b) {
        Ok(ii) => ii,
        Err(code) => return code,
    };
    let pear = match graphcraft_analysis::column_pearson(&table, ia, ib) {
        Ok(c) => c,
        Err(e) => return err_out(&format!("pearson failed: {e}")),
    };
    let spear = match graphcraft_analysis::column_spearman(&table, ia, ib) {
        Ok(c) => c,
        Err(e) => return err_out(&format!("spearman failed: {e}")),
    };
    emit_json(&serde_json::json!({
        "pearson": {
            "n": pear.n, "r": pear.r, "t": pear.t, "df": pear.df,
            "p_two_sided": pear.p_two_sided,
            "ci95": [pear.ci95.0, pear.ci95.1],
        },
        "spearman": {
            "n": spear.n, "r": spear.r, "t": spear.t, "df": spear.df,
            "p_two_sided": spear.p_two_sided,
        }
    }))
}

fn do_regress(csv: &str, x: &str, y: &str) -> ExitCode {
    let table = match load_column_table(csv) {
        Ok(t) => t,
        Err(code) => return code,
    };
    let (ix, iy) = match resolve_two_columns(&table, x, y) {
        Ok(ii) => ii,
        Err(code) => return code,
    };
    match graphcraft_analysis::column_simple_regression(&table, ix, iy) {
        Ok(r) => emit_json(&serde_json::json!({
            "simple_linear_regression": {
                "intercept": r.coefficients[0],
                "slope": r.coefficients[1],
                "se_intercept": r.se[0],
                "se_slope": r.se[1],
                "t_intercept": r.t[0],
                "t_slope": r.t[1],
                "p_intercept": r.p[0],
                "p_slope": r.p[1],
                "rss": r.rss,
                "r_squared": r.r_squared,
                "adj_r_squared": r.adj_r_squared,
                "residual_se": r.residual_se,
                "f_stat": r.f_stat,
                "f_p": r.f_p,
                "n": r.n,
            }
        })),
        Err(e) => err_out(&format!("regress failed: {e}")),
    }
}

fn do_chi2(csv: &str) -> ExitCode {
    // Treat the CSV as a contingency count table: first row is
    // headers (col labels), first column is row label, remaining
    // cells are u64 counts.
    let doc = match read_csv_doc(csv) {
        Ok(d) => d,
        Err(code) => return code,
    };
    let table = match doc.project.sheets.first().map(|s| &s.body) {
        Some(SheetBody::Data {
            table: AnyTable::Column(t),
        }) => t.clone(),
        _ => {
            return err_out("csv did not import as a Column data sheet");
        }
    };
    let mut counts: Vec<Vec<u64>> = Vec::with_capacity(table.row_titles.len().max(1));
    let n_rows = table.columns.first().map(|c| c.values.len()).unwrap_or(0);
    for r in 0..n_rows {
        let mut row = Vec::with_capacity(table.columns.len());
        for col in &table.columns {
            match col.values.get(r).copied().flatten() {
                Some(v) if v.is_finite() && v >= 0.0 => row.push(v.round() as u64),
                _ => {
                    return err_out(
                        "chi2 input must be a non-negative integer count table \
                         (no missing / non-finite cells)",
                    );
                }
            }
        }
        counts.push(row);
    }
    match graphcraft_stats::chi_squared_independence(&counts) {
        Ok(r) => emit_json(&serde_json::json!({
            "chi_squared_independence": {
                "n_rows": r.n_rows, "n_cols": r.n_cols,
                "n": r.n,
                "chi2": r.chi2, "df": r.df, "p": r.p,
                "observed": r.observed,
                "expected": r.expected,
            }
        })),
        Err(e) => err_out(&format!("chi2 failed: {e}")),
    }
}

// ---------------------------------------------------------------
// Plots.
// ---------------------------------------------------------------

fn do_plot_xy(csv: &str, x: &str, y: &str, out: &str) -> ExitCode {
    let table = match load_column_table(csv) {
        Ok(t) => t,
        Err(code) => return code,
    };
    let (ix, iy) = match resolve_two_columns(&table, x, y) {
        Ok(ii) => ii,
        Err(code) => return code,
    };
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let xs = &table.columns[ix].values;
    let ys = &table.columns[iy].values;
    for (a, b) in xs.iter().zip(ys.iter()) {
        if let (Some(xa), Some(yb)) = (a, b) {
            if xa.is_finite() && yb.is_finite() {
                pts.push((*xa, *yb));
            }
        }
    }
    if pts.is_empty() {
        return err_out("no finite (x, y) points to plot");
    }
    let opts = graphcraft_plot::XyOptions {
        title: Some(table.title.clone()),
        x_title: Some(table.columns[ix].title.clone()),
        y_title: Some(table.columns[iy].title.clone()),
        ..graphcraft_plot::XyOptions::default()
    };
    let scene = match graphcraft_plot::xy_scene_from_points(
        &pts,
        &opts,
        format!("{} vs {}", table.columns[iy].title, table.columns[ix].title),
    ) {
        Ok(s) => s,
        Err(e) => return err_out(&format!("xy scene failed: {e}")),
    };
    write_scene(&scene, out)
}

fn do_plot_column(csv: &str, out: &str) -> ExitCode {
    let table = match load_column_table(csv) {
        Ok(t) => t,
        Err(code) => return code,
    };
    let opts = graphcraft_plot::ColumnOptions {
        title: Some(table.title.clone()),
        y_title: None,
        error_bars: graphcraft_plot::ColumnErrorBars::Sd,
        ..graphcraft_plot::ColumnOptions::default()
    };
    let scene = match graphcraft_plot::column_scene(&table, &opts) {
        Ok(s) => s,
        Err(e) => return err_out(&format!("column scene failed: {e}")),
    };
    write_scene(&scene, out)
}

fn write_scene(scene: &graphcraft_plot::Scene, out: &str) -> ExitCode {
    let out_lower = out.to_lowercase();
    if out_lower.ends_with(".svg") {
        let s = graphcraft_render::render_svg(scene);
        match std::fs::write(out, s) {
            Ok(()) => {
                println!("wrote {out}");
                ExitCode::SUCCESS
            }
            Err(e) => err_out(&format!("write failed: {e}")),
        }
    } else if out_lower.ends_with(".pdf") {
        let bytes = graphcraft_render::render_pdf(scene);
        match std::fs::write(out, bytes) {
            Ok(()) => {
                println!("wrote {out}");
                ExitCode::SUCCESS
            }
            Err(e) => err_out(&format!("write failed: {e}")),
        }
    } else {
        err_out("output path must end in .svg or .pdf")
    }
}

// ---------------------------------------------------------------
// Shared helpers.
// ---------------------------------------------------------------

fn read_csv_doc(src: &str) -> Result<graphcraft_format::Document, ExitCode> {
    let bytes = match std::fs::read(src) {
        Ok(b) => b,
        Err(e) => return Err(err_out(&format!("read failed: {e}"))),
    };
    graphcraft_format::import_csv(&bytes).map_err(|e| err_out(&format!("import failed: {e}")))
}

fn load_column_table(src: &str) -> Result<ColumnTable, ExitCode> {
    let doc = read_csv_doc(src)?;
    match doc.project.sheets.into_iter().next().map(|s| s.body) {
        Some(SheetBody::Data {
            table: AnyTable::Column(t),
        }) => Ok(t),
        Some(SheetBody::Data { .. }) => Err(err_out("expected a Column table (CSV import)")),
        None => Err(err_out("empty project")),
    }
}

fn resolve_column(table: &ColumnTable, spec: &str) -> Result<usize, ExitCode> {
    if let Ok(i) = spec.parse::<usize>() {
        if i < table.columns.len() {
            return Ok(i);
        }
        return Err(err_out(&format!(
            "column index {i} out of range (0..{})",
            table.columns.len()
        )));
    }
    for (i, c) in table.columns.iter().enumerate() {
        if c.title == spec {
            return Ok(i);
        }
    }
    Err(err_out(&format!("no column named {spec:?}")))
}

fn resolve_two_columns(table: &ColumnTable, a: &str, b: &str) -> Result<(usize, usize), ExitCode> {
    let ia = resolve_column(table, a)?;
    let ib = resolve_column(table, b)?;
    Ok((ia, ib))
}

fn emit_json(v: &serde_json::Value) -> ExitCode {
    match serde_json::to_string_pretty(v) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => err_out(&format!("json emit failed: {e}")),
    }
}

fn err_out(msg: &str) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::from(1)
}

fn print_usage() {
    eprintln!(
        "graphcraft-cli — scientific graphing & analysis\n\
         \n\
         usage:\n  \
         graphcraft-cli commands\n  \
         graphcraft-cli run <command_id>\n  \
         graphcraft-cli new <path>\n  \
         graphcraft-cli import <csv-path> <graphcraft-path>\n\
         \n\
         analyses (reads CSV, prints JSON):\n  \
         graphcraft-cli describe <csv>\n  \
         graphcraft-cli anova <csv>\n  \
         graphcraft-cli ttest-unpaired <csv> <col-a> <col-b>\n  \
         graphcraft-cli ttest-paired   <csv> <col-a> <col-b>\n  \
         graphcraft-cli correlate      <csv> <col-a> <col-b>\n  \
         graphcraft-cli regress        <csv> <x-col> <y-col>\n  \
         graphcraft-cli chi2           <csv>\n\
         \n\
         plots (writes .svg or .pdf by extension):\n  \
         graphcraft-cli plot xy     <csv> <x-col> <y-col> <out>\n  \
         graphcraft-cli plot column <csv> <out>\n\
         \n\
         columns can be referenced by 0-based index or by header name."
    );
}
