use crate::diagram::*;

pub fn serialize(graph: &DiagramGraph) -> String {
    let mut out = String::new();

    let dir = match graph.direction {
        Direction::LR => "right",
        Direction::RL => "left",
        Direction::BT => "up",
        _ => "down",
    };
    out.push_str(&format!("direction: {dir}\n\n"));

    let mut node_ids: Vec<&String> = graph.nodes.keys().collect();
    node_ids.sort();

    for id in &node_ids {
        let node = &graph.nodes[*id];
        let style = graph.styles.get(*id);
        let shape_name = node_shape_name(node.shape);
        let has_shape = node.shape != NodeShape::Rect;
        let has_style = style.map_or(false, |s| {
            s.fill.is_some() || s.stroke.is_some() || s.color.is_some() || s.stroke_width.is_some()
        });

        if !has_shape && !has_style {
            out.push_str(&format!("{id}: \"{}\"\n", escape_d2(&node.label)));
        } else {
            out.push_str(&format!("{id}: \"{}\" {{\n", escape_d2(&node.label)));
            if has_shape {
                out.push_str(&format!("  shape: {shape_name}\n"));
            }
            if let Some(s) = style {
                let style_lines = style_block(s);
                if !style_lines.is_empty() {
                    out.push_str("  style: {\n");
                    for line in &style_lines {
                        out.push_str(&format!("    {line}\n"));
                    }
                    out.push_str("  }\n");
                }
            }
            out.push_str("}\n");
        }
    }

    if !graph.nodes.is_empty() && !graph.edges.is_empty() {
        out.push('\n');
    }

    for edge in &graph.edges {
        let arrow = edge_arrow(edge.edge_type);
        match &edge.label {
            Some(lbl) => out.push_str(&format!(
                "{} {} {}: \"{}\"\n",
                edge.from,
                arrow,
                edge.to,
                escape_d2(lbl)
            )),
            None => out.push_str(&format!("{} {} {}\n", edge.from, arrow, edge.to)),
        }
    }

    for sg in &graph.subgraphs {
        out.push_str(&format!("\n{}: {{\n", escape_d2_id(&sg.title)));
        for nid in &sg.node_ids {
            out.push_str(&format!("  {nid}\n"));
        }
        out.push_str("}\n");
    }

    out
}

fn node_shape_name(shape: NodeShape) -> &'static str {
    match shape {
        NodeShape::Rect => "rectangle",
        NodeShape::Rounded => "rectangle",
        NodeShape::Diamond => "diamond",
        NodeShape::Circle => "circle",
        NodeShape::Oval => "oval",
        NodeShape::Hexagon => "hexagon",
        NodeShape::Parallelogram => "parallelogram",
        NodeShape::Cylinder => "cylinder",
        NodeShape::Cloud => "cloud",
        NodeShape::Page => "page",
        NodeShape::Document => "document",
        NodeShape::Person => "person",
        NodeShape::Queue => "queue",
        NodeShape::Package => "package",
        NodeShape::Step => "step",
        NodeShape::Callout => "callout",
        NodeShape::StoredData => "stored_data",
        NodeShape::Text => "text",
        NodeShape::Class => "class",
        NodeShape::SqlTable => "sql_table",
        _ => "rectangle",
    }
}

fn edge_arrow(edge_type: EdgeType) -> &'static str {
    match edge_type {
        EdgeType::Arrow | EdgeType::DottedArrow | EdgeType::ThickArrow => "->",
        EdgeType::Line | EdgeType::DottedLine | EdgeType::ThickLine => "--",
        EdgeType::BidiArrow | EdgeType::BidiDottedArrow | EdgeType::BidiThickArrow => "<->",
    }
}

