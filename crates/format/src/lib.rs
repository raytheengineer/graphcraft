//! L4 format: the `.graphcraft` native file format and CSV import.
//!
//! `.pzfx` is deliberately NOT supported (see `AGENTS.md` ›
//! Clean-room).
//!
//! The on-disk file is a JSON object:
//!
//! ```text
//! { "format": "graphcraft", "version": 2, "project": { ... } }
//! ```
//!
//! Version 1 (which carried a bare `tables` list) is still read on
//! load and auto-migrated into a v2 `Project`: each table in the
//! old file becomes one `Sheet` of kind `Data`.

pub mod csv;
pub use csv::import_csv;

use graphcraft_table::AnyTable;
use serde::{Deserialize, Serialize};

pub const FORMAT_TAG: &str = "graphcraft";
pub const FORMAT_VERSION: u32 = 2;
pub const EXTENSION: &str = "graphcraft";

/// Stable identifier for a sheet within a project. Project-unique;
/// not stable across projects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SheetId(pub u64);

impl SheetId {
    pub const INVALID: SheetId = SheetId(0);
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

/// One sheet in a project. The kind is driven by [`SheetBody`].
///
/// Analysis and Graph bodies that reference another sheet carry
/// that sheet's `id`; staleness checks are added in M8.4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub id: SheetId,
    pub title: String,
    pub body: SheetBody,
}

/// What a sheet actually holds. The Layout variant lands in a
/// later M8 slice; the serde tag keeps old files forward-readable
/// as we grow the enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SheetBody {
    /// A data table of one of the 8 Prism-style layouts.
    Data { table: AnyTable },
    /// An analysis bound to a data sheet, with its parameters and
    /// cached result. The result shape depends on `spec` and is
    /// stored as a JSON value for format stability; the running
    /// dispatcher lives in `graphcraft_analysis`.
    Analysis {
        /// Id of the data sheet this analysis consumed.
        source: SheetId,
        /// Which analysis, with its parameters.
        spec: AnalysisSpec,
        /// Cached result (shape depends on `spec`). On failure the
        /// dispatcher stores `{ "error": "..." }` so the sheet
        /// always carries a readable record.
        #[serde(default)]
        result: serde_json::Value,
        /// Content hash of the source table at the time the result
        /// was computed. Used by M8.4 staleness; `None` means the
        /// sheet was never computed (or came from an older file).
        #[serde(default)]
        source_hash: Option<String>,
    },
    /// A graph bound to a data sheet. The scene is derived from
    /// `(source, spec)` on demand — not stored in the file — so
    /// opening a project doesn't duplicate pixel coordinates.
    Graph {
        source: SheetId,
        spec: GraphSpec,
        /// Content hash of the source table at the last build.
        /// `None` means the graph was never built or came from an
        /// older file.
        #[serde(default)]
        source_hash: Option<String>,
    },
}

/// Which graph to build from a data sheet.
///
/// Column references are strings interpreted at run time as either
/// a 0-based index or the exact header name (matching the CLI /
/// MCP / Analysis convention).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum GraphSpec {
    /// Scatter of two columns from a Column data sheet.
    Xy {
        x: String,
        y: String,
        #[serde(default)]
        title: Option<String>,
    },
    /// One bar per non-empty column of a Column data sheet.
    Column {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        error_bars: GraphErrorBars,
        #[serde(default)]
        y_title: Option<String>,
    },
    /// Grouped bars from a Grouped data sheet (dispatch returns an
    /// error on a Column source until Grouped data sheets land).
    Grouped {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        error_bars: GraphErrorBars,
    },
    /// Kaplan–Meier curves from a Survival data sheet.
    Survival {
        #[serde(default)]
        title: Option<String>,
    },
    /// Heatmap over the cells of a Column data sheet treated as a
    /// row-major matrix (one row per data row, one column per
    /// column of the sheet).
    Heatmap {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        colormap: GraphColormap,
    },
}

impl GraphSpec {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Xy { .. } => "xy",
            Self::Column { .. } => "column",
            Self::Grouped { .. } => "grouped",
            Self::Survival { .. } => "survival",
            Self::Heatmap { .. } => "heatmap",
        }
    }

    pub fn default_title(&self) -> String {
        match self {
            Self::Xy { x, y, .. } => format!("Scatter: {y} vs {x}"),
            Self::Column { .. } => "Column plot".to_string(),
            Self::Grouped { .. } => "Grouped plot".to_string(),
            Self::Survival { .. } => "Survival curves".to_string(),
            Self::Heatmap { .. } => "Heatmap".to_string(),
        }
    }
}

