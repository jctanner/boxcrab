use crate::layout::{LayoutEdge, LayoutNode, LayoutResult, LayoutSubgraph};
use crate::diagram::{EdgeType, NodeShape};
use ab_glyph::{Font, FontRef, ScaleFont};
use std::path::Path;
use tiny_skia::*;

const FONT_SIZE: f32 = 14.0;
const SMALL_FONT_SIZE: f32 = 12.0;
const PADDING: f32 = 30.0;

fn load_font() -> FontRef<'static> {
    let font_data: &[u8] = include_bytes!("../../assets/DejaVuSans.ttf");
    FontRef::try_from_slice(font_data).expect("Failed to load embedded font")
}

pub fn export_png(layout: &LayoutResult, path: &Path, scale: f32) -> Result<(), String> {
    let width = ((layout.total_width + PADDING) * scale) as u32;
    let height = ((layout.total_height + PADDING) * scale) as u32;

    if width == 0 || height == 0 {
        return Err("Diagram has no content to render".to_string());
    }

    let mut pixmap =
        Pixmap::new(width, height).ok_or_else(|| "Failed to create pixmap".to_string())?;

    // White background
    pixmap.fill(Color::WHITE);

    let font = load_font();

    let transform = Transform::from_scale(scale, scale);

    for sg in &layout.subgraphs {
        draw_subgraph(&mut pixmap, sg, transform, &font);
    }

    for edge in &layout.edges {
        draw_edge(&mut pixmap, edge, transform);
    }

    for node in &layout.nodes {
        draw_node(&mut pixmap, node, transform, &font);
    }

    pixmap
        .save_png(path)
        .map_err(|e| format!("Failed to save PNG: {e}"))
}

fn color_from_rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba8(r, g, b, 255)
}

fn color_from_rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

fn paint_from_color(c: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(c);
    paint.anti_alias = true;
    paint
}

fn stroke_from(width: f32) -> Stroke {
    let mut stroke = Stroke::default();
    stroke.width = width;
    stroke
}

