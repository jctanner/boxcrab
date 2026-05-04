use crate::diagram::*;
use std::collections::HashMap;

pub fn parse(input: &str) -> Result<DiagramGraph, Box<dyn std::error::Error>> {
    let mut graph = DiagramGraph {
        direction: Direction::LR,
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

    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("erDiagram") {
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

        // Entity with attributes: ENTITY {
        if trimmed.ends_with('{') {
            let entity_name = trimmed.trim_end_matches('{').trim().to_string();
            if !entity_name.is_empty() && !entity_name.contains(' ') {
                let mut columns = Vec::new();
                i += 1;
                while i < lines.len() {
                    let attr_line = lines[i].trim();
                    if attr_line == "}" {
                        i += 1;
                        break;
                    }
                    if !attr_line.is_empty() {
                        let parts: Vec<&str> = attr_line.split_whitespace().collect();
                        let type_str = parts.first().map(|s| s.to_string()).unwrap_or_default();
                        let name = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
                        let constraint = parts.get(2..).map(|s| s.join(" ")).unwrap_or_default();
                        columns.push(SqlColumn {
                            name,
                            type_str,
                            constraint,
                        });
                    }
                    i += 1;
                }
                ensure_entity_node(&mut graph, &entity_name, columns);
                continue;
            }
        }

        // Relationship: ENTITY1 ||--o{ ENTITY2 : label
        if let Some(rel) = try_parse_er_relationship(trimmed) {
            ensure_entity_node(&mut graph, &rel.from, Vec::new());
            ensure_entity_node(&mut graph, &rel.to, Vec::new());
            graph.edges.push(EdgeDef {
                from: rel.from,
                to: rel.to,
                edge_type: EdgeType::Arrow,
                label: rel.label,
                src_arrowhead: rel.src_arrowhead,
                dst_arrowhead: rel.dst_arrowhead,
                style: StyleProps::default(),
            });
            i += 1;
            continue;
        }

        i += 1;
    }

    Ok(graph)
}

fn ensure_entity_node(graph: &mut DiagramGraph, name: &str, columns: Vec<SqlColumn>) {
    let node = graph.nodes.entry(name.to_string()).or_insert_with(|| NodeDef {
        label: name.to_string(),
        shape: NodeShape::SqlTable,
        classes: Vec::new(),
        class_fields: Vec::new(),
        class_methods: Vec::new(),
        sql_columns: Vec::new(),
        near: None,
        tooltip: None,
        link: None,
    });
    node.sql_columns.extend(columns);
}

struct ErRelInfo {
    from: String,
    to: String,
    label: Option<String>,
    src_arrowhead: Option<ArrowheadType>,
    dst_arrowhead: Option<ArrowheadType>,
}

const ER_OPERATORS: &[(&str, Option<ArrowheadType>, Option<ArrowheadType>)] = &[
    ("||--||", Some(ArrowheadType::CfOneRequired), Some(ArrowheadType::CfOneRequired)),
    ("||--o{", Some(ArrowheadType::CfOneRequired), Some(ArrowheadType::CfMany)),
    ("||--|{", Some(ArrowheadType::CfOneRequired), Some(ArrowheadType::CfManyRequired)),
    ("||--o|", Some(ArrowheadType::CfOneRequired), Some(ArrowheadType::CfOne)),
    ("}o--||", Some(ArrowheadType::CfMany), Some(ArrowheadType::CfOneRequired)),
    ("}|--||", Some(ArrowheadType::CfManyRequired), Some(ArrowheadType::CfOneRequired)),
    ("|o--||", Some(ArrowheadType::CfOne), Some(ArrowheadType::CfOneRequired)),
    ("o{--||", Some(ArrowheadType::CfMany), Some(ArrowheadType::CfOneRequired)),
    ("{|--||", Some(ArrowheadType::CfManyRequired), Some(ArrowheadType::CfOneRequired)),
    ("}o--o{", Some(ArrowheadType::CfMany), Some(ArrowheadType::CfMany)),
    ("}|--|{", Some(ArrowheadType::CfManyRequired), Some(ArrowheadType::CfManyRequired)),
    ("||..||", Some(ArrowheadType::CfOneRequired), Some(ArrowheadType::CfOneRequired)),
    ("||..o{", Some(ArrowheadType::CfOneRequired), Some(ArrowheadType::CfMany)),
    ("||..|{", Some(ArrowheadType::CfOneRequired), Some(ArrowheadType::CfManyRequired)),
    ("}o..||", Some(ArrowheadType::CfMany), Some(ArrowheadType::CfOneRequired)),
];

fn try_parse_er_relationship(line: &str) -> Option<ErRelInfo> {
    let (line_part, label) = if let Some(idx) = line.rfind(':') {
        let lbl = line[idx + 1..].trim().to_string();
        (line[..idx].trim(), if lbl.is_empty() { None } else { Some(lbl) })
    } else {
        return None;
    };

    for &(op, src_ah, dst_ah) in ER_OPERATORS {
        if let Some(idx) = line_part.find(op) {
            let from = line_part[..idx].trim().to_string();
            let to = line_part[idx + op.len()..].trim().to_string();
            if !from.is_empty() && !to.is_empty() {
                return Some(ErRelInfo {
                    from,
                    to,
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
    fn test_er_diagram_basic() {
        let input = r#"erDiagram
    CUSTOMER {
        string name
        int id PK
    }
    ORDER {
        int orderNumber
        string deliveryAddress
    }
    CUSTOMER ||--o{ ORDER : places
"#;
        let g = parse(input).unwrap();
        assert_eq!(g.nodes.len(), 2);
        assert!(g.nodes.contains_key("CUSTOMER"));
        assert!(g.nodes.contains_key("ORDER"));
        assert_eq!(g.nodes["CUSTOMER"].sql_columns.len(), 2);
        assert_eq!(g.nodes["ORDER"].sql_columns.len(), 2);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].label.as_deref(), Some("places"));
    }

    #[test]
    fn test_er_no_attributes() {
        let input = r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE_ITEM : contains
"#;
        let g = parse(input).unwrap();
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.edges.len(), 2);
    }
}
