pub mod export;
pub mod shapes;

use crate::layout::{LayoutEdge, LayoutResult, LayoutSubgraph};
use crate::diagram::{DiagramGraph, NodeShape};
use egui::{Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use std::collections::HashMap;

pub fn measure_node_texts(
    ui: &Ui,
    graph: &Option<DiagramGraph>,
) -> Option<HashMap<String, Vec2>> {
    let graph = graph.as_ref()?;
    let font = FontId::proportional(14.0);
    let mut sizes = HashMap::new();

    for (id, node) in &graph.nodes {
        let mut max_w: f32 = 0.0;
        let mut total_h: f32 = 0.0;
        for line in node.label.split('\n') {
            let galley = ui.painter().layout_no_wrap(
                line.to_string(),
                font.clone(),
                Color32::BLACK,
            );
            let sz = galley.size();
            max_w = max_w.max(sz.x);
            total_h += sz.y;
        }
        sizes.insert(id.clone(), Vec2::new(max_w, total_h));
    }

    Some(sizes)
}

pub fn render_diagram(ui: &mut Ui, layout: &LayoutResult, drillable_ids: &[String]) {
    for sg in &layout.subgraphs {
        render_subgraph(ui, sg);
    }

    for edge in &layout.edges {
        render_edge(ui, edge);
    }

    for node in &layout.nodes {
        let painter = ui.painter();
        let rect = Rect::from_center_size(
            Pos2::new(node.x, node.y),
            Vec2::new(node.width, node.height),
        );

        let fill = node
            .style
            .fill
            .map(|c| Color32::from_rgb(c[0], c[1], c[2]))
            .unwrap_or(Color32::from_rgb(255, 255, 255));

        let stroke_color = node
            .style
            .stroke
            .map(|c| Color32::from_rgb(c[0], c[1], c[2]))
            .unwrap_or(Color32::from_rgb(80, 80, 80));

        let stroke_width = node.style.stroke_width.unwrap_or(1.5);
        let stroke = Stroke::new(stroke_width, stroke_color);

        let text_color = node
            .style
            .color
            .map(|c| Color32::from_rgb(c[0], c[1], c[2]))
            .unwrap_or(Color32::from_rgb(30, 30, 30));

        match node.shape {
            NodeShape::Rect => shapes::draw_rect(painter, rect, fill, stroke),
            NodeShape::Rounded => shapes::draw_rounded_rect(painter, rect, fill, stroke),
            NodeShape::Diamond => shapes::draw_diamond(painter, rect, fill, stroke),
            NodeShape::Circle => shapes::draw_circle(painter, rect, fill, stroke),
            NodeShape::Flag => shapes::draw_flag(painter, rect, fill, stroke),
        }

        let font = FontId::proportional(14.0);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &node.label,
            font,
            text_color,
        );

        let drillable = drillable_ids.contains(&node.id);

        if drillable {
            let icon_size = 10.0;
            let margin = 4.0;
            let icon_center = Pos2::new(
                rect.right() - icon_size / 2.0 - margin,
                rect.bottom() - icon_size / 2.0 - margin,
            );
            let icon_color = Color32::from_rgba_unmultiplied(
                text_color.r(),
                text_color.g(),
                text_color.b(),
                160,
            );
            let r = icon_size / 2.0;
            painter.circle_stroke(icon_center, r, Stroke::new(1.2, icon_color));
            let arrow_size = 3.0;
            painter.line_segment(
                [
                    Pos2::new(icon_center.x - arrow_size * 0.5, icon_center.y - arrow_size * 0.4),
                    Pos2::new(icon_center.x + arrow_size * 0.5, icon_center.y),
                ],
                Stroke::new(1.2, icon_color),
            );
            painter.line_segment(
                [
                    Pos2::new(icon_center.x - arrow_size * 0.5, icon_center.y + arrow_size * 0.4),
                    Pos2::new(icon_center.x + arrow_size * 0.5, icon_center.y),
                ],
                Stroke::new(1.2, icon_color),
            );
        }

        let resp = ui.allocate_rect(rect, egui::Sense::hover());
        if drillable {
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            resp.on_hover_text(format!("{} (click to drill down)", &node.id));
        } else {
            resp.on_hover_text(&node.id);
        }
    }
}