fn draw_node(pixmap: &mut Pixmap, node: &LayoutNode, transform: Transform, font: &FontRef) {
    let x = node.x - node.width / 2.0;
    let y = node.y - node.height / 2.0;

    let fill_rgb = node.style.fill.unwrap_or([255, 255, 255]);
    let stroke_rgb = node.style.stroke.unwrap_or([80, 80, 80]);
    let text_rgb = node.style.color.unwrap_or([30, 30, 30]);
    let stroke_width = node.style.stroke_width.unwrap_or(1.5);

    let fill_paint = paint_from_color(color_from_rgb(fill_rgb[0], fill_rgb[1], fill_rgb[2]));
    let stroke_paint =
        paint_from_color(color_from_rgb(stroke_rgb[0], stroke_rgb[1], stroke_rgb[2]));

    match node.shape {
        NodeShape::Rect => {
            let rect = Rect::from_xywh(x, y, node.width, node.height);
            if let Some(rect) = rect {
                pixmap.fill_rect(rect, &fill_paint, transform, None);
                let path = PathBuilder::from_rect(rect);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Rounded => {
            let r = 8.0f32;
            if let Some(path) = build_rounded_rect(x, y, node.width, node.height, r) {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Diamond => {
            let cx = node.x;
            let cy = node.y;
            let hw = node.width / 2.0;
            let hh = node.height / 2.0;
            let mut pb = PathBuilder::new();
            pb.move_to(cx, cy - hh);
            pb.line_to(cx + hw, cy);
            pb.line_to(cx, cy + hh);
            pb.line_to(cx - hw, cy);
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Circle => {
            let radius = node.width.min(node.height) / 2.0;
            let mut pb = PathBuilder::new();
            pb.push_circle(node.x, node.y, radius);
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Flag => {
            let notch = 10.0;
            let mut pb = PathBuilder::new();
            pb.move_to(x + notch, y);
            pb.line_to(x + node.width, y);
            pb.line_to(x + node.width, y + node.height);
            pb.line_to(x + notch, y + node.height);
            pb.line_to(x, node.y);
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
    }

    draw_text_centered(
        pixmap,
        &node.label,
        node.x,
        node.y,
        FONT_SIZE,
        text_rgb,
        font,
        transform,
    );
}

fn draw_edge(pixmap: &mut Pixmap, edge: &LayoutEdge, transform: Transform) {
    if edge.points.len() < 2 {
        return;
    }

    let color = color_from_rgb(100, 100, 100);
    let paint = paint_from_color(color);

    let (stroke_width, dashed) = match edge.edge_type {
        EdgeType::Arrow | EdgeType::BidiArrow | EdgeType::Line => (1.5, false),
        EdgeType::DottedArrow | EdgeType::BidiDottedArrow | EdgeType::DottedLine => (1.5, true),
        EdgeType::ThickArrow | EdgeType::BidiThickArrow | EdgeType::ThickLine => (3.0, false),
    };

    let has_arrow = matches!(
        edge.edge_type,
        EdgeType::Arrow | EdgeType::DottedArrow | EdgeType::ThickArrow
            | EdgeType::BidiArrow | EdgeType::BidiDottedArrow | EdgeType::BidiThickArrow
    );

    let has_arrow_both = matches!(
        edge.edge_type,
        EdgeType::BidiArrow | EdgeType::BidiDottedArrow | EdgeType::BidiThickArrow
    );

    let stroke = stroke_from(stroke_width);

    let (sx, sy) = (edge.points[0][0], edge.points[0][1]);
    let last = edge.points.len() - 1;
    let (ex, ey) = (edge.points[last][0], edge.points[last][1]);

    if edge.points.len() > 2 && edge.control_points.is_none() {
        draw_catmull_rom_spline(pixmap, &edge.points, &paint, &stroke, dashed, 6.0, 4.0, transform);

        if has_arrow {
            let n = edge.points.len();
            if edge.reversed {
                let (px, py) = (edge.points[1][0], edge.points[1][1]);
                draw_arrowhead(pixmap, px, py, sx, sy, &paint, stroke_width, transform);
                if has_arrow_both {
                    let (px, py) = (edge.points[n - 2][0], edge.points[n - 2][1]);
                    draw_arrowhead(pixmap, px, py, ex, ey, &paint, stroke_width, transform);
                }
            } else {
                let (px, py) = (edge.points[n - 2][0], edge.points[n - 2][1]);
                draw_arrowhead(pixmap, px, py, ex, ey, &paint, stroke_width, transform);
                if has_arrow_both {
                    let (px, py) = (edge.points[1][0], edge.points[1][1]);
                    draw_arrowhead(pixmap, px, py, sx, sy, &paint, stroke_width, transform);
                }
            }
        }
    } else if let Some(cp) = &edge.control_points {
        if dashed {
            let samples = sample_bezier(sx, sy, cp[0][0], cp[0][1], cp[1][0], cp[1][1], ex, ey, 24);
            for i in 0..samples.len() - 1 {
                draw_dashed_line(
                    pixmap, samples[i].0, samples[i].1,
                    samples[i + 1].0, samples[i + 1].1,
                    &paint, &stroke, 6.0, 4.0, transform,
                );
            }
        } else {
            let mut pb = PathBuilder::new();
            pb.move_to(sx, sy);
            pb.cubic_to(cp[0][0], cp[0][1], cp[1][0], cp[1][1], ex, ey);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, transform, None);
            }
        }

        if has_arrow {
            let (prev_x, prev_y, tip_x, tip_y) = if edge.reversed {
                (cp[0][0], cp[0][1], sx, sy)
            } else {
                (cp[1][0], cp[1][1], ex, ey)
            };
            draw_arrowhead(pixmap, prev_x, prev_y, tip_x, tip_y, &paint, stroke_width, transform);
            if has_arrow_both {
                let (prev_x, prev_y, tip_x, tip_y) = if edge.reversed {
                    (cp[1][0], cp[1][1], ex, ey)
                } else {
                    (cp[0][0], cp[0][1], sx, sy)
                };
                draw_arrowhead(pixmap, prev_x, prev_y, tip_x, tip_y, &paint, stroke_width, transform);
            }
        }
    } else {
        if dashed {
            draw_dashed_line(pixmap, sx, sy, ex, ey, &paint, &stroke, 6.0, 4.0, transform);
        } else {
            let mut pb = PathBuilder::new();
            pb.move_to(sx, sy);
            pb.line_to(ex, ey);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, transform, None);
            }
        }

        if has_arrow {
            let (prev_x, prev_y, tip_x, tip_y) = if edge.reversed {
                (ex, ey, sx, sy)
            } else {
                (sx, sy, ex, ey)
            };
            draw_arrowhead(pixmap, prev_x, prev_y, tip_x, tip_y, &paint, stroke_width, transform);
            if has_arrow_both {
                let (prev_x, prev_y, tip_x, tip_y) = if edge.reversed {
                    (sx, sy, ex, ey)
                } else {
                    (ex, ey, sx, sy)
                };
                draw_arrowhead(pixmap, prev_x, prev_y, tip_x, tip_y, &paint, stroke_width, transform);
            }
        }
    }

    if let (Some(label), Some(pos)) = (&edge.label, &edge.label_pos) {
        let bg_w = label.len() as f32 * 8.0 + 8.0;
        let bg_rect = Rect::from_xywh(pos[0] - bg_w / 2.0, pos[1] - 9.0, bg_w, 18.0);
        let bg_paint = paint_from_color(color_from_rgb(245, 245, 245));
        if let Some(r) = bg_rect {
            pixmap.fill_rect(r, &bg_paint, transform, None);
        }
        // edge label font loaded inline (same font)
        let font = load_font();
        draw_text_centered(
            pixmap,
            label,
            pos[0],
            pos[1],
            SMALL_FONT_SIZE,
            [80, 80, 80],
            &font,
            transform,
        );
    }
}

fn draw_subgraph(
    pixmap: &mut Pixmap,
    sg: &LayoutSubgraph,
    transform: Transform,
    font: &FontRef,
) {
    let rect = Rect::from_xywh(sg.x, sg.y, sg.width, sg.height);
    let fill_paint = paint_from_color(color_from_rgba(255, 248, 220, 80));
    let stroke_paint = paint_from_color(color_from_rgb(200, 185, 140));

    if let Some(r) = rect {
        pixmap.fill_rect(r, &fill_paint, transform, None);
        let path = PathBuilder::from_rect(r);
        pixmap.stroke_path(&path, &stroke_paint, &stroke_from(1.0), transform, None);
    }

    draw_text_at(
        pixmap,
        &sg.title,
        sg.x + 8.0,
        sg.y + 4.0,
        SMALL_FONT_SIZE,
        [130, 115, 80],
        font,
        transform,
    );
}

fn draw_arrowhead(
    pixmap: &mut Pixmap,
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
    paint: &Paint,
    width: f32,
    transform: Transform,
) {
    let dx = to_x - from_x;
    let dy = to_y - from_y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.1 {
        return;
    }
    let dx = dx / len;
    let dy = dy / len;
    let px = -dy;
    let py = dx;

    let arrow_len = 10.0 + width;
    let arrow_w = 5.0 + width * 0.5;

    let bx = to_x - dx * arrow_len;
    let by = to_y - dy * arrow_len;

    let mut pb = PathBuilder::new();
    pb.move_to(to_x, to_y);
    pb.line_to(bx + px * arrow_w, by + py * arrow_w);
    pb.line_to(bx - px * arrow_w, by - py * arrow_w);
    pb.close();
    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, paint, FillRule::Winding, transform, None);
    }
}

fn draw_dashed_line(
    pixmap: &mut Pixmap,
    sx: f32,
    sy: f32,
    ex: f32,
    ey: f32,
    paint: &Paint,
    stroke: &Stroke,
    dash_len: f32,
    gap_len: f32,
    transform: Transform,
) {
    let dx = ex - sx;
    let dy = ey - sy;
    let total = (dx * dx + dy * dy).sqrt();
    if total < 0.1 {
        return;
    }
    let dx = dx / total;
    let dy = dy / total;
    let segment = dash_len + gap_len;
    let mut cursor = 0.0;

    while cursor < total {
        let x0 = sx + dx * cursor;
        let y0 = sy + dy * cursor;
        let end_t = (cursor + dash_len).min(total);
        let x1 = sx + dx * end_t;
        let y1 = sy + dy * end_t;

        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, paint, stroke, transform, None);
        }
        cursor += segment;
    }
}

