use crate::diagram::*;

pub fn serialize(graph: &DiagramGraph) -> String {
    if graph.diagram_type == DiagramType::Sequence {
        return serialize_sequence(graph);
    }
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

    let mut class_names: Vec<&String> = graph.class_defs.keys().collect();
    class_names.sort();
    for name in &class_names {
        let props = style_props(&graph.class_defs[*name]);
        if !props.is_empty() {
            out.push_str(&format!("    classDef {} {}\n", name, props.join(",")));
        }
    }
    for name in &class_names {
        let members: Vec<&str> = node_ids
            .iter()
            .filter(|id| graph.nodes[**id].classes.contains(*name))
            .map(|id| id.as_str())
            .collect();
        if !members.is_empty() {
            out.push_str(&format!("    class {} {}\n", members.join(","), name));
        }
    }

    let mut styled: Vec<&String> = graph.styles.keys().collect();
    styled.sort();
    for id in styled {
        let props = style_props(&graph.styles[id]);
        if !props.is_empty() {
            out.push_str(&format!("    style {} {}\n", id, props.join(",")));
        }
    }

    out
}

/// Serialize a sequence diagram (see `seq_model` for how it is stored).
fn serialize_sequence(g: &DiagramGraph) -> String {
    use crate::seq_model::{arrow_token, is_note, name_of, note_info, participant_ids};

    let mut out = String::from("sequenceDiagram\n");
    for id in participant_ids(g) {
        let def = &g.nodes[&id];
        let name = name_of(&id);
        let kw = if def.shape == NodeShape::Person { "actor" } else { "participant" };
        if def.label == name {
            out.push_str(&format!("    {kw} {name}\n"));
        } else {
            out.push_str(&format!("    {kw} {name} as {}\n", def.label));
        }
    }

    let n = g.edges.len();
    let span = |a: &(String, usize, usize)| (a.1.min(n), a.2.min(n));
    let group_span = |sg: &SubgraphDef| {
        let start = sg.grid_rows.unwrap_or(0).min(n);
        let end = sg.grid_columns.unwrap_or(n).min(n).max(start);
        (start, end)
    };

    let mut depth = 0usize;
    let indent = |depth: usize| " ".repeat(4 + depth * 2);

    for slot in 0..=n {
        for a in &g.seq_activations {
            let (start, end) = span(a);
            if end == slot && end > start && g.nodes.contains_key(&a.0) {
                out.push_str(&format!("{}deactivate {}\n", indent(depth), name_of(&a.0)));
            }
        }

        let mut ending: Vec<&SubgraphDef> = g
            .subgraphs
            .iter()
            .filter(|sg| group_span(sg).1 == slot)
            .collect();
        ending.sort_by_key(|sg| std::cmp::Reverse(group_span(sg).0));
        for _ in ending {
            depth = depth.saturating_sub(1);
            out.push_str(&format!("{}end\n", indent(depth)));
        }

        for sg in &g.subgraphs {
            for (idx, label) in &sg.branches {
                if (*idx).min(n) == slot {
                    let kw = if sg.title.starts_with("par") { "and" } else { "else" };
                    out.push_str(&format!(
                        "{}{} {}\n",
                        indent(depth.saturating_sub(1)),
                        kw,
                        label
                    ));
                }
            }
        }

        let mut starting: Vec<&SubgraphDef> = g
            .subgraphs
            .iter()
            .filter(|sg| group_span(sg).0 == slot)
            .collect();
        starting.sort_by_key(|sg| std::cmp::Reverse(group_span(sg).1));
        for sg in starting {
            out.push_str(&format!("{}{}\n", indent(depth), sg.title.trim()));
            depth += 1;
        }

        for a in &g.seq_activations {
            let (start, end) = span(a);
            if start == slot && end > start && g.nodes.contains_key(&a.0) {
                out.push_str(&format!("{}activate {}\n", indent(depth), name_of(&a.0)));
            }
        }

        if slot == n {
            break;
        }
        let e = &g.edges[slot];
        if is_note(&e.to) {
            let Some(def) = g.nodes.get(&e.to) else { continue };
            let info = note_info(def);
            let mut names: Vec<String> = info
                .names
                .into_iter()
                .filter(|nm| participant_ids(g).iter().any(|p| name_of(p) == nm))
                .collect();
            if names.is_empty() {
                names.push(name_of(&e.from).to_string());
            }
            let pos = match info.position.as_str() {
                "left" => "left of",
                "over" => "over",
                _ => "right of",
            };
            out.push_str(&format!(
                "{}Note {} {}: {}\n",
                indent(depth),
                pos,
                names.join(","),
                def.label
            ));
        } else {
            let line = format!("{}{}{}", name_of(&e.from), arrow_token(e), name_of(&e.to));
            match &e.label {
                Some(l) => out.push_str(&format!("{}{}: {}\n", indent(depth), line, l)),
                None => out.push_str(&format!("{}{}\n", indent(depth), line)),
            }
        }
    }
    out
}

