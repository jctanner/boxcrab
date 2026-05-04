use crate::diagram::{ArrowheadType, ClassField, ClassMethod, FillPattern, SqlColumn};
use egui::{Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, epaint::PathShape};

pub fn draw_rect(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    painter.rect_filled(rect, 0.0, fill);
    painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Outside);
}

pub fn draw_rounded_rect(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    painter.rect_filled(rect, 8.0, fill);
    painter.rect_stroke(rect, 8.0, stroke, StrokeKind::Outside);
}

pub fn draw_diamond(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let c = rect.center();
    let hw = rect.width() / 2.0;
    let hh = rect.height() / 2.0;

    let points = vec![
        Pos2::new(c.x, c.y - hh),
        Pos2::new(c.x + hw, c.y),
        Pos2::new(c.x, c.y + hh),
        Pos2::new(c.x - hw, c.y),
    ];

    let shape = PathShape::convex_polygon(points, fill, stroke);
    painter.add(shape);
}

pub fn draw_circle(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0;
    painter.circle_filled(center, radius, fill);
    painter.circle_stroke(center, radius, stroke);
}

pub fn draw_flag(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let notch = 10.0;
    let points = vec![
        Pos2::new(rect.left() + notch, rect.top()),
        Pos2::new(rect.right(), rect.top()),
        Pos2::new(rect.right(), rect.bottom()),
        Pos2::new(rect.left() + notch, rect.bottom()),
        Pos2::new(rect.left(), rect.center().y),
    ];

    let shape = PathShape::convex_polygon(points, fill, stroke);
    painter.add(shape);
}

pub fn draw_arrowhead_typed(
    painter: &Painter,
    from: Pos2,
    to: Pos2,
    color: Color32,
    width: f32,
    ah_type: ArrowheadType,
) {
    match ah_type {
        ArrowheadType::None => {}
        ArrowheadType::Triangle => draw_arrowhead(painter, from, to, color, width),
        ArrowheadType::Arrow => draw_arrowhead_chevron(painter, from, to, color, width),
        ArrowheadType::UnfilledTriangle => draw_arrowhead_unfilled_triangle(painter, from, to, color, width),
        ArrowheadType::Diamond => draw_arrowhead_diamond(painter, from, to, color, width, false),
        ArrowheadType::FilledDiamond => draw_arrowhead_diamond(painter, from, to, color, width, true),
        ArrowheadType::Circle => draw_arrowhead_circle(painter, from, to, color, width, false),
        ArrowheadType::FilledCircle => draw_arrowhead_circle(painter, from, to, color, width, true),
        ArrowheadType::Cross => draw_arrowhead_cross(painter, from, to, color, width),
        ArrowheadType::Box => draw_arrowhead_box(painter, from, to, color, width, false),
        ArrowheadType::FilledBox => draw_arrowhead_box(painter, from, to, color, width, true),
        ArrowheadType::Line => draw_arrowhead_line(painter, from, to, color, width),
        ArrowheadType::CfOne => draw_arrowhead_cf_one(painter, from, to, color, width),
        ArrowheadType::CfMany => draw_arrowhead_cf_many(painter, from, to, color, width),
        ArrowheadType::CfOneRequired => draw_arrowhead_cf_one_required(painter, from, to, color, width),
        ArrowheadType::CfManyRequired => draw_arrowhead_cf_many_required(painter, from, to, color, width),
    }
}

fn arrowhead_basis(from: Pos2, to: Pos2, width: f32) -> Option<(Vec2, Vec2, f32, f32)> {
    let dir = Vec2::new(to.x - from.x, to.y - from.y);
    let len = dir.length();
    if len < 0.1 { return None; }
    let dir = dir / len;
    let perp = Vec2::new(-dir.y, dir.x);
    let arrow_len = 10.0 + width;
    let arrow_w = 5.0 + width * 0.5;
    Some((dir, perp, arrow_len, arrow_w))
}

