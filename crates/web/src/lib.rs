#![allow(clippy::needless_range_loop)]
//! WASM bindings for GraphCraft.
//!
//! Two surfaces:
//!
//! 1. **Stateless analyses** (unchanged from M7.4): one-shot
//!    functions that take a CSV string and return a JSON string.
//!    Useful for scripted embedding and for the quick-check path
//!    on the web page.
//! 2. **Stateful project** (new in M8.5): a WASM-side `Project`
//!    carries data / analysis / graph sheets as the user builds
//!    them. The page renders the sidebar + detail panes against
//!    this state. Save/Load uses `.graphcraft` bytes so a user can
//!    download the whole project and re-open it later.
//!
//! Every exported function returns a JSON string. Errors are
//! returned as `{ "error": "..." }`, not thrown.

use std::sync::Mutex;

use graphcraft_analysis::{
    add_analysis_sheet, add_graph_sheet, build_graph_scene, recompute_all_stale,
    recompute_analysis_sheet, recompute_graph_sheet,
};
use graphcraft_format::{
    source_hash as table_source_hash, AnalysisSpec, Document, GraphSpec, Project, Sheet, SheetBody,
    SheetId, SheetStatus,
};
use graphcraft_table::{AnyTable, ColumnTable};
use serde_json::{json, Value};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Attribute that becomes `#[wasm_bindgen]` only on wasm32 so the
/// native build stays plain-Rust and `cargo test` keeps working.
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

// ---------------------------------------------------------------
// State.
// ---------------------------------------------------------------

static PROJECT: Mutex<Option<Project>> = Mutex::new(None);

fn with_project<R>(f: impl FnOnce(&mut Project) -> R) -> R {
    let mut guard = match PROJECT.lock() {
        Ok(g) => g,
        Err(poison) => poison.into_inner(),
    };
    let project = guard.get_or_insert_with(|| Project::new("Untitled project"));
    f(project)
}

// ---------------------------------------------------------------
// Project lifecycle.
// ---------------------------------------------------------------

export_fn!(
    pub fn graphcraft_version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
);

export_fn!(
    /// Replace the in-memory project with an empty one.
    pub fn project_reset(title: &str) -> String {
        with_project(|p| {
            *p = Project::new(if title.is_empty() {
                "Untitled project"
            } else {
                title
            });
            summarise(p)
        })
    }
);

export_fn!(
    /// Load a project from `.graphcraft` bytes (as a UTF-8 string).
    pub fn project_load(file_text: &str) -> String {
        match graphcraft_format::parse(file_text.as_bytes()) {
            Ok(doc) => with_project(|p| {
                *p = doc.project;
                summarise(p)
            }),
            Err(e) => err_json(&format!("load failed: {e}")),
        }
    }
);

export_fn!(
    /// Serialize the in-memory project to `.graphcraft` text.
    pub fn project_save() -> String {
        with_project(|p| {
            let doc = Document::from_project(p.clone());
            match graphcraft_format::serialize(&doc) {
                Ok(bytes) => String::from_utf8(bytes).unwrap_or_default(),
                Err(e) => err_json(&format!("save failed: {e}")),
            }
        })
    }
);

export_fn!(
    /// Rename the project.
    pub fn project_set_title(title: &str) -> String {
        with_project(|p| {
            p.title = title.to_string();
            summarise(p)
        })
    }
);

// ---------------------------------------------------------------
// Reading project / sheets.
// ---------------------------------------------------------------

export_fn!(
    /// `{ title, sheets: [{ id, title, kind, source?, status }, ...] }`.
    pub fn project_summary() -> String {
        with_project(|p| summarise(p))
    }
);

export_fn!(
    /// Full detail for one sheet. Shape depends on `kind`:
    /// - `data`: `{ kind, table: AnyTable JSON }`
    /// - `analysis`: `{ kind, source, spec, result, status }`
    /// - `graph`: `{ kind, source, spec, status, svg }` (SVG built
    ///   on demand from the current source).
    pub fn sheet_detail(id_u64: u64) -> String {
        let id = SheetId(id_u64);
        with_project(|p| sheet_detail_json(p, id))
    }
);

// ---------------------------------------------------------------
// Adding sheets.
// ---------------------------------------------------------------

