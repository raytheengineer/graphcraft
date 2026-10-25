#![allow(clippy::needless_range_loop)]
//! PDF 1.4 renderer for `graphcraft_plot::Scene`.
//!
//! Writes a single-page PDF 1.4 document as a `Vec<u8>` from a
//! scene — no third-party crate. The page uses one built-in
//! Helvetica font (one of the 14 "standard 14" fonts every PDF
//! viewer carries by spec, so no font file needs to be embedded).
//!
//! Coordinate note: PDF's origin is bottom-left and y increases
//! upward; our scene uses top-left origin. The renderer flips y
//! during projection rather than applying a page-level transform.
//!
//! Reference (behaviour): Adobe Systems (2006) *PDF Reference,
//! sixth edition, version 1.7*, §3 ("Syntax") and §8 ("Graphics").
//! Classical page-description model; no third-party or GraphPad
//! code.

use std::fmt::Write as _;
use std::io::Write as _;

use graphcraft_plot::{Color, Legend, LegendPosition, Marker, MarkerStyle, Scene, Series};

/// Render a scene to a PDF 1.4 byte vector.
pub fn render_pdf(scene: &Scene) -> Vec<u8> {
    let content = build_content(scene);
    let mut doc = PdfDoc::new();
    // Object 1: catalog, 2: pages, 3: page, 4: content, 5: font.
    let catalog = doc.add(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    let pages = doc.add(
        format!(
            "<< /Type /Pages /Count 1 /Kids [3 0 R] /MediaBox [0 0 {w} {h}] >>",
            w = fmt_f(scene.width_pt),
            h = fmt_f(scene.height_pt),
        )
        .into_bytes(),
    );
    let _ = (catalog, pages);
    let content_obj = doc.add_stream(content.into_bytes());
    let _page = doc.add(
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] \
             /Contents {c} 0 R \
             /Resources << /Font << /F1 5 0 R >> >> >>",
            w = fmt_f(scene.width_pt),
            h = fmt_f(scene.height_pt),
            c = content_obj,
        )
        .into_bytes(),
    );
    let _font = doc.add(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    doc.finish(1 /* catalog */)
}

