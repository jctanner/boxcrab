use crate::diagram::*;
use std::collections::HashMap;

pub fn parse(input: &str) -> Result<DiagramGraph, Box<dyn std::error::Error>> {
    let mut graph = DiagramGraph {
        direction: Direction::TD,
        nodes: HashMap::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
        styles: HashMap::new(),
        class_defs: HashMap::new(),
        layer_spacing: None,
        node_spacing: None,
    };

    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;
    let mut star_counter: usize = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("stateDiagram") {
            i += 1;
            break;
        }
        i += 1;
    }

    parse_block(&lines, &mut i, &mut graph, &mut star_counter);

    Ok(graph)
}

fn make_star_node(graph: &mut DiagramGraph, counter: &mut usize) -> String {
    let id = format!("__star_{counter}");
    *counter += 1;
    graph.nodes.entry(id.clone()).or_insert_with(|| NodeDef {
        label: String::new(),
        shape: NodeShape::DoubleCircle,
        classes: Vec::new(),
        class_fields: Vec::new(),
        class_methods: Vec::new(),
        sql_columns: Vec::new(),
        near: None,
        tooltip: None,
        link: None,
    });
    id
}

fn resolve_id(s: &str, graph: &mut DiagramGraph, counter: &mut usize) -> String {
    if s == "[*]" {
        make_star_node(graph, counter)
    } else {
        ensure_state_node(graph, s, None);
        s.to_string()
    }
}

fn parse_block(
    lines: &[&str],
    i: &mut usize,
    graph: &mut DiagramGraph,
    star_counter: &mut usize,
) {
    while *i < lines.len() {
        let trimmed = lines[*i].trim();

        if trimmed.is_empty() || trimmed.starts_with("%%") {
            *i += 1;
            continue;
        }

        if trimmed == "}" {
            *i += 1;
            break;
        }

        if trimmed.starts_with("direction ") {
            let dir = trimmed.strip_prefix("direction ").unwrap().trim();
            graph.direction = match dir {
                "LR" => Direction::LR,
                "RL" => Direction::RL,
                "BT" => Direction::BT,
                "TB" => Direction::TB,
                _ => Direction::TD,
            };
            *i += 1;
            continue;
        }

        // Composite state: state "Name" as id { or state id {
        if trimmed.starts_with("state ") && trimmed.ends_with('{') {
            let rest = trimmed.strip_prefix("state ").unwrap().trim_end_matches('{').trim();
            let (id, title) = parse_state_decl(rest);
            ensure_state_node(graph, &id, Some(&title));
            *i += 1;
            let mut inner_nodes = Vec::new();
            parse_composite_block(lines, i, graph, &mut inner_nodes, star_counter);
            graph.subgraphs.push(SubgraphDef {
                title,
                node_ids: inner_nodes,
                grid_rows: None,
                grid_columns: None,
                grid_gap: None,
            });
            continue;
        }

        // State declaration: state "Name" as id
        if trimmed.starts_with("state ") {
            let rest = trimmed.strip_prefix("state ").unwrap().trim();
            if rest.contains("<<fork>>") || rest.contains("<<join>>") {
                let id = rest.split_whitespace().next().unwrap_or(rest);
                ensure_state_node(graph, id, Some(id));
                if let Some(node) = graph.nodes.get_mut(id) {
                    node.shape = NodeShape::Rect;
                }
            } else if rest.contains("<<choice>>") {
                let id = rest.split_whitespace().next().unwrap_or(rest);
                ensure_state_node(graph, id, Some(id));
                if let Some(node) = graph.nodes.get_mut(id) {
                    node.shape = NodeShape::Diamond;
                }
            } else {
                let (id, title) = parse_state_decl(rest);
                ensure_state_node(graph, &id, Some(&title));
            }
            *i += 1;
            continue;
        }

        // Transition: s1 --> s2 : label
        if let Some((from, to, label)) = try_parse_transition(trimmed) {
            let from_id = resolve_id(&from, graph, star_counter);
            let to_id = resolve_id(&to, graph, star_counter);
            graph.edges.push(EdgeDef {
                from: from_id,
                to: to_id,
                edge_type: EdgeType::Arrow,
                label,
                src_arrowhead: None,
                dst_arrowhead: None,
                style: StyleProps::default(),
            });
            *i += 1;
            continue;
        }

        // Simple state reference: s1 : Description
        if let Some((id, desc)) = trimmed.split_once(':') {
            let id = id.trim();
            let desc = desc.trim();
            if !id.is_empty() && !id.contains(' ') {
                ensure_state_node(graph, id, Some(desc));
                *i += 1;
                continue;
            }
        }

        // note lines — skip
        if trimmed.starts_with("note ") {
            *i += 1;
            while *i < lines.len() {
                let t = lines[*i].trim();
                if t == "end note" || t.starts_with("end note") {
                    *i += 1;
                    break;
                }
                *i += 1;
            }
            continue;
        }

        *i += 1;
    }
}