fn draw_arrowhead_chevron(painter: &Painter, from: Pos2, to: Pos2, color: Color32, width: f32) {
    let Some((dir, perp, arrow_len, arrow_w)) = arrowhead_basis(from, to, width) else { return };
    let base = Pos2::new(to.x - dir.x * arrow_len, to.y - dir.y * arrow_len);
    let left = Pos2::new(base.x + perp.x * arrow_w, base.y + perp.y * arrow_w);
    let right = Pos2::new(base.x - perp.x * arrow_w, base.y - perp.y * arrow_w);
    let notch = Pos2::new(to.x - dir.x * arrow_len * 0.4, to.y - dir.y * arrow_len * 0.4);
    let shape = PathShape::convex_polygon(vec![to, left, notch, right], color, Stroke::NONE);
    painter.add(shape);
}

fn draw_arrowhead_unfilled_triangle(painter: &Painter, from: Pos2, to: Pos2, color: Color32, width: f32) {
    let Some((dir, perp, arrow_len, arrow_w)) = arrowhead_basis(from, to, width) else { return };
    let base = Pos2::new(to.x - dir.x * arrow_len, to.y - dir.y * arrow_len);
    let left = Pos2::new(base.x + perp.x * arrow_w, base.y + perp.y * arrow_w);
    let right = Pos2::new(base.x - perp.x * arrow_w, base.y - perp.y * arrow_w);
    let shape = PathShape::convex_polygon(
        vec![to, left, right],
        Color32::WHITE,
        Stroke::new(1.5, color),
    );
    painter.add(shape);
}

fn draw_arrowhead_diamond(painter: &Painter, from: Pos2, to: Pos2, color: Color32, width: f32, filled: bool) {
    let Some((dir, perp, arrow_len, arrow_w)) = arrowhead_basis(from, to, width) else { return };
    let tip = to;
    let mid_l = Pos2::new(to.x - dir.x * arrow_len * 0.5 + perp.x * arrow_w, to.y - dir.y * arrow_len * 0.5 + perp.y * arrow_w);
    let mid_r = Pos2::new(to.x - dir.x * arrow_len * 0.5 - perp.x * arrow_w, to.y - dir.y * arrow_len * 0.5 - perp.y * arrow_w);
    let back = Pos2::new(to.x - dir.x * arrow_len, to.y - dir.y * arrow_len);
    let fill = if filled { color } else { Color32::WHITE };
    let shape = PathShape::convex_polygon(vec![tip, mid_l, back, mid_r], fill, Stroke::new(1.5, color));
    painter.add(shape);
}

fn draw_arrowhead_circle(painter: &Painter, _from: Pos2, to: Pos2, color: Color32, width: f32, filled: bool) {
    let r = 4.0 + width * 0.5;
    if filled {
        painter.circle_filled(to, r, color);
    } else {
        painter.circle_filled(to, r, Color32::WHITE);
        painter.circle_stroke(to, r, Stroke::new(1.5, color));
    }
}

fn draw_arrowhead_cross(painter: &Painter, from: Pos2, to: Pos2, color: Color32, width: f32) {
    let Some((dir, perp, _, arrow_w)) = arrowhead_basis(from, to, width) else { return };
    let size = arrow_w;
    let c = to;
    painter.line_segment(
        [Pos2::new(c.x - perp.x * size - dir.x * size, c.y - perp.y * size - dir.y * size),
         Pos2::new(c.x + perp.x * size + dir.x * size, c.y + perp.y * size + dir.y * size)],
        Stroke::new(1.5, color),
    );
    painter.line_segment(
        [Pos2::new(c.x + perp.x * size - dir.x * size, c.y + perp.y * size - dir.y * size),
         Pos2::new(c.x - perp.x * size + dir.x * size, c.y - perp.y * size + dir.y * size)],
        Stroke::new(1.5, color),
    );
}

fn draw_arrowhead_box(painter: &Painter, from: Pos2, to: Pos2, color: Color32, width: f32, filled: bool) {
    let Some((dir, perp, _, arrow_w)) = arrowhead_basis(from, to, width) else { return };
    let s = arrow_w;
    let c = Pos2::new(to.x - dir.x * s, to.y - dir.y * s);
    let points = vec![
        Pos2::new(c.x - perp.x * s - dir.x * s, c.y - perp.y * s - dir.y * s),
        Pos2::new(c.x + perp.x * s - dir.x * s, c.y + perp.y * s - dir.y * s),
        Pos2::new(c.x + perp.x * s + dir.x * s, c.y + perp.y * s + dir.y * s),
        Pos2::new(c.x - perp.x * s + dir.x * s, c.y - perp.y * s + dir.y * s),
    ];
    let fill = if filled { color } else { Color32::WHITE };
    let shape = PathShape::convex_polygon(points, fill, Stroke::new(1.5, color));
    painter.add(shape);
}

