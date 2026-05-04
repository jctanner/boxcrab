use crate::diagram::*;
use pest::Parser;
use pest_derive::Parser;
use std::collections::HashMap;

#[derive(Parser)]
#[grammar = "parser/mermaid/grammar.pest"]
struct MermaidParser;

pub fn parse(input: &str) -> Result<DiagramGraph, Box<dyn std::error::Error>> {
    let pairs = MermaidParser::parse(Rule::diagram, input)?;

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

    for pair in pairs {
        if pair.as_rule() == Rule::diagram {
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::graph_decl => {
                        for p in inner.into_inner() {
                            if p.as_rule() == Rule::direction {
                                graph.direction = parse_direction(p.as_str());
                            }
                        }
                    }
                    Rule::link_stmt => parse_link_stmt(inner, &mut graph),
                    Rule::node_stmt => parse_node_stmt(inner, &mut graph),
                    Rule::subgraph_stmt => parse_subgraph_stmt(inner, &mut graph),
                    Rule::style_stmt => parse_style_stmt(inner, &mut graph),
                    Rule::class_def_stmt => parse_class_def_stmt(inner, &mut graph),
                    Rule::class_stmt => parse_class_stmt(inner, &mut graph),
                    Rule::direction_stmt => {}
                    Rule::comment => {}
                    _ => {}
                }
            }
        }
    }

    Ok(graph)
}

fn parse_direction(s: &str) -> Direction {
    match s {
        "TD" => Direction::TD,
        "TB" => Direction::TB,
        "LR" => Direction::LR,
        "RL" => Direction::RL,
        "BT" => Direction::BT,
        _ => Direction::TD,
    }
}

fn clean_label(s: &str) -> String {
    let s = strip_quotes(s);
    s.replace("<br/>", "\n").replace("<br>", "\n")
}

fn ensure_node(graph: &mut DiagramGraph, id: &str, shape: Option<NodeShape>, label: Option<&str>) {
    let entry = graph.nodes.entry(id.to_string());
    entry
        .and_modify(|n| {
            if let Some(s) = shape {
                n.shape = s;
            }
            if let Some(l) = label {
                n.label = clean_label(l);
            }
        })
        .or_insert_with(|| NodeDef {
            label: label.map(clean_label).unwrap_or_else(|| id.to_string()),
            shape: shape.unwrap_or(NodeShape::Rect),
            classes: Vec::new(),
            class_fields: Vec::new(),
            class_methods: Vec::new(),
            sql_columns: Vec::new(),
            near: None,
            tooltip: None,
            link: None,
        });
}

fn parse_node_ref(
    pair: pest::iterators::Pair<Rule>,
) -> (String, Option<NodeShape>, Option<String>) {
    let mut id = String::new();
    let mut shape = None;
    let mut label = None;

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::node_id => id = p.as_str().to_string(),
            Rule::rect_shape => {
                shape = Some(NodeShape::Rect);
                for inner in p.into_inner() {
                    if inner.as_rule() == Rule::shape_text_bracket {
                        label = Some(inner.as_str().trim().to_string());
                    }
                }
            }
            Rule::rounded_shape => {
                shape = Some(NodeShape::Rounded);
                for inner in p.into_inner() {
                    if inner.as_rule() == Rule::shape_text_paren {
                        label = Some(inner.as_str().trim().to_string());
                    }
                }
            }
            Rule::diamond_shape => {
                shape = Some(NodeShape::Diamond);
                for inner in p.into_inner() {
                    if inner.as_rule() == Rule::shape_text_brace {
                        label = Some(inner.as_str().trim().to_string());
                    }
                }
            }
            Rule::circle_shape => {
                shape = Some(NodeShape::Circle);
                for inner in p.into_inner() {
                    if inner.as_rule() == Rule::shape_text_paren {
                        label = Some(inner.as_str().trim().to_string());
                    }
                }
            }
            Rule::flag_shape => {
                shape = Some(NodeShape::Flag);
                for inner in p.into_inner() {
                    if inner.as_rule() == Rule::shape_text_bracket {
                        label = Some(inner.as_str().trim().to_string());
                    }
                }
            }
            _ => {}
        }
    }

    (id, shape, label)
}

