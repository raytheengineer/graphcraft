//! SVG renderer for `graphcraft_plot::Scene`.
//!
//! Writes a stand-alone SVG document into a buffer. The SVG is
//! viewer-only; it uses only basic shapes (`<rect>`, `<line>`,
//! `<circle>`, `<text>`) and inline attributes, no CSS and no
//! external resources.
//!
//! Coordinate system: canvas origin is top-left; the renderer
//! projects data-space `(x, y)` into pixel-space with y-axis
//! inversion so that larger y values appear higher.

use std::fmt::Write;

use graphcraft_plot::{Color, Legend, LegendPosition, Marker, MarkerStyle, Scene, Series};

/// Render a scene to a stand-alone SVG string.
pub fn render_svg(scene: &Scene) -> String {
    let mut out = String::with_capacity(2048);
    let _ = writeln!(
        out,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>"#
    );
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
        w = fmt_f(scene.width_pt),
        h = fmt_f(scene.height_pt),
    );
    out.push_str(r#"<rect x="0" y="0" width="100%" height="100%" fill="white"/>"#);
    out.push('\n');

    if let Some(ref t) = scene.title {
        let x = scene.width_pt * 0.5;
        let y = scene.margins_pt.0 * 0.5;
        let _ = writeln!(
            out,
            r#"<text x="{x}" y="{y}" text-anchor="middle" font-family="sans-serif" font-size="12" fill="black">{t}</text>"#,
            x = fmt_f(x),
            y = fmt_f(y),
            t = escape_xml(t),
        );
    }

    let (px, py, pw, ph) = scene.plot_rect();
    // Plot-area border.
    let _ = writeln!(
        out,
        r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="none" stroke="black" stroke-width="1"/>"#,
        x = fmt_f(px),
        y = fmt_f(py),
        w = fmt_f(pw),
        h = fmt_f(ph),
    );

    let x_span = scene.x_axis.max - scene.x_axis.min;
    let y_span = scene.y_axis.max - scene.y_axis.min;
    let x_safe = if x_span == 0.0 { 1.0 } else { x_span };
    let y_safe = if y_span == 0.0 { 1.0 } else { y_span };
    let to_px = |dx: f64, dy: f64| {
        let xi = px + (dx - scene.x_axis.min) / x_safe * pw;
        let yi = py + ph - (dy - scene.y_axis.min) / y_safe * ph;
        (xi, yi)
    };

    // X-axis ticks and labels.
    for &tv in &scene.x_axis.ticks {
        if tv < scene.x_axis.min || tv > scene.x_axis.max {
            continue;
        }
        let (xi, _) = to_px(tv, scene.y_axis.min);
        let y_tick = py + ph;
        let _ = writeln!(
            out,
            r#"<line x1="{x}" y1="{y}" x2="{x}" y2="{y2}" stroke="black" stroke-width="1"/>"#,
            x = fmt_f(xi),
            y = fmt_f(y_tick),
            y2 = fmt_f(y_tick + 4.0),
        );
        let _ = writeln!(
            out,
            r#"<text x="{x}" y="{y}" text-anchor="middle" font-family="sans-serif" font-size="10" fill="black">{t}</text>"#,
            x = fmt_f(xi),
            y = fmt_f(y_tick + 16.0),
            t = fmt_tick(tv),
        );
    }

    // Y-axis ticks and labels.
    for &tv in &scene.y_axis.ticks {
        if tv < scene.y_axis.min || tv > scene.y_axis.max {
            continue;
        }
        let (_, yi) = to_px(scene.x_axis.min, tv);
        let _ = writeln!(
            out,
            r#"<line x1="{x}" y1="{y}" x2="{x2}" y2="{y}" stroke="black" stroke-width="1"/>"#,
            x = fmt_f(px - 4.0),
            x2 = fmt_f(px),
            y = fmt_f(yi),
        );
        let _ = writeln!(
            out,
            r#"<text x="{x}" y="{y}" text-anchor="end" font-family="sans-serif" font-size="10" fill="black">{t}</text>"#,
            x = fmt_f(px - 7.0),
            y = fmt_f(yi + 3.0),
            t = fmt_tick(tv),
        );
    }

    // Axis titles.
    if let Some(ref t) = scene.x_axis.title {
        let x = px + pw * 0.5;
        let y = py + ph + 40.0;
        let _ = writeln!(
            out,
            r#"<text x="{x}" y="{y}" text-anchor="middle" font-family="sans-serif" font-size="11" fill="black">{t}</text>"#,
            x = fmt_f(x),
            y = fmt_f(y),
            t = escape_xml(t),
        );
    }
    if let Some(ref t) = scene.y_axis.title {
        let x = px - 42.0;
        let y = py + ph * 0.5;
        let _ = writeln!(
            out,
            r#"<text x="{x}" y="{y}" text-anchor="middle" transform="rotate(-90 {x} {y})" font-family="sans-serif" font-size="11" fill="black">{t}</text>"#,
            x = fmt_f(x),
            y = fmt_f(y),
            t = escape_xml(t),
        );
    }

    // Series.
    for s in &scene.series {
        match s {
            Series::Scatter { points, marker, .. } => {
                for &(dx, dy) in points {
                    let (xi, yi) = to_px(dx, dy);
                    render_marker(&mut out, xi, yi, marker);
                }
            }
            Series::Line { points, style, .. } => {
                if points.len() >= 2 {
                    let mut path = String::new();
                    for (i, &(dx, dy)) in points.iter().enumerate() {
                        let (xi, yi) = to_px(dx, dy);
                        let cmd = if i == 0 { 'M' } else { 'L' };
                        let _ = write!(path, "{cmd}{} {} ", fmt_f(xi), fmt_f(yi));
                    }
                    let _ = writeln!(
                        out,
                        r#"<path d="{d}" fill="none" stroke="{c}" stroke-width="{w}"/>"#,
                        d = path.trim_end(),
                        c = fmt_color(style.stroke),
                        w = fmt_f(style.width_pt),
                    );
                }
            }
        }
    }

    if let Some(ref legend) = scene.legend {
        render_legend(&mut out, scene, legend);
    }

    out.push_str("</svg>\n");
    out
}