/// Build the drawing content stream.
fn build_content(scene: &Scene) -> String {
    let mut c = String::with_capacity(4096);
    let (px, py_top, pw, ph) = scene.plot_rect();
    // py_top is the TOP of the plotting area in scene coordinates
    // (origin top-left). PDF uses bottom-left, so the plot's bottom
    // in PDF-y is `page_h - (py_top + ph)` and its top is
    // `page_h - py_top`. We expose a projector that returns
    // PDF-space (x, y).
    let page_h = scene.height_pt;
    let plot_bottom_pdf = page_h - (py_top + ph);
    let plot_top_pdf = page_h - py_top;

    // White background.
    let _ = writeln!(
        c,
        "1 1 1 rg 0 0 {w} {h} re f",
        w = fmt_f(scene.width_pt),
        h = fmt_f(page_h),
    );

    // Title.
    if let Some(ref t) = scene.title {
        let x = scene.width_pt * 0.5;
        let y = page_h - scene.margins_pt.0 * 0.5;
        emit_text(&mut c, x, y, 12.0, TextAnchor::Middle, t);
    }

    // Plot-area border.
    let _ = writeln!(
        c,
        "0 0 0 RG 1 w {x} {y} {w} {h} re S",
        x = fmt_f(px),
        y = fmt_f(plot_bottom_pdf),
        w = fmt_f(pw),
        h = fmt_f(ph),
    );

    let x_span = scene.x_axis.max - scene.x_axis.min;
    let y_span = scene.y_axis.max - scene.y_axis.min;
    let x_safe = if x_span == 0.0 { 1.0 } else { x_span };
    let y_safe = if y_span == 0.0 { 1.0 } else { y_span };
    let to_pdf = |dx: f64, dy: f64| {
        let xi = px + (dx - scene.x_axis.min) / x_safe * pw;
        // PDF y increases upward; our data y maps to plot bottom + fraction·ph.
        let yi = plot_bottom_pdf + (dy - scene.y_axis.min) / y_safe * ph;
        (xi, yi)
    };

    // X-axis ticks and labels.
    for &tv in &scene.x_axis.ticks {
        if tv < scene.x_axis.min || tv > scene.x_axis.max {
            continue;
        }
        let (xi, _) = to_pdf(tv, scene.y_axis.min);
        let _ = writeln!(
            c,
            "0 0 0 RG 1 w {x} {y1} m {x} {y2} l S",
            x = fmt_f(xi),
            y1 = fmt_f(plot_bottom_pdf),
            y2 = fmt_f(plot_bottom_pdf - 4.0),
        );
        emit_text(
            &mut c,
            xi,
            plot_bottom_pdf - 14.0,
            10.0,
            TextAnchor::Middle,
            &fmt_tick(tv),
        );
    }
    // Y-axis ticks and labels.
    for &tv in &scene.y_axis.ticks {
        if tv < scene.y_axis.min || tv > scene.y_axis.max {
            continue;
        }
        let (_, yi) = to_pdf(scene.x_axis.min, tv);
        let _ = writeln!(
            c,
            "0 0 0 RG 1 w {x1} {y} m {x2} {y} l S",
            x1 = fmt_f(px - 4.0),
            x2 = fmt_f(px),
            y = fmt_f(yi),
        );
        emit_text(
            &mut c,
            px - 7.0,
            yi - 3.0,
            10.0,
            TextAnchor::End,
            &fmt_tick(tv),
        );
    }

    // Axis titles.
    if let Some(ref t) = scene.x_axis.title {
        emit_text(
            &mut c,
            px + pw * 0.5,
            plot_bottom_pdf - 32.0,
            11.0,
            TextAnchor::Middle,
            t,
        );
    }
    if let Some(ref t) = scene.y_axis.title {
        // Rotate 90°: PDF text matrix.
        let x = px - 42.0;
        let y = plot_bottom_pdf + ph * 0.5;
        emit_text_rot(&mut c, x, y, 11.0, 90.0, t);
    }

    // Series.
    for s in &scene.series {
        match s {
            Series::Scatter { points, marker, .. } => {
                for &(dx, dy) in points {
                    let (xi, yi) = to_pdf(dx, dy);
                    draw_marker(&mut c, xi, yi, marker);
                }
            }
            Series::Line { points, style, .. } => {
                if points.len() >= 2 {
                    emit_stroke_color(&mut c, style.stroke);
                    let _ = writeln!(c, "{w} w", w = fmt_f(style.width_pt));
                    for (i, &(dx, dy)) in points.iter().enumerate() {
                        let (xi, yi) = to_pdf(dx, dy);
                        let op = if i == 0 { 'm' } else { 'l' };
                        let _ = writeln!(c, "{} {} {op}", fmt_f(xi), fmt_f(yi));
                    }
                    c.push_str("S\n");
                }
            }
            Series::Bars { bars, marker, .. } => {
                let bar_half = 0.3;
                for (i, b) in bars.iter().enumerate() {
                    let x_center = i as f64 + 1.0;
                    let (x0, y_top) = to_pdf(x_center - bar_half, b.mean.max(0.0));
                    let (x1, y_base) = to_pdf(x_center + bar_half, 0.0);
                    let per_bar = graphcraft_plot::MarkerStyle {
                        fill: b.color.unwrap_or(marker.fill),
                        ..*marker
                    };
                    draw_filled_rect(&mut c, x0, y_base, x1, y_top, &per_bar);
                    if b.error > 0.0 {
                        let (xm, _) = to_pdf(x_center, 0.0);
                        let (_, y_up) = to_pdf(x_center, b.mean + b.error);
                        let (_, y_dn) = to_pdf(x_center, b.mean - b.error);
                        draw_error_bar(&mut c, xm, y_up, y_dn, marker.stroke);
                    }
                    let label_x = (x0 + x1) * 0.5;
                    emit_text(
                        &mut c,
                        label_x,
                        plot_bottom_pdf - 14.0,
                        10.0,
                        TextAnchor::Middle,
                        &b.label,
                    );
                }
            }
            Series::GroupedBars {
                bars,
                subcolumn,
                subcolumns_per_group,
                marker,
                ..
            } => {
                let total_slot = 0.8_f64;
                let slot = total_slot / *subcolumns_per_group as f64;
                let bar_half = slot * 0.4;
                let left_edge = -total_slot * 0.5 + slot * *subcolumn as f64;
                let center_within_group = left_edge + slot * 0.5;
                for b in bars {
                    let xc = (b.group_index as f64 + 1.0) + center_within_group;
                    let (x0, y_top) = to_pdf(xc - bar_half, b.mean.max(0.0));
                    let (x1, y_base) = to_pdf(xc + bar_half, 0.0);
                    draw_filled_rect(&mut c, x0, y_base, x1, y_top, marker);
                    if b.error > 0.0 {
                        let (xm, _) = to_pdf(xc, 0.0);
                        let (_, y_up) = to_pdf(xc, b.mean + b.error);
                        let (_, y_dn) = to_pdf(xc, b.mean - b.error);
                        draw_error_bar(&mut c, xm, y_up, y_dn, marker.stroke);
                    }
                }
            }
            Series::Heatmap {
                n_rows,
                n_cols,
                cells,
                row_labels,
                col_labels,
                ..
            } => {
                let cell_w = pw / *n_cols as f64;
                let cell_h = ph / *n_rows as f64;
                for cell in cells {
                    // Row 0 is drawn at the top of the plot in scene
                    // coordinates. In PDF-y (upward), that is near
                    // the top of the plot: plot_top_pdf − row·cell_h.
                    let x = px + cell.col as f64 * cell_w;
                    let y_top = plot_top_pdf - cell.row as f64 * cell_h;
                    let y_bot = y_top - cell_h;
                    emit_fill_color(&mut c, cell.color);
                    emit_stroke_color(&mut c, Color::WHITE);
                    let _ = writeln!(
                        c,
                        "0.5 w {x} {y} {w} {h} re B",
                        x = fmt_f(x),
                        y = fmt_f(y_bot),
                        w = fmt_f(cell_w),
                        h = fmt_f(y_top - y_bot),
                    );
                }
                for (j, label) in col_labels.iter().enumerate() {
                    let x = px + (j as f64 + 0.5) * cell_w;
                    emit_text(
                        &mut c,
                        x,
                        plot_top_pdf + 4.0,
                        10.0,
                        TextAnchor::Middle,
                        label,
                    );
                }
                for (i, label) in row_labels.iter().enumerate() {
                    let y = plot_top_pdf - (i as f64 + 0.5) * cell_h - 3.0;
                    emit_text(&mut c, px - 6.0, y, 10.0, TextAnchor::End, label);
                }
            }
            Series::CensorTicks { points, style, .. } => {
                emit_stroke_color(&mut c, style.stroke);
                let _ = writeln!(c, "{w} w", w = fmt_f(style.width_pt));
                for &(dx, dy) in points {
                    let (xi, yi) = to_pdf(dx, dy);
                    let _ = writeln!(
                        c,
                        "{x} {y1} m {x} {y2} l S",
                        x = fmt_f(xi),
                        y1 = fmt_f(yi - 4.0),
                        y2 = fmt_f(yi + 4.0),
                    );
                }
            }
        }
    }

    if let Some(ref legend) = scene.legend {
        draw_legend(&mut c, scene, legend, plot_top_pdf, plot_bottom_pdf);
    }

    c
}

