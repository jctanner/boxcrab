use crate::layout::{LayoutEdge, LayoutNode, LayoutResult, LayoutSubgraph};
use crate::diagram::{ArrowheadType, ClassField, ClassMethod, EdgeType, FillPattern, NodeShape, SqlColumn};
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

    if node.style.shadow == Some(true) {
        let so = 6.0;
        let shadow_paint = paint_from_color(color_from_rgba(0, 0, 0, 30));
        let r = 8.0f32;
        if let Some(path) = build_rounded_rect(x + so, y + so, node.width, node.height, r) {
            pixmap.fill_path(&path, &shadow_paint, FillRule::Winding, transform, None);
        }
    }

    if node.style.multiple == Some(true) {
        let mo = 8.0;
        let r = 8.0f32;
        if let Some(path) = build_rounded_rect(x + mo, y + mo, node.width, node.height, r) {
            pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
            pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
        }
    }

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
        NodeShape::Oval => {
            let mut pb = PathBuilder::new();
            pb.push_oval(Rect::from_xywh(x, y, node.width, node.height).unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()));
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Hexagon => {
            let cx = node.x;
            let cy = node.y;
            let hw = node.width / 2.0;
            let hh = node.height / 2.0;
            let inset = hw * 0.25;
            let mut pb = PathBuilder::new();
            pb.move_to(cx - hw + inset, cy - hh);
            pb.line_to(cx + hw - inset, cy - hh);
            pb.line_to(cx + hw, cy);
            pb.line_to(cx + hw - inset, cy + hh);
            pb.line_to(cx - hw + inset, cy + hh);
            pb.line_to(cx - hw, cy);
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Parallelogram => {
            let skew = node.width * 0.15;
            let mut pb = PathBuilder::new();
            pb.move_to(x + skew, y);
            pb.line_to(x + node.width, y);
            pb.line_to(x + node.width - skew, y + node.height);
            pb.line_to(x, y + node.height);
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Cylinder => {
            let ry = (node.height * 0.1).min(12.0);
            let body_top = y + ry;
            let body_bottom = y + node.height - ry;
            // Body
            if let Some(r) = Rect::from_xywh(x, body_top, node.width, body_bottom - body_top) {
                pixmap.fill_rect(r, &fill_paint, transform, None);
            }
            // Top ellipse
            if let Some(r) = Rect::from_xywh(x, body_top - ry, node.width, ry * 2.0) {
                let mut pb = PathBuilder::new();
                pb.push_oval(r);
                if let Some(path) = pb.finish() {
                    pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                    pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
                }
            }
            // Side lines
            let mut pb = PathBuilder::new();
            pb.move_to(x, body_top);
            pb.line_to(x, body_bottom);
            pb.move_to(x + node.width, body_top);
            pb.line_to(x + node.width, body_bottom);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
            // Bottom half-ellipse
            if let Some(r) = Rect::from_xywh(x, body_bottom - ry, node.width, ry * 2.0) {
                let mut pb = PathBuilder::new();
                pb.push_oval(r);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
                }
            }
        }
        NodeShape::Cloud => {
            // Approximate cloud as bumpy ellipse
            let cx = node.x;
            let cy = node.y;
            let rx = node.width / 2.0;
            let ry = node.height / 2.0;
            let n = 64;
            let mut pb = PathBuilder::new();
            for i in 0..n {
                let angle = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
                let bump = 1.0 + 0.15 * (angle * 6.0).sin().abs();
                let px = cx + rx * bump * angle.cos();
                let py = cy + ry * bump * angle.sin();
                if i == 0 { pb.move_to(px, py); } else { pb.line_to(px, py); }
            }
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Page => {
            let fold = 12.0f32.min(node.width * 0.15).min(node.height * 0.15);
            let mut pb = PathBuilder::new();
            pb.move_to(x, y);
            pb.line_to(x + node.width - fold, y);
            pb.line_to(x + node.width, y + fold);
            pb.line_to(x + node.width, y + node.height);
            pb.line_to(x, y + node.height);
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
            // Fold line
            let mut pb2 = PathBuilder::new();
            pb2.move_to(x + node.width - fold, y);
            pb2.line_to(x + node.width - fold, y + fold);
            pb2.line_to(x + node.width, y + fold);
            if let Some(path) = pb2.finish() {
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Document => {
            let wave_h = (node.height * 0.08).min(8.0);
            let mut pb = PathBuilder::new();
            pb.move_to(x, y);
            pb.line_to(x + node.width, y);
            pb.line_to(x + node.width, y + node.height - wave_h);
            let steps = 24;
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let px = x + node.width - t * node.width;
                let py = y + node.height - wave_h + (t * std::f32::consts::PI * 2.0).sin() * wave_h;
                pb.line_to(px, py);
            }
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Person => {
            let head_r = (node.width * 0.18).min(node.height * 0.2);
            let head_cy = y + head_r + 2.0;
            let cx = node.x;
            // Head
            let mut pb = PathBuilder::new();
            pb.push_circle(cx, head_cy, head_r);
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
            // Body trapezoid
            let body_top = head_cy + head_r + 2.0;
            let body_bottom = y + node.height;
            let top_hw = node.width * 0.2;
            let bot_hw = node.width * 0.45;
            let mut pb2 = PathBuilder::new();
            pb2.move_to(cx - top_hw, body_top);
            pb2.line_to(cx + top_hw, body_top);
            pb2.line_to(cx + bot_hw, body_bottom);
            pb2.line_to(cx - bot_hw, body_bottom);
            pb2.close();
            if let Some(path) = pb2.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Queue => {
            let rx = (node.width * 0.1).min(12.0);
            let body_left = x + rx;
            let body_right = x + node.width - rx;
            // Body
            if let Some(r) = Rect::from_xywh(body_left, y, body_right - body_left, node.height) {
                pixmap.fill_rect(r, &fill_paint, transform, None);
            }
            // Left ellipse
            if let Some(r) = Rect::from_xywh(body_left - rx, y, rx * 2.0, node.height) {
                let mut pb = PathBuilder::new();
                pb.push_oval(r);
                if let Some(path) = pb.finish() {
                    pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                    pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
                }
            }
            // Right ellipse
            if let Some(r) = Rect::from_xywh(body_right - rx, y, rx * 2.0, node.height) {
                let mut pb = PathBuilder::new();
                pb.push_oval(r);
                if let Some(path) = pb.finish() {
                    pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                    pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
                }
            }
            // Top and bottom lines
            let mut pb = PathBuilder::new();
            pb.move_to(body_left, y);
            pb.line_to(body_right, y);
            pb.move_to(body_left, y + node.height);
            pb.line_to(body_right, y + node.height);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Package => {
            let tab_w = node.width * 0.35;
            let tab_h = 12.0f32.min(node.height * 0.15);
            // Tab
            if let Some(r) = Rect::from_xywh(x, y, tab_w, tab_h) {
                pixmap.fill_rect(r, &fill_paint, transform, None);
                let path = PathBuilder::from_rect(r);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
            // Body
            if let Some(r) = Rect::from_xywh(x, y + tab_h, node.width, node.height - tab_h) {
                pixmap.fill_rect(r, &fill_paint, transform, None);
                let path = PathBuilder::from_rect(r);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Step => {
            let arrow = node.width * 0.15;
            let mut pb = PathBuilder::new();
            pb.move_to(x, y);
            pb.line_to(x + node.width - arrow, y);
            pb.line_to(x + node.width, node.y);
            pb.line_to(x + node.width - arrow, y + node.height);
            pb.line_to(x, y + node.height);
            pb.line_to(x + arrow, node.y);
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Callout => {
            let tail_w = 10.0;
            let tail_h = 12.0f32.min(node.height * 0.2);
            let tail_x = x + node.width * 0.25;
            let mut pb = PathBuilder::new();
            pb.move_to(x, y);
            pb.line_to(x + node.width, y);
            pb.line_to(x + node.width, y + node.height - tail_h);
            pb.line_to(tail_x + tail_w, y + node.height - tail_h);
            pb.line_to(tail_x, y + node.height);
            pb.line_to(tail_x - tail_w * 0.3, y + node.height - tail_h);
            pb.line_to(x, y + node.height - tail_h);
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::StoredData => {
            let curve = node.width * 0.1;
            let mut pb = PathBuilder::new();
            pb.move_to(x + node.width, y);
            pb.line_to(x + node.width, y + node.height);
            let steps = 16;
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let py = y + node.height - t * node.height;
                let px = x + curve * (1.0 - (t * std::f32::consts::PI).sin());
                pb.line_to(px, py);
            }
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Text => {
            // No border/fill for text shape
        }
        NodeShape::Class => {
            draw_class_shape_export(
                pixmap, x, y, node.width, node.height,
                &fill_paint, &stroke_paint, stroke_width,
                &node.label, &node.class_fields, &node.class_methods,
                text_rgb, font, transform, stroke_rgb,
            );
        }
        NodeShape::SqlTable => {
            draw_sql_table_shape_export(
                pixmap, x, y, node.width, node.height,
                &fill_paint, &stroke_paint, stroke_width,
                &node.label, &node.sql_columns,
                text_rgb, font, transform, stroke_rgb,
            );
        }
        NodeShape::Stadium => {
            let r = node.height / 2.0;
            let body_left = x + r;
            let body_right = x + node.width - r;
            let cy = node.y;
            let n = 16;
            let mut pb = PathBuilder::new();
            for i in 0..=n {
                let angle = std::f32::consts::PI / 2.0 + std::f32::consts::PI * i as f32 / n as f32;
                let px = body_left + r * angle.cos();
                let py = cy + r * angle.sin();
                if i == 0 { pb.move_to(px, py); } else { pb.line_to(px, py); }
            }
            for i in 0..=n {
                let angle = -std::f32::consts::PI / 2.0 + std::f32::consts::PI * i as f32 / n as f32;
                let px = body_right + r * angle.cos();
                let py = cy + r * angle.sin();
                pb.line_to(px, py);
            }
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Subroutine => {
            if let Some(rect) = Rect::from_xywh(x, y, node.width, node.height) {
                pixmap.fill_rect(rect, &fill_paint, transform, None);
                let path = PathBuilder::from_rect(rect);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
            let inset = 8.0f32.min(node.width * 0.08);
            let mut pb = PathBuilder::new();
            pb.move_to(x + inset, y);
            pb.line_to(x + inset, y + node.height);
            pb.move_to(x + node.width - inset, y);
            pb.line_to(x + node.width - inset, y + node.height);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::DoubleCircle => {
            let outer_r = node.width.min(node.height) / 2.0;
            let inner_r = (outer_r - 6.0).max(2.0);
            let mut pb = PathBuilder::new();
            pb.push_circle(node.x, node.y, outer_r);
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
            let mut pb2 = PathBuilder::new();
            pb2.push_circle(node.x, node.y, inner_r);
            if let Some(path) = pb2.finish() {
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::Trapezoid => {
            let inset = node.width * 0.15;
            let mut pb = PathBuilder::new();
            pb.move_to(x, y);
            pb.line_to(x + node.width, y);
            pb.line_to(x + node.width - inset, y + node.height);
            pb.line_to(x + inset, y + node.height);
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
        NodeShape::TrapezoidAlt => {
            let inset = node.width * 0.15;
            let mut pb = PathBuilder::new();
            pb.move_to(x + inset, y);
            pb.line_to(x + node.width - inset, y);
            pb.line_to(x + node.width, y + node.height);
            pb.line_to(x, y + node.height);
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &fill_paint, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
            }
        }
    }

    if node.style.three_d == Some(true) {
        let depth = 10.0;
        let dark_rgb = [(fill_rgb[0] as f32 * 0.7) as u8, (fill_rgb[1] as f32 * 0.7) as u8, (fill_rgb[2] as f32 * 0.7) as u8];
        let darker_rgb = [(fill_rgb[0] as f32 * 0.85) as u8, (fill_rgb[1] as f32 * 0.85) as u8, (fill_rgb[2] as f32 * 0.85) as u8];

        // Right side
        let mut pb = PathBuilder::new();
        pb.move_to(x + node.width, y);
        pb.line_to(x + node.width + depth, y - depth);
        pb.line_to(x + node.width + depth, y + node.height - depth);
        pb.line_to(x + node.width, y + node.height);
        pb.close();
        if let Some(path) = pb.finish() {
            let dark_paint = paint_from_color(color_from_rgb(dark_rgb[0], dark_rgb[1], dark_rgb[2]));
            pixmap.fill_path(&path, &dark_paint, FillRule::Winding, transform, None);
            pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
        }

        // Top side
        let mut pb = PathBuilder::new();
        pb.move_to(x, y);
        pb.line_to(x + depth, y - depth);
        pb.line_to(x + node.width + depth, y - depth);
        pb.line_to(x + node.width, y);
        pb.close();
        if let Some(path) = pb.finish() {
            let darker_paint = paint_from_color(color_from_rgb(darker_rgb[0], darker_rgb[1], darker_rgb[2]));
            pixmap.fill_path(&path, &darker_paint, FillRule::Winding, transform, None);
            pixmap.stroke_path(&path, &stroke_paint, &stroke_from(stroke_width), transform, None);
        }
    }

    if let Some(pattern) = node.style.fill_pattern {
        draw_fill_pattern_export(pixmap, x, y, node.width, node.height, pattern, stroke_rgb, transform);
    }

    let draws_own_text = matches!(node.shape, NodeShape::Class | NodeShape::SqlTable);
    if !draws_own_text {
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
}

fn draw_class_shape_export(
    pixmap: &mut Pixmap,
    x: f32, y: f32, w: f32, h: f32,
    fill_paint: &Paint, stroke_paint: &Paint, stroke_width: f32,
    label: &str,
    fields: &[ClassField],
    methods: &[ClassMethod],
    text_rgb: [u8; 3],
    font: &FontRef,
    transform: Transform,
    stroke_rgb: [u8; 3],
) {
    // Outer rect
    if let Some(rect) = Rect::from_xywh(x, y, w, h) {
        pixmap.fill_rect(rect, fill_paint, transform, None);
        let path = PathBuilder::from_rect(rect);
        pixmap.stroke_path(&path, stroke_paint, &stroke_from(stroke_width), transform, None);
    }

    let row_h = 18.0;
    let header_h = 28.0;

    let header_paint = paint_from_color(color_from_rgba(stroke_rgb[0], stroke_rgb[1], stroke_rgb[2], 40));
    if let Some(rect) = Rect::from_xywh(x, y, w, header_h) {
        pixmap.fill_rect(rect, &header_paint, transform, None);
    }

    draw_text_centered(pixmap, label, x + w / 2.0, y + header_h / 2.0, FONT_SIZE, text_rgb, font, transform);

    let mut cy = y + header_h;

    let mut pb = PathBuilder::new();
    pb.move_to(x, cy);
    pb.line_to(x + w, cy);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, stroke_paint, &stroke_from(stroke_width), transform, None);
    }

    for field in fields {
        let vis = match field.visibility { '+' => "+", '-' => "-", '#' => "#", _ => " " };
        let text = if field.type_str.is_empty() {
            format!("{}{}", vis, field.name)
        } else {
            format!("{}{}: {}", vis, field.name, field.type_str)
        };
        draw_text_at(pixmap, &text, x + 8.0, cy + 2.0, SMALL_FONT_SIZE, text_rgb, font, transform);
        cy += row_h;
    }

    if !methods.is_empty() && !fields.is_empty() {
        let mut pb = PathBuilder::new();
        pb.move_to(x, cy);
        pb.line_to(x + w, cy);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, stroke_paint, &stroke_from(stroke_width), transform, None);
        }
    }

    for method in methods {
        let vis = match method.visibility { '+' => "+", '-' => "-", '#' => "#", _ => " " };
        let text = if method.return_type.is_empty() {
            format!("{}{}", vis, method.name)
        } else {
            format!("{}{}: {}", vis, method.name, method.return_type)
        };
        draw_text_at(pixmap, &text, x + 8.0, cy + 2.0, SMALL_FONT_SIZE, text_rgb, font, transform);
        cy += row_h;
    }
}

fn draw_sql_table_shape_export(
    pixmap: &mut Pixmap,
    x: f32, y: f32, w: f32, h: f32,
    fill_paint: &Paint, stroke_paint: &Paint, stroke_width: f32,
    label: &str,
    columns: &[SqlColumn],
    text_rgb: [u8; 3],
    font: &FontRef,
    transform: Transform,
    stroke_rgb: [u8; 3],
) {
    if let Some(rect) = Rect::from_xywh(x, y, w, h) {
        pixmap.fill_rect(rect, fill_paint, transform, None);
        let path = PathBuilder::from_rect(rect);
        pixmap.stroke_path(&path, stroke_paint, &stroke_from(stroke_width), transform, None);
    }

    let row_h = 18.0;
    let header_h = 28.0;

    let header_paint = paint_from_color(color_from_rgba(stroke_rgb[0], stroke_rgb[1], stroke_rgb[2], 40));
    if let Some(rect) = Rect::from_xywh(x, y, w, header_h) {
        pixmap.fill_rect(rect, &header_paint, transform, None);
    }

    draw_text_centered(pixmap, label, x + w / 2.0, y + header_h / 2.0, FONT_SIZE, text_rgb, font, transform);

    let mut cy = y + header_h;

    let mut pb = PathBuilder::new();
    pb.move_to(x, cy);
    pb.line_to(x + w, cy);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, stroke_paint, &stroke_from(stroke_width), transform, None);
    }

    for col in columns {
        let constraint_str = if col.constraint.is_empty() {
            String::new()
        } else {
            format!(" {}", col.constraint)
        };
        let text = format!("{} {}{}", col.name, col.type_str, constraint_str);
        draw_text_at(pixmap, &text, x + 8.0, cy + 2.0, SMALL_FONT_SIZE, text_rgb, font, transform);
        cy += row_h;
    }
}

fn draw_fill_pattern_export(
    pixmap: &mut Pixmap,
    x: f32, y: f32, w: f32, h: f32,
    pattern: FillPattern,
    stroke_rgb: [u8; 3],
    transform: Transform,
) {
    let paint = paint_from_color(color_from_rgba(stroke_rgb[0], stroke_rgb[1], stroke_rgb[2], 40));
    let thin = stroke_from(0.8);

    match pattern {
        FillPattern::Dots => {
            let spacing = 8.0;
            let r = 1.2;
            let mut py = y + spacing / 2.0;
            while py < y + h {
                let mut px = x + spacing / 2.0;
                while px < x + w {
                    let mut pb = PathBuilder::new();
                    pb.push_circle(px, py, r);
                    if let Some(path) = pb.finish() {
                        pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
                    }
                    px += spacing;
                }
                py += spacing;
            }
        }
        FillPattern::Lines => {
            let spacing = 6.0;
            let mut ly = y;
            while ly < y + h {
                let mut pb = PathBuilder::new();
                pb.move_to(x, ly);
                pb.line_to(x + w, ly);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &paint, &thin, transform, None);
                }
                ly += spacing;
            }
        }
        FillPattern::Grain => {
            let spacing = 5.0;
            let mut offset = 0.0_f32;
            while offset < w + h {
                let x0 = (x + offset).min(x + w);
                let y0 = y + (x + offset - x0);
                let y1 = (y + offset).min(y + h);
                let x1 = x + (y + offset - y1).max(0.0);
                if y0 <= y + h && x1 <= x + w {
                    let mut pb = PathBuilder::new();
                    pb.move_to(x0, y0);
                    pb.line_to(x1, y1);
                    if let Some(path) = pb.finish() {
                        pixmap.stroke_path(&path, &paint, &thin, transform, None);
                    }
                }
                offset += spacing;
            }
        }
        FillPattern::Paper => {
            let spacing_h = 10.0;
            let spacing_v = 14.0;
            let mut ly = y + spacing_h;
            while ly < y + h {
                let mut pb = PathBuilder::new();
                pb.move_to(x, ly);
                pb.line_to(x + w, ly);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &paint, &thin, transform, None);
                }
                ly += spacing_h;
            }
            let mut lx = x + spacing_v;
            while lx < x + w {
                let mut pb = PathBuilder::new();
                pb.move_to(lx, y);
                pb.line_to(lx, y + h);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &paint, &thin, transform, None);
                }
                lx += spacing_v;
            }
        }
    }
}

fn draw_edge(pixmap: &mut Pixmap, edge: &LayoutEdge, transform: Transform) {
    if edge.points.len() < 2 {
        return;
    }

    let stroke_rgb = edge.style.stroke.unwrap_or([100, 100, 100]);
    let color = color_from_rgb(stroke_rgb[0], stroke_rgb[1], stroke_rgb[2]);
    let paint = paint_from_color(color);

    let (base_width, type_dashed) = match edge.edge_type {
        EdgeType::Arrow | EdgeType::BidiArrow | EdgeType::Line => (1.5, false),
        EdgeType::DottedArrow | EdgeType::BidiDottedArrow | EdgeType::DottedLine => (1.5, true),
        EdgeType::ThickArrow | EdgeType::BidiThickArrow | EdgeType::ThickLine => (3.0, false),
    };

    let stroke_width = edge.style.stroke_width.unwrap_or(base_width);
    let dashed = type_dashed || edge.style.stroke_dash.is_some() || edge.style.animated == Some(true);

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

    let (dst_ah, src_ah) = if edge.reversed {
        (edge.src_arrowhead, edge.dst_arrowhead)
    } else {
        (edge.dst_arrowhead, edge.src_arrowhead)
    };

    if edge.points.len() > 2 && edge.control_points.is_none() {
        draw_catmull_rom_spline(pixmap, &edge.points, &paint, &stroke, dashed, 6.0, 4.0, transform);

        if has_arrow {
            let n = edge.points.len();
            if edge.reversed {
                let (px, py) = (edge.points[1][0], edge.points[1][1]);
                draw_edge_arrowhead_export(pixmap, px, py, sx, sy, &paint, stroke_width, transform, dst_ah);
                if has_arrow_both {
                    let (px, py) = (edge.points[n - 2][0], edge.points[n - 2][1]);
                    draw_edge_arrowhead_export(pixmap, px, py, ex, ey, &paint, stroke_width, transform, src_ah);
                }
            } else {
                let (px, py) = (edge.points[n - 2][0], edge.points[n - 2][1]);
                draw_edge_arrowhead_export(pixmap, px, py, ex, ey, &paint, stroke_width, transform, dst_ah);
                if has_arrow_both {
                    let (px, py) = (edge.points[1][0], edge.points[1][1]);
                    draw_edge_arrowhead_export(pixmap, px, py, sx, sy, &paint, stroke_width, transform, src_ah);
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
            draw_edge_arrowhead_export(pixmap, prev_x, prev_y, tip_x, tip_y, &paint, stroke_width, transform, dst_ah);
            if has_arrow_both {
                let (prev_x, prev_y, tip_x, tip_y) = if edge.reversed {
                    (cp[1][0], cp[1][1], ex, ey)
                } else {
                    (cp[0][0], cp[0][1], sx, sy)
                };
                draw_edge_arrowhead_export(pixmap, prev_x, prev_y, tip_x, tip_y, &paint, stroke_width, transform, src_ah);
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
            draw_edge_arrowhead_export(pixmap, prev_x, prev_y, tip_x, tip_y, &paint, stroke_width, transform, dst_ah);
            if has_arrow_both {
                let (prev_x, prev_y, tip_x, tip_y) = if edge.reversed {
                    (sx, sy, ex, ey)
                } else {
                    (ex, ey, sx, sy)
                };
                draw_edge_arrowhead_export(pixmap, prev_x, prev_y, tip_x, tip_y, &paint, stroke_width, transform, src_ah);
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

fn arrowhead_basis_export(from_x: f32, from_y: f32, to_x: f32, to_y: f32, width: f32) -> Option<(f32, f32, f32, f32, f32, f32)> {
    let dx = to_x - from_x;
    let dy = to_y - from_y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.1 { return None; }
    let dx = dx / len;
    let dy = dy / len;
    let px = -dy;
    let py = dx;
    let arrow_len = 10.0 + width;
    let arrow_w = 5.0 + width * 0.5;
    Some((dx, dy, px, py, arrow_len, arrow_w))
}

fn draw_arrowhead_typed_export(
    pixmap: &mut Pixmap,
    from_x: f32, from_y: f32,
    to_x: f32, to_y: f32,
    paint: &Paint,
    width: f32,
    transform: Transform,
    ah_type: ArrowheadType,
) {
    match ah_type {
        ArrowheadType::None => {}
        ArrowheadType::Triangle => draw_arrowhead(pixmap, from_x, from_y, to_x, to_y, paint, width, transform),
        ArrowheadType::Arrow => {
            let Some((dx, dy, px, py, arrow_len, arrow_w)) = arrowhead_basis_export(from_x, from_y, to_x, to_y, width) else { return };
            let bx = to_x - dx * arrow_len;
            let by = to_y - dy * arrow_len;
            let notch_x = to_x - dx * arrow_len * 0.4;
            let notch_y = to_y - dy * arrow_len * 0.4;
            let mut pb = PathBuilder::new();
            pb.move_to(to_x, to_y);
            pb.line_to(bx + px * arrow_w, by + py * arrow_w);
            pb.line_to(notch_x, notch_y);
            pb.line_to(bx - px * arrow_w, by - py * arrow_w);
            pb.close();
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, paint, FillRule::Winding, transform, None);
            }
        }
        ArrowheadType::UnfilledTriangle => {
            let Some((dx, dy, px, py, arrow_len, arrow_w)) = arrowhead_basis_export(from_x, from_y, to_x, to_y, width) else { return };
            let bx = to_x - dx * arrow_len;
            let by = to_y - dy * arrow_len;
            let mut pb = PathBuilder::new();
            pb.move_to(to_x, to_y);
            pb.line_to(bx + px * arrow_w, by + py * arrow_w);
            pb.line_to(bx - px * arrow_w, by - py * arrow_w);
            pb.close();
            if let Some(path) = pb.finish() {
                let white = paint_from_color(Color::WHITE);
                pixmap.fill_path(&path, &white, FillRule::Winding, transform, None);
                pixmap.stroke_path(&path, paint, &stroke_from(1.5), transform, None);
            }
        }
        ArrowheadType::Diamond | ArrowheadType::FilledDiamond => {
            let filled = matches!(ah_type, ArrowheadType::FilledDiamond);
            let Some((dx, dy, px, py, arrow_len, arrow_w)) = arrowhead_basis_export(from_x, from_y, to_x, to_y, width) else { return };
            let mid_x = to_x - dx * arrow_len * 0.5;
            let mid_y = to_y - dy * arrow_len * 0.5;
            let back_x = to_x - dx * arrow_len;
            let back_y = to_y - dy * arrow_len;
            let mut pb = PathBuilder::new();
            pb.move_to(to_x, to_y);
            pb.line_to(mid_x + px * arrow_w, mid_y + py * arrow_w);
            pb.line_to(back_x, back_y);
            pb.line_to(mid_x - px * arrow_w, mid_y - py * arrow_w);
            pb.close();
            if let Some(path) = pb.finish() {
                if filled {
                    pixmap.fill_path(&path, paint, FillRule::Winding, transform, None);
                } else {
                    let white = paint_from_color(Color::WHITE);
                    pixmap.fill_path(&path, &white, FillRule::Winding, transform, None);
                    pixmap.stroke_path(&path, paint, &stroke_from(1.5), transform, None);
                }
            }
        }
        ArrowheadType::Circle | ArrowheadType::FilledCircle => {
            let filled = matches!(ah_type, ArrowheadType::FilledCircle);
            let r = 4.0 + width * 0.5;
            let mut pb = PathBuilder::new();
            pb.push_circle(to_x, to_y, r);
            if let Some(path) = pb.finish() {
                if filled {
                    pixmap.fill_path(&path, paint, FillRule::Winding, transform, None);
                } else {
                    let white = paint_from_color(Color::WHITE);
                    pixmap.fill_path(&path, &white, FillRule::Winding, transform, None);
                    pixmap.stroke_path(&path, paint, &stroke_from(1.5), transform, None);
                }
            }
        }
        ArrowheadType::Cross => {
            let Some((_dx, _dy, px, py, _, arrow_w)) = arrowhead_basis_export(from_x, from_y, to_x, to_y, width) else { return };
            let s = arrow_w;
            let mut pb = PathBuilder::new();
            pb.move_to(to_x - px * s - _dx * s, to_y - py * s - _dy * s);
            pb.line_to(to_x + px * s + _dx * s, to_y + py * s + _dy * s);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, paint, &stroke_from(1.5), transform, None);
            }
            let mut pb2 = PathBuilder::new();
            pb2.move_to(to_x + px * s - _dx * s, to_y + py * s - _dy * s);
            pb2.line_to(to_x - px * s + _dx * s, to_y - py * s + _dy * s);
            if let Some(path) = pb2.finish() {
                pixmap.stroke_path(&path, paint, &stroke_from(1.5), transform, None);
            }
        }
        ArrowheadType::Box | ArrowheadType::FilledBox => {
            let filled = matches!(ah_type, ArrowheadType::FilledBox);
            let Some((dx, dy, px, py, _, arrow_w)) = arrowhead_basis_export(from_x, from_y, to_x, to_y, width) else { return };
            let s = arrow_w;
            let cx = to_x - dx * s;
            let cy = to_y - dy * s;
            let mut pb = PathBuilder::new();
            pb.move_to(cx - px * s - dx * s, cy - py * s - dy * s);
            pb.line_to(cx + px * s - dx * s, cy + py * s - dy * s);
            pb.line_to(cx + px * s + dx * s, cy + py * s + dy * s);
            pb.line_to(cx - px * s + dx * s, cy - py * s + dy * s);
            pb.close();
            if let Some(path) = pb.finish() {
                if filled {
                    pixmap.fill_path(&path, paint, FillRule::Winding, transform, None);
                } else {
                    let white = paint_from_color(Color::WHITE);
                    pixmap.fill_path(&path, &white, FillRule::Winding, transform, None);
                    pixmap.stroke_path(&path, paint, &stroke_from(1.5), transform, None);
                }
            }
        }
        ArrowheadType::Line => {
            let Some((_dx, _dy, px, py, _, arrow_w)) = arrowhead_basis_export(from_x, from_y, to_x, to_y, width) else { return };
            let mut pb = PathBuilder::new();
            pb.move_to(to_x + px * arrow_w, to_y + py * arrow_w);
            pb.line_to(to_x - px * arrow_w, to_y - py * arrow_w);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, paint, &stroke_from(2.0), transform, None);
            }
        }
        ArrowheadType::CfOne => {
            draw_arrowhead_typed_export(pixmap, from_x, from_y, to_x, to_y, paint, width, transform, ArrowheadType::Line);
        }
        ArrowheadType::CfMany => {
            let Some((dx, dy, px, py, arrow_len, arrow_w)) = arrowhead_basis_export(from_x, from_y, to_x, to_y, width) else { return };
            let bx = to_x - dx * arrow_len * 0.6;
            let by = to_y - dy * arrow_len * 0.6;
            for &(lx, ly) in &[(to_x + px * arrow_w, to_y + py * arrow_w), (to_x - px * arrow_w, to_y - py * arrow_w), (to_x, to_y)] {
                let mut pb = PathBuilder::new();
                pb.move_to(bx, by);
                pb.line_to(lx, ly);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, paint, &stroke_from(1.5), transform, None);
                }
            }
        }
        ArrowheadType::CfOneRequired => {
            let Some((dx, dy, arrow_len, ..)) = arrowhead_basis_export(from_x, from_y, to_x, to_y, width) else { return };
            draw_arrowhead_typed_export(pixmap, from_x, from_y, to_x, to_y, paint, width, transform, ArrowheadType::Line);
            let off_x = to_x - dx * arrow_len * 0.4;
            let off_y = to_y - dy * arrow_len * 0.4;
            draw_arrowhead_typed_export(pixmap, from_x, from_y, off_x, off_y, paint, width, transform, ArrowheadType::Line);
        }
        ArrowheadType::CfManyRequired => {
            let Some((dx, dy, _, _, arrow_len, _)) = arrowhead_basis_export(from_x, from_y, to_x, to_y, width) else { return };
            draw_arrowhead_typed_export(pixmap, from_x, from_y, to_x, to_y, paint, width, transform, ArrowheadType::CfMany);
            let off_x = to_x - dx * arrow_len * 0.5;
            let off_y = to_y - dy * arrow_len * 0.5;
            draw_arrowhead_typed_export(pixmap, from_x, from_y, off_x, off_y, paint, width, transform, ArrowheadType::Line);
        }
    }
}

fn draw_edge_arrowhead_export(
    pixmap: &mut Pixmap,
    from_x: f32, from_y: f32,
    to_x: f32, to_y: f32,
    paint: &Paint,
    width: f32,
    transform: Transform,
    custom: Option<ArrowheadType>,
) {
    match custom {
        Some(ah) => draw_arrowhead_typed_export(pixmap, from_x, from_y, to_x, to_y, paint, width, transform, ah),
        None => draw_arrowhead(pixmap, from_x, from_y, to_x, to_y, paint, width, transform),
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