fn render_marker(out: &mut String, cx: f64, cy: f64, m: &MarkerStyle) {
    let r = m.size_pt * 0.5;
    let fill = fmt_color(m.fill);
    let stroke = fmt_color(m.stroke);
    let sw = fmt_f(m.stroke_width_pt);
    match m.shape {
        Marker::Circle => {
            let _ = writeln!(
                out,
                r#"<circle cx="{cx}" cy="{cy}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>"#,
                cx = fmt_f(cx),
                cy = fmt_f(cy),
                r = fmt_f(r),
            );
        }
        Marker::Square => {
            let _ = writeln!(
                out,
                r#"<rect x="{x}" y="{y}" width="{s}" height="{s}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>"#,
                x = fmt_f(cx - r),
                y = fmt_f(cy - r),
                s = fmt_f(m.size_pt),
            );
        }
        Marker::Triangle => {
            let p1 = (cx, cy - r);
            let p2 = (cx - r, cy + r);
            let p3 = (cx + r, cy + r);
            let _ = writeln!(
                out,
                r#"<polygon points="{a},{b} {c},{d} {e},{f}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>"#,
                a = fmt_f(p1.0),
                b = fmt_f(p1.1),
                c = fmt_f(p2.0),
                d = fmt_f(p2.1),
                e = fmt_f(p3.0),
                f = fmt_f(p3.1),
            );
        }
        Marker::None => {}
    }
}

