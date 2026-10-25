//! Layout sheet composition.
//!
//! Takes a [`SheetBody::Layout`] and composes its items — graph
//! references, analysis references, text blocks — into one SVG
//! page. Each referenced graph has its scene built from the
//! current source table, so a layout always reflects the data as
//! it stands.

use std::fmt::Write;

use graphcraft_format::{LayoutContent, Project, SheetBody, SheetId};

/// Error path for the layout composer.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutError {
    NotFound(SheetId),
    NotLayout(SheetId),
    ItemError { index: usize, message: String },
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "layout sheet {} not found", id.0),
            Self::NotLayout(id) => write!(f, "sheet {} is not a layout", id.0),
            Self::ItemError { index, message } => {
                write!(f, "layout item {index}: {message}")
            }
        }
    }
}

impl std::error::Error for LayoutError {}

/// Compose a layout sheet into one SVG document. Each
/// [`LayoutContent::Graph`] item builds its scene from the current
/// source and lands at the item's position / size (the graph is
/// rendered to its own SVG and nested via a `<g>` with a scale +
/// translate transform). Analysis items are rendered as a wrapped
/// text block; Text items are plain text.
pub fn build_layout_svg(project: &Project, id: SheetId) -> Result<String, LayoutError> {
    let sheet = project.get(id).ok_or(LayoutError::NotFound(id))?;
    let (page_w, page_h, items) = match &sheet.body {
        SheetBody::Layout {
            page_width_pt,
            page_height_pt,
            items,
        } => (*page_width_pt, *page_height_pt, items),
        _ => return Err(LayoutError::NotLayout(id)),
    };

    let mut out = String::with_capacity(4096);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n");
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
        fmt_f(page_w),
        fmt_f(page_h),
        fmt_f(page_w),
        fmt_f(page_h),
    );
    let _ = writeln!(
        out,
        "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"white\"/>",
        fmt_f(page_w),
        fmt_f(page_h),
    );

    for (idx, item) in items.iter().enumerate() {
        match &item.content {
            LayoutContent::Graph { sheet } => {
                compose_graph_item(project, &mut out, idx, item, *sheet)?;
            }
            LayoutContent::Analysis { sheet } => {
                compose_analysis_item(project, &mut out, idx, item, *sheet)?;
            }
            LayoutContent::Text { text, font_size_pt } => {
                compose_text_item(&mut out, item, text, *font_size_pt);
            }
        }
    }

    out.push_str("</svg>\n");
    Ok(out)
}

fn compose_graph_item(
    project: &Project,
    out: &mut String,
    idx: usize,
    item: &graphcraft_format::LayoutItem,
    graph_id: SheetId,
) -> Result<(), LayoutError> {
    let graph = project
        .get(graph_id)
        .ok_or_else(|| LayoutError::ItemError {
            index: idx,
            message: format!("graph sheet {} not found", graph_id.0),
        })?;
    if !matches!(&graph.body, SheetBody::Graph { .. }) {
        return Err(LayoutError::ItemError {
            index: idx,
            message: format!("sheet {} is not a graph", graph_id.0),
        });
    }
    let scene = crate::graph::build_graph_scene_for_sheet(project, graph_id).map_err(|e| {
        LayoutError::ItemError {
            index: idx,
            message: format!("{e}"),
        }
    })?;
    let scene_w = scene.width_pt;
    let scene_h = scene.height_pt;
    // Render the scene to its own SVG, strip the outer <svg>
    // wrapper, and nest the inner markup in a <g> with the
    // placement transform. render_svg always emits the same
    // outer-tag shape (we authored it in M5.1), so this strip is
    // deterministic.
    let inner_svg = graphcraft_render::render_svg(&scene);
    let inner = strip_outer_svg(&inner_svg);
    let sx = if scene_w > 0.0 {
        item.width_pt / scene_w
    } else {
        1.0
    };
    let sy = if scene_h > 0.0 {
        item.height_pt / scene_h
    } else {
        1.0
    };
    let _ = writeln!(
        out,
        "<g transform=\"translate({} {}) scale({} {})\">",
        fmt_f(item.x_pt),
        fmt_f(item.y_pt),
        fmt_f(sx),
        fmt_f(sy),
    );
    out.push_str(inner);
    out.push_str("</g>\n");
    Ok(())
}