fn parse_node_stmt(pair: pest::iterators::Pair<Rule>, graph: &mut DiagramGraph) {
    for p in pair.into_inner() {
        if p.as_rule() == Rule::node_ref {
            let (id, shape, label) = parse_node_ref(p);
            ensure_node(graph, &id, shape, label.as_deref());
        }
    }
}

fn parse_node_ref_group(
    pair: pest::iterators::Pair<Rule>,
) -> Vec<(String, Option<NodeShape>, Option<String>)> {
    let mut group = Vec::new();
    for p in pair.into_inner() {
        if p.as_rule() == Rule::node_ref {
            group.push(parse_node_ref(p));
        }
    }
    group
}

fn parse_link_stmt(pair: pest::iterators::Pair<Rule>, graph: &mut DiagramGraph) {
    let mut groups: Vec<Vec<(String, Option<NodeShape>, Option<String>)>> = Vec::new();
    let mut edge_infos: Vec<(EdgeType, Option<String>)> = Vec::new();

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::node_ref_group => {
                groups.push(parse_node_ref_group(p));
            }
            Rule::edge => {
                let mut etype = EdgeType::Arrow;
                let mut elabel = None;
                for ep in p.into_inner() {
                    match ep.as_rule() {
                        Rule::edge_type => {
                            for tp in ep.into_inner() {
                                etype = match tp.as_rule() {
                                    Rule::arrow => EdgeType::Arrow,
                                    Rule::line => EdgeType::Line,
                                    Rule::dotted_arrow => EdgeType::DottedArrow,
                                    Rule::dotted_line => EdgeType::DottedLine,
                                    Rule::thick_arrow => EdgeType::ThickArrow,
                                    Rule::thick_line => EdgeType::ThickLine,
                                    Rule::bidi_arrow => EdgeType::BidiArrow,
                                    Rule::bidi_dotted_arrow => EdgeType::BidiDottedArrow,
                                    Rule::bidi_thick_arrow => EdgeType::BidiThickArrow,
                                    _ => EdgeType::Arrow,
                                };
                            }
                        }
                        Rule::edge_label => {
                            for lp in ep.into_inner() {
                                if lp.as_rule() == Rule::edge_label_text {
                                    elabel = Some(clean_label(lp.as_str().trim()));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                edge_infos.push((etype, elabel));
            }
            _ => {}
        }
    }

    for group in &groups {
        for (id, shape, label) in group {
            ensure_node(graph, id, *shape, label.as_deref());
        }
    }

    for (i, (etype, elabel)) in edge_infos.into_iter().enumerate() {
        if i + 1 < groups.len() {
            for from_node in &groups[i] {
                for to_node in &groups[i + 1] {
                    graph.edges.push(EdgeDef {
                        from: from_node.0.clone(),
                        to: to_node.0.clone(),
                        edge_type: etype,
                        label: elabel.clone(),
                        src_arrowhead: None,
                        dst_arrowhead: None,
                        style: StyleProps::default(),
                    });
                }
            }
        }
    }
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_subgraph_stmt(pair: pest::iterators::Pair<Rule>, graph: &mut DiagramGraph) {
    let mut title = String::new();
    let mut node_ids = Vec::new();

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::subgraph_id_title => {
                for inner in p.into_inner() {
                    match inner.as_rule() {
                        Rule::subgraph_display_title => {
                            for dt in inner.into_inner() {
                                if dt.as_rule() == Rule::shape_text_bracket {
                                    title = strip_quotes(dt.as_str());
                                }
                            }
                        }
                        Rule::subgraph_bare_title => {
                            title = inner.as_str().trim().to_string();
                        }
                        Rule::subgraph_id => {
                            if title.is_empty() {
                                title = inner.as_str().trim().to_string();
                            }
                        }
                        _ => {}
                    }
                }
            }
            Rule::link_stmt => {
                for inner in p.clone().into_inner() {
                    if inner.as_rule() == Rule::node_ref_group {
                        for nr in inner.into_inner() {
                            if nr.as_rule() == Rule::node_ref {
                                let (id, _, _) = parse_node_ref(nr);
                                node_ids.push(id);
                            }
                        }
                    }
                }
                parse_link_stmt(p, graph);
            }
            Rule::node_stmt => {
                for inner in p.clone().into_inner() {
                    if inner.as_rule() == Rule::node_ref {
                        let (id, _, _) = parse_node_ref(inner);
                        node_ids.push(id);
                    }
                }
                parse_node_stmt(p, graph);
            }
            Rule::subgraph_stmt => {
                collect_subgraph_node_ids(p.clone(), &mut node_ids);
                parse_subgraph_stmt(p, graph);
            }
            Rule::direction_stmt => {}
            _ => {}
        }
    }

    graph.subgraphs.push(SubgraphDef {
        title,
        node_ids,
        grid_rows: None,
        grid_columns: None,
        grid_gap: None,
    });
}

fn collect_subgraph_node_ids(pair: pest::iterators::Pair<Rule>, node_ids: &mut Vec<String>) {
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::link_stmt => {
                for inner in p.into_inner() {
                    if inner.as_rule() == Rule::node_ref_group {
                        for nr in inner.into_inner() {
                            if nr.as_rule() == Rule::node_ref {
                                let (id, _, _) = parse_node_ref(nr);
                                node_ids.push(id);
                            }
                        }
                    }
                }
            }
            Rule::node_stmt => {
                for inner in p.into_inner() {
                    if inner.as_rule() == Rule::node_ref {
                        let (id, _, _) = parse_node_ref(inner);
                        node_ids.push(id);
                    }
                }
            }
            Rule::subgraph_stmt => {
                collect_subgraph_node_ids(p, node_ids);
            }
            _ => {}
        }
    }
}

