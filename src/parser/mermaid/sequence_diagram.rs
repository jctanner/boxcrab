use crate::diagram::*;
use std::collections::HashMap;

pub fn parse(input: &str) -> Result<DiagramGraph, Box<dyn std::error::Error>> {
    let mut graph = DiagramGraph {
        diagram_type: DiagramType::Sequence,
        direction: Direction::TD,
        nodes: HashMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        styles: HashMap::new(),
        class_defs: HashMap::new(),
        layer_spacing: None,
        node_spacing: None,
        seq_activations: Vec::new(),
    };

    let mut participant_order: u32 = 0;
    let mut alias_map: HashMap<String, String> = HashMap::new();
    let mut msg_index: usize = 0;
    let mut auto_number = false;
    let mut group_stack: Vec<GroupFrame> = Vec::new();
    let mut activations: HashMap<String, usize> = HashMap::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") || line == "sequenceDiagram" {
            continue;
        }

        if line == "autonumber" {
            auto_number = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("participant ") {
            let (id, label) = parse_participant_decl(rest.trim());
            let seq_id = format!("seq_{:03}_{}", participant_order, id);
            alias_map.insert(id, seq_id.clone());
            participant_order += 1;
            graph.nodes.insert(
                seq_id,
                NodeDef {
                    label,
                    shape: NodeShape::Rect,
                    classes: Vec::new(),
                    class_fields: Vec::new(),
                    class_methods: Vec::new(),
                    sql_columns: Vec::new(),
                    near: None,
                    tooltip: None,
                    link: None,
                },
            );
            continue;
        }

        if let Some(rest) = line.strip_prefix("actor ") {
            let (id, label) = parse_participant_decl(rest.trim());
            let seq_id = format!("seq_{:03}_{}", participant_order, id);
            alias_map.insert(id, seq_id.clone());
            participant_order += 1;
            graph.nodes.insert(
                seq_id,
                NodeDef {
                    label,
                    shape: NodeShape::Person,
                    classes: Vec::new(),
                    class_fields: Vec::new(),
                    class_methods: Vec::new(),
                    sql_columns: Vec::new(),
                    near: None,
                    tooltip: None,
                    link: None,
                },
            );
            continue;
        }

        if let Some(rest) = line.strip_prefix("activate ") {
            let id = rest.trim();
            let seq_id = resolve_participant(
                id,
                &mut alias_map,
                &mut participant_order,
                &mut graph,
            );
            activations.insert(seq_id, msg_index);
            continue;
        }

        if let Some(rest) = line.strip_prefix("deactivate ") {
            let id = rest.trim();
            let seq_id = resolve_participant(
                id,
                &mut alias_map,
                &mut participant_order,
                &mut graph,
            );
            if let Some(start) = activations.remove(&seq_id) {
                graph.seq_activations.push((seq_id, start, msg_index));
            }
            continue;
        }

        if let Some(rest) = strip_note_prefix(line) {
            let (position, participants, text) = parse_note(rest);
            let note_id = format!("__note_{msg_index}");
            let first_p = resolve_participant(
                participants.first().map(|s| s.as_str()).unwrap_or(""),
                &mut alias_map,
                &mut participant_order,
                &mut graph,
            );
            let _ = position;
            graph.nodes.insert(
                note_id.clone(),
                NodeDef {
                    label: text,
                    shape: NodeShape::Text,
                    classes: Vec::new(),
                    class_fields: Vec::new(),
                    class_methods: Vec::new(),
                    sql_columns: Vec::new(),
                    near: None,
                    tooltip: Some(format!(
                        "note:{}:{}",
                        position,
                        participants.join(",")
                    )),
                    link: None,
                },
            );
            graph.edges.push(EdgeDef {
                from: first_p,
                to: note_id,
                edge_type: EdgeType::DottedLine,
                label: None,
                src_arrowhead: None,
                dst_arrowhead: None,
                style: StyleProps::default(),
            });
            msg_index += 1;
            continue;
        }

        if let Some(kind) = parse_group_start(line) {
            let label = line
                .splitn(2, ' ')
                .nth(1)
                .unwrap_or("")
                .trim()
                .to_string();
            group_stack.push(GroupFrame {
                kind,
                label,
                start_msg: msg_index,
                branches: Vec::new(),
            });
            continue;
        }

        if line.starts_with("else") || line.starts_with("and") {
            if let Some(frame) = group_stack.last_mut() {
                let branch_label = line
                    .splitn(2, ' ')
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                frame.branches.push((msg_index, branch_label));
            }
            continue;
        }

        if line == "end" {
            if let Some(frame) = group_stack.pop() {
                let title = format!("{} {}", frame.kind, frame.label);
                graph.subgraphs.push(SubgraphDef {
                    title,
                    node_ids: Vec::new(),
                    grid_rows: Some(frame.start_msg),
                    grid_columns: Some(msg_index),
                    grid_gap: None,
                    branches: frame.branches,
                });
            }
            continue;
        }

        if let Some((from, to, arrow, label, activate_target, deactivate_target)) =
            try_parse_message(line)
        {
            let from_id = resolve_participant(
                &from,
                &mut alias_map,
                &mut participant_order,
                &mut graph,
            );
            let to_id = resolve_participant(
                &to,
                &mut alias_map,
                &mut participant_order,
                &mut graph,
            );

            let (edge_type, dst_arrowhead) = arrow_to_edge_type(&arrow);

            let final_label = if auto_number {
                let num = msg_index + 1;
                match label {
                    Some(l) => Some(format!("{num}. {l}")),
                    None => Some(format!("{num}.")),
                }
            } else {
                label
            };

            graph.edges.push(EdgeDef {
                from: from_id.clone(),
                to: to_id.clone(),
                edge_type,
                label: final_label,
                src_arrowhead: None,
                dst_arrowhead,
                style: StyleProps::default(),
            });

            if activate_target {
                activations.insert(to_id.clone(), msg_index);
            }
            if deactivate_target {
                if let Some(start) = activations.remove(&to_id) {
                    graph.seq_activations.push((to_id, start, msg_index + 1));
                }
            }

            msg_index += 1;
        }
    }

    for (pid, start) in activations {
        graph
            .seq_activations
            .push((pid, start, msg_index));
    }

    Ok(graph)
}