fn draw_filled_rect(c: &mut String, x0: f64, y0: f64, x1: f64, y1: f64, m: &MarkerStyle) {
    let x = x0.min(x1);
    let y = y0.min(y1);
    let w = (x1 - x0).abs();
    let h = (y1 - y0).abs();
    emit_fill_color(c, m.fill);
    emit_stroke_color(c, m.stroke);
    let _ = writeln!(
        c,
        "{sw} w {x} {y} {w} {h} re B",
        sw = fmt_f(m.stroke_width_pt),
        x = fmt_f(x),
        y = fmt_f(y),
        w = fmt_f(w),
        h = fmt_f(h),
    );
}

fn draw_marker(c: &mut String, cx: f64, cy: f64, m: &MarkerStyle) {
    let r = m.size_pt * 0.5;
    emit_fill_color(c, m.fill);
    emit_stroke_color(c, m.stroke);
    let _ = writeln!(c, "{sw} w", sw = fmt_f(m.stroke_width_pt));
    match m.shape {
        Marker::None => {}
        Marker::Square => {
            let _ = writeln!(
                c,
                "{x} {y} {s} {s} re B",
                x = fmt_f(cx - r),
                y = fmt_f(cy - r),
                s = fmt_f(m.size_pt),
            );
        }
        Marker::Triangle => {
            let _ = writeln!(
                c,
                "{x1} {y1} m {x2} {y2} l {x3} {y3} l h B",
                x1 = fmt_f(cx),
                y1 = fmt_f(cy + r),
                x2 = fmt_f(cx - r),
                y2 = fmt_f(cy - r),
                x3 = fmt_f(cx + r),
                y3 = fmt_f(cy - r),
            );
        }
        Marker::Circle => {
            // Four-cubic-Bezier circle approximation (κ = 0.5522847).
            const KAPPA: f64 = 0.552_284_749_830_793_4;
            let k = r * KAPPA;
            let _ = writeln!(
                c,
                "{x0} {y0} m {c1x} {c1y} {c2x} {c2y} {x1} {y1} c {c3x} {c3y} {c4x} {c4y} {x2} {y2} c {c5x} {c5y} {c6x} {c6y} {x3} {y3} c {c7x} {c7y} {c8x} {c8y} {x4} {y4} c h B",
                x0 = fmt_f(cx + r),
                y0 = fmt_f(cy),
                c1x = fmt_f(cx + r),
                c1y = fmt_f(cy + k),
                c2x = fmt_f(cx + k),
                c2y = fmt_f(cy + r),
                x1 = fmt_f(cx),
                y1 = fmt_f(cy + r),
                c3x = fmt_f(cx - k),
                c3y = fmt_f(cy + r),
                c4x = fmt_f(cx - r),
                c4y = fmt_f(cy + k),
                x2 = fmt_f(cx - r),
                y2 = fmt_f(cy),
                c5x = fmt_f(cx - r),
                c5y = fmt_f(cy - k),
                c6x = fmt_f(cx - k),
                c6y = fmt_f(cy - r),
                x3 = fmt_f(cx),
                y3 = fmt_f(cy - r),
                c7x = fmt_f(cx + k),
                c7y = fmt_f(cy - r),
                c8x = fmt_f(cx + r),
                c8y = fmt_f(cy - k),
                x4 = fmt_f(cx + r),
                y4 = fmt_f(cy),
            );
        }
    }
}

