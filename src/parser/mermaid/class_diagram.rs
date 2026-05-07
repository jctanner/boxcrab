use crate::diagram::*;
use std::collections::HashMap;

pub fn parse(input: &str) -> Result<DiagramGraph, Box<dyn std::error::Error>> {
    let mut graph = DiagramGraph {
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
    };

    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    // Skip the classDiagram header
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("classDiagram") {
            i += 1;
            break;
        }
        i += 1;
    }

    while i < lines.len() {
        let trimmed = lines[i].trim();

        if trimmed.is_empty() || trimmed.starts_with("%%") {
            i += 1;
            continue;
        }

        // direction directive
        if trimmed.starts_with("direction ") {
            let dir = trimmed.strip_prefix("direction ").unwrap().trim();
            graph.direction = match dir {
                "LR" => Direction::LR,
                "RL" => Direction::RL,
                "BT" => Direction::BT,
                "TB" => Direction::TB,
                _ => Direction::TD,
            };
            i += 1;
            continue;
        }

        // class ClassName { ... } block
        if trimmed.starts_with("class ") && trimmed.ends_with('{') {
            let name = trimmed
                .strip_prefix("class ")
                .unwrap()
                .trim()
                .trim_end_matches('{')
                .trim()
                .trim_end_matches(&['~', '<'][..])
                .split(&['~', '<'][..])
                .next()
                .unwrap_or("")
                .trim();
            let class_name = name.to_string();
            let mut fields = Vec::new();
            let mut methods = Vec::new();
            i += 1;
            while i < lines.len() {
                let member = lines[i].trim();
                if member == "}" {
                    i += 1;
                    break;
                }
                if !member.is_empty() {
                    parse_member(member, &mut fields, &mut methods);
                }
                i += 1;
            }
            ensure_class_node(&mut graph, &class_name, fields, methods);
            continue;
        }

        // class ClassName with annotation: class ClassName~Generic~
        if trimmed.starts_with("class ") && !trimmed.contains('{') {
            let rest = trimmed.strip_prefix("class ").unwrap().trim();
            let name = rest
                .split(&['~', '<', ' '][..])
                .next()
                .unwrap_or(rest)
                .trim();
            if !name.is_empty() {
                ensure_class_node(&mut graph, name, Vec::new(), Vec::new());
            }
            i += 1;
            continue;
        }

        // Annotation: <<interface>> ClassName
        if trimmed.starts_with("<<") {
            i += 1;
            continue;
        }

        // Relationship: ClassA <|-- ClassB : label
        if let Some(rel) = try_parse_relationship(trimmed) {
            ensure_class_node(&mut graph, &rel.from, Vec::new(), Vec::new());
            ensure_class_node(&mut graph, &rel.to, Vec::new(), Vec::new());
            graph.edges.push(EdgeDef {
                from: rel.from,
                to: rel.to,
                edge_type: rel.edge_type,
                label: rel.label,
                src_arrowhead: rel.src_arrowhead,
                dst_arrowhead: rel.dst_arrowhead,
                style: StyleProps::default(),
            });
            i += 1;
            continue;
        }

        // Shorthand member: ClassName : +field or ClassName : +method()
        if let Some((class_name, member_str)) = trimmed.split_once(':') {
            let class_name = class_name.trim();
            let member_str = member_str.trim();
            if !class_name.is_empty()
                && !class_name.contains(' ')
                && !member_str.is_empty()
            {
                let mut fields = Vec::new();
                let mut methods = Vec::new();
                parse_member(member_str, &mut fields, &mut methods);
                let node = graph.nodes.entry(class_name.to_string()).or_insert_with(|| NodeDef {
                    label: class_name.to_string(),
                    shape: NodeShape::Class,
                    classes: Vec::new(),
                    class_fields: Vec::new(),
                    class_methods: Vec::new(),
                    sql_columns: Vec::new(),
                    near: None,
                    tooltip: None,
                    link: None,
                });
                node.class_fields.extend(fields);
                node.class_methods.extend(methods);
                i += 1;
                continue;
            }
        }

        i += 1;
    }

    Ok(graph)
}

fn ensure_class_node(
    graph: &mut DiagramGraph,
    name: &str,
    fields: Vec<ClassField>,
    methods: Vec<ClassMethod>,
) {
    let node = graph.nodes.entry(name.to_string()).or_insert_with(|| NodeDef {
        label: name.to_string(),
        shape: NodeShape::Class,
        classes: Vec::new(),
        class_fields: Vec::new(),
        class_methods: Vec::new(),
        sql_columns: Vec::new(),
        near: None,
        tooltip: None,
        link: None,
    });
    node.class_fields.extend(fields);
    node.class_methods.extend(methods);
}