/// Error-bar choice for Column / Grouped graphs. Matches the plot
/// layer's `ColumnErrorBars` / `GroupedErrorBars` by value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GraphErrorBars {
    None,
    #[default]
    Sd,
    Sem,
}

/// Colormap for a heatmap graph. Matches the plot layer's
/// `Colormap` by value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GraphColormap {
    #[default]
    DivergingBlueRed,
    Grayscale,
    SequentialBlue,
}

/// Which analysis to run, along with its parameters.
///
/// Column references are strings interpreted at run time as either
/// a 0-based index or the exact header name, matching the CLI and
/// MCP conventions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AnalysisSpec {
    /// Descriptive statistics over all non-empty columns.
    Descriptives,
    /// One-way ANOVA across all non-empty columns.
    OneWayAnova,
    /// Welch's two-sample t-test on columns `a` and `b`.
    WelchT { a: String, b: String },
    /// Pearson correlation on columns `a` and `b` (also returns
    /// Spearman alongside for free).
    Pearson { a: String, b: String },
    /// Simple linear OLS regression `y ~ x`.
    OlsRegression { x: String, y: String },
    /// Shapiro–Wilk normality on one column.
    ShapiroWilk { col: String },
}

impl AnalysisSpec {
    /// Short stable id — "descriptives", "pearson", … — used for
    /// routing and in default titles.
    pub fn id(&self) -> &'static str {
        match self {
            Self::Descriptives => "descriptives",
            Self::OneWayAnova => "one-way-anova",
            Self::WelchT { .. } => "welch-t",
            Self::Pearson { .. } => "pearson",
            Self::OlsRegression { .. } => "ols-regression",
            Self::ShapiroWilk { .. } => "shapiro-wilk",
        }
    }

    /// Human-readable default title for a sheet carrying this spec.
    pub fn default_title(&self) -> String {
        match self {
            Self::Descriptives => "Descriptives".to_string(),
            Self::OneWayAnova => "One-way ANOVA".to_string(),
            Self::WelchT { a, b } => format!("Welch t: {a} vs {b}"),
            Self::Pearson { a, b } => format!("Pearson: {a} vs {b}"),
            Self::OlsRegression { x, y } => format!("OLS: {y} ~ {x}"),
            Self::ShapiroWilk { col } => format!("Shapiro-Wilk: {col}"),
        }
    }
}

/// A GraphCraft project — the top of the file, and the in-memory
/// state the engine carries for the open document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub sheets: Vec<Sheet>,
}

impl Project {
    /// New empty project.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            sheets: Vec::new(),
        }
    }

    /// Allocate the next unused sheet id (max of existing ids + 1,
    /// starting at 1).
    pub fn next_sheet_id(&self) -> SheetId {
        let max = self.sheets.iter().map(|s| s.id.0).max().unwrap_or(0);
        SheetId(max + 1)
    }

    /// Append a sheet with a freshly-allocated id. Returns the id.
    pub fn add_sheet(&mut self, title: impl Into<String>, body: SheetBody) -> SheetId {
        let id = self.next_sheet_id();
        self.sheets.push(Sheet {
            id,
            title: title.into(),
            body,
        });
        id
    }

    /// Append a Data sheet carrying `table`.
    pub fn add_data_sheet(&mut self, title: impl Into<String>, table: AnyTable) -> SheetId {
        self.add_sheet(title, SheetBody::Data { table })
    }

    /// Lookup by id. `None` if the id does not refer to a current
    /// sheet.
    pub fn get(&self, id: SheetId) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.id == id)
    }

    pub fn get_mut(&mut self, id: SheetId) -> Option<&mut Sheet> {
        self.sheets.iter_mut().find(|s| s.id == id)
    }

    /// Count sheets whose body is a Data sheet.
    pub fn data_sheet_count(&self) -> usize {
        self.sheets
            .iter()
            .filter(|s| matches!(s.body, SheetBody::Data { .. }))
            .count()
    }

    /// Count sheets whose body is an Analysis sheet.
    pub fn analysis_sheet_count(&self) -> usize {
        self.sheets
            .iter()
            .filter(|s| matches!(s.body, SheetBody::Analysis { .. }))
            .count()
    }

    /// Count sheets whose body is a Graph sheet.
    pub fn graph_sheet_count(&self) -> usize {
        self.sheets
            .iter()
            .filter(|s| matches!(s.body, SheetBody::Graph { .. }))
            .count()
    }

    /// If `id` refers to a Data sheet, return its table.
    pub fn data_sheet_table(&self, id: SheetId) -> Option<&AnyTable> {
        match self.get(id)?.body {
            SheetBody::Data { ref table } => Some(table),
            _ => None,
        }
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

/// Top-level document stored in a `.graphcraft` file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub project: Project,
}