fn draw_error_bar(c: &mut String, x: f64, y_up: f64, y_dn: f64, color: Color) {
    emit_stroke_color(c, color);
    let _ = writeln!(c, "1 w");
    let _ = writeln!(
        c,
        "{x} {y1} m {x} {y2} l S",
        x = fmt_f(x),
        y1 = fmt_f(y_up),
        y2 = fmt_f(y_dn),
    );
    let cap = 3.0;
    let _ = writeln!(
        c,
        "{x1} {y} m {x2} {y} l S",
        x1 = fmt_f(x - cap),
        x2 = fmt_f(x + cap),
        y = fmt_f(y_up),
    );
    let _ = writeln!(
        c,
        "{x1} {y} m {x2} {y} l S",
        x1 = fmt_f(x - cap),
        x2 = fmt_f(x + cap),
        y = fmt_f(y_dn),
    );
}

fn draw_legend(c: &mut String, scene: &Scene, legend: &Legend, top: f64, bottom: f64) {
    let padding = 8.0;
    let line_height = 14.0;
    let swatch = 8.0;
    let entries: Vec<&Series> = scene.series.iter().collect();
    let box_h = padding * 2.0 + line_height * entries.len() as f64;
    let max_label = entries.iter().map(|s| s.label().len()).max().unwrap_or(1);
    let box_w = padding * 2.0 + swatch + 4.0 + 6.5 * max_label as f64;
    let (px, _py_top, pw, _ph) = scene.plot_rect();
    let (x0, y_top) = match legend.position {
        LegendPosition::TopRight => (px + pw - box_w - 6.0, top - 6.0),
        LegendPosition::TopLeft => (px + 6.0, top - 6.0),
        LegendPosition::BottomRight => (px + pw - box_w - 6.0, bottom + box_h + 6.0),
        LegendPosition::BottomLeft => (px + 6.0, bottom + box_h + 6.0),
    };
    let y0 = y_top - box_h;
    let _ = writeln!(
        c,
        "1 1 1 rg 0 0 0 RG 0.5 w {x} {y} {w} {h} re B",
        x = fmt_f(x0),
        y = fmt_f(y0),
        w = fmt_f(box_w),
        h = fmt_f(box_h),
    );
    for (i, s) in entries.iter().enumerate() {
        // Place top-down.
        let y_line = y_top - padding - line_height * (i as f64 + 0.5);
        let color = match s {
            Series::Scatter { marker, .. }
            | Series::Bars { marker, .. }
            | Series::GroupedBars { marker, .. } => marker.fill,
            Series::Line { style, .. } | Series::CensorTicks { style, .. } => style.stroke,
            Series::Heatmap { .. } => Color::BLACK,
        };
        emit_fill_color(c, color);
        let _ = writeln!(
            c,
            "0 0 0 RG 0.5 w {x} {y} {s} {s} re B",
            x = fmt_f(x0 + padding),
            y = fmt_f(y_line - swatch * 0.5),
            s = fmt_f(swatch),
        );
        emit_text(
            c,
            x0 + padding + swatch + 4.0,
            y_line - 3.0,
            10.0,
            TextAnchor::Start,
            s.label(),
        );
    }
}