fn parse_member(s: &str, fields: &mut Vec<ClassField>, methods: &mut Vec<ClassMethod>) {
    let s = s.trim();
    let (vis, rest) = extract_visibility(s);

    if rest.contains('(') {
        let name = rest.split('(').next().unwrap_or("").trim().to_string();
        let return_type = rest
            .rsplit(')')
            .next()
            .unwrap_or("")
            .trim()
            .trim_start_matches(':')
            .trim()
            .to_string();
        methods.push(ClassMethod {
            visibility: vis,
            name,
            return_type,
        });
    } else {
        let parts: Vec<&str> = rest.splitn(2, ':').collect();
        let (name, type_str) = if parts.len() == 2 {
            (parts[0].trim().to_string(), parts[1].trim().to_string())
        } else {
            let parts: Vec<&str> = rest.splitn(2, ' ').collect();
            if parts.len() == 2 {
                (parts[1].trim().to_string(), parts[0].trim().to_string())
            } else {
                (rest.to_string(), String::new())
            }
        };
        fields.push(ClassField {
            visibility: vis,
            name,
            type_str,
        });
    }
}

fn extract_visibility(s: &str) -> (char, &str) {
    if let Some(first) = s.chars().next() {
        match first {
            '+' | '-' | '#' | '~' => (first, &s[1..]),
            _ => ('+', s),
        }
    } else {
        ('+', s)
    }
}

struct RelInfo {
    from: String,
    to: String,
    edge_type: EdgeType,
    label: Option<String>,
    src_arrowhead: Option<ArrowheadType>,
    dst_arrowhead: Option<ArrowheadType>,
}

const REL_OPERATORS: &[(&str, EdgeType, Option<ArrowheadType>, Option<ArrowheadType>)] = &[
    ("<|..", EdgeType::DottedArrow, Some(ArrowheadType::UnfilledTriangle), None),
    ("..|>", EdgeType::DottedArrow, None, Some(ArrowheadType::UnfilledTriangle)),
    ("<|--", EdgeType::Arrow, Some(ArrowheadType::UnfilledTriangle), None),
    ("--|>", EdgeType::Arrow, None, Some(ArrowheadType::UnfilledTriangle)),
    ("*--", EdgeType::Arrow, Some(ArrowheadType::FilledDiamond), None),
    ("--*", EdgeType::Arrow, None, Some(ArrowheadType::FilledDiamond)),
    ("o--", EdgeType::Arrow, Some(ArrowheadType::Diamond), None),
    ("--o", EdgeType::Arrow, None, Some(ArrowheadType::Diamond)),
    ("..", EdgeType::DottedArrow, None, None),
    ("--", EdgeType::Arrow, None, None),
    ("-->", EdgeType::Arrow, None, Some(ArrowheadType::Triangle)),
    ("..>", EdgeType::DottedArrow, None, Some(ArrowheadType::Triangle)),
];

fn try_parse_relationship(line: &str) -> Option<RelInfo> {
    let (line_no_label, label) = if let Some(idx) = line.rfind(':') {
        let lbl = line[idx + 1..].trim().to_string();
        let rest = line[..idx].trim();
        (rest, if lbl.is_empty() { None } else { Some(lbl) })
    } else {
        (line, None)
    };

    for &(op, edge_type, src_ah, dst_ah) in REL_OPERATORS {
        if let Some(idx) = line_no_label.find(op) {
            let from = line_no_label[..idx].trim().trim_matches('"').to_string();
            let to = line_no_label[idx + op.len()..].trim().trim_matches('"').to_string();
            if !from.is_empty() && !to.is_empty() && !from.contains(' ') && !to.contains(' ') {
                return Some(RelInfo {
                    from,
                    to,
                    edge_type,
                    label,
                    src_arrowhead: src_ah,
                    dst_arrowhead: dst_ah,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_class_diagram_basic() {
        let input = r#"classDiagram
    class Animal {
        +String name
        +makeSound()
    }
    class Dog {
        +fetch()
    }
    Animal <|-- Dog
"#;
        let g = parse(input).unwrap();
        assert_eq!(g.nodes.len(), 2);
        assert!(g.nodes.contains_key("Animal"));
        assert!(g.nodes.contains_key("Dog"));
        assert_eq!(g.nodes["Animal"].shape, NodeShape::Class);
        assert_eq!(g.nodes["Animal"].class_fields.len(), 1);
        assert_eq!(g.nodes["Animal"].class_methods.len(), 1);
        assert_eq!(g.edges.len(), 1);
    }

    #[test]
    fn test_class_relationships() {
        let input = r#"classDiagram
    Animal <|-- Dog
    Animal *-- Leg
    Vehicle o-- Wheel
    Car --|> Vehicle
    Shape ..|> Drawable
"#;
        let g = parse(input).unwrap();
        assert_eq!(g.edges.len(), 5);
    }

    #[test]
    fn test_shorthand_members() {
        let input = r#"classDiagram
    class Cat
    Cat : +String name
    Cat : +meow()
"#;
        let g = parse(input).unwrap();
        assert_eq!(g.nodes["Cat"].class_fields.len(), 1);
        assert_eq!(g.nodes["Cat"].class_methods.len(), 1);
    }
}