fn draw_arrowhead_line(painter: &Painter, from: Pos2, to: Pos2, color: Color32, width: f32) {
    let Some((_dir, perp, _, arrow_w)) = arrowhead_basis(from, to, width) else { return };
    painter.line_segment(
        [Pos2::new(to.x + perp.x * arrow_w, to.y + perp.y * arrow_w),
         Pos2::new(to.x - perp.x * arrow_w, to.y - perp.y * arrow_w)],
        Stroke::new(2.0, color),
    );
}

fn draw_arrowhead_cf_one(painter: &Painter, from: Pos2, to: Pos2, color: Color32, width: f32) {
    draw_arrowhead_line(painter, from, to, color, width);
}

fn draw_arrowhead_cf_many(painter: &Painter, from: Pos2, to: Pos2, color: Color32, width: f32) {
    let Some((dir, perp, arrow_len, arrow_w)) = arrowhead_basis(from, to, width) else { return };
    let base = Pos2::new(to.x - dir.x * arrow_len * 0.6, to.y - dir.y * arrow_len * 0.6);
    painter.line_segment([base, Pos2::new(to.x + perp.x * arrow_w, to.y + perp.y * arrow_w)], Stroke::new(1.5, color));
    painter.line_segment([base, Pos2::new(to.x - perp.x * arrow_w, to.y - perp.y * arrow_w)], Stroke::new(1.5, color));
    painter.line_segment([base, to], Stroke::new(1.5, color));
}

fn draw_arrowhead_cf_one_required(painter: &Painter, from: Pos2, to: Pos2, color: Color32, width: f32) {
    let Some((dir, _perp, arrow_len, _)) = arrowhead_basis(from, to, width) else { return };
    draw_arrowhead_line(painter, from, to, color, width);
    let offset = Pos2::new(to.x - dir.x * arrow_len * 0.4, to.y - dir.y * arrow_len * 0.4);
    draw_arrowhead_line(painter, from, offset, color, width);
}

fn draw_arrowhead_cf_many_required(painter: &Painter, from: Pos2, to: Pos2, color: Color32, width: f32) {
    let Some((dir, _, arrow_len, _)) = arrowhead_basis(from, to, width) else { return };
    let offset = Pos2::new(to.x - dir.x * arrow_len * 0.5, to.y - dir.y * arrow_len * 0.5);
    draw_arrowhead_cf_many(painter, from, to, color, width);
    draw_arrowhead_line(painter, from, offset, color, width);
}

pub fn draw_oval(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let center = rect.center();
    let rx = rect.width() / 2.0;
    let ry = rect.height() / 2.0;
    let points = ellipse_points(center, rx, ry, 48);
    let shape = PathShape::convex_polygon(points, fill, stroke);
    painter.add(shape);
}

pub fn draw_hexagon(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let c = rect.center();
    let hw = rect.width() / 2.0;
    let hh = rect.height() / 2.0;
    let inset = hw * 0.25;
    let points = vec![
        Pos2::new(c.x - hw + inset, c.y - hh),
        Pos2::new(c.x + hw - inset, c.y - hh),
        Pos2::new(c.x + hw, c.y),
        Pos2::new(c.x + hw - inset, c.y + hh),
        Pos2::new(c.x - hw + inset, c.y + hh),
        Pos2::new(c.x - hw, c.y),
    ];
    let shape = PathShape::convex_polygon(points, fill, stroke);
    painter.add(shape);
}

pub fn draw_parallelogram(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let skew = rect.width() * 0.15;
    let points = vec![
        Pos2::new(rect.left() + skew, rect.top()),
        Pos2::new(rect.right(), rect.top()),
        Pos2::new(rect.right() - skew, rect.bottom()),
        Pos2::new(rect.left(), rect.bottom()),
    ];
    let shape = PathShape::convex_polygon(points, fill, stroke);
    painter.add(shape);
}

