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
    Oval,
    Hexagon,
    Parallelogram,
    Cylinder,
    Cloud,
    Page,
    Document,
    Queue,
    Package,
    Step,
    Callout,
    StoredData,
    Person,
    Text,
    Class,
    SqlTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassField {
    pub visibility: char,
    pub name: String,
    pub type_str: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassMethod {
    pub visibility: char,
    pub name: String,
    pub return_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlColumn {
    pub name: String,
    pub type_str: String,
    pub constraint: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NearPosition {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Debug, Clone)]
pub struct NodeDef {
    pub label: String,
    pub shape: NodeShape,
    pub classes: Vec<String>,
    pub class_fields: Vec<ClassField>,
    pub class_methods: Vec<ClassMethod>,
    pub sql_columns: Vec<SqlColumn>,
    pub near: Option<NearPosition>,
    pub tooltip: Option<String>,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowheadType {
    None,
    Triangle,
    Arrow,
    UnfilledTriangle,
    Diamond,
    FilledDiamond,
    Circle,
    FilledCircle,
    Cross,
    Box,
    FilledBox,
    Line,
    CfOne,
    CfMany,
    CfOneRequired,
    CfManyRequired,
}

impl Default for ArrowheadType {
    fn default() -> Self {
        ArrowheadType::Triangle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillPattern {
    Dots,
    Lines,
    Grain,
    Paper,
}

#[derive(Debug, Clone)]
pub struct EdgeDef {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
    pub src_arrowhead: Option<ArrowheadType>,
    pub dst_arrowhead: Option<ArrowheadType>,
    pub style: StyleProps,
}

#[derive(Debug, Clone)]
pub struct SubgraphDef {
    pub title: String,
    pub node_ids: Vec<String>,
    pub grid_rows: Option<usize>,
    pub grid_columns: Option<usize>,
    pub grid_gap: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct StyleProps {
    pub fill: Option<[u8; 3]>,
    pub stroke: Option<[u8; 3]>,
    pub stroke_width: Option<f32>,
    pub color: Option<[u8; 3]>,
    pub border_radius: Option<f32>,
    pub opacity: Option<f32>,
    pub stroke_dash: Option<f32>,
    pub shadow: Option<bool>,
    pub three_d: Option<bool>,
    pub multiple: Option<bool>,
    pub double_border: Option<bool>,
    pub font_size: Option<f32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub fill_pattern: Option<FillPattern>,
    pub animated: Option<bool>,
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