fn render_legend(out: &mut String, scene: &Scene, legend: &Legend) {
    let (px, py, pw, ph) = scene.plot_rect();
    let padding = 8.0;
    let line_height = 14.0;
    let swatch = 8.0;
    let entries: Vec<&Series> = scene.series.iter().collect();
    let box_h = padding * 2.0 + line_height * entries.len() as f64;
    // Rough width based on longest label.
    let max_label = entries.iter().map(|s| s.label().len()).max().unwrap_or(1);
    let box_w = padding * 2.0 + swatch + 4.0 + 6.5 * max_label as f64;
    let (x0, y0) = match legend.position {
        LegendPosition::TopRight => (px + pw - box_w - 6.0, py + 6.0),
        LegendPosition::TopLeft => (px + 6.0, py + 6.0),
        LegendPosition::BottomRight => (px + pw - box_w - 6.0, py + ph - box_h - 6.0),
        LegendPosition::BottomLeft => (px + 6.0, py + ph - box_h - 6.0),
    };
    let _ = writeln!(
        out,
        r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="white" stroke="black" stroke-width="0.5"/>"#,
        x = fmt_f(x0),
        y = fmt_f(y0),
        w = fmt_f(box_w),
        h = fmt_f(box_h),
    );
    for (i, s) in entries.iter().enumerate() {
        let y = y0 + padding + line_height * (i as f64 + 0.5);
        let color = match s {
            Series::Scatter { marker, .. } => marker.fill,
            Series::Line { style, .. } => style.stroke,
        };
        let _ = writeln!(
            out,
            r#"<rect x="{x}" y="{y}" width="{s}" height="{s}" fill="{c}" stroke="black" stroke-width="0.5"/>"#,
            x = fmt_f(x0 + padding),
            y = fmt_f(y - swatch * 0.5),
            s = fmt_f(swatch),
            c = fmt_color(color),
        );
        let _ = writeln!(
            out,
            r#"<text x="{x}" y="{y}" font-family="sans-serif" font-size="10" fill="black">{t}</text>"#,
            x = fmt_f(x0 + padding + swatch + 4.0),
            y = fmt_f(y + 3.0),
            t = escape_xml(s.label()),
        );
    }
}

fn fmt_f(v: f64) -> String {
    // 3 decimal places is plenty for pixel-space; strip trailing zeros.
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn fmt_tick(v: f64) -> String {
    // Data-space tick label — avoid unnecessary decimals when the
    // value is integral.
    if (v.round() - v).abs() < 1e-9 * (1.0 + v.abs()) {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{:.4}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn fmt_color(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use graphcraft_plot::{xy_scene_from_points, XyOptions};

    use super::*;

    #[test]
    fn svg_contains_structural_elements() {
        let pts = vec![(1.0, 2.0), (2.0, 4.0), (3.0, 6.0), (4.0, 8.0), (5.0, 10.0)];
        let opts = XyOptions {
            title: Some("test plot".into()),
            x_title: Some("x axis".into()),
            y_title: Some("y axis".into()),
            ..XyOptions::default()
        };
        let scene = xy_scene_from_points(&pts, &opts, "A".into()).unwrap();
        let svg = render_svg(&scene);
        assert!(svg.contains("<?xml"));
        assert!(svg.contains("<svg "));
        assert!(svg.contains("test plot"));
        assert!(svg.contains("x axis"));
        assert!(svg.contains("y axis"));
        assert!(svg.contains("<circle"));
        assert!(svg.ends_with("</svg>\n"));
    }

    #[test]
    fn tick_labels_present() {
        let pts = vec![(0.0, 0.0), (10.0, 100.0)];
        let scene = xy_scene_from_points(&pts, &XyOptions::default(), "A".into()).unwrap();
        let svg = render_svg(&scene);
        // The nice-tick axis should include 0 and 100 for data at
        // [0, 100] on the y side.
        assert!(svg.contains(">0<"));
        assert!(svg.contains(">100<"));
    }

    #[test]
    fn xml_escape() {
        let pts = vec![(1.0, 1.0)];
        let opts = XyOptions {
            title: Some("A & B < C".into()),
            ..XyOptions::default()
        };
        let scene = xy_scene_from_points(&pts, &opts, "x".into()).unwrap();
        let svg = render_svg(&scene);
        assert!(svg.contains("A &amp; B &lt; C"));
        assert!(!svg.contains("A & B"));
    }
}