export_fn!(
    /// Import a CSV and add it as a Data sheet. `title` is the
    /// sheet title; empty string falls back to a default.
    pub fn add_data_from_csv(title: &str, csv: &str) -> String {
        let doc = match graphcraft_format::import_csv(csv.as_bytes()) {
            Ok(d) => d,
            Err(e) => return err_json(&format!("import_csv failed: {e}")),
        };
        let table = match doc.project.sheets.into_iter().next().map(|s| s.body) {
            Some(SheetBody::Data { table }) => table,
            _ => return err_json("import_csv did not produce a Data sheet"),
        };
        let title = if title.is_empty() {
            "CSV data".to_string()
        } else {
            title.to_string()
        };
        with_project(|p| {
            let id = p.add_data_sheet(title.clone(), table);
            json!({ "id": id.0, "title": title }).to_string()
        })
    }
);

export_fn!(
    /// Add an Analysis sheet. `spec_json` is a serialized
    /// `AnalysisSpec` (same shape used in the file format).
    pub fn add_analysis(source_u64: u64, spec_json: &str) -> String {
        let spec: AnalysisSpec = match serde_json::from_str(spec_json) {
            Ok(s) => s,
            Err(e) => return err_json(&format!("bad spec: {e}")),
        };
        let source = SheetId(source_u64);
        with_project(|p| match add_analysis_sheet(p, source, spec) {
            Ok(id) => json!({ "id": id.0 }).to_string(),
            Err(e) => err_json(&format!("{e}")),
        })
    }
);

export_fn!(
    /// Add a Graph sheet. `spec_json` is a serialized `GraphSpec`.
    pub fn add_graph(source_u64: u64, spec_json: &str) -> String {
        let spec: GraphSpec = match serde_json::from_str(spec_json) {
            Ok(s) => s,
            Err(e) => return err_json(&format!("bad spec: {e}")),
        };
        let source = SheetId(source_u64);
        with_project(|p| match add_graph_sheet(p, source, spec) {
            Ok(id) => json!({ "id": id.0 }).to_string(),
            Err(e) => err_json(&format!("{e}")),
        })
    }
);

// ---------------------------------------------------------------
// Mutations.
// ---------------------------------------------------------------

export_fn!(
    pub fn recompute(id_u64: u64) -> String {
        let id = SheetId(id_u64);
        with_project(|p| {
            let is_analysis = match p.get(id) {
                Some(s) => matches!(s.body, SheetBody::Analysis { .. }),
                None => return err_json("sheet not found"),
            };
            let r = if is_analysis {
                recompute_analysis_sheet(p, id).map_err(|e| format!("{e}"))
            } else {
                recompute_graph_sheet(p, id).map_err(|e| format!("{e}"))
            };
            match r {
                Ok(()) => json!({ "ok": true, "id": id_u64 }).to_string(),
                Err(e) => err_json(&e),
            }
        })
    }
);

export_fn!(
    pub fn recompute_all() -> String {
        with_project(|p| {
            let n = recompute_all_stale(p);
            json!({ "refreshed": n }).to_string()
        })
    }
);

export_fn!(
    pub fn delete_sheet(id_u64: u64) -> String {
        let id = SheetId(id_u64);
        with_project(|p| {
            let before = p.sheets.len();
            p.sheets.retain(|s| s.id != id);
            if p.sheets.len() == before {
                err_json("sheet not found")
            } else {
                json!({ "ok": true }).to_string()
            }
        })
    }
);

// ---------------------------------------------------------------
// Stateless passthrough (M7.4 compatibility).
// ---------------------------------------------------------------

export_fn!(
    pub fn describe(csv: &str) -> String {
        stateless(|| {
            let t = load(csv)?;
            run_descriptives_json(&t)
        })
    }
);

fn run_descriptives_json(t: &ColumnTable) -> Result<Value, String> {
    let rows = graphcraft_analysis::column_descriptives(t)
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
}

// ---------------------------------------------------------------
// Internal helpers.
// ---------------------------------------------------------------

fn stateless<F: FnOnce() -> Result<Value, String>>(f: F) -> String {
    match f() {
        Ok(v) => v.to_string(),
        Err(e) => err_json(&e),
    }
}

fn err_json(e: &str) -> String {
    json!({ "error": e }).to_string()
}