fn style_block(style: &StyleProps) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(fill) = style.fill {
        lines.push(format!(
            "fill: \"#{:02x}{:02x}{:02x}\"",
            fill[0], fill[1], fill[2]
        ));
    }
    if let Some(stroke) = style.stroke {
        lines.push(format!(
            "stroke: \"#{:02x}{:02x}{:02x}\"",
            stroke[0], stroke[1], stroke[2]
        ));
    }
    if let Some(color) = style.color {
        lines.push(format!(
            "font-color: \"#{:02x}{:02x}{:02x}\"",
            color[0], color[1], color[2]
        ));
    }
    if let Some(sw) = style.stroke_width {
        lines.push(format!("stroke-width: {sw}"));
    }
    lines
}

fn escape_d2(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_d2_id(s: &str) -> String {
    if s.contains(' ') || s.contains('.') || s.contains('-') {
        format!("\"{}\"", escape_d2(s))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_graph() -> DiagramGraph {
        DiagramGraph {
            diagram_type: DiagramType::Flowchart,
            direction: Direction::TD,
            nodes: HashMap::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            styles: HashMap::new(),
            class_defs: HashMap::new(),
            layer_spacing: None,
            node_spacing: None,
            seq_activations: Vec::new(),
        }
    }

    fn node(label: &str, shape: NodeShape) -> NodeDef {
        NodeDef {
            label: label.to_string(),
            shape,
            classes: Vec::new(),
            class_fields: Vec::new(),
            class_methods: Vec::new(),
            sql_columns: Vec::new(),
            near: None,
            tooltip: None,
            link: None,
        }
    }

    #[test]
    fn test_serialize_empty() {
        let g = empty_graph();
        let out = serialize(&g);
        assert_eq!(out, "direction: down\n\n");
    }

    #[test]
    fn test_serialize_nodes_and_edges() {
        let mut g = empty_graph();
        g.nodes.insert("A".into(), node("Start", NodeShape::Circle));
        g.nodes.insert("B".into(), node("End", NodeShape::Rect));
        g.edges.push(EdgeDef {
            from: "A".into(),
            to: "B".into(),
            edge_type: EdgeType::Arrow,
            label: Some("go".into()),
            src_arrowhead: None,
            dst_arrowhead: None,
            style: StyleProps::default(),
        });

        let out = serialize(&g);
        assert!(out.contains("direction: down"));
        assert!(out.contains("A: \"Start\" {\n  shape: circle\n}"));
        assert!(out.contains("B: \"End\""));
        assert!(out.contains("A -> B: \"go\""));
    }

    #[test]
    fn test_serialize_with_styles() {
        let mut g = empty_graph();
        g.nodes.insert("X".into(), node("Box", NodeShape::Rect));
        g.styles.insert(
            "X".into(),
            StyleProps {
                fill: Some([255, 0, 0]),
                stroke: Some([0, 0, 255]),
                ..StyleProps::default()
            },
        );

        let out = serialize(&g);
        assert!(out.contains("fill: \"#ff0000\""));
        assert!(out.contains("stroke: \"#0000ff\""));
    }

    #[test]
    fn test_roundtrip() {
        let mut g = empty_graph();
        g.direction = Direction::LR;
        g.nodes.insert("x".into(), node("x", NodeShape::Diamond));
        g.nodes.insert("y".into(), node("y", NodeShape::Hexagon));
        g.edges.push(EdgeDef {
            from: "x".into(),
            to: "y".into(),
            edge_type: EdgeType::Arrow,
            label: None,
            src_arrowhead: None,
            dst_arrowhead: None,
            style: StyleProps::default(),
        });

        let text = serialize(&g);
        let parsed = crate::parser::d2::parse(&text, None).unwrap();
        assert_eq!(parsed.direction, Direction::LR);
        assert_eq!(parsed.nodes["x"].shape, NodeShape::Diamond);
        assert_eq!(parsed.nodes["y"].shape, NodeShape::Hexagon);
        assert_eq!(parsed.edges.len(), 1);
        assert_eq!(parsed.edges[0].edge_type, EdgeType::Arrow);
    }
}
