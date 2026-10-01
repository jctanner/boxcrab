use crate::diagram::{DiagramGraph, EdgeType, NodeShape, StyleProps};
use super::{LayoutEdge, LayoutNode, LayoutResult, LayoutSubgraph};
use std::collections::HashMap;

const PARTICIPANT_SPACING: f32 = 200.0;
const PARTICIPANT_BOX_WIDTH: f32 = 120.0;
const PARTICIPANT_BOX_HEIGHT: f32 = 40.0;
const MESSAGE_SPACING: f32 = 40.0;
const LIFELINE_TOP_MARGIN: f32 = 30.0;
const SELF_MSG_WIDTH: f32 = 40.0;
const SELF_MSG_HEIGHT: f32 = 25.0;
const ACTIVATION_WIDTH: f32 = 12.0;
const NOTE_WIDTH: f32 = 120.0;
const NOTE_HEIGHT: f32 = 30.0;
const MARGIN: f32 = 40.0;

const LINE_HEIGHT: f32 = 14.0;

fn line_count(s: &str) -> usize {
    s.split('\n').count().max(1)
}

/// Rough rendered width of an edge label (12pt proportional font), used to
/// position labels that must start at an anchor rather than be centered on it.
/// Multi-line labels are as wide as their longest line.
pub fn estimate_label_width(label: &str) -> f32 {
    label.split('\n').map(|l| l.chars().count()).max().unwrap_or(0) as f32 * 6.4 + 8.0
}

fn note_height(label: &str) -> f32 {
    (line_count(label) as f32 * LINE_HEIGHT + 16.0).max(NOTE_HEIGHT)
}

/// Vertical geometry of a sequence diagram: participant box height and the
/// y coordinate of every timeline slot. Slots grow taller to fit multi-line
/// message labels and notes, so positions are not a simple multiple of the
/// message spacing.
pub struct SeqMetrics {
    pub box_h: f32,
    /// y of each slot; `ys[n]` is the position just past the last slot.
    ys: Vec<f32>,
}

impl SeqMetrics {
    pub fn new(graph: &DiagramGraph, measured: Option<&HashMap<String, egui::Vec2>>) -> Self {
        // Participant boxes grow to fit multi-line labels.
        let mut box_h = PARTICIPANT_BOX_HEIGHT;
        for (id, def) in &graph.nodes {
            if id.starts_with("__note_") {
                continue;
            }
            let text_h = measured
                .and_then(|m| m.get(id))
                .map(|v| v.y)
                .unwrap_or(line_count(&def.label) as f32 * 17.0);
            box_h = box_h.max(text_h + 20.0);
        }

        let n = graph.edges.len();
        // Space each timeline entry needs above and below its anchor y.
        let extent = |e: &crate::diagram::EdgeDef| -> (f32, f32) {
            if e.to.starts_with("__note_") {
                let h = graph.nodes.get(&e.to).map_or(NOTE_HEIGHT, |nd| note_height(&nd.label));
                (h / 2.0, h / 2.0)
            } else {
                let k = e.label.as_deref().map_or(1, line_count) as f32;
                if e.from == e.to {
                    // Loop below the anchor, label hanging beside it.
                    (6.0, SELF_MSG_HEIGHT.max(4.0 + k * LINE_HEIGHT / 2.0))
                } else {
                    // Label sits above the arrow.
                    (9.0 + k * LINE_HEIGHT, 3.0)
                }
            }
        };
        let extents: Vec<(f32, f32)> = graph.edges.iter().map(extent).collect();

        let start = MARGIN + box_h + LIFELINE_TOP_MARGIN;
        let mut ys = Vec::with_capacity(n + 1);
        // The margin below the participant boxes is sized for a single-line
        // message label (23 above its arrow) plus 7 of clearance, enough for
        // an actor figure's legs; taller first entries push everything down.
        let first_up = extents.first().map_or(0.0, |e| e.0);
        let mut y = start + (first_up + 7.0 - LIFELINE_TOP_MARGIN).max(0.0);
        ys.push(y);
        for i in 0..n {
            let lower = extents[i].1;
            let gap = match extents.get(i + 1) {
                Some(next) => (lower + next.0 + 6.0).max(MESSAGE_SPACING),
                None => (lower + 20.0).max(MESSAGE_SPACING),
            };
            y += gap;
            ys.push(y);
        }
        SeqMetrics { box_h, ys }
    }

    pub fn slot_count(&self) -> usize {
        self.ys.len() - 1
    }

