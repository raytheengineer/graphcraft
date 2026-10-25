#![allow(clippy::needless_range_loop)]
//! Native scene painter.
//!
//! Paints a `graphcraft_plot::Scene` into an egui `Painter` at
//! 1 scene point = 1 scaled pixel, preserving aspect ratio within
//! the UI's available area. The painter supports every `Series`
//! variant the SVG and PDF renderers do: Scatter, Line, Bars,
//! GroupedBars, CensorTicks, Heatmap.
//!
//! Why native: it skips the SVG-text round-trip, picks up egui's
//! anti-aliasing, and lets later interactive gestures (zoom, hover
//! read-out, click-select) hang off the same code path.

use egui::{
    Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui,
    Vec2,
};
use graphcraft_plot::{
    Color as PlotColor, ColumnBar, GroupedBar, HeatmapCell, Marker, MarkerStyle, Scene, Series,
};

/// Paint `scene` into the UI's available area, preserving aspect
/// ratio. Returns the egui `Response` for the allocated rect so
/// the caller can wire future interactions (hover readouts, etc.)
/// to it.
pub fn paint_scene(ui: &mut Ui, scene: &Scene) -> Response {
    let available = ui.available_size();
    // Minimum paint area so a very small pane still shows a
    // readable thumbnail rather than a one-pixel smear.
    let w_pt = scene.width_pt.max(1.0) as f32;
    let h_pt = scene.height_pt.max(1.0) as f32;
    let aspect = w_pt / h_pt;
    let (w, h) = if available.x / available.y > aspect {
        (available.y * aspect, available.y)
    } else {
        (available.x, available.x / aspect)
    };
    let w = w.max(200.0);
    let h = h.max(140.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let painter = ui.painter_at(rect);
    paint_into(&painter, rect, scene);
    response
}

fn paint_into(p: &Painter, rect: Rect, scene: &Scene) {
    // Scale factor: scene points → pixels. We use `sy` for text
    // too so labels shrink/grow with the plot.
    let sx = rect.width() / scene.width_pt as f32;
    let sy = rect.height() / scene.height_pt as f32;
    let s = sx.min(sy);

    let to_px = |pt_x: f64, pt_y: f64| -> Pos2 {
        Pos2::new(
            rect.left() + pt_x as f32 * sx,
            rect.top() + pt_y as f32 * sy,
        )
    };

    // Background.
    p.rect_filled(rect, 0.0, Color32::WHITE);

    // Plot area.
    let (plot_x, plot_y, plot_w, plot_h) = scene.plot_rect();
    let plot_px = Rect::from_min_size(
        to_px(plot_x, plot_y),
        Vec2::new(plot_w as f32 * sx, plot_h as f32 * sy),
    );
    p.rect_stroke(
        plot_px,
        0.0,
        Stroke::new(1.0, Color32::BLACK),
        egui::StrokeKind::Outside,
    );

    // Data-space → plot pixels.
    let x_span = scene.x_axis.max - scene.x_axis.min;
    let y_span = scene.y_axis.max - scene.y_axis.min;
    let x_safe = if x_span == 0.0 { 1.0 } else { x_span };
    let y_safe = if y_span == 0.0 { 1.0 } else { y_span };
    let data_to_px = |dx: f64, dy: f64| -> Pos2 {
        let fx = (dx - scene.x_axis.min) / x_safe;
        let fy = (dy - scene.y_axis.min) / y_safe;
        Pos2::new(
            plot_px.left() + fx as f32 * plot_px.width(),
            plot_px.bottom() - fy as f32 * plot_px.height(),
        )
    };

    // Title.
    if let Some(ref t) = scene.title {
        p.text(
            to_px(scene.width_pt * 0.5, scene.margins_pt.0 * 0.5),
            Align2::CENTER_CENTER,
            t,
            prop(12.0 * s),
            Color32::BLACK,
        );
    }

    // Axis titles.
    if let Some(ref t) = scene.x_axis.title {
        p.text(
            to_px(
                plot_x + plot_w * 0.5,
                plot_y + plot_h + scene.margins_pt.2 * 0.6,
            ),
            Align2::CENTER_CENTER,
            t,
            prop(11.0 * s),
            Color32::BLACK,
        );
    }
    if let Some(ref t) = scene.y_axis.title {
        // Rotate 90° counter-clockwise so the title reads
        // bottom-to-top up the left edge of the plot. We
        // lay the galley out horizontally, then stamp a
        // TextShape with a negative angle so its upper-left
        // pivot lands at the correct side-of-plot position.
        let galley = p.layout_no_wrap(t.clone(), prop(11.0 * s), Color32::BLACK);
        let text_w = galley.size().x;
        let text_h = galley.size().y;
        // Place the rotated text so its centre sits on
        // (plot_left - gap, plot_centre_y).
        let anchor_x = plot_px.left() - 32.0 * s;
        let anchor_y = plot_px.center().y;
        // After rotation by -π/2 radians (counter-clockwise),
        // the galley's local (0, 0) ends up at `pos`, the local
        // (width, 0) at `pos + (0, -width)`, and (0, height) at
        // `pos + (height, 0)`. To centre the text, offset by
        // (-h/2, +w/2) from the anchor.
        let pos = Pos2::new(anchor_x - text_h * 0.5, anchor_y + text_w * 0.5);
        let mut shape = egui::epaint::TextShape::new(pos, galley, Color32::BLACK);
        shape.angle = -std::f32::consts::FRAC_PI_2;
        p.add(egui::Shape::Text(shape));
    }

    // X ticks.
    for &tv in &scene.x_axis.ticks {
        if tv < scene.x_axis.min || tv > scene.x_axis.max {
            continue;
        }
        let xi = data_to_px(tv, scene.y_axis.min).x;
        p.line_segment(
            [
                Pos2::new(xi, plot_px.bottom()),
                Pos2::new(xi, plot_px.bottom() + 4.0 * s),
            ],
            Stroke::new(1.0, Color32::BLACK),
        );
        p.text(
            Pos2::new(xi, plot_px.bottom() + 14.0 * s),
            Align2::CENTER_CENTER,
            fmt_tick(tv),
            prop(10.0 * s),
            Color32::BLACK,
        );
    }
    // Y ticks.
    for &tv in &scene.y_axis.ticks {
        if tv < scene.y_axis.min || tv > scene.y_axis.max {
            continue;
        }
        let yi = data_to_px(scene.x_axis.min, tv).y;
        p.line_segment(
            [
                Pos2::new(plot_px.left() - 4.0 * s, yi),
                Pos2::new(plot_px.left(), yi),
            ],
            Stroke::new(1.0, Color32::BLACK),
        );
        p.text(
            Pos2::new(plot_px.left() - 7.0 * s, yi),
            Align2::RIGHT_CENTER,
            fmt_tick(tv),
            prop(10.0 * s),
            Color32::BLACK,
        );
    }

    // Series.
    for series in &scene.series {
        match series {
            Series::Scatter { points, marker, .. } => {
                for &(dx, dy) in points {
                    paint_marker(p, data_to_px(dx, dy), *marker, s);
                }
            }
            Series::Line { points, style, .. } => {
                if points.len() >= 2 {
                    let pts: Vec<Pos2> = points.iter().map(|&(x, y)| data_to_px(x, y)).collect();
                    p.add(Shape::line(
                        pts,
                        Stroke::new(style.width_pt as f32 * s, to_c32(style.stroke)),
                    ));
                }
            }
            Series::Bars { bars, marker, .. } => {
                paint_bars(p, bars, *marker, s, &data_to_px);
            }
            Series::GroupedBars {
                bars,
                subcolumn,
                subcolumns_per_group,
                marker,
                ..
            } => {
                paint_grouped_bars(
                    p,
                    bars,
                    *subcolumn,
                    *subcolumns_per_group,
                    *marker,
                    s,
                    &data_to_px,
                );
            }
            Series::CensorTicks { points, style, .. } => {
                for &(dx, dy) in points {
                    let pos = data_to_px(dx, dy);
                    p.line_segment(
                        [
                            Pos2::new(pos.x, pos.y - 4.0 * s),
                            Pos2::new(pos.x, pos.y + 4.0 * s),
                        ],
                        Stroke::new(style.width_pt as f32 * s, to_c32(style.stroke)),
                    );
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
                paint_heatmap(
                    p, plot_px, *n_rows, *n_cols, cells, row_labels, col_labels, s,
                );
            }
        }
    }
}

fn paint_marker(p: &Painter, pos: Pos2, m: MarkerStyle, s: f32) {
    let r = m.size_pt as f32 * 0.5 * s;
    let fill = to_c32(m.fill);
    let stroke = Stroke::new(m.stroke_width_pt as f32 * s, to_c32(m.stroke));
    match m.shape {
        Marker::None => {}
        Marker::Circle => {
            p.circle_filled(pos, r, fill);
            p.circle_stroke(pos, r, stroke);
        }
        Marker::Square => {
            let rect = Rect::from_center_size(pos, Vec2::splat(m.size_pt as f32 * s));
            p.rect_filled(rect, 0.0, fill);
            p.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Outside);
        }
        Marker::Triangle => {
            let pts = vec![
                Pos2::new(pos.x, pos.y - r),
                Pos2::new(pos.x - r, pos.y + r),
                Pos2::new(pos.x + r, pos.y + r),
            ];
            p.add(Shape::convex_polygon(pts, fill, stroke));
        }
    }
}

fn paint_bars(
    p: &Painter,
    bars: &[ColumnBar],
    marker: MarkerStyle,
    s: f32,
    data_to_px: &impl Fn(f64, f64) -> Pos2,
) {
    let bar_half = 0.3;
    let default_fill = to_c32(marker.fill);
    let stroke = Stroke::new(marker.stroke_width_pt as f32 * s, to_c32(marker.stroke));
    for (i, b) in bars.iter().enumerate() {
        let x_center = i as f64 + 1.0;
        let p0 = data_to_px(x_center - bar_half, b.mean);
        let p1 = data_to_px(x_center + bar_half, 0.0);
        let rect = Rect::from_two_pos(p0, p1);
        let fill = b.color.map(to_c32).unwrap_or(default_fill);
        p.rect_filled(rect, 0.0, fill);
        p.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Outside);
        if b.error > 0.0 {
            let mid = data_to_px(x_center, 0.0).x;
            let up = data_to_px(x_center, b.mean + b.error).y;
            let dn = data_to_px(x_center, b.mean - b.error).y;
            draw_error_bar(p, mid, up, dn, to_c32(marker.stroke), s);
        }
    }
}