pub fn draw_cylinder(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let ry = (rect.height() * 0.1).min(12.0);
    let body_top = rect.top() + ry;
    let body_bottom = rect.bottom() - ry;
    let cx = rect.center().x;
    let hw = rect.width() / 2.0;

    // Body rectangle
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(rect.left(), body_top),
            Pos2::new(rect.right(), body_bottom),
        ),
        0.0,
        fill,
    );

    // Top ellipse
    let top_pts = ellipse_points(Pos2::new(cx, body_top), hw, ry, 48);
    let top_shape = PathShape::convex_polygon(top_pts, fill, stroke);
    painter.add(top_shape);

    // Bottom half-ellipse
    let bottom_pts = half_ellipse_points(Pos2::new(cx, body_bottom), hw, ry, 24, false);
    let mut all_bottom = vec![Pos2::new(rect.right(), body_bottom)];
    all_bottom.extend(bottom_pts);
    all_bottom.push(Pos2::new(rect.left(), body_bottom));
    let bottom_shape = PathShape::convex_polygon(all_bottom, fill, stroke);
    painter.add(bottom_shape);

    // Side lines
    painter.line_segment(
        [Pos2::new(rect.left(), body_top), Pos2::new(rect.left(), body_bottom)],
        stroke,
    );
    painter.line_segment(
        [Pos2::new(rect.right(), body_top), Pos2::new(rect.right(), body_bottom)],
        stroke,
    );
}

pub fn draw_cloud(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let pts = cloud_points(rect, 64);
    let shape = PathShape::convex_polygon(pts, fill, stroke);
    painter.add(shape);
}

pub fn draw_page(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let fold = 12.0f32.min(rect.width() * 0.15).min(rect.height() * 0.15);
    let points = vec![
        Pos2::new(rect.left(), rect.top()),
        Pos2::new(rect.right() - fold, rect.top()),
        Pos2::new(rect.right(), rect.top() + fold),
        Pos2::new(rect.right(), rect.bottom()),
        Pos2::new(rect.left(), rect.bottom()),
    ];
    let shape = PathShape::convex_polygon(points, fill, stroke);
    painter.add(shape);
    // Fold triangle
    painter.line_segment(
        [Pos2::new(rect.right() - fold, rect.top()), Pos2::new(rect.right() - fold, rect.top() + fold)],
        stroke,
    );
    painter.line_segment(
        [Pos2::new(rect.right() - fold, rect.top() + fold), Pos2::new(rect.right(), rect.top() + fold)],
        stroke,
    );
}

pub fn draw_document(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    // Rectangle with wavy bottom - approximate as polygon
    let wave_h = (rect.height() * 0.08).min(8.0);
    let steps = 24;
    let mut points = vec![
        Pos2::new(rect.left(), rect.top()),
        Pos2::new(rect.right(), rect.top()),
        Pos2::new(rect.right(), rect.bottom() - wave_h),
    ];
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = rect.right() - t * rect.width();
        let y = rect.bottom() - wave_h + (t * std::f32::consts::PI * 2.0).sin() * wave_h;
        points.push(Pos2::new(x, y));
    }
    let shape = PathShape::convex_polygon(points, fill, stroke);
    painter.add(shape);
}

pub fn draw_person(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let head_r = (rect.width() * 0.18).min(rect.height() * 0.2);
    let head_cy = rect.top() + head_r + 2.0;
    let cx = rect.center().x;

    // Head
    painter.circle_filled(Pos2::new(cx, head_cy), head_r, fill);
    painter.circle_stroke(Pos2::new(cx, head_cy), head_r, stroke);

    // Body (trapezoid)
    let body_top = head_cy + head_r + 2.0;
    let body_bottom = rect.bottom();
    let top_hw = rect.width() * 0.2;
    let bot_hw = rect.width() * 0.45;
    let body_pts = vec![
        Pos2::new(cx - top_hw, body_top),
        Pos2::new(cx + top_hw, body_top),
        Pos2::new(cx + bot_hw, body_bottom),
        Pos2::new(cx - bot_hw, body_bottom),
    ];
    let body_shape = PathShape::convex_polygon(body_pts, fill, stroke);
    painter.add(body_shape);
}