fn parse_composite_block(
    lines: &[&str],
    i: &mut usize,
    graph: &mut DiagramGraph,
    inner_nodes: &mut Vec<String>,
    star_counter: &mut usize,
) {
    while *i < lines.len() {
        let trimmed = lines[*i].trim();

        if trimmed.is_empty() || trimmed.starts_with("%%") {
            *i += 1;
            continue;
        }

        if trimmed == "}" {
            *i += 1;
            return;
        }

        if trimmed.starts_with("state ") && trimmed.ends_with('{') {
            let rest = trimmed.strip_prefix("state ").unwrap().trim_end_matches('{').trim();
            let (id, title) = parse_state_decl(rest);
            ensure_state_node(graph, &id, Some(&title));
            inner_nodes.push(id.clone());
            *i += 1;
            let mut sub_inner = Vec::new();
            parse_composite_block(lines, i, graph, &mut sub_inner, star_counter);
            graph.subgraphs.push(SubgraphDef {
                title,
                node_ids: sub_inner,
                grid_rows: None,
                grid_columns: None,
                grid_gap: None,
            });
            continue;
        }

        if let Some((from, to, label)) = try_parse_transition(trimmed) {
            let from_id = resolve_id(&from, graph, star_counter);
            let to_id = resolve_id(&to, graph, star_counter);
            inner_nodes.push(from_id.clone());
            inner_nodes.push(to_id.clone());
            graph.edges.push(EdgeDef {
                from: from_id,
                to: to_id,
                edge_type: EdgeType::Arrow,
                label,
                src_arrowhead: None,
                dst_arrowhead: None,
                style: StyleProps::default(),
            });
            *i += 1;
            continue;
        }

        if trimmed.starts_with("state ") {
            let rest = trimmed.strip_prefix("state ").unwrap().trim();
            let (id, title) = parse_state_decl(rest);
            ensure_state_node(graph, &id, Some(&title));
            inner_nodes.push(id);
            *i += 1;
            continue;
        }

        if let Some((id, desc)) = trimmed.split_once(':') {
            let id = id.trim();
            let desc = desc.trim();
            if !id.is_empty() && !id.contains(' ') {
                ensure_state_node(graph, id, Some(desc));
                inner_nodes.push(id.to_string());
                *i += 1;
                continue;
            }
        }

        *i += 1;
    }
}

fn parse_state_decl(rest: &str) -> (String, String) {
    if rest.starts_with('"') {
        if let Some(end_quote) = rest[1..].find('"') {
            let display = rest[1..1 + end_quote].to_string();
            let after = rest[2 + end_quote..].trim();
            if let Some(id) = after.strip_prefix("as").map(|s| s.trim()) {
                if !id.is_empty() {
                    return (id.to_string(), display);
                }
            }
            return (display.clone(), display);
        }
    }
    let id = rest.split_whitespace().next().unwrap_or(rest).to_string();
    (id.clone(), id)
}

fn ensure_state_node(graph: &mut DiagramGraph, id: &str, label: Option<&str>) {
    let entry = graph.nodes.entry(id.to_string());
    entry
        .and_modify(|n| {
            if let Some(l) = label {
                if !l.is_empty() {
                    n.label = l.to_string();
                }
            }
        })
        .or_insert_with(|| NodeDef {
            label: label.unwrap_or(id).to_string(),
            shape: NodeShape::Rounded,
            classes: Vec::new(),
            class_fields: Vec::new(),
            class_methods: Vec::new(),
            sql_columns: Vec::new(),
            near: None,
            tooltip: None,
            link: None,
        });
}

fn try_parse_transition(line: &str) -> Option<(String, String, Option<String>)> {
    let (line_no_label, label) = if let Some(idx) = line.find(':') {
        let after_arrow = &line[..idx];
        if after_arrow.contains("-->") {
            let lbl = line[idx + 1..].trim().to_string();
            (line[..idx].trim(), if lbl.is_empty() { None } else { Some(lbl) })
        } else {
            return None;
        }
    } else if line.contains("-->") {
        (line.trim(), None)
    } else {
        return None;
    };

    if let Some(idx) = line_no_label.find("-->") {
        let from = line_no_label[..idx].trim().to_string();
        let to = line_no_label[idx + 3..].trim().to_string();
        if !from.is_empty() && !to.is_empty() {
            return Some((from, to, label));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_diagram_basic() {
        let input = r#"stateDiagram-v2
    [*] --> Still
    Still --> [*]
    Still --> Moving
    Moving --> Still
    Moving --> Crash
    Crash --> [*]
"#;
        let g = parse(input).unwrap();
        assert!(g.nodes.contains_key("Still"));
        assert!(g.nodes.contains_key("Moving"));
        assert!(g.nodes.contains_key("Crash"));
        assert!(g.edges.len() >= 4);
        let star_nodes: Vec<_> = g.nodes.keys().filter(|k| k.starts_with("__star_")).collect();
        assert_eq!(star_nodes.len(), 3);
        for k in &star_nodes {
            assert_eq!(g.nodes[*k].shape, NodeShape::DoubleCircle);
            assert_eq!(g.nodes[*k].label, "");
        }
    }

    #[test]
    fn test_state_with_descriptions() {
        let input = r#"stateDiagram-v2
    s1 : Idle state
    s2 : Processing
    s1 --> s2 : start
"#;
        let g = parse(input).unwrap();
        assert_eq!(g.nodes["s1"].label, "Idle state");
        assert_eq!(g.nodes["s2"].label, "Processing");
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].label.as_deref(), Some("start"));
    }

    #[test]
    fn test_composite_state() {
        let input = r#"stateDiagram-v2
    state "Active" as active {
        idle --> running
        running --> idle
    }
"#;
        let g = parse(input).unwrap();
        assert!(g.nodes.contains_key("active"));
        assert!(g.nodes.contains_key("idle"));
        assert!(g.nodes.contains_key("running"));
        assert_eq!(g.subgraphs.len(), 1);
        assert_eq!(g.subgraphs[0].title, "Active");
    }
}
