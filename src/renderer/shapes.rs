use egui::{Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, epaint::PathShape};

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