fn compose_analysis_item(
    project: &Project,
    out: &mut String,
    idx: usize,
    item: &graphcraft_format::LayoutItem,
    sheet: SheetId,
) -> Result<(), LayoutError> {
    let analysis = project.get(sheet).ok_or_else(|| LayoutError::ItemError {
        index: idx,
        message: format!("analysis sheet {} not found", sheet.0),
    })?;
    let (title, result) = match &analysis.body {
        SheetBody::Analysis { result, .. } => (analysis.title.clone(), result.clone()),
        _ => {
            return Err(LayoutError::ItemError {
                index: idx,
                message: format!("sheet {} is not an analysis", sheet.0),
            });
        }
    };
    // Simple presentation: title line, then pretty-printed JSON
    // in a monospace font. Clipped to the item rect.
    let _ = writeln!(
        out,
        "<g transform=\"translate({} {})\">",
        fmt_f(item.x_pt),
        fmt_f(item.y_pt),
    );
    let _ = writeln!(
        out,
        "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"#f7f7f7\" stroke=\"#ccc\" stroke-width=\"0.5\"/>",
        fmt_f(item.width_pt),
        fmt_f(item.height_pt),
    );
    let _ = writeln!(
        out,
        "<text x=\"8\" y=\"16\" font-family=\"sans-serif\" font-size=\"12\" fill=\"black\">{}</text>",
        escape_xml(&title),
    );
    let body_text = serde_json::to_string_pretty(&result).unwrap_or_default();
    let mut y = 32.0_f64;
    for line in body_text.lines() {
        if y + 10.0 > item.height_pt {
            break;
        }
        let _ = writeln!(
            out,
            "<text x=\"8\" y=\"{}\" font-family=\"ui-monospace, monospace\" font-size=\"9\" fill=\"black\" xml:space=\"preserve\">{}</text>",
            fmt_f(y),
            escape_xml(line),
        );
        y += 11.0;
    }
    out.push_str("</g>\n");
    Ok(())
}

fn compose_text_item(
    out: &mut String,
    item: &graphcraft_format::LayoutItem,
    text: &str,
    font_size_pt: f64,
) {
    let _ = writeln!(
        out,
        "<g transform=\"translate({} {})\">",
        fmt_f(item.x_pt),
        fmt_f(item.y_pt),
    );
    let mut y = font_size_pt;
    for line in text.lines() {
        if y > item.height_pt {
            break;
        }
        let _ = writeln!(
            out,
            "<text x=\"0\" y=\"{}\" font-family=\"sans-serif\" font-size=\"{}\" fill=\"black\">{}</text>",
            fmt_f(y),
            fmt_f(font_size_pt),
            escape_xml(line),
        );
        y += font_size_pt * 1.25;
    }
    out.push_str("</g>\n");
}

fn strip_outer_svg(doc: &str) -> &str {
    let start = match doc.find("<svg") {
        Some(i) => i,
        None => return doc,
    };
    let open_end = match doc[start..].find('>') {
        Some(i) => start + i + 1,
        None => return doc,
    };
    let close = match doc.rfind("</svg>") {
        Some(i) => i,
        None => return &doc[open_end..],
    };
    &doc[open_end..close]
}