#[derive(Clone, Copy)]
enum TextAnchor {
    Start,
    Middle,
    End,
}

fn emit_text(c: &mut String, x: f64, y: f64, size: f64, anchor: TextAnchor, text: &str) {
    let w = text_width(text, size);
    let x_shift = match anchor {
        TextAnchor::Start => 0.0,
        TextAnchor::Middle => -w * 0.5,
        TextAnchor::End => -w,
    };
    let _ = writeln!(
        c,
        "BT 0 0 0 rg /F1 {s} Tf {x} {y} Td ({t}) Tj ET",
        s = fmt_f(size),
        x = fmt_f(x + x_shift),
        y = fmt_f(y),
        t = pdf_string(text),
    );
}

fn emit_text_rot(c: &mut String, x: f64, y: f64, size: f64, angle_deg: f64, text: &str) {
    let w = text_width(text, size);
    let (sin, cos) = (angle_deg.to_radians().sin(), angle_deg.to_radians().cos());
    // Center the text on (x, y) along its own baseline direction.
    let x0 = x - w * 0.5 * cos;
    let y0 = y - w * 0.5 * sin;
    let _ = writeln!(
        c,
        "BT 0 0 0 rg /F1 {s} Tf {a} {b} {d} {e} {x} {y} Tm ({t}) Tj ET",
        s = fmt_f(size),
        a = fmt_f(cos),
        b = fmt_f(sin),
        d = fmt_f(-sin),
        e = fmt_f(cos),
        x = fmt_f(x0),
        y = fmt_f(y0),
        t = pdf_string(text),
    );
}

fn text_width(text: &str, size: f64) -> f64 {
    // Rough Helvetica width — most ASCII glyphs are ~0.5 em.
    text.len() as f64 * size * 0.5
}

fn pdf_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '(' => out.push_str(r"\("),
            ')' => out.push_str(r"\)"),
            '\\' => out.push_str(r"\\"),
            c if (c as u32) < 0x20 => out.push('?'),
            c if (c as u32) > 0x7e => out.push('?'), // keep ASCII-safe for WinAnsi
            c => out.push(c),
        }
    }
    out
}

fn emit_fill_color(c: &mut String, color: Color) {
    let _ = writeln!(
        c,
        "{r} {g} {b} rg",
        r = fmt_f(color.0 as f64 / 255.0),
        g = fmt_f(color.1 as f64 / 255.0),
        b = fmt_f(color.2 as f64 / 255.0),
    );
}