impl Document {
    pub fn empty() -> Self {
        Self {
            format: FORMAT_TAG.to_string(),
            version: FORMAT_VERSION,
            project: Project::default(),
        }
    }

    /// Build a document that wraps the given project.
    pub fn from_project(project: Project) -> Self {
        Self {
            format: FORMAT_TAG.to_string(),
            version: FORMAT_VERSION,
            project,
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("not a graphcraft document")]
    WrongFormat,
    #[error("unsupported format version {0}")]
    UnsupportedVersion(u32),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("csv input is empty")]
    CsvEmpty,
    #[error("csv input is not valid UTF-8")]
    CsvNotUtf8,
    #[error("csv input too large: {0} bytes")]
    CsvTooLarge(usize),
    #[error("csv line {line}: unterminated quoted field")]
    CsvUnterminatedQuote { line: usize },
    #[error("csv line {line}: row has {got} cells but header has {header}")]
    CsvRowTooLong {
        line: usize,
        got: usize,
        header: usize,
    },
    #[error("csv line {line} column {column}: cell {cell:?} is not numeric")]
    CsvNotNumeric {
        line: usize,
        column: usize,
        cell: String,
    },
}

/// Parse a `.graphcraft` byte stream. Accepts v2 directly and
/// v1 with an in-memory one-way migration.
pub fn parse(bytes: &[u8]) -> Result<Document, FormatError> {
    // Inspect the version first so we can dispatch to the right
    // shape. Unrecognised shapes fall through to WrongFormat.
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    let format = value.get("format").and_then(|v| v.as_str()).unwrap_or("");
    if format != FORMAT_TAG {
        return Err(FormatError::WrongFormat);
    }
    let version = value
        .get("version")
        .and_then(|v| v.as_u64())
        .ok_or(FormatError::WrongFormat)? as u32;
    match version {
        2 => {
            let doc: Document = serde_json::from_value(value)?;
            Ok(doc)
        }
        1 => Ok(migrate_v1(value)?),
        other => Err(FormatError::UnsupportedVersion(other)),
    }
}

/// v1 shape, kept just long enough to read a file and move it to v2.
#[derive(Deserialize)]
struct V1Document {
    #[serde(default)]
    tables: Vec<AnyTable>,
}

fn migrate_v1(value: serde_json::Value) -> Result<Document, FormatError> {
    let v1: V1Document = serde_json::from_value(value)?;
    let mut project = Project::new("Untitled");
    for (i, table) in v1.tables.into_iter().enumerate() {
        let title = default_title_for(&table, i);
        project.add_data_sheet(title, table);
    }
    Ok(Document::from_project(project))
}

fn default_title_for(table: &AnyTable, index: usize) -> String {
    let own = match table {
        AnyTable::Column(t) => &t.title,
        AnyTable::Xy(t) => &t.title,
        AnyTable::Grouped(t) => &t.title,
        AnyTable::Contingency(t) => &t.title,
        AnyTable::Survival(t) => &t.title,
        AnyTable::PartsOfWhole(t) => &t.title,
        AnyTable::MultipleVariables(t) => &t.title,
        AnyTable::Nested(t) => &t.title,
    };
    if own.is_empty() {
        format!("Data {}", index + 1)
    } else {
        own.clone()
    }
}