fn parse_color(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() == 3 {
        let r = u8::from_str_radix(&s[0..1], 16).ok()? * 17;
        let g = u8::from_str_radix(&s[1..2], 16).ok()? * 17;
        let b = u8::from_str_radix(&s[2..3], 16).ok()? * 17;
        Some([r, g, b])
    } else if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some([r, g, b])
    } else {
        None
    }
}

fn parse_style_props_str(s: &str) -> StyleProps {
    let mut props = StyleProps::default();
    for part in s.split(',') {
        let part = part.trim();
        if let Some((key, val)) = part.split_once(':') {
            let key = key.trim();
            let val = val.trim();
            match key {
                "fill" => props.fill = parse_color(val),
                "stroke" => props.stroke = parse_color(val),
                "stroke-width" => {
                    props.stroke_width = val.trim_end_matches("px").parse().ok();
                }
                "color" => props.color = parse_color(val),
                _ => {}
            }
        }
    }
    props
}

fn parse_style_stmt(pair: pest::iterators::Pair<Rule>, graph: &mut DiagramGraph) {
    let mut node_id = String::new();
    let mut props_str = String::new();

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::node_id => node_id = p.as_str().to_string(),
            Rule::style_props => props_str = p.as_str().to_string(),
            _ => {}
        }
    }

    graph
        .styles
        .insert(node_id, parse_style_props_str(&props_str));
}

fn parse_class_def_stmt(pair: pest::iterators::Pair<Rule>, graph: &mut DiagramGraph) {
    let mut class_name = String::new();
    let mut props_str = String::new();

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::class_name => class_name = p.as_str().to_string(),
            Rule::style_props => props_str = p.as_str().to_string(),
            _ => {}
        }
    }

    graph
        .class_defs
        .insert(class_name, parse_style_props_str(&props_str));
}