pub fn draw_queue(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    // Horizontal cylinder (rotated cylinder)
    let rx = (rect.width() * 0.1).min(12.0);
    let body_left = rect.left() + rx;
    let body_right = rect.right() - rx;
    let cy = rect.center().y;
    let hh = rect.height() / 2.0;

    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(body_left, rect.top()),
            Pos2::new(body_right, rect.bottom()),
        ),
        0.0,
        fill,
    );

    // Left ellipse
    let left_pts = ellipse_points(Pos2::new(body_left, cy), rx, hh, 48);
    let left_shape = PathShape::convex_polygon(left_pts, fill, stroke);
    painter.add(left_shape);

    // Right ellipse
    let right_pts = ellipse_points(Pos2::new(body_right, cy), rx, hh, 48);
    let right_shape = PathShape::convex_polygon(right_pts, fill, stroke);
    painter.add(right_shape);

    // Top and bottom lines
    painter.line_segment(
        [Pos2::new(body_left, rect.top()), Pos2::new(body_right, rect.top())],
        stroke,
    );
    painter.line_segment(
        [Pos2::new(body_left, rect.bottom()), Pos2::new(body_right, rect.bottom())],
        stroke,
    );
}

pub fn draw_package(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let tab_w = rect.width() * 0.35;
    let tab_h = 12.0f32.min(rect.height() * 0.15);

    // Tab
    let tab_rect = Rect::from_min_size(
        Pos2::new(rect.left(), rect.top()),
        Vec2::new(tab_w, tab_h),
    );
    painter.rect_filled(tab_rect, 0.0, fill);
    painter.rect_stroke(tab_rect, 0.0, stroke, StrokeKind::Outside);

    // Body
    let body_rect = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + tab_h),
        Pos2::new(rect.right(), rect.bottom()),
    );
    painter.rect_filled(body_rect, 0.0, fill);
    painter.rect_stroke(body_rect, 0.0, stroke, StrokeKind::Outside);
}

pub fn draw_step(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let arrow = rect.width() * 0.15;
    let points = vec![
        Pos2::new(rect.left(), rect.top()),
        Pos2::new(rect.right() - arrow, rect.top()),
        Pos2::new(rect.right(), rect.center().y),
        Pos2::new(rect.right() - arrow, rect.bottom()),
        Pos2::new(rect.left(), rect.bottom()),
        Pos2::new(rect.left() + arrow, rect.center().y),
    ];
    let shape = PathShape::convex_polygon(points, fill, stroke);
    painter.add(shape);
}

pub fn draw_callout(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let tail_w = 10.0;
    let tail_h = 12.0f32.min(rect.height() * 0.2);
    let tail_x = rect.left() + rect.width() * 0.25;

    let points = vec![
        Pos2::new(rect.left(), rect.top()),
        Pos2::new(rect.right(), rect.top()),
        Pos2::new(rect.right(), rect.bottom() - tail_h),
        Pos2::new(tail_x + tail_w, rect.bottom() - tail_h),
        Pos2::new(tail_x, rect.bottom()),
        Pos2::new(tail_x - tail_w * 0.3, rect.bottom() - tail_h),
        Pos2::new(rect.left(), rect.bottom() - tail_h),
    ];
    let shape = PathShape::convex_polygon(points, fill, stroke);
    painter.add(shape);
}

pub fn draw_stored_data(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    // Curved left edge (like a magnetic disk)
    let curve = rect.width() * 0.1;
    let steps = 16;
    let mut points = Vec::new();

    // Right side (straight)
    points.push(Pos2::new(rect.right(), rect.top()));
    points.push(Pos2::new(rect.right(), rect.bottom()));

    // Bottom-left curve
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let y = rect.bottom() - t * rect.height();
        let x = rect.left() + curve * (1.0 - (t * std::f32::consts::PI).sin());
        points.push(Pos2::new(x, y));
    }

    let shape = PathShape::convex_polygon(points, fill, stroke);
    painter.add(shape);
}

pub fn draw_text_shape(painter: &Painter, rect: Rect, _fill: Color32, _stroke: Stroke) {
    let _ = (painter, rect);
}

