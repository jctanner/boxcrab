pub mod mermaid;
pub mod structurizr;

use crate::diagram::DiagramGraph;

#[derive(Debug, Clone, Copy)]
pub enum DiagramFormat {
    Mermaid,
    Structurizr,
}

pub fn detect_format(path: &std::path::Path) -> Option<DiagramFormat> {
    match path.extension()?.to_str()? {
        "mmd" => Some(DiagramFormat::Mermaid),
        "dsl" => Some(DiagramFormat::Structurizr),
        _ => None,
    }
}

pub fn parse(source: &str, format: DiagramFormat, view_index: usize) -> Result<DiagramGraph, Box<dyn std::error::Error>> {
    match format {
        DiagramFormat::Mermaid => mermaid::parse(source),
        DiagramFormat::Structurizr => {
            let workspace = structurizr::parse_workspace_v2(source)?;
            structurizr::to_diagram_graph(&workspace, view_index)
        }
    }
}