fn parse_class_stmt(pair: pest::iterators::Pair<Rule>, graph: &mut DiagramGraph) {
    let mut node_ids = Vec::new();
    let mut class_name = String::new();

    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::node_id_list => {
                for inner in p.into_inner() {
                    if inner.as_rule() == Rule::node_id {
                        node_ids.push(inner.as_str().to_string());
                    }
                }
            }
            Rule::class_name => class_name = p.as_str().to_string(),
            _ => {}
        }
    }

    for nid in node_ids {
        if let Some(node) = graph.nodes.get_mut(&nid) {
            node.classes.push(class_name.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_graph() {
        let input = "graph TD\n    A --> B";
        let g = parse(input).unwrap();
        assert_eq!(g.direction, Direction::TD);
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].from, "A");
        assert_eq!(g.edges[0].to, "B");
    }

    #[test]
    fn test_node_shapes() {
        let input = "graph LR\n    A[Rect] --> B{Diamond} --> C(Rounded) --> D((Circle))";
        let g = parse(input).unwrap();
        assert_eq!(g.direction, Direction::LR);
        assert_eq!(g.nodes["A"].shape, NodeShape::Rect);
        assert_eq!(g.nodes["A"].label, "Rect");
        assert_eq!(g.nodes["B"].shape, NodeShape::Diamond);
        assert_eq!(g.nodes["C"].shape, NodeShape::Rounded);
        assert_eq!(g.nodes["D"].shape, NodeShape::Circle);
        assert_eq!(g.edges.len(), 3);
    }

    #[test]
    fn test_edge_labels() {
        let input = "graph TD\n    A -->|Yes| B\n    A -->|No| C";
        let g = parse(input).unwrap();
        assert_eq!(g.edges.len(), 2);
        assert_eq!(g.edges[0].label.as_deref(), Some("Yes"));
        assert_eq!(g.edges[1].label.as_deref(), Some("No"));
    }

    #[test]
    fn test_edge_types() {
        let input = "graph TD\n    A --> B\n    B --- C\n    C -.-> D\n    D ==> E";
        let g = parse(input).unwrap();
        assert_eq!(g.edges[0].edge_type, EdgeType::Arrow);
        assert_eq!(g.edges[1].edge_type, EdgeType::Line);
        assert_eq!(g.edges[2].edge_type, EdgeType::DottedArrow);
        assert_eq!(g.edges[3].edge_type, EdgeType::ThickArrow);
    }

    #[test]
    fn test_subgraph() {
        let input = "graph TD\n    subgraph Group\n        A --> B\n    end\n    B --> C";
        let g = parse(input).unwrap();
        assert_eq!(g.subgraphs.len(), 1);
        assert_eq!(g.subgraphs[0].title, "Group");
        assert!(g.subgraphs[0].node_ids.contains(&"A".to_string()));
        assert!(g.subgraphs[0].node_ids.contains(&"B".to_string()));
    }

    #[test]
    fn test_comments() {
        let input = "graph TD\n    %% This is a comment\n    A --> B";
        let g = parse(input).unwrap();
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 1);
    }

    #[test]
    fn test_chain() {
        let input = "graph TD\n    A --> B --> C --> D";
        let g = parse(input).unwrap();
        assert_eq!(g.edges.len(), 3);
        assert_eq!(g.edges[0].from, "A");
        assert_eq!(g.edges[0].to, "B");
        assert_eq!(g.edges[1].from, "B");
        assert_eq!(g.edges[1].to, "C");
        assert_eq!(g.edges[2].from, "C");
        assert_eq!(g.edges[2].to, "D");
    }

    #[test]
    fn test_style() {
        let input = "graph TD\n    A --> B\n    style A fill:#f9f,stroke:#333";
        let g = parse(input).unwrap();
        let s = g.styles.get("A").unwrap();
        assert_eq!(s.fill, Some([255, 153, 255]));
        assert_eq!(s.stroke, Some([51, 51, 51]));
    }

    #[test]
    fn test_direction_variants() {
        for dir_str in &["TD", "TB", "LR", "RL", "BT"] {
            let input = format!("graph {dir_str}\n    A --> B");
            let g = parse(&input).unwrap();
            let expected = match *dir_str {
                "TD" => Direction::TD,
                "TB" => Direction::TB,
                "LR" => Direction::LR,
                "RL" => Direction::RL,
                "BT" => Direction::BT,
                _ => unreachable!(),
            };
            assert_eq!(g.direction, expected);
        }
    }

    #[test]
    fn test_platform_component() {
        let input = std::fs::read_to_string("test_diagrams/platform-dependency-graph.mmd").unwrap();
        let result = parse(&input);
        match &result {
            Ok(g) => {
                eprintln!("Parsed OK: {} nodes, {} edges, {} subgraphs",
                    g.nodes.len(), g.edges.len(), g.subgraphs.len());
            }
            Err(e) => {
                eprintln!("Parse error:\n{e}");
            }
        }
        result.unwrap();
    }
}