pub fn draw_class_shape(
    painter: &Painter,
    rect: Rect,
    fill: Color32,
    stroke: Stroke,
    label: &str,
    fields: &[ClassField],
    methods: &[ClassMethod],
    text_color: Color32,
) {
    painter.rect_filled(rect, 0.0, fill);
    painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Outside);

    let row_h = 18.0;
    let font = FontId::proportional(13.0);
    let small_font = FontId::proportional(12.0);

    // Header
    let header_h = 28.0;
    let header_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), header_h));
    let header_fill = Color32::from_rgba_unmultiplied(
        stroke.color.r(), stroke.color.g(), stroke.color.b(), 40,
    );
    painter.rect_filled(header_rect, 0.0, header_fill);
    painter.text(
        Pos2::new(rect.center().x, rect.top() + header_h / 2.0),
        egui::Align2::CENTER_CENTER,
        label,
        font.clone(),
        text_color,
    );

    let mut y = rect.top() + header_h;

    // Divider after header
    painter.line_segment(
        [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
        stroke,
    );

    // Fields
    for field in fields {
        let vis = match field.visibility {
            '+' => "+",
            '-' => "-",
            '#' => "#",
            _ => " ",
        };
        let text = if field.type_str.is_empty() {
            format!("{}{}", vis, field.name)
        } else {
            format!("{}{}: {}", vis, field.name, field.type_str)
        };
        painter.text(
            Pos2::new(rect.left() + 8.0, y + row_h / 2.0),
            egui::Align2::LEFT_CENTER,
            &text,
            small_font.clone(),
            text_color,
        );
        y += row_h;
    }

    if !methods.is_empty() && !fields.is_empty() {
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            stroke,
        );
    }

    // Methods
    for method in methods {
        let vis = match method.visibility {
            '+' => "+",
            '-' => "-",
            '#' => "#",
            _ => " ",
        };
        let text = if method.return_type.is_empty() {
            format!("{}{}", vis, method.name)
        } else {
            format!("{}{}: {}", vis, method.name, method.return_type)
        };
        painter.text(
            Pos2::new(rect.left() + 8.0, y + row_h / 2.0),
            egui::Align2::LEFT_CENTER,
            &text,
            small_font.clone(),
            text_color,
        );
        y += row_h;
    }
}

pub fn draw_sql_table_shape(
    painter: &Painter,
    rect: Rect,
    fill: Color32,
    stroke: Stroke,
    label: &str,
    columns: &[SqlColumn],
    text_color: Color32,
) {
    painter.rect_filled(rect, 0.0, fill);
    painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Outside);

    let row_h = 18.0;
    let font = FontId::proportional(13.0);
    let small_font = FontId::proportional(12.0);

    // Header
    let header_h = 28.0;
    let header_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), header_h));
    let header_fill = Color32::from_rgba_unmultiplied(
        stroke.color.r(), stroke.color.g(), stroke.color.b(), 40,
    );
    painter.rect_filled(header_rect, 0.0, header_fill);
    painter.text(
        Pos2::new(rect.center().x, rect.top() + header_h / 2.0),
        egui::Align2::CENTER_CENTER,
        label,
        font.clone(),
        text_color,
    );

    let mut y = rect.top() + header_h;

    painter.line_segment(
        [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
        stroke,
    );

    // Columns
    for col in columns {
        let constraint_str = if col.constraint.is_empty() {
            String::new()
        } else {
            format!(" {}", col.constraint)
        };
        let text = format!("{} {}{}", col.name, col.type_str, constraint_str);
        painter.text(
            Pos2::new(rect.left() + 8.0, y + row_h / 2.0),
            egui::Align2::LEFT_CENTER,
            &text,
            small_font.clone(),
            text_color,
        );
        y += row_h;
    }
}