fn emit_stroke_color(c: &mut String, color: Color) {
    let _ = writeln!(
        c,
        "{r} {g} {b} RG",
        r = fmt_f(color.0 as f64 / 255.0),
        g = fmt_f(color.1 as f64 / 255.0),
        b = fmt_f(color.2 as f64 / 255.0),
    );
}

fn fmt_f(v: f64) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn fmt_tick(v: f64) -> String {
    if (v.round() - v).abs() < 1e-9 * (1.0 + v.abs()) {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{:.4}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Low-level PDF object writer.
struct PdfDoc {
    buf: Vec<u8>,
    offsets: Vec<usize>,
}

impl PdfDoc {
    fn new() -> Self {
        let mut buf = Vec::new();
        // Header + a binary-marker comment so viewers detect this as
        // binary (Adobe PDF ref §3.4.1).
        buf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        Self {
            buf,
            offsets: Vec::new(),
        }
    }

    fn add(&mut self, body: Vec<u8>) -> usize {
        let obj_num = self.offsets.len() + 1;
        self.offsets.push(self.buf.len());
        let _ = writeln!(self.buf, "{obj_num} 0 obj");
        self.buf.extend_from_slice(&body);
        self.buf.extend_from_slice(b"\nendobj\n");
        obj_num
    }

    fn add_stream(&mut self, data: Vec<u8>) -> usize {
        let obj_num = self.offsets.len() + 1;
        self.offsets.push(self.buf.len());
        let _ = writeln!(
            self.buf,
            "{obj_num} 0 obj\n<< /Length {} >>\nstream",
            data.len()
        );
        self.buf.extend_from_slice(&data);
        self.buf.extend_from_slice(b"\nendstream\nendobj\n");
        obj_num
    }

    fn finish(mut self, root_obj: usize) -> Vec<u8> {
        let xref_offset = self.buf.len();
        let n = self.offsets.len();
        let _ = writeln!(self.buf, "xref\n0 {}", n + 1);
        self.buf.extend_from_slice(b"0000000000 65535 f \n");
        for off in &self.offsets {
            let _ = writeln!(self.buf, "{:010} 00000 n ", off);
        }
        let _ = write!(
            self.buf,
            "trailer\n<< /Size {size} /Root {root} 0 R >>\nstartxref\n{off}\n%%EOF\n",
            size = n + 1,
            root = root_obj,
            off = xref_offset,
        );
        self.buf
    }
}

#[cfg(test)]
mod tests {
    use graphcraft_plot::{xy_scene_from_points, XyOptions};

    use super::*;

    #[test]
    fn pdf_has_header_and_eof() {
        let pts = vec![(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)];
        let scene = xy_scene_from_points(&pts, &XyOptions::default(), "A".into()).unwrap();
        let bytes = render_pdf(&scene);
        assert!(bytes.starts_with(b"%PDF-1.4"));
        let tail: Vec<u8> = bytes.iter().rev().take(10).cloned().collect();
        let tail: Vec<u8> = tail.into_iter().rev().collect();
        let s = std::str::from_utf8(&tail).unwrap_or("");
        assert!(s.contains("%%EOF"));
    }

    #[test]
    fn pdf_contains_xref_and_trailer() {
        let pts = vec![(0.0, 0.0), (1.0, 1.0)];
        let scene = xy_scene_from_points(&pts, &XyOptions::default(), "A".into()).unwrap();
        let bytes = render_pdf(&scene);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("xref"));
        assert!(text.contains("startxref"));
        assert!(text.contains("trailer"));
        assert!(text.contains("/Root"));
    }

    #[test]
    fn pdf_text_escapes_parentheses() {
        let pts = vec![(1.0, 1.0)];
        let opts = XyOptions {
            title: Some("Title (with parens)".into()),
            ..XyOptions::default()
        };
        let scene = xy_scene_from_points(&pts, &opts, "A".into()).unwrap();
        let bytes = render_pdf(&scene);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains(r"\(with parens\)"));
    }
}