#[allow(dead_code)]
struct GroupFrame {
    kind: String,
    label: String,
    start_msg: usize,
    branches: Vec<(usize, String)>,
}

fn parse_participant_decl(s: &str) -> (String, String) {
    if let Some(idx) = s.find(" as ") {
        let id = s[..idx].trim().to_string();
        let label = s[idx + 4..].trim().to_string();
        (id, label)
    } else {
        (s.to_string(), s.to_string())
    }
}

fn resolve_participant(
    id: &str,
    alias_map: &mut HashMap<String, String>,
    order: &mut u32,
    graph: &mut DiagramGraph,
) -> String {
    if let Some(seq_id) = alias_map.get(id) {
        return seq_id.clone();
    }
    let seq_id = format!("seq_{:03}_{}", *order, id);
    alias_map.insert(id.to_string(), seq_id.clone());
    *order += 1;
    graph.nodes.insert(
        seq_id.clone(),
        NodeDef {
            label: id.to_string(),
            shape: NodeShape::Rect,
            classes: Vec::new(),
            class_fields: Vec::new(),
            class_methods: Vec::new(),
            sql_columns: Vec::new(),
            near: None,
            tooltip: None,
            link: None,
        },
    );
    seq_id
}

fn strip_note_prefix(line: &str) -> Option<&str> {
    let lower = line.to_lowercase();
    if lower.starts_with("note ") {
        Some(&line[5..])
    } else {
        None
    }
}

fn parse_note(rest: &str) -> (String, Vec<String>, String) {
    let lower = rest.to_lowercase();
    if lower.starts_with("left of ") {
        let after = &rest[8..];
        let (participants, text) = split_note_content(after);
        ("left".to_string(), participants, text)
    } else if lower.starts_with("right of ") {
        let after = &rest[9..];
        let (participants, text) = split_note_content(after);
        ("right".to_string(), participants, text)
    } else if lower.starts_with("over ") {
        let after = &rest[5..];
        let (participants, text) = split_note_content(after);
        ("over".to_string(), participants, text)
    } else {
        ("over".to_string(), Vec::new(), rest.to_string())
    }
}

fn split_note_content(s: &str) -> (Vec<String>, String) {
    if let Some(idx) = s.find(':') {
        let parts_str = s[..idx].trim();
        let text = s[idx + 1..].trim().to_string();
        let participants: Vec<String> = parts_str
            .split(',')
            .map(|p| p.trim().to_string())
            .collect();
        (participants, text)
    } else {
        (Vec::new(), s.trim().to_string())
    }
}

fn parse_group_start(line: &str) -> Option<String> {
    let keywords = ["alt", "loop", "opt", "par", "critical", "break", "rect"];
    for kw in &keywords {
        if line.starts_with(kw) && (line.len() == kw.len() || line.as_bytes()[kw.len()] == b' ') {
            return Some(kw.to_string());
        }
    }
    None
}

const ARROWS: &[(&str, &str)] = &[
    ("-->>", "dashed_arrow"),
    ("->>", "solid_arrow"),
    ("-->", "dashed_open"),
    ("->", "solid_open"),
    ("--x", "dashed_cross"),
    ("-x", "solid_cross"),
    ("--)", "dashed_async"),
    ("-)", "solid_async"),
];

fn try_parse_message(
    line: &str,
) -> Option<(String, String, String, Option<String>, bool, bool)> {
    for (arrow_str, _) in ARROWS {
        if let Some(idx) = line.find(arrow_str) {
            let from = line[..idx].trim().to_string();
            if from.is_empty() {
                continue;
            }
            let after = &line[idx + arrow_str.len()..];
            let (mut to, label) = if let Some(colon_idx) = after.find(':') {
                let t = after[..colon_idx].trim().to_string();
                let l = after[colon_idx + 1..].trim().to_string();
                (t, if l.is_empty() { None } else { Some(l) })
            } else {
                (after.trim().to_string(), None)
            };

            let mut activate = false;
            let mut deactivate = false;
            if to.starts_with('+') {
                activate = true;
                to = to[1..].trim().to_string();
            } else if to.starts_with('-') {
                deactivate = true;
                to = to[1..].trim().to_string();
            }

            if to.is_empty() {
                continue;
            }

            return Some((from, to, arrow_str.to_string(), label, activate, deactivate));
        }
    }
    None
}