pub fn draw_fill_pattern(painter: &Painter, rect: Rect, pattern: FillPattern, stroke_color: Color32) {
    let color = Color32::from_rgba_unmultiplied(
        stroke_color.r(), stroke_color.g(), stroke_color.b(), 40,
    );
    let thin = Stroke::new(0.8, color);

    match pattern {
        FillPattern::Dots => {
            let spacing = 8.0;
            let r = 1.2;
            let mut y = rect.top() + spacing / 2.0;
            while y < rect.bottom() {
                let mut x = rect.left() + spacing / 2.0;
                while x < rect.right() {
                    painter.circle_filled(Pos2::new(x, y), r, color);
                    x += spacing;
                }
                y += spacing;
            }
        }
        FillPattern::Lines => {
            let spacing = 6.0;
            let mut y = rect.top();
            while y < rect.bottom() {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    thin,
                );
                y += spacing;
            }
        }
        FillPattern::Grain => {
            let spacing = 5.0;
            let mut offset = 0.0_f32;
            while offset < rect.width() + rect.height() {
                let x0 = rect.left() + offset;
                let y0 = rect.top();
                let x1 = rect.left();
                let y1 = rect.top() + offset;
                let cx0 = x0.min(rect.right());
                let cy0 = y0 + (x0 - cx0);
                let cx1 = x1.max(rect.left());
                let cy1 = y1.min(rect.bottom());
                if cy0 <= rect.bottom() && cx1 <= rect.right() {
                    painter.line_segment([Pos2::new(cx0, cy0), Pos2::new(cx1, cy1)], thin);
                }
                offset += spacing;
            }
        }
        FillPattern::Paper => {
            let spacing_h = 10.0;
            let spacing_v = 14.0;
            let mut y = rect.top() + spacing_h;
            while y < rect.bottom() {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    thin,
                );
                y += spacing_h;
            }
            let mut x = rect.left() + spacing_v;
            while x < rect.right() {
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    thin,
                );
                x += spacing_v;
            }
        }
    }
}

// Helper: generate points on an ellipse
fn ellipse_points(center: Pos2, rx: f32, ry: f32, n: usize) -> Vec<Pos2> {
    (0..n)
        .map(|i| {
            let angle = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
            Pos2::new(center.x + rx * angle.cos(), center.y + ry * angle.sin())
        })
        .collect()
}

// Helper: generate half-ellipse points (bottom half when bottom=false means going from right to left along bottom)
fn half_ellipse_points(center: Pos2, rx: f32, ry: f32, n: usize, _top: bool) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let angle = std::f32::consts::PI * i as f32 / n as f32;
            Pos2::new(center.x + rx * angle.cos(), center.y + ry * angle.sin())
        })
        .collect()
}

// Helper: cloud shape points
fn cloud_points(rect: Rect, n: usize) -> Vec<Pos2> {
    let cx = rect.center().x;
    let cy = rect.center().y;
    let rx = rect.width() / 2.0;
    let ry = rect.height() / 2.0;
    (0..n)
        .map(|i| {
            let angle = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
            let bump = 1.0 + 0.15 * (angle * 6.0).sin().abs();
            Pos2::new(cx + rx * bump * angle.cos(), cy + ry * bump * angle.sin())
        })
        .collect()
}

pub fn draw_arrowhead(painter: &Painter, from: Pos2, to: Pos2, color: Color32, width: f32) {
    let dir = Vec2::new(to.x - from.x, to.y - from.y);
    let len = dir.length();
    if len < 0.1 {
        return;
    }
    let dir = dir / len;
    let perp = Vec2::new(-dir.y, dir.x);

    let arrow_len = 10.0 + width;
    let arrow_width = 5.0 + width * 0.5;

    let base = Pos2::new(to.x - dir.x * arrow_len, to.y - dir.y * arrow_len);
    let left = Pos2::new(
        base.x + perp.x * arrow_width,
        base.y + perp.y * arrow_width,
    );
    let right = Pos2::new(
        base.x - perp.x * arrow_width,
        base.y - perp.y * arrow_width,
    );

    let shape = PathShape::convex_polygon(vec![to, left, right], color, Stroke::NONE);
    painter.add(shape);
}

pub fn sample_cubic_bezier(
    p0: Pos2,
    p1: Pos2,
    p2: Pos2,
    p3: Pos2,
    steps: usize,
) -> Vec<Pos2> {
    (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let mt = 1.0 - t;
            Pos2::new(
                mt * mt * mt * p0.x
                    + 3.0 * mt * mt * t * p1.x
                    + 3.0 * mt * t * t * p2.x
                    + t * t * t * p3.x,
                mt * mt * mt * p0.y
                    + 3.0 * mt * mt * t * p1.y
                    + 3.0 * mt * t * t * p2.y
                    + t * t * t * p3.y,
            )
        })
        .collect()
}