    /// Y coordinate at which the timeline entry in `slot` is drawn.
    pub fn y(&self, slot: usize) -> f32 {
        self.ys[slot.min(self.slot_count())]
    }

    /// Nearest timeline slot (0..=max) for a scene-space y coordinate.
    pub fn slot_at_y(&self, y: f32, max: usize) -> usize {
        let max = max.min(self.slot_count());
        (0..=max)
            .min_by(|a, b| (self.ys[*a] - y).abs().total_cmp(&(self.ys[*b] - y).abs()))
            .unwrap_or(0)
    }

    /// (top, bottom) y of the lifelines.
    pub fn lifeline_range(&self) -> (f32, f32) {
        (MARGIN + self.box_h, self.y(self.slot_count()) + LIFELINE_TOP_MARGIN)
    }
}

pub fn layout_sequence(
    graph: &DiagramGraph,
    measured_sizes: Option<&HashMap<String, egui::Vec2>>,
) -> Result<LayoutResult, String> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut subgraphs = Vec::new();

    let mut participant_ids: Vec<String> = graph.nodes.keys().cloned().collect();
    participant_ids.sort();
    let participant_ids: Vec<String> = participant_ids
        .into_iter()
        .filter(|id| !id.starts_with("__note_"))
        .collect();

    if participant_ids.is_empty() {
        return Ok(LayoutResult {
            nodes: Vec::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            total_width: 0.0,
            total_height: 0.0,
        });
    }

    let mut participant_x: HashMap<String, f32> = HashMap::new();
    let mut participant_widths: HashMap<String, f32> = HashMap::new();
    for (i, pid) in participant_ids.iter().enumerate() {
        let w = measured_sizes
            .and_then(|m| m.get(pid))
            .map(|v| v.x.max(PARTICIPANT_BOX_WIDTH))
            .unwrap_or(PARTICIPANT_BOX_WIDTH);
        let x = MARGIN + i as f32 * PARTICIPANT_SPACING + PARTICIPANT_SPACING / 2.0;
        participant_x.insert(pid.clone(), x);
        participant_widths.insert(pid.clone(), w);
    }

    let metrics = SeqMetrics::new(graph, measured_sizes);
    let box_h = metrics.box_h;
    let top_y = MARGIN;
    for pid in &participant_ids {
        let node_def = &graph.nodes[pid];
        let x = participant_x[pid];
        let w = participant_widths[pid];
        nodes.push(LayoutNode {
            id: pid.clone(),
            x,
            y: top_y + box_h / 2.0,
            width: w,
            height: box_h,
            label: node_def.label.clone(),
            shape: node_def.shape,
            style: graph.styles.get(pid).cloned().unwrap_or_default(),
            class_fields: Vec::new(),
            class_methods: Vec::new(),
            sql_columns: Vec::new(),
            tooltip: node_def.tooltip.clone(),
            link: node_def.link.clone(),
        });
    }

    let messages_start_y = metrics.y(0);
    let mut msg_y_positions: Vec<f32> = Vec::new();
    let mut slot_index: usize = 0;

    for edge in &graph.edges {
        let y = metrics.y(slot_index);
        msg_y_positions.push(y);

        if edge.to.starts_with("__note_") {
            let from_x = participant_x.get(&edge.from).copied().unwrap_or(MARGIN);
            if let Some(note_node) = graph.nodes.get(&edge.to) {
                let tooltip = note_node.tooltip.as_deref().unwrap_or("");
                let parts: Vec<&str> = tooltip.strip_prefix("note:").unwrap_or("").splitn(2, ':').collect();
                let position = parts.first().unwrap_or(&"right");
                let participant_names = parts.get(1).unwrap_or(&"");

                let note_x = match *position {
                    "left" => from_x - NOTE_WIDTH - 20.0,
                    "over" => {
                        let pids: Vec<&str> = participant_names.split(',').collect();
                        if pids.len() >= 2 {
                            let second = pids[1].trim();
                            let second_x = participant_x.iter()
                                .find(|(k, _)| k.ends_with(&format!("_{second}")))
                                .map(|(_, &x)| x)
                                .unwrap_or(from_x);
                            (from_x + second_x) / 2.0 - NOTE_WIDTH / 2.0
                        } else {
                            from_x - NOTE_WIDTH / 2.0
                        }
                    }
                    _ => from_x + 20.0,
                };

                let note_w = measured_sizes
                    .and_then(|m| m.get(&edge.to))
                    .map(|v| v.x.max(NOTE_WIDTH))
                    .unwrap_or(NOTE_WIDTH);

                nodes.push(LayoutNode {
                    id: edge.to.clone(),
                    x: note_x + note_w / 2.0,
                    y,
                    width: note_w,
                    height: note_height(&note_node.label),
                    label: note_node.label.clone(),
                    shape: NodeShape::Rect,
                    style: StyleProps {
                        fill: Some([255, 255, 210]),
                        stroke: Some([200, 200, 150]),
                        ..StyleProps::default()
                    },
                    class_fields: Vec::new(),
                    class_methods: Vec::new(),
                    sql_columns: Vec::new(),
                    tooltip: None,
                    link: None,
                });
            }
            slot_index += 1;
            continue;
        }

        let from_x = participant_x.get(&edge.from).copied().unwrap_or(MARGIN);
        let to_x = participant_x.get(&edge.to).copied().unwrap_or(MARGIN + PARTICIPANT_SPACING);

        if edge.from == edge.to {
            let x = from_x;
            edges.push(LayoutEdge {
                points: vec![
                    [x, y],
                    [x + SELF_MSG_WIDTH, y],
                    [x + SELF_MSG_WIDTH, y + SELF_MSG_HEIGHT],
                    [x, y + SELF_MSG_HEIGHT],
                ],
                control_points: None,
                edge_type: edge.edge_type,
                label: edge.label.clone(),
                // Labels are drawn centered on label_pos, so shift right by half the
                // label width to start just past the loop, and sit on its top row so
                // the next message's label (drawn above its own arrow) can't collide.
                label_pos: Some([
                    x + SELF_MSG_WIDTH + 8.0
                        + edge.label.as_deref().map_or(0.0, estimate_label_width) / 2.0,
                    y + 4.0,
                ]),
                reversed: false,
                src_arrowhead: edge.src_arrowhead,
                dst_arrowhead: edge.dst_arrowhead,
                style: edge.style.clone(),
            });
        } else {
            let label_x = (from_x + to_x) / 2.0;
            edges.push(LayoutEdge {
                points: vec![[from_x, y], [to_x, y]],
                control_points: None,
                edge_type: edge.edge_type,
                label: edge.label.clone(),
                label_pos: Some([
                    label_x,
                    y - 9.0 - LINE_HEIGHT / 2.0 * edge.label.as_deref().map_or(1, line_count) as f32,
                ]),
                reversed: false,
                src_arrowhead: edge.src_arrowhead,
                dst_arrowhead: edge.dst_arrowhead,
                style: edge.style.clone(),
            });
        }
        slot_index += 1;
    }

    let num_messages = slot_index;
    let bottom_y = metrics.lifeline_range().1;

    for pid in &participant_ids {
        let x = participant_x[pid];

        let lifeline_start_y = top_y + box_h;
        let lifeline_end_y = bottom_y;
        edges.push(LayoutEdge {
            points: vec![[x, lifeline_start_y], [x, lifeline_end_y]],
            control_points: None,
            edge_type: EdgeType::DottedLine,
            label: None,
            label_pos: None,
            reversed: false,
            src_arrowhead: None,
            dst_arrowhead: None,
            style: StyleProps {
                stroke: Some([180, 180, 180]),
                ..StyleProps::default()
            },
        });

        let node_def = &graph.nodes[pid];
        let w = participant_widths[pid];
        nodes.push(LayoutNode {
            id: format!("{pid}_bottom"),
            x,
            y: bottom_y + box_h / 2.0,
            width: w,
            height: box_h,
            label: node_def.label.clone(),
            shape: node_def.shape,
            style: graph.styles.get(pid).cloned().unwrap_or_default(),
            class_fields: Vec::new(),
            class_methods: Vec::new(),
            sql_columns: Vec::new(),
            tooltip: None,
            link: None,
        });
    }

    for (pid, start_msg, end_msg) in &graph.seq_activations {
        if let Some(&x) = participant_x.get(pid) {
            let start_y = if *start_msg < msg_y_positions.len() {
                msg_y_positions[*start_msg]
            } else {
                messages_start_y
            };
            let end_y = if *end_msg < msg_y_positions.len() {
                msg_y_positions[*end_msg]
            } else {
                bottom_y - LIFELINE_TOP_MARGIN
            };

            let act_h = (end_y - start_y).max(10.0);
            nodes.push(LayoutNode {
                id: format!("__activation_{}_{}", pid, start_msg),
                x,
                y: start_y + act_h / 2.0,
                width: ACTIVATION_WIDTH,
                height: act_h,
                label: String::new(),
                shape: NodeShape::Rect,
                style: StyleProps {
                    fill: Some([220, 235, 250]),
                    stroke: Some([100, 140, 180]),
                    ..StyleProps::default()
                },
                class_fields: Vec::new(),
                class_methods: Vec::new(),
                sql_columns: Vec::new(),
                tooltip: None,
                link: None,
            });
        }
    }

    for sg in &graph.subgraphs {
        let start_msg = sg.grid_rows.unwrap_or(0);
        let end_msg = sg.grid_columns.unwrap_or(num_messages);

        let sg_start_y = metrics.y(start_msg) - MESSAGE_SPACING / 3.0;
        let sg_end_y = metrics.y(end_msg) + MESSAGE_SPACING / 3.0;

        let sg_x = MARGIN / 2.0;
        let sg_width = participant_ids.len() as f32 * PARTICIPANT_SPACING + MARGIN;

        let layout_branches: Vec<(f32, String)> = sg.branches.iter().map(|(msg_idx, label)| {
            let y = metrics.y(*msg_idx) - MESSAGE_SPACING / 3.0;
            (y, label.clone())
        }).collect();

        subgraphs.push(LayoutSubgraph {
            title: sg.title.clone(),
            x: sg_x,
            y: sg_start_y,
            width: sg_width,
            height: (sg_end_y - sg_start_y).max(MESSAGE_SPACING),
            branches: layout_branches,
        });
    }

    let total_width = MARGIN * 2.0 + participant_ids.len() as f32 * PARTICIPANT_SPACING;
    let total_height = bottom_y + box_h + MARGIN;

    Ok(LayoutResult {
        nodes,
        edges,
        subgraphs,
        total_width,
        total_height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_message_label_does_not_collide_with_next_label() {
        let src = "sequenceDiagram
    participant CI
    participant FS
    CI->>CI: prepare-request.py writes .prototype/request.json
    CI->>FS: fullsend run rhaistrat with creator checkout and RHAISTRAT_MODEL
";
        let g = crate::parser::mermaid::parse(src).unwrap();
        let r = layout_sequence(&g, None).unwrap();
        let rects: Vec<egui::Rect> = r
            .edges
            .iter()
            .filter_map(|e| {
                let lp = e.label_pos?;
                let w = estimate_label_width(e.label.as_deref()?);
                Some(egui::Rect::from_center_size(
                    egui::Pos2::new(lp[0], lp[1]),
                    egui::Vec2::new(w, 16.0),
                ))
            })
            .collect();
        assert_eq!(rects.len(), 2);
        assert!(!rects[0].intersects(rects[1]), "{:?} vs {:?}", rects[0], rects[1]);
        // The self label starts to the right of the lifeline instead of
        // straddling it.
        let lifeline_x = r.edges[0].points[0][0];
        assert!(rects[0].min.x > lifeline_x);
    }

    #[test]
    fn multiline_labels_get_room() {
        let single = crate::parser::mermaid::parse(
            "sequenceDiagram\n    participant A\n    participant B\n    A->>B: one\n    B->>A: two\n",
        )
        .unwrap();
        let multi = crate::parser::mermaid::parse(
            "sequenceDiagram\n    participant A as First<br/>Second<br/>Third\n    participant B\n    A->>B: one<br/>two<br/>three\n    Note over A: n1<br/>n2<br/>n3\n    B->>A: two\n",
        )
        .unwrap();
        assert_eq!(multi.nodes.values().find(|n| n.label.contains("First")).unwrap().label, "First\nSecond\nThird");

        let m1 = SeqMetrics::new(&single, None);
        let m2 = SeqMetrics::new(&multi, None);
        assert!(m2.box_h > m1.box_h, "{} vs {}", m2.box_h, m1.box_h);
        // A 3-line message label sits above its arrow, pushing the slot down;
        // a 3-line note's lower half pushes the following slot down.
        assert!(m2.y(0) > m1.y(0));
        assert!(m2.y(2) - m2.y(1) > 40.0);
        // slot_at_y maps back to the same slots.
        for s in 0..=3 {
            assert_eq!(m2.slot_at_y(m2.y(s), 3), s);
        }

        // Laid-out participant boxes use the taller height, and the first
        // message line starts below them.
        let r = layout_sequence(&multi, None).unwrap();
        let top = r.nodes.iter().find(|n| n.label.contains("First")).unwrap();
        assert_eq!(top.height, m2.box_h);
        let first_msg_y = r.edges[0].points[0][1];
        assert!(first_msg_y > top.y + top.height / 2.0);
    }
}