fn arrow_to_edge_type(arrow: &str) -> (EdgeType, Option<ArrowheadType>) {
    match arrow {
        "->>" => (EdgeType::Arrow, None),
        "-->>" => (EdgeType::DottedArrow, None),
        "->" => (EdgeType::Line, None),
        "-->" => (EdgeType::DottedLine, None),
        "-x" => (EdgeType::Arrow, Some(ArrowheadType::Cross)),
        "--x" => (EdgeType::DottedArrow, Some(ArrowheadType::Cross)),
        "-)" => (EdgeType::Arrow, Some(ArrowheadType::Arrow)),
        "--)" => (EdgeType::DottedArrow, Some(ArrowheadType::Arrow)),
        _ => (EdgeType::Arrow, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_sequence() {
        let input = "sequenceDiagram\n    participant A as Alice\n    participant B as Bob\n    A->>B: Hello\n    B-->>A: Hi back\n";
        let graph = parse(input).unwrap();
        assert_eq!(graph.diagram_type, DiagramType::Sequence);
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 2);

        let alice_node = graph.nodes.values().find(|n| n.label == "Alice").unwrap();
        assert_eq!(alice_node.shape, NodeShape::Rect);

        assert_eq!(graph.edges[0].label, Some("Hello".to_string()));
        assert_eq!(graph.edges[0].edge_type, EdgeType::Arrow);
        assert_eq!(graph.edges[1].edge_type, EdgeType::DottedArrow);
    }

    #[test]
    fn test_actor() {
        let input = "sequenceDiagram\n    actor U as User\n    participant S as System\n    U->>S: Request\n";
        let graph = parse(input).unwrap();
        let user_node = graph.nodes.values().find(|n| n.label == "User").unwrap();
        assert_eq!(user_node.shape, NodeShape::Person);
    }

    #[test]
    fn test_autonumber() {
        let input = "sequenceDiagram\n    autonumber\n    A->>B: Hello\n    B->>A: World\n";
        let graph = parse(input).unwrap();
        assert_eq!(graph.edges[0].label, Some("1. Hello".to_string()));
        assert_eq!(graph.edges[1].label, Some("2. World".to_string()));
    }

    #[test]
    fn test_implicit_participants() {
        let input = "sequenceDiagram\n    Alice->>Bob: Hi\n";
        let graph = parse(input).unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.values().any(|n| n.label == "Alice"));
        assert!(graph.nodes.values().any(|n| n.label == "Bob"));
    }

    #[test]
    fn test_activations() {
        let input = "sequenceDiagram\n    A->>B: req\n    activate B\n    B-->>A: resp\n    deactivate B\n";
        let graph = parse(input).unwrap();
        assert_eq!(graph.seq_activations.len(), 1);
        let (ref pid, start, end) = graph.seq_activations[0];
        assert!(pid.contains("B"));
        assert_eq!(start, 1);
        assert_eq!(end, 2);
    }

    #[test]
    fn test_activation_shorthand() {
        let input = "sequenceDiagram\n    A->>+B: req\n    B-->>-A: resp\n";
        let graph = parse(input).unwrap();
        assert_eq!(graph.seq_activations.len(), 1);
    }

    #[test]
    fn test_groups() {
        let input = "sequenceDiagram\n    A->>B: req\n    alt success\n        B->>A: ok\n    else failure\n        B->>A: err\n    end\n";
        let graph = parse(input).unwrap();
        assert_eq!(graph.subgraphs.len(), 1);
        assert!(graph.subgraphs[0].title.starts_with("alt"));
        assert_eq!(graph.subgraphs[0].branches.len(), 1);
        assert_eq!(graph.subgraphs[0].branches[0].1, "failure");
    }

    #[test]
    fn test_notes() {
        let input = "sequenceDiagram\n    participant A\n    Note right of A: Important\n";
        let graph = parse(input).unwrap();
        let note = graph.nodes.values().find(|n| n.label == "Important");
        assert!(note.is_some());
    }

    #[test]
    fn test_arrow_types() {
        let input = "sequenceDiagram\n    A->>B: solid arrow\n    A-->>B: dashed arrow\n    A->B: solid line\n    A-->B: dashed line\n    A-xB: cross\n    A-)B: async\n";
        let graph = parse(input).unwrap();
        assert_eq!(graph.edges.len(), 6);
        assert_eq!(graph.edges[0].edge_type, EdgeType::Arrow);
        assert_eq!(graph.edges[1].edge_type, EdgeType::DottedArrow);
        assert_eq!(graph.edges[2].edge_type, EdgeType::Line);
        assert_eq!(graph.edges[3].edge_type, EdgeType::DottedLine);
        assert_eq!(graph.edges[4].dst_arrowhead, Some(ArrowheadType::Cross));
    }
}