pub fn draw_catmull_rom_spline(
    painter: &Painter,
    points: &[Pos2],
    stroke: Stroke,
    dashed: bool,
    dash_len: f32,
    gap_len: f32,
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

        let cp1 = Pos2::new(
            p1.x + (p2.x - p0.x) / 6.0,
            p1.y + (p2.y - p0.y) / 6.0,
        );
        let cp2 = Pos2::new(
            p2.x - (p3.x - p1.x) / 6.0,
            p2.y - (p3.y - p1.y) / 6.0,
        );

        let samples = sample_cubic_bezier(p1, cp1, cp2, p2, 12);
        for j in 0..samples.len() - 1 {
            if dashed {
                draw_dashed_line(painter, samples[j], samples[j + 1], stroke, dash_len, gap_len);
            } else {
                painter.line_segment([samples[j], samples[j + 1]], stroke);
            }
        }
    }
}

pub fn draw_dashed_line(
    painter: &Painter,
    start: Pos2,
    end: Pos2,
    stroke: Stroke,
    dash_len: f32,
    gap_len: f32,
) {
    let dir = Vec2::new(end.x - start.x, end.y - start.y);
    let total = dir.length();
    if total < 0.1 {
        return;
    }
    let dir = dir / total;
    let segment = dash_len + gap_len;
    let mut cursor = 0.0;

    while cursor < total {
        let seg_start = Pos2::new(
            start.x + dir.x * cursor,
            start.y + dir.y * cursor,
        );
        let seg_end_t = (cursor + dash_len).min(total);
        let seg_end = Pos2::new(
            start.x + dir.x * seg_end_t,
            start.y + dir.y * seg_end_t,
        );
        painter.line_segment([seg_start, seg_end], stroke);
        cursor += segment;
    }
}

pub fn draw_stadium(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let r = rect.height() / 2.0;
    let body_left = rect.left() + r;
    let body_right = rect.right() - r;
    let cy = rect.center().y;
    let mut pts = Vec::new();
    let n = 16;
    for i in 0..=n {
        let angle = std::f32::consts::PI / 2.0 + std::f32::consts::PI * i as f32 / n as f32;
        pts.push(Pos2::new(body_left + r * angle.cos(), cy + r * angle.sin()));
    }
    for i in 0..=n {
        let angle = -std::f32::consts::PI / 2.0 + std::f32::consts::PI * i as f32 / n as f32;
        pts.push(Pos2::new(body_right + r * angle.cos(), cy + r * angle.sin()));
    }
    painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
}

pub fn draw_subroutine(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    painter.rect(rect, 0.0, fill, stroke, StrokeKind::Outside);
    let inset = 8.0f32.min(rect.width() * 0.08);
    painter.line_segment(
        [Pos2::new(rect.left() + inset, rect.top()), Pos2::new(rect.left() + inset, rect.bottom())],
        stroke,
    );
    painter.line_segment(
        [Pos2::new(rect.right() - inset, rect.top()), Pos2::new(rect.right() - inset, rect.bottom())],
        stroke,
    );
}

pub fn draw_double_circle(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let outer_r = rect.width().min(rect.height()) / 2.0;
    let inner_r = outer_r - 6.0;
    let center = rect.center();
    painter.circle(center, outer_r, fill, stroke);
    painter.circle_stroke(center, inner_r.max(2.0), stroke);
}

pub fn draw_trapezoid(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let inset = rect.width() * 0.15;
    let pts = vec![
        Pos2::new(rect.left(), rect.top()),
        Pos2::new(rect.right(), rect.top()),
        Pos2::new(rect.right() - inset, rect.bottom()),
        Pos2::new(rect.left() + inset, rect.bottom()),
    ];
    painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
}

pub fn draw_trapezoid_alt(painter: &Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let inset = rect.width() * 0.15;
    let pts = vec![
        Pos2::new(rect.left() + inset, rect.top()),
        Pos2::new(rect.right() - inset, rect.top()),
        Pos2::new(rect.right(), rect.bottom()),
        Pos2::new(rect.left(), rect.bottom()),
    ];
    painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
}