fn paint_grouped_bars(
    p: &Painter,
    bars: &[GroupedBar],
    subcolumn: usize,
    subcolumns_per_group: usize,
    marker: MarkerStyle,
    s: f32,
    data_to_px: &impl Fn(f64, f64) -> Pos2,
) {
    let total_slot = 0.8_f64;
    let slot = total_slot / subcolumns_per_group as f64;
    let bar_half = slot * 0.4;
    let left_edge = -total_slot * 0.5 + slot * subcolumn as f64;
    let center_within_group = left_edge + slot * 0.5;
    let fill = to_c32(marker.fill);
    let stroke = Stroke::new(marker.stroke_width_pt as f32 * s, to_c32(marker.stroke));
    for b in bars {
        let xc = (b.group_index as f64 + 1.0) + center_within_group;
        let p0 = data_to_px(xc - bar_half, b.mean);
        let p1 = data_to_px(xc + bar_half, 0.0);
        let rect = Rect::from_two_pos(p0, p1);
        p.rect_filled(rect, 0.0, fill);
        p.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Outside);
        if b.error > 0.0 {
            let mid = data_to_px(xc, 0.0).x;
            let up = data_to_px(xc, b.mean + b.error).y;
            let dn = data_to_px(xc, b.mean - b.error).y;
            draw_error_bar(p, mid, up, dn, to_c32(marker.stroke), s);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_heatmap(
    p: &Painter,
    plot_px: Rect,
    n_rows: usize,
    n_cols: usize,
    cells: &[HeatmapCell],
    row_labels: &[String],
    col_labels: &[String],
    s: f32,
) {
    if n_rows == 0 || n_cols == 0 {
        return;
    }
    let cell_w = plot_px.width() / n_cols as f32;
    let cell_h = plot_px.height() / n_rows as f32;
    for cell in cells {
        let x = plot_px.left() + cell.col as f32 * cell_w;
        let y = plot_px.top() + cell.row as f32 * cell_h;
        let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(cell_w, cell_h));
        p.rect_filled(rect, 0.0, to_c32(cell.color));
        p.rect_stroke(
            rect,
            0.0,
            Stroke::new(0.5, Color32::WHITE),
            egui::StrokeKind::Outside,
        );
    }
    for (j, label) in col_labels.iter().enumerate() {
        let x = plot_px.left() + (j as f32 + 0.5) * cell_w;
        p.text(
            Pos2::new(x, plot_px.top() - 4.0 * s),
            Align2::CENTER_BOTTOM,
            label,
            prop(10.0 * s),
            Color32::BLACK,
        );
    }
    for (i, label) in row_labels.iter().enumerate() {
        let y = plot_px.top() + (i as f32 + 0.5) * cell_h;
        p.text(
            Pos2::new(plot_px.left() - 6.0 * s, y),
            Align2::RIGHT_CENTER,
            label,
            prop(10.0 * s),
            Color32::BLACK,
        );
    }
}

fn draw_error_bar(p: &Painter, x: f32, y_up: f32, y_dn: f32, color: Color32, s: f32) {
    let stroke = Stroke::new(1.0 * s, color);
    p.line_segment([Pos2::new(x, y_up), Pos2::new(x, y_dn)], stroke);
    let cap = 3.0 * s;
    p.line_segment([Pos2::new(x - cap, y_up), Pos2::new(x + cap, y_up)], stroke);
    p.line_segment([Pos2::new(x - cap, y_dn), Pos2::new(x + cap, y_dn)], stroke);
}

fn to_c32(c: PlotColor) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

fn prop(size: f32) -> FontId {
    FontId::new(size.max(6.0), FontFamily::Proportional)
}

fn fmt_tick(v: f64) -> String {
    if (v.round() - v).abs() < 1e-9 * (1.0 + v.abs()) {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