fn load(csv: &str) -> Result<ColumnTable, String> {
    let doc = graphcraft_format::import_csv(csv.as_bytes()).map_err(|e| format!("import: {e}"))?;
    match doc.project.sheets.into_iter().next().map(|s| s.body) {
        Some(SheetBody::Data {
            table: AnyTable::Column(t),
        }) => Ok(t),
        _ => Err("expected a Column data sheet".to_string()),
    }
}

fn summarise(p: &Project) -> String {
    let sheets: Vec<Value> = p.sheets.iter().map(|s| sheet_summary_json(p, s)).collect();
    json!({
        "title": p.title,
        "sheets": sheets,
    })
    .to_string()
}

fn sheet_summary_json(p: &Project, s: &Sheet) -> Value {
    let kind = match &s.body {
        SheetBody::Data { .. } => "data",
        SheetBody::Analysis { .. } => "analysis",
        SheetBody::Graph { .. } => "graph",
    };
    let (source, spec_id) = match &s.body {
        SheetBody::Data { .. } => (None, None),
        SheetBody::Analysis { source, spec, .. } => (Some(source.0), Some(spec.id())),
        SheetBody::Graph { source, spec, .. } => (Some(source.0), Some(spec.id())),
    };
    let status = match p.sheet_status(s.id) {
        SheetStatus::Fresh => "fresh",
        SheetStatus::Stale => "stale",
        SheetStatus::Unknown => "unknown",
    };
    json!({
        "id": s.id.0,
        "title": s.title,
        "kind": kind,
        "source": source,
        "spec_id": spec_id,
        "status": status,
    })
}

fn sheet_detail_json(p: &Project, id: SheetId) -> String {
    let sheet = match p.get(id) {
        Some(s) => s,
        None => return err_json("sheet not found"),
    };
    let status = match p.sheet_status(id) {
        SheetStatus::Fresh => "fresh",
        SheetStatus::Stale => "stale",
        SheetStatus::Unknown => "unknown",
    };
    match &sheet.body {
        SheetBody::Data { table } => json!({
            "id": id.0,
            "title": sheet.title,
            "kind": "data",
            "status": status,
            "table": table,
            "columns": column_names(table),
            "n_rows": column_rows(table),
            "source_hash": table_source_hash(table),
        })
        .to_string(),
        SheetBody::Analysis {
            source,
            spec,
            result,
            source_hash: _,
        } => json!({
            "id": id.0,
            "title": sheet.title,
            "kind": "analysis",
            "status": status,
            "source": source.0,
            "spec": spec,
            "result": result,
        })
        .to_string(),
        SheetBody::Graph { source, spec, .. } => {
            // Build the scene on demand against the current source
            // table so the SVG matches reality even if the source
            // changed between save and reopen.
            let (svg_or_err, source_title) = match p.data_sheet_table(*source) {
                Some(table) => {
                    let title = p.get(*source).map(|s| s.title.clone()).unwrap_or_default();
                    match build_graph_scene(table, spec) {
                        Ok(scene) => (Ok(graphcraft_render::render_svg(&scene)), title),
                        Err(e) => (Err(format!("{e}")), title),
                    }
                }
                None => (Err("source data sheet missing".to_string()), String::new()),
            };
            let mut out = json!({
                "id": id.0,
                "title": sheet.title,
                "kind": "graph",
                "status": status,
                "source": source.0,
                "source_title": source_title,
                "spec": spec,
            });
            match svg_or_err {
                Ok(svg) => {
                    out["svg"] = Value::String(svg);
                }
                Err(e) => {
                    out["error"] = Value::String(e);
                }
            }
            out.to_string()
        }
    }
}

fn column_names(table: &AnyTable) -> Vec<String> {
    match table {
        AnyTable::Column(c) => c.columns.iter().map(|col| col.title.clone()).collect(),
        _ => Vec::new(),
    }
}

fn column_rows(table: &AnyTable) -> usize {
    match table {
        AnyTable::Column(c) => c
            .columns
            .iter()
            .map(|col| col.values.len())
            .max()
            .unwrap_or(0),
        _ => 0,
    }
}