fn fmt_f(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use graphcraft_format::{
        AnalysisSpec, GraphSpec, LayoutContent, LayoutItem, Project, Sheet, SheetBody,
    };
    use graphcraft_table::{Column, ColumnTable};

    use super::*;

    fn sample_project() -> (Project, SheetId, SheetId, SheetId) {
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
        let data = p.add_data_sheet("tbl", table.into());
        let graph = crate::add_graph_sheet(
            &mut p,
            data,
            GraphSpec::Xy {
                x: "x".into(),
                y: "y".into(),
                title: None,
            },
        )
        .unwrap();
        let analysis = crate::add_analysis_sheet(&mut p, data, AnalysisSpec::Descriptives).unwrap();
        (p, data, graph, analysis)
    }

    #[test]
    fn empty_layout_renders_blank_page() {
        let mut p = Project::new("x");
        let id = p.add_layout_sheet("Figure 1");
        let svg = build_layout_svg(&p, id).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("width=\"612\""));
        assert!(svg.ends_with("</svg>\n"));
    }

    #[test]
    fn graph_item_composes_into_layout() {
        let (mut p, _, graph, _) = sample_project();
        let layout = p.add_layout_sheet("Figure");
        p.add_layout_item(
            layout,
            LayoutItem {
                x_pt: 36.0,
                y_pt: 36.0,
                width_pt: 300.0,
                height_pt: 220.0,
                content: LayoutContent::Graph { sheet: graph },
            },
        )
        .unwrap();
        let svg = build_layout_svg(&p, layout).unwrap();
        // The transform wraps the inner graph markup.
        assert!(svg.contains("transform=\"translate(36 36)"));
        assert!(svg.contains("<circle")); // from the scatter marker
    }

    #[test]
    fn analysis_item_renders_title_and_body() {
        let (mut p, _, _, analysis) = sample_project();
        let layout = p.add_layout_sheet("Figure");
        p.add_layout_item(
            layout,
            LayoutItem {
                x_pt: 40.0,
                y_pt: 300.0,
                width_pt: 260.0,
                height_pt: 200.0,
                content: LayoutContent::Analysis { sheet: analysis },
            },
        )
        .unwrap();
        let svg = build_layout_svg(&p, layout).unwrap();
        assert!(svg.contains("Descriptives"));
        assert!(svg.contains("descriptives")); // JSON body
    }

    #[test]
    fn text_item_renders_each_line() {
        let mut p = Project::new("x");
        let layout = p.add_layout_sheet("Figure");
        p.add_layout_item(
            layout,
            LayoutItem {
                x_pt: 10.0,
                y_pt: 10.0,
                width_pt: 300.0,
                height_pt: 100.0,
                content: LayoutContent::Text {
                    text: "Hello\nWorld".into(),
                    font_size_pt: 14.0,
                },
            },
        )
        .unwrap();
        let svg = build_layout_svg(&p, layout).unwrap();
        assert!(svg.contains(">Hello<"));
        assert!(svg.contains(">World<"));
    }

    #[test]
    fn not_a_layout_rejected() {
        let (p, data, _, _) = sample_project();
        let err = build_layout_svg(&p, data).unwrap_err();
        assert!(matches!(err, LayoutError::NotLayout(_)));
    }

    #[test]
    fn missing_graph_errors_cleanly() {
        let mut p = Project::new("x");
        let layout = p.add_layout_sheet("Figure");
        p.add_layout_item(
            layout,
            LayoutItem {
                x_pt: 0.0,
                y_pt: 0.0,
                width_pt: 100.0,
                height_pt: 100.0,
                content: LayoutContent::Graph {
                    sheet: SheetId(999),
                },
            },
        )
        .unwrap();
        let err = build_layout_svg(&p, layout).unwrap_err();
        assert!(matches!(err, LayoutError::ItemError { .. }));
    }

    #[test]
    fn add_layout_item_rejects_non_layout() {
        let (mut p, data, _, _) = sample_project();
        let err = p
            .add_layout_item(
                data,
                LayoutItem {
                    x_pt: 0.0,
                    y_pt: 0.0,
                    width_pt: 1.0,
                    height_pt: 1.0,
                    content: LayoutContent::Text {
                        text: "hi".into(),
                        font_size_pt: 10.0,
                    },
                },
            )
            .unwrap_err();
        assert_eq!(err, "not a layout sheet");
    }

    #[test]
    fn ensure_sheet_rebind_in_scope() {
        // Keep the Sheet import used so we don't drift on refactor.
        let _ = Sheet {
            id: SheetId(1),
            title: "x".into(),
            body: SheetBody::Data {
                table: graphcraft_table::AnyTable::Column(ColumnTable {
                    title: String::new(),
                    row_titles: Vec::new(),
                    columns: Vec::new(),
                }),
            },
        };
    }
}