fn style_props(style: &StyleProps) -> Vec<String> {
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
    props
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

    #[test]
    fn class_defs_round_trip() {
        let src = "flowchart TD\n    A[One]\n    B[Two]\n    A --> B\n    classDef hot fill:#ff0000,stroke:#000000\n    class A,B hot\n";
        let g = crate::parser::parse(src, crate::parser::DiagramFormat::Mermaid, 0, None).unwrap();
        let out = serialize(&g);
        assert!(out.contains("classDef hot fill:#ff0000,stroke:#000000"), "{out}");
        assert!(out.contains("class A,B hot"), "{out}");
        let g2 = crate::parser::parse(&out, crate::parser::DiagramFormat::Mermaid, 0, None).unwrap();
        assert_eq!(g2.nodes["A"].classes, vec!["hot".to_string()]);
        assert_eq!(g2.class_defs["hot"].fill, Some([255, 0, 0]));
    }

    fn seq_summary(g: &DiagramGraph) -> String {
        let mut nodes: Vec<_> = g
            .nodes
            .iter()
            .map(|(k, n)| format!("{k}|{}|{:?}|{:?}", n.label, n.shape, n.tooltip))
            .collect();
        nodes.sort();
        let edges: Vec<_> = g
            .edges
            .iter()
            .map(|e| format!("{}>{}|{:?}|{:?}|{:?}", e.from, e.to, e.edge_type, e.label, e.dst_arrowhead))
            .collect();
        let mut acts: Vec<_> = g.seq_activations.iter().map(|a| format!("{a:?}")).collect();
        acts.sort();
        let groups: Vec<_> = g
            .subgraphs
            .iter()
            .map(|s| format!("{}|{:?}|{:?}|{:?}", s.title, s.grid_rows, s.grid_columns, s.branches))
            .collect();
        let text = format!("{nodes:#?}\n{edges:#?}\n{acts:#?}\n{groups:#?}");
        // Note ids are numbered differently by the parser; ignore the number.
        let mut norm = String::new();
        let mut rest = text.as_str();
        while let Some(i) = rest.find("__note_") {
            norm.push_str(&rest[..i + 7]);
            rest = rest[i + 7..].trim_start_matches(|c: char| c.is_ascii_digit());
        }
        norm.push_str(rest);
        norm
    }

    #[test]
    fn sequence_round_trip() {
        let src = "sequenceDiagram
    participant A as Alice
    actor B as Bob
    participant C
    A->>+B: hello
    Note right of A: thinking
    B-->>-A: hi
    alt ok
        A->>C: do
        Note over A,C: both
    else bad
        C-->>A: err
    end
    loop forever
        A-)B: ping
        A->>A: self
        A-xC: boom
    end
    par x
        A->>B: one
    and y
        A->>C: two
    end
    A-->B: dashed line
";
        let g1 = crate::parser::parse(src, crate::parser::DiagramFormat::Mermaid, 0, None).unwrap();
        assert_eq!(g1.diagram_type, DiagramType::Sequence);
        let text = serialize(&g1);
        assert!(text.starts_with("sequenceDiagram"), "{text}");
        let g2 = crate::parser::parse(&text, crate::parser::DiagramFormat::Mermaid, 0, None).unwrap();
        assert_eq!(seq_summary(&g1), seq_summary(&g2), "{text}");
        // And it is stable once serialized.
        assert_eq!(text, serialize(&g2));
    }

    #[test]
    fn sequence_edited_graph_serializes() {
        use crate::seq_model::*;
        let mut g = new_sequence_graph();
        let a = add_participant(&mut g, 0, NodeShape::Rect);
        let b = add_participant(&mut g, 1, NodeShape::Person);
        add_message(&mut g, 0, &a, &b, 1);
        add_note(&mut g, 1, &b, "left");
        let text = serialize(&g);
        assert!(text.contains("participant P1 as Participant 1"), "{text}");
        assert!(text.contains("actor P2 as Actor 2"), "{text}");
        assert!(text.contains("P1-->>P2: Message"), "{text}");
        assert!(text.contains("Note left of P2: Note"), "{text}");
        let g2 = crate::parser::parse(&text, crate::parser::DiagramFormat::Mermaid, 0, None).unwrap();
        assert_eq!(seq_summary(&g), seq_summary(&g2));
    }
}