// ---------------------------------------------------------------
// Tests (native-side only).
// ---------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Tests share a single global PROJECT. Serialize access so
    // cargo's parallel runner doesn't interleave setup/teardown.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn setup() -> std::sync::MutexGuard<'static, ()> {
        let guard = match TEST_LOCK.lock() {
            Ok(g) => g,
            Err(poison) => poison.into_inner(),
        };
        project_reset("test");
        guard
    }

    #[test]
    fn project_add_csv_lists_data_sheet() {
        let _lock = setup();
        let s = add_data_from_csv("clinical", "a,b\n1,2\n3,4\n");
        let v: Value = serde_json::from_str(&s).unwrap();
        assert!(v["id"].is_u64());
        let summary: Value = serde_json::from_str(&project_summary()).unwrap();
        let sheets = summary["sheets"].as_array().unwrap();
        assert_eq!(sheets.len(), 1);
        assert_eq!(sheets[0]["kind"], "data");
    }

    #[test]
    fn full_lifecycle_data_analysis_graph_recompute() {
        let _lock = setup();
        let data = serde_json::from_str::<Value>(&add_data_from_csv(
            "d",
            "x,y\n1,2\n2,4\n3,6\n4,8\n5,10\n",
        ))
        .unwrap();
        let data_id = data["id"].as_u64().unwrap();

        let ana = serde_json::from_str::<Value>(&add_analysis(
            data_id,
            r#"{"kind":"pearson","a":"x","b":"y"}"#,
        ))
        .unwrap();
        assert!(ana["id"].is_u64());

        let graph = serde_json::from_str::<Value>(&add_graph(
            data_id,
            r#"{"kind":"xy","x":"x","y":"y","title":null}"#,
        ))
        .unwrap();
        let graph_id = graph["id"].as_u64().unwrap();

        // Fresh across the board.
        let summary: Value = serde_json::from_str(&project_summary()).unwrap();
        for sheet in summary["sheets"].as_array().unwrap() {
            assert_eq!(sheet["status"], "fresh");
        }

        // The graph detail carries a built SVG.
        let detail: Value = serde_json::from_str(&sheet_detail(graph_id)).unwrap();
        assert!(detail["svg"].as_str().unwrap().contains("<svg"));

        // Round-trip through save/load preserves the project.
        let bytes = project_save();
        project_reset("fresh"); // clear state without re-locking
        project_load(&bytes);
        let summary: Value = serde_json::from_str(&project_summary()).unwrap();
        assert_eq!(summary["sheets"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn delete_sheet_removes_it() {
        let _lock = setup();
        let d: Value = serde_json::from_str(&add_data_from_csv("d", "x\n1\n2\n3\n")).unwrap();
        let id = d["id"].as_u64().unwrap();
        delete_sheet(id);
        let summary: Value = serde_json::from_str(&project_summary()).unwrap();
        assert_eq!(summary["sheets"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn stale_sheet_recomputes() {
        let _lock = setup();
        let d: Value =
            serde_json::from_str(&add_data_from_csv("d", "a,b\n1,10\n2,20\n3,30\n")).unwrap();
        let did = d["id"].as_u64().unwrap();
        let a: Value =
            serde_json::from_str(&add_analysis(did, r#"{"kind":"descriptives"}"#)).unwrap();
        let aid = a["id"].as_u64().unwrap();

        // Mutate the source table directly via the project mutex so
        // we can simulate "the user edited the data".
        with_project(|p| {
            if let Some(sheet) = p.get_mut(SheetId(did)) {
                if let SheetBody::Data {
                    table: AnyTable::Column(t),
                } = &mut sheet.body
                {
                    t.columns[0].values.push(Some(99.0));
                }
            }
        });

        let summary: Value = serde_json::from_str(&project_summary()).unwrap();
        let analysis_entry = summary["sheets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"].as_u64() == Some(aid))
            .unwrap();
        assert_eq!(analysis_entry["status"], "stale");

        let r: Value = serde_json::from_str(&recompute_all()).unwrap();
        assert!(r["refreshed"].as_u64().unwrap() >= 1);

        let summary: Value = serde_json::from_str(&project_summary()).unwrap();
        for sheet in summary["sheets"].as_array().unwrap() {
            assert_eq!(sheet["status"], "fresh");
        }
    }

    #[test]
    fn stateless_describe_still_works() {
        let out = describe("a,b\n1,10\n2,20\n3,30\n");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["descriptives"].as_array().unwrap().len(), 2);
    }
}