pub fn serialize(doc: &Document) -> Result<Vec<u8>, FormatError> {
    Ok(serde_json::to_vec_pretty(doc)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphcraft_table::{Column, ColumnTable};

    fn sample_column_table(title: &str) -> AnyTable {
        ColumnTable {
            title: title.into(),
            row_titles: Vec::new(),
            columns: vec![Column {
                title: "ctrl".into(),
                values: vec![Some(1.0), Some(2.5), None, Some(3.3)],
            }],
        }
        .into()
    }

    #[test]
    fn empty_doc_round_trips() {
        let doc = Document::empty();
        let bytes = serialize(&doc).unwrap();
        let parsed = parse(&bytes).unwrap();
        assert_eq!(doc, parsed);
        assert_eq!(parsed.version, FORMAT_VERSION);
    }

    #[test]
    fn wrong_format_rejected() {
        let bytes = br#"{"format":"notgraphcraft","version":2,"project":{"title":"","sheets":[]}}"#;
        assert!(matches!(parse(bytes), Err(FormatError::WrongFormat)));
    }

    #[test]
    fn unsupported_version_rejected() {
        let bytes = br#"{"format":"graphcraft","version":99,"project":{"title":"","sheets":[]}}"#;
        assert!(matches!(
            parse(bytes),
            Err(FormatError::UnsupportedVersion(99))
        ));
    }

    #[test]
    fn project_with_data_sheet_round_trips() {
        let mut project = Project::new("Experiment 1");
        let id = project.add_data_sheet("cells", sample_column_table("cells"));
        assert_eq!(id, SheetId(1));
        assert_eq!(project.next_sheet_id(), SheetId(2));
        let doc = Document::from_project(project);
        let bytes = serialize(&doc).unwrap();
        let parsed = parse(&bytes).unwrap();
        assert_eq!(doc, parsed);
        assert_eq!(parsed.project.data_sheet_count(), 1);
    }

    #[test]
    fn next_sheet_id_tracks_max() {
        let mut p = Project::new("x");
        p.sheets.push(Sheet {
            id: SheetId(5),
            title: "manual".into(),
            body: SheetBody::Data {
                table: sample_column_table("a"),
            },
        });
        assert_eq!(p.next_sheet_id(), SheetId(6));
        let id = p.add_data_sheet("b", sample_column_table("b"));
        assert_eq!(id, SheetId(6));
    }

    #[test]
    fn v1_file_auto_migrates_to_project() {
        // Hand-write a v1 shape with two tables.
        let v1 = serde_json::json!({
            "format": "graphcraft",
            "version": 1,
            "tables": [
                { "kind": "column", "title": "A",
                  "columns": [{"title": "x", "values": [1.0, 2.0]}] },
                { "kind": "column", "title": "",
                  "columns": [{"title": "y", "values": [3.0]}] }
            ]
        });
        let bytes = serde_json::to_vec(&v1).unwrap();
        let doc = parse(&bytes).unwrap();
        assert_eq!(doc.version, 2);
        assert_eq!(doc.project.sheets.len(), 2);
        assert_eq!(doc.project.sheets[0].title, "A");
        // Empty table title gets a default.
        assert_eq!(doc.project.sheets[1].title, "Data 2");
        // Ids are sequential starting at 1.
        assert_eq!(doc.project.sheets[0].id, SheetId(1));
        assert_eq!(doc.project.sheets[1].id, SheetId(2));
    }

    #[test]
    fn migrated_project_round_trips_at_v2() {
        let v1 = serde_json::json!({
            "format": "graphcraft",
            "version": 1,
            "tables": [
                { "kind": "column", "title": "cells",
                  "columns": [{"title": "ctrl", "values": [1.0, 2.0]}] }
            ]
        });
        let doc1 = parse(&serde_json::to_vec(&v1).unwrap()).unwrap();
        let bytes = serialize(&doc1).unwrap();
        let doc2 = parse(&bytes).unwrap();
        assert_eq!(doc1, doc2);
    }

    #[test]
    fn sheet_body_tag_is_stable() {
        // The serde discriminator is `"kind": "data"`. New
        // variants (Analysis, Graph) will add their own tags
        // without changing this one, so old files stay readable.
        let p = {
            let mut p = Project::new("x");
            p.add_data_sheet("one", sample_column_table("one"));
            p
        };
        let bytes = serialize(&Document::from_project(p)).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains(r#""kind": "data""#));
    }

    #[test]
    fn sheet_lookup_by_id() {
        let mut p = Project::new("x");
        let a = p.add_data_sheet("a", sample_column_table("a"));
        let b = p.add_data_sheet("b", sample_column_table("b"));
        assert_eq!(p.get(a).unwrap().title, "a");
        assert_eq!(p.get(b).unwrap().title, "b");
        assert!(p.get(SheetId(999)).is_none());
    }
}