fn render_edge(ui: &Ui, edge: &LayoutEdge) {
    let painter = ui.painter();

    if edge.points.len() < 2 {
        return;
    }

    let color = Color32::from_rgb(100, 100, 100);

    let (stroke_width, dashed) = match edge.edge_type {
        crate::diagram::EdgeType::Arrow | crate::diagram::EdgeType::BidiArrow | crate::diagram::EdgeType::Line => {
            (1.5, false)
        }
        crate::diagram::EdgeType::DottedArrow
        | crate::diagram::EdgeType::BidiDottedArrow
        | crate::diagram::EdgeType::DottedLine => (1.5, true),
        crate::diagram::EdgeType::ThickArrow
        | crate::diagram::EdgeType::BidiThickArrow
        | crate::diagram::EdgeType::ThickLine => (3.0, false),
    };

    let has_arrow = matches!(
        edge.edge_type,
        crate::diagram::EdgeType::Arrow
            | crate::diagram::EdgeType::DottedArrow
            | crate::diagram::EdgeType::ThickArrow
            | crate::diagram::EdgeType::BidiArrow
            | crate::diagram::EdgeType::BidiDottedArrow
            | crate::diagram::EdgeType::BidiThickArrow
    );

    let has_arrow_both = matches!(
        edge.edge_type,
        crate::diagram::EdgeType::BidiArrow
            | crate::diagram::EdgeType::BidiDottedArrow
            | crate::diagram::EdgeType::BidiThickArrow
    );

    let stroke = Stroke::new(stroke_width, color);

    let start = Pos2::new(edge.points[0][0], edge.points[0][1]);
    let end = Pos2::new(edge.points[edge.points.len() - 1][0], edge.points[edge.points.len() - 1][1]);

    if edge.points.len() > 2 && edge.control_points.is_none() {
        let pts: Vec<Pos2> = edge.points.iter().map(|p| Pos2::new(p[0], p[1])).collect();
        shapes::draw_catmull_rom_spline(painter, &pts, stroke, dashed, 6.0, 4.0);

        if has_arrow {
            let n = pts.len();
            if edge.reversed {
                shapes::draw_arrowhead(painter, pts[1], pts[0], color, stroke_width);
                if has_arrow_both {
                    shapes::draw_arrowhead(painter, pts[n - 2], pts[n - 1], color, stroke_width);
                }
            } else {
                shapes::draw_arrowhead(painter, pts[n - 2], pts[n - 1], color, stroke_width);
                if has_arrow_both {
                    shapes::draw_arrowhead(painter, pts[1], pts[0], color, stroke_width);
                }
            }
        }
    } else if let Some(cp) = &edge.control_points {
        let cp1 = Pos2::new(cp[0][0], cp[0][1]);
        let cp2 = Pos2::new(cp[1][0], cp[1][1]);
        let samples = shapes::sample_cubic_bezier(start, cp1, cp2, end, 24);

        for i in 0..samples.len() - 1 {
            if dashed {
                shapes::draw_dashed_line(painter, samples[i], samples[i + 1], stroke, 6.0, 4.0);
            } else {
                painter.line_segment([samples[i], samples[i + 1]], stroke);
            }
        }

        if has_arrow {
            let (arrow_from, arrow_to) = if edge.reversed {
                (cp1, start)
            } else {
                (cp2, end)
            };
            shapes::draw_arrowhead(painter, arrow_from, arrow_to, color, stroke_width);
            if has_arrow_both {
                let (arrow_from, arrow_to) = if edge.reversed {
                    (cp2, end)
                } else {
                    (cp1, start)
                };
                shapes::draw_arrowhead(painter, arrow_from, arrow_to, color, stroke_width);
            }
        }
    } else {
        if dashed {
            shapes::draw_dashed_line(painter, start, end, stroke, 6.0, 4.0);
        } else {
            painter.line_segment([start, end], stroke);
        }

        if has_arrow {
            if edge.reversed {
                shapes::draw_arrowhead(painter, end, start, color, stroke_width);
                if has_arrow_both {
                    shapes::draw_arrowhead(painter, start, end, color, stroke_width);
                }
            } else {
                shapes::draw_arrowhead(painter, start, end, color, stroke_width);
                if has_arrow_both {
                    shapes::draw_arrowhead(painter, end, start, color, stroke_width);
                }
            }
        }
    }

    if let (Some(label), Some(pos)) = (&edge.label, &edge.label_pos) {
        let font = FontId::proportional(12.0);
        let galley = painter.layout_no_wrap(label.clone(), font.clone(), Color32::from_rgb(80, 80, 80));
        let text_size = galley.size();
        let bg_rect = Rect::from_center_size(
            Pos2::new(pos[0], pos[1]),
            Vec2::new(text_size.x + 8.0, text_size.y + 4.0),
        );
        painter.rect_filled(bg_rect, 2.0, Color32::from_rgb(245, 245, 245));
        painter.text(
            Pos2::new(pos[0], pos[1]),
            egui::Align2::CENTER_CENTER,
            label,
            font,
            Color32::from_rgb(80, 80, 80),
        );
    }
}

fn render_subgraph(ui: &Ui, sg: &LayoutSubgraph) {
    let painter = ui.painter();
    let rect = Rect::from_min_size(
        Pos2::new(sg.x, sg.y),
        Vec2::new(sg.width, sg.height),
    );

    painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(255, 248, 220, 80));
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(200, 185, 140)), StrokeKind::Outside);

    painter.text(
        Pos2::new(sg.x + 8.0, sg.y + 4.0),
        egui::Align2::LEFT_TOP,
        &sg.title,
        FontId::proportional(12.0),
        Color32::from_rgb(130, 115, 80),
    );
}
