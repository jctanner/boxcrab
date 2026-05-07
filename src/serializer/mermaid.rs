use crate::diagram::*;

pub fn serialize(graph: &DiagramGraph) -> String {
    let mut out = String::new();

    let dir = match graph.direction {
        Direction::TD => "TD",
        Direction::TB => "TB",
        Direction::LR => "LR",
        Direction::RL => "RL",
        Direction::BT => "BT",
    };
    out.push_str(&format!("flowchart {dir}\n"));

    let mut node_ids: Vec<&String> = graph.nodes.keys().collect();
    node_ids.sort();

    for id in &node_ids {
        let node = &graph.nodes[*id];
        let label = &node.label;
        let (open, close) = shape_brackets(node.shape);
        if label == *id {
            out.push_str(&format!("    {id}{open}{label}{close}\n"));
        } else {
            out.push_str(&format!("    {id}{open}{label}{close}\n"));
        }
    }

    if !graph.nodes.is_empty() && !graph.edges.is_empty() {
        out.push('\n');
    }

    for edge in &graph.edges {
        let arrow = edge_arrow(edge.edge_type);
        match &edge.label {
            Some(lbl) => out.push_str(&format!("    {} {}|{lbl}| {}\n", edge.from, arrow, edge.to)),
            None => out.push_str(&format!("    {} {} {}\n", edge.from, arrow, edge.to)),
        }
    }

    for sg in &graph.subgraphs {
        out.push_str(&format!("\n    subgraph {}\n", sg.title));
        for nid in &sg.node_ids {
            out.push_str(&format!("        {nid}\n"));
        }
        out.push_str("    end\n");
    }

    for (id, style) in &graph.styles {
        let mut props = Vec::new();
        if let Some(fill) = style.fill {
            props.push(format!("fill:#{:02x}{:02x}{:02x}", fill[0], fill[1], fill[2]));
        }
        if let Some(stroke) = style.stroke {
            props.push(format!("stroke:#{:02x}{:02x}{:02x}", stroke[0], stroke[1], stroke[2]));
        }
        if let Some(color) = style.color {
            props.push(format!("color:#{:02x}{:02x}{:02x}", color[0], color[1], color[2]));
        }
        if let Some(sw) = style.stroke_width {
            props.push(format!("stroke-width:{sw}px"));
        }
        if !props.is_empty() {
            out.push_str(&format!("    style {} {}\n", id, props.join(",")));
        }
    }

    out
}

fn shape_brackets(shape: NodeShape) -> (&'static str, &'static str) {
    match shape {
        NodeShape::Rect => ("[", "]"),
        NodeShape::Rounded => ("(", ")"),
        NodeShape::Diamond => ("{", "}"),
        NodeShape::Circle => ("((", "))"),
        NodeShape::Hexagon => ("{{", "}}"),
        NodeShape::Parallelogram => ("[/", "/]"),
        NodeShape::Stadium => ("([", "])"),
        NodeShape::Cylinder => ("[(", ")]"),
        NodeShape::Subroutine => ("[[", "]]"),
        NodeShape::DoubleCircle => ("(((", ")))"),
        NodeShape::Trapezoid => ("[/", "\\]"),
        NodeShape::TrapezoidAlt => ("[\\", "/]"),
        NodeShape::Flag => (">", "]"),
        _ => ("[", "]"),
    }
}

fn edge_arrow(edge_type: EdgeType) -> &'static str {
    match edge_type {
        EdgeType::Arrow => "-->",
        EdgeType::Line => "---",
        EdgeType::DottedArrow => "-.->",
        EdgeType::DottedLine => "-.-",
        EdgeType::ThickArrow => "==>",
        EdgeType::ThickLine => "===",
        EdgeType::BidiArrow => "<-->",
        EdgeType::BidiDottedArrow => "<-.->",
        EdgeType::BidiThickArrow => "<==>",
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
        assert_eq!(out, "flowchart TD\n");
    }

    #[test]
    fn test_serialize_nodes_and_edges() {
        let mut g = empty_graph();
        g.nodes.insert("A".into(), node("Start", NodeShape::Rounded));
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
        assert!(out.contains("flowchart TD"));
        assert!(out.contains("A(Start)"));
        assert!(out.contains("B[End]"));
        assert!(out.contains("A -->|go| B"));
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
            edge_type: EdgeType::DottedArrow,
            label: None,
            src_arrowhead: None,
            dst_arrowhead: None,
            style: StyleProps::default(),
        });

        let text = serialize(&g);
        let parsed = crate::parser::mermaid::parse(&text).unwrap();
        assert_eq!(parsed.direction, Direction::LR);
        assert_eq!(parsed.nodes["x"].shape, NodeShape::Diamond);
        assert_eq!(parsed.nodes["y"].shape, NodeShape::Hexagon);
        assert_eq!(parsed.edges.len(), 1);
        assert_eq!(parsed.edges[0].edge_type, EdgeType::DottedArrow);
    }
}
