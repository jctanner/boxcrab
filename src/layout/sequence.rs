use crate::diagram::{DiagramGraph, EdgeType, NodeShape, StyleProps};
use super::{LayoutEdge, LayoutNode, LayoutResult, LayoutSubgraph};
use std::collections::HashMap;

const PARTICIPANT_SPACING: f32 = 200.0;
const PARTICIPANT_BOX_WIDTH: f32 = 120.0;
const PARTICIPANT_BOX_HEIGHT: f32 = 40.0;
const MESSAGE_SPACING: f32 = 40.0;
const LIFELINE_TOP_MARGIN: f32 = 15.0;
const SELF_MSG_WIDTH: f32 = 40.0;
const SELF_MSG_HEIGHT: f32 = 25.0;
const ACTIVATION_WIDTH: f32 = 12.0;
const NOTE_WIDTH: f32 = 120.0;
const NOTE_HEIGHT: f32 = 30.0;
const MARGIN: f32 = 40.0;

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

    let top_y = MARGIN;
    for pid in &participant_ids {
        let node_def = &graph.nodes[pid];
        let x = participant_x[pid];
        let w = participant_widths[pid];
        nodes.push(LayoutNode {
            id: pid.clone(),
            x,
            y: top_y + PARTICIPANT_BOX_HEIGHT / 2.0,
            width: w,
            height: PARTICIPANT_BOX_HEIGHT,
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

    let messages_start_y = top_y + PARTICIPANT_BOX_HEIGHT + LIFELINE_TOP_MARGIN;
    let mut msg_y_positions: Vec<f32> = Vec::new();
    let mut slot_index: usize = 0;

    for edge in &graph.edges {
        let y = messages_start_y + slot_index as f32 * MESSAGE_SPACING;
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
                    height: NOTE_HEIGHT,
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
                label_pos: Some([x + SELF_MSG_WIDTH + 5.0, y + SELF_MSG_HEIGHT / 2.0]),
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
                label_pos: Some([label_x, y - 16.0]),
                reversed: false,
                src_arrowhead: edge.src_arrowhead,
                dst_arrowhead: edge.dst_arrowhead,
                style: edge.style.clone(),
            });
        }
        slot_index += 1;
    }

    let num_messages = slot_index;
    let bottom_y = messages_start_y + num_messages as f32 * MESSAGE_SPACING + LIFELINE_TOP_MARGIN;

    for pid in &participant_ids {
        let x = participant_x[pid];

        let lifeline_start_y = top_y + PARTICIPANT_BOX_HEIGHT;
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
            y: bottom_y + PARTICIPANT_BOX_HEIGHT / 2.0,
            width: w,
            height: PARTICIPANT_BOX_HEIGHT,
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

        let sg_start_y = messages_start_y + start_msg as f32 * MESSAGE_SPACING - MESSAGE_SPACING / 3.0;
        let sg_end_y = messages_start_y + end_msg as f32 * MESSAGE_SPACING + MESSAGE_SPACING / 3.0;

        let sg_x = MARGIN / 2.0;
        let sg_width = participant_ids.len() as f32 * PARTICIPANT_SPACING + MARGIN;

        let layout_branches: Vec<(f32, String)> = sg.branches.iter().map(|(msg_idx, label)| {
            let y = messages_start_y + *msg_idx as f32 * MESSAGE_SPACING - MESSAGE_SPACING / 3.0;
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
    let total_height = bottom_y + PARTICIPANT_BOX_HEIGHT + MARGIN;

    Ok(LayoutResult {
        nodes,
        edges,
        subgraphs,
        total_width,
        total_height,
    })
}
