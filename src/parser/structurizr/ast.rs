#[derive(Debug, Clone)]
pub struct Workspace {
    pub model: Model,
    pub views: Views,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub elements: Vec<Element>,
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementKind {
    Person,
    SoftwareSystem,
    Container,
    Component,
}

#[derive(Debug, Clone)]
pub struct Element {
    pub id: String,
    pub kind: ElementKind,
    pub name: String,
    pub description: Option<String>,
    pub technology: Option<String>,
    pub tags: Vec<String>,
    pub children: Vec<Element>,
}

#[derive(Debug, Clone)]
pub struct Relationship {
    pub source_id: String,
    pub target_id: String,
    pub description: Option<String>,
    pub technology: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Views {
    pub view_defs: Vec<ViewDef>,
    pub styles: StylesDef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewKind {
    SystemContext,
    Container,
    Component,
    SystemLandscape,
}

#[derive(Debug, Clone)]
pub struct ViewDef {
    pub kind: ViewKind,
    pub target_id: Option<String>,
    pub key: Option<String>,
    pub auto_layout: Option<AutoLayout>,
}

#[derive(Debug, Clone)]
pub struct AutoLayout {
    pub direction: Option<String>,
    pub rank_sep: Option<u32>,
    pub node_sep: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct StylesDef {
    pub element_styles: Vec<ElementStyleDef>,
    pub relationship_styles: Vec<RelationshipStyleDef>,
}

#[derive(Debug, Clone)]
pub struct ElementStyleDef {
    pub tag: String,
    pub background: Option<String>,
    pub color: Option<String>,
    pub shape: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RelationshipStyleDef {
    pub tag: String,
    pub color: Option<String>,
    pub dashed: Option<bool>,
    pub thickness: Option<u32>,
}