fn sample_bezier(
    sx: f32, sy: f32,
    c1x: f32, c1y: f32,
    c2x: f32, c2y: f32,
    ex: f32, ey: f32,
    steps: usize,
) -> Vec<(f32, f32)> {
    (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let mt = 1.0 - t;
            (
                mt * mt * mt * sx + 3.0 * mt * mt * t * c1x + 3.0 * mt * t * t * c2x + t * t * t * ex,
                mt * mt * mt * sy + 3.0 * mt * mt * t * c1y + 3.0 * mt * t * t * c2y + t * t * t * ey,
            )
        })
        .collect()
}

fn draw_catmull_rom_spline(
    pixmap: &mut Pixmap,
    points: &[[f32; 2]],
    paint: &Paint,
    stroke: &Stroke,
    dashed: bool,
    dash_len: f32,
    gap_len: f32,
    transform: Transform,
) {
    if points.len() < 2 {
        return;
    }

    let n = points.len();
    for i in 0..n - 1 {
        let p0 = if i > 0 { points[i - 1] } else { points[0] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n { points[i + 2] } else { points[n - 1] };

        let cp1x = p1[0] + (p2[0] - p0[0]) / 6.0;
        let cp1y = p1[1] + (p2[1] - p0[1]) / 6.0;
        let cp2x = p2[0] - (p3[0] - p1[0]) / 6.0;
        let cp2y = p2[1] - (p3[1] - p1[1]) / 6.0;

        if dashed {
            let samples = sample_bezier(p1[0], p1[1], cp1x, cp1y, cp2x, cp2y, p2[0], p2[1], 12);
            for j in 0..samples.len() - 1 {
                draw_dashed_line(
                    pixmap,
                    samples[j].0, samples[j].1,
                    samples[j + 1].0, samples[j + 1].1,
                    paint, stroke, dash_len, gap_len, transform,
                );
            }
        } else {
            let mut pb = PathBuilder::new();
            pb.move_to(p1[0], p1[1]);
            pb.cubic_to(cp1x, cp1y, cp2x, cp2y, p2[0], p2[1]);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, paint, stroke, transform, None);
            }
        }
    }
}

