#![allow(clippy::needless_range_loop)]
//! WASM bindings for GraphCraft analyses + plot export.
//!
//! Each exported function takes a CSV string and returns either
//! JSON (analyses) or the SVG text (plots). Errors are returned as
//! JSON `{ "error": "..." }`, not as JS exceptions, so the client
//! can render them inline without a try/catch dance.
//!
//! All strings cross the boundary via `wasm-bindgen`'s standard
//! UTF-8 marshalling. The non-wasm `rlib` build is a plain-Rust
//! shim so unit tests and the native toolchain can still link.

use graphcraft_format::SheetBody;
use graphcraft_table::{AnyTable, ColumnTable};
use serde_json::{json, Value};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Attribute that becomes `#[wasm_bindgen]` only on wasm32 so the
/// native build stays plain-Rust.
#[cfg(target_arch = "wasm32")]
macro_rules! export_fn {
    ($t:item) => {
        #[wasm_bindgen]
        $t
    };
}
#[cfg(not(target_arch = "wasm32"))]
macro_rules! export_fn {
    ($t:item) => {
        $t
    };
}

export_fn!(
    /// Return the GraphCraft web-binding library version.
    pub fn graphcraft_version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
);

export_fn!(
    /// Per-column descriptive statistics as JSON.
    pub fn describe(csv: &str) -> String {
        wrap(|| {
            let t = load(csv)?;
            let rows = graphcraft_analysis::column_descriptives(&t)
                .map_err(|e| format!("descriptives failed: {e}"))?;
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
        })
    }
);

export_fn!(
    /// One-way ANOVA across all non-empty columns.
    pub fn anova(csv: &str) -> String {
        wrap(|| {
            let t = load(csv)?;
            let a = graphcraft_analysis::column_one_way_anova(&t)
                .map_err(|e| format!("anova failed: {e}"))?;
            Ok(json!({
                "k": a.k, "n_total": a.n_total,
                "group_ns": a.group_ns, "group_means": a.group_means,
                "ss_between": a.ss_between, "ss_within": a.ss_within,
                "df_between": a.df_between, "df_within": a.df_within,
                "f": a.f, "p": a.p,
            }))
        })
    }
);

export_fn!(
    /// Pearson + Spearman correlation on columns `a` and `b`.
    pub fn correlate(csv: &str, a: &str, b: &str) -> String {
        wrap(|| {
            let t = load(csv)?;
            let ia = resolve_col(&t, a)?;
            let ib = resolve_col(&t, b)?;
            let p = graphcraft_analysis::column_pearson(&t, ia, ib)
                .map_err(|e| format!("pearson failed: {e}"))?;
            let s = graphcraft_analysis::column_spearman(&t, ia, ib)
                .map_err(|e| format!("spearman failed: {e}"))?;
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
        })
    }
);

export_fn!(
    /// Welch's unpaired t-test on columns `a` vs `b`.
    pub fn ttest_welch(csv: &str, a: &str, b: &str) -> String {
        wrap(|| {
            let t = load(csv)?;
            let ia = resolve_col(&t, a)?;
            let ib = resolve_col(&t, b)?;
            let r = graphcraft_analysis::column_unpaired_t(
                &t,
                ia,
                ib,
                graphcraft_stats::TwoSampleKind::Welch,
            )
            .map_err(|e| format!("ttest failed: {e}"))?;
            Ok(json!({
                "n1": r.n1, "n2": r.n2,
                "mean1": r.mean1, "mean2": r.mean2,
                "mean_diff": r.mean_diff, "se_diff": r.se_diff,
                "t": r.t, "df": r.df, "p_two_sided": r.p_two_sided,
            }))
        })
    }
);

export_fn!(
    /// Simple linear regression `y ~ x`.
    pub fn regress(csv: &str, x: &str, y: &str) -> String {
        wrap(|| {
            let t = load(csv)?;
            let ix = resolve_col(&t, x)?;
            let iy = resolve_col(&t, y)?;
            let r = graphcraft_analysis::column_simple_regression(&t, ix, iy)
                .map_err(|e| format!("regress failed: {e}"))?;
            Ok(json!({
                "intercept": r.coefficients[0],
                "slope": r.coefficients[1],
                "se_intercept": r.se[0],
                "se_slope": r.se[1],
                "rss": r.rss, "r_squared": r.r_squared,
                "residual_se": r.residual_se, "n": r.n,
            }))
        })
    }
);

export_fn!(
    /// Build an XY scatter from columns `x` and `y` and return the
    /// SVG text (the client inserts it into the DOM).
    pub fn plot_xy_svg(csv: &str, x: &str, y: &str) -> String {
        wrap(|| {
            let t = load(csv)?;
            let ix = resolve_col(&t, x)?;
            let iy = resolve_col(&t, y)?;
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
                return Err("no finite (x, y) points".to_string());
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
            .map_err(|e| format!("scene failed: {e}"))?;
            let svg = graphcraft_render::render_svg(&scene);
            Ok(json!({ "svg": svg, "n_points": pts.len() }))
        })
    }
);

// ---------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------

fn wrap<F: FnOnce() -> Result<Value, String>>(f: F) -> String {
    let v = match f() {
        Ok(v) => v,
        Err(e) => json!({ "error": e }),
    };
    serde_json::to_string(&v).unwrap_or_else(|e| format!(r#"{{"error":"json serialize: {e}"}}"#))
}

fn load(csv: &str) -> Result<ColumnTable, String> {
    let doc = graphcraft_format::import_csv(csv.as_bytes())
        .map_err(|e| format!("import_csv failed: {e}"))?;
    match doc.project.sheets.into_iter().next().map(|s| s.body) {
        Some(SheetBody::Data {
            table: AnyTable::Column(t),
        }) => Ok(t),
        Some(SheetBody::Data { .. }) => Err("expected a Column data sheet".to_string()),
        None => Err("empty project".to_string()),
    }
}

fn resolve_col(table: &ColumnTable, spec: &str) -> Result<usize, String> {
    if let Ok(i) = spec.parse::<usize>() {
        if i < table.columns.len() {
            return Ok(i);
        }
        return Err(format!("column index {i} out of range"));
    }
    for (i, c) in table.columns.iter().enumerate() {
        if c.title == spec {
            return Ok(i);
        }
    }
    Err(format!("no column named {spec:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_round_trip() {
        let out = describe("a,b\n1,10\n2,20\n3,30\n");
        let v: Value = serde_json::from_str(&out).unwrap();
        let rows = v["descriptives"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn error_is_json_not_panic() {
        let out = describe("not a csv");
        let v: Value = serde_json::from_str(&out).unwrap();
        // Even for degenerate input, we return a JSON object.
        assert!(v.is_object());
    }

    #[test]
    fn plot_xy_returns_svg_payload() {
        let out = plot_xy_svg("x,y\n1,2\n2,4\n3,6\n4,8\n5,10\n", "x", "y");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v["svg"].as_str().unwrap().contains("<svg"));
        assert_eq!(v["n_points"].as_u64().unwrap(), 5);
    }
}
