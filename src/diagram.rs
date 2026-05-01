use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    TD,
    TB,
    LR,
    RL,
    BT,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeShape {
    Rect,
    Rounded,
    Diamond,
    Circle,
    Flag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    Arrow,
    Line,
    DottedArrow,
    DottedLine,
    ThickArrow,
    ThickLine,
    BidiArrow,
    BidiDottedArrow,
    BidiThickArrow,
}

#[derive(Debug, Clone)]
pub struct NodeDef {
    pub label: String,
    pub shape: NodeShape,
    pub classes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EdgeDef {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SubgraphDef {
    pub title: String,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct StyleProps {
    pub fill: Option<[u8; 3]>,
    pub stroke: Option<[u8; 3]>,
    pub stroke_width: Option<f32>,
    pub color: Option<[u8; 3]>,
}

#[derive(Debug, Clone)]
pub struct DiagramGraph {
    pub direction: Direction,
    pub nodes: HashMap<String, NodeDef>,
    pub edges: Vec<EdgeDef>,
    pub subgraphs: Vec<SubgraphDef>,
    pub styles: HashMap<String, StyleProps>,
    pub class_defs: HashMap<String, StyleProps>,
    pub layer_spacing: Option<f32>,
    pub node_spacing: Option<f32>,
}