fn build_rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

fn draw_text_centered(
    pixmap: &mut Pixmap,
    text: &str,
    cx: f32,
    cy: f32,
    font_size: f32,
    color_rgb: [u8; 3],
    font: &FontRef,
    transform: Transform,
) {
    let scaled = font.as_scaled(font_size);
    let lines: Vec<&str> = text.split('\n').collect();
    let line_height = scaled.height();
    let total_h = line_height * lines.len() as f32;
    let start_y = cy - total_h / 2.0 + scaled.ascent();

    for (i, line) in lines.iter().enumerate() {
        let line_w: f32 = line
            .chars()
            .map(|c| scaled.h_advance(font.glyph_id(c)))
            .sum();
        let lx = cx - line_w / 2.0;
        let ly = start_y + i as f32 * line_height;
        draw_glyphs(pixmap, line, lx, ly, font_size, color_rgb, font, transform);
    }
}

fn draw_text_at(
    pixmap: &mut Pixmap,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    color_rgb: [u8; 3],
    font: &FontRef,
    transform: Transform,
) {
    let scaled = font.as_scaled(font_size);
    let ly = y + scaled.ascent();
    draw_glyphs(pixmap, text, x, ly, font_size, color_rgb, font, transform);
}

fn draw_glyphs(
    pixmap: &mut Pixmap,
    text: &str,
    mut x: f32,
    y: f32,
    font_size: f32,
    color_rgb: [u8; 3],
    font: &FontRef,
    base_transform: Transform,
) {
    let scale_factor = base_transform.sx;
    let render_font_size = font_size * scale_factor;
    for ch in text.chars() {
        let glyph_id = font.glyph_id(ch);
        let advance_unscaled = font.as_scaled(font_size).h_advance(glyph_id);

        if let Some(outlined) = font.outline_glyph(ab_glyph::Glyph {
            id: glyph_id,
            scale: ab_glyph::PxScale::from(render_font_size),
            position: ab_glyph::point(0.0, 0.0),
        }) {
            let bounds = outlined.px_bounds();
            let gx = (x * scale_factor + bounds.min.x) as i32;
            let gy = (y * scale_factor + bounds.min.y) as i32;
            let gw = (bounds.max.x - bounds.min.x).ceil() as u32;
            let gh = (bounds.max.y - bounds.min.y).ceil() as u32;

            if gw > 0 && gh > 0 {
                if let Some(mut glyph_pixmap) = Pixmap::new(gw, gh) {
                    outlined.draw(|px, py, coverage| {
                        if px < gw && py < gh {
                            let alpha = (coverage * 255.0) as u8;
                            let c = ColorU8::from_rgba(
                                color_rgb[0],
                                color_rgb[1],
                                color_rgb[2],
                                alpha,
                            )
                            .premultiply();
                            glyph_pixmap.pixels_mut()[(py * gw + px) as usize] = c;
                        }
                    });

                    pixmap.draw_pixmap(
                        gx,
                        gy,
                        glyph_pixmap.as_ref(),
                        &PixmapPaint::default(),
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
        x += advance_unscaled;
    }
}
