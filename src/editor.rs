use crate::diagram::*;
use crate::layout;
use crate::parser;
use crate::renderer;
use crate::serializer;
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub enum EditorAction {
    None,
    OpenFile(PathBuf, parser::DiagramFormat),
    EditFile(PathBuf, parser::DiagramFormat),
}

#[derive(Debug, Clone, PartialEq)]
pub enum InteractionState {
    Idle,
    PlacingNode { shape: NodeShape },
    ConnectingEdge { source_id: String },
    DraggingNode { node_id: String },
}

#[derive(Clone)]
struct ClipboardData {
    nodes: HashMap<String, NodeDef>,
    positions: HashMap<String, (f32, f32)>,
    edges: Vec<EdgeDef>,
    styles: HashMap<String, StyleProps>,
}

struct UndoSnapshot {
    graph: DiagramGraph,
    manual_positions: HashMap<String, (f32, f32)>,
    next_node_id: u32,
}

pub struct EditorState {
    pub graph: DiagramGraph,
    pub layout_result: Option<layout::LayoutResult>,
    pub node_sizes: Option<HashMap<String, egui::Vec2>>,
    pub needs_layout: bool,
    pub manual_positions: HashMap<String, (f32, f32)>,
    pub next_node_id: u32,
    pub interaction: InteractionState,
    pub selected_nodes: HashSet<String>,
    pub selected_edge: Option<usize>,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub scene_rect: egui::Rect,
    pub full_scene_rect: egui::Rect,
    undo_stack: Vec<UndoSnapshot>,
    redo_stack: Vec<UndoSnapshot>,
    clipboard: Option<ClipboardData>,
}

impl EditorState {
    pub fn new() -> Self {
        let scene = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(2000.0, 2000.0),
        );
        Self {
            graph: DiagramGraph {
                direction: Direction::TD,
                nodes: HashMap::new(),
                edges: Vec::new(),
                subgraphs: Vec::new(),
                styles: HashMap::new(),
                class_defs: HashMap::new(),
                layer_spacing: None,
                node_spacing: None,
            },
            layout_result: None,
            node_sizes: None,
            needs_layout: false,
            manual_positions: HashMap::new(),
            next_node_id: 1,
            interaction: InteractionState::Idle,
            selected_nodes: HashSet::new(),
            selected_edge: None,
            file_path: None,
            dirty: false,
            scene_rect: scene,
            full_scene_rect: scene,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: None,
        }
    }

    pub fn from_file(
        path: PathBuf,
        format: parser::DiagramFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let source = std::fs::read_to_string(&path)?;
        let base_dir = path.parent();
        let graph = parser::parse(&source, format, 0, base_dir)?;
        let mut state = Self::new();

        let mut max_id = 0u32;
        for key in graph.nodes.keys() {
            if let Some(rest) = key.strip_prefix("node_") {
                if let Ok(n) = rest.parse::<u32>() {
                    max_id = max_id.max(n);
                }
            }
        }
        state.next_node_id = max_id + 1;
        state.graph = graph;
        state.file_path = Some(path);
        state.dirty = false;
        Ok(state)
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(UndoSnapshot {
            graph: self.graph.clone(),
            manual_positions: self.manual_positions.clone(),
            next_node_id: self.next_node_id,
        });
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            self.redo_stack.push(UndoSnapshot {
                graph: self.graph.clone(),
                manual_positions: self.manual_positions.clone(),
                next_node_id: self.next_node_id,
            });
            self.graph = snapshot.graph;
            self.manual_positions = snapshot.manual_positions;
            self.next_node_id = snapshot.next_node_id;
            self.selected_nodes.clear();
            self.selected_edge = None;
            self.dirty = true;
            self.rebuild_layout();
        }
    }

    fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            self.undo_stack.push(UndoSnapshot {
                graph: self.graph.clone(),
                manual_positions: self.manual_positions.clone(),
                next_node_id: self.next_node_id,
            });
            self.graph = snapshot.graph;
            self.manual_positions = snapshot.manual_positions;
            self.next_node_id = snapshot.next_node_id;
            self.selected_nodes.clear();
            self.selected_edge = None;
            self.dirty = true;
            self.rebuild_layout();
        }
    }

    fn copy_selection(&mut self) {
        if self.selected_nodes.is_empty() {
            return;
        }
        let mut nodes = HashMap::new();
        let mut positions = HashMap::new();
        let mut styles = HashMap::new();

        for id in &self.selected_nodes {
            if let Some(node) = self.graph.nodes.get(id) {
                nodes.insert(id.clone(), node.clone());
            }
            if let Some(pos) = self.manual_positions.get(id) {
                positions.insert(id.clone(), *pos);
            }
            if let Some(style) = self.graph.styles.get(id) {
                styles.insert(id.clone(), style.clone());
            }
        }

        let edges: Vec<EdgeDef> = self
            .graph
            .edges
            .iter()
            .filter(|e| self.selected_nodes.contains(&e.from) && self.selected_nodes.contains(&e.to))
            .cloned()
            .collect();

        self.clipboard = Some(ClipboardData {
            nodes,
            positions,
            edges,
            styles,
        });
    }

    fn paste_clipboard(&mut self) {
        let clip = match self.clipboard.clone() {
            Some(c) => c,
            None => return,
        };
        if clip.nodes.is_empty() {
            return;
        }

        self.push_undo();

        let mut id_map: HashMap<String, String> = HashMap::new();
        for old_id in clip.nodes.keys() {
            let new_id = self.gen_node_id();
            id_map.insert(old_id.clone(), new_id);
        }

        for (old_id, node) in &clip.nodes {
            let new_id = &id_map[old_id];
            let mut new_node = node.clone();
            new_node.label = new_id.clone();
            self.graph.nodes.insert(new_id.clone(), new_node);

            if let Some(pos) = clip.positions.get(old_id) {
                self.manual_positions
                    .insert(new_id.clone(), (pos.0 + 50.0, pos.1 + 50.0));
            }
            if let Some(style) = clip.styles.get(old_id) {
                self.graph.styles.insert(new_id.clone(), style.clone());
            }
        }

        for edge in &clip.edges {
            if let (Some(new_from), Some(new_to)) = (id_map.get(&edge.from), id_map.get(&edge.to))
            {
                let mut new_edge = edge.clone();
                new_edge.from = new_from.clone();
                new_edge.to = new_to.clone();
                self.graph.edges.push(new_edge);
            }
        }

        self.selected_nodes.clear();
        for new_id in id_map.values() {
            self.selected_nodes.insert(new_id.clone());
        }
        self.selected_edge = None;
        self.dirty = true;
        self.rebuild_layout();
    }

    fn gen_node_id(&mut self) -> String {
        let id = format!("node_{}", self.next_node_id);
        self.next_node_id += 1;
        id
    }

    fn place_node(&mut self, shape: NodeShape, scene_pos: egui::Pos2) {
        self.push_undo();
        let id = self.gen_node_id();
        self.graph.nodes.insert(
            id.clone(),
            NodeDef {
                label: id.clone(),
                shape,
                classes: Vec::new(),
                class_fields: Vec::new(),
                class_methods: Vec::new(),
                sql_columns: Vec::new(),
                near: None,
                tooltip: None,
                link: None,
            },
        );
        let snapped = snap_to_grid(scene_pos.x, scene_pos.y);
        self.manual_positions.insert(id, snapped);
        self.dirty = true;
        self.rebuild_layout();
    }

    fn add_edge(&mut self, from: String, to: String) {
        self.push_undo();
        self.graph.edges.push(EdgeDef {
            from,
            to,
            edge_type: EdgeType::Arrow,
            label: None,
            src_arrowhead: None,
            dst_arrowhead: None,
            style: StyleProps::default(),
        });
        self.dirty = true;
        self.rebuild_layout();
    }

    fn rebuild_layout(&mut self) {
        let mut nodes = Vec::new();
        for (id, def) in &self.graph.nodes {
            let (x, y) = self
                .manual_positions
                .get(id)
                .copied()
                .unwrap_or((400.0, 300.0));
            let (w, h) = match self.node_sizes.as_ref().and_then(|s| s.get(id)) {
                Some(sz) => (sz.x + 48.0, sz.y + 32.0),
                None => (120.0, 40.0),
            };
            nodes.push(layout::LayoutNode {
                id: id.clone(),
                x,
                y,
                width: w,
                height: h,
                label: def.label.clone(),
                shape: def.shape,
                style: self
                    .graph
                    .styles
                    .get(id)
                    .cloned()
                    .unwrap_or_default(),
                class_fields: def.class_fields.clone(),
                class_methods: def.class_methods.clone(),
                sql_columns: def.sql_columns.clone(),
                tooltip: def.tooltip.clone(),
                link: def.link.clone(),
            });
        }

        let edges = layout::route_edges_manual(&self.graph.edges, &nodes);

        let (max_x, max_y) = nodes.iter().fold((0.0f32, 0.0f32), |(mx, my), n| {
            (mx.max(n.x + n.width), my.max(n.y + n.height))
        });

        self.layout_result = Some(layout::LayoutResult {
            nodes,
            edges,
            subgraphs: Vec::new(),
            total_width: max_x.max(2000.0),
            total_height: max_y.max(2000.0),
        });
    }

    fn node_at_scene_pos(
        &self,
        scene_pos: egui::Pos2,
    ) -> Option<String> {
        if let Some(layout) = &self.layout_result {
            for node in layout.nodes.iter().rev() {
                let rect = egui::Rect::from_center_size(
                    egui::Pos2::new(node.x, node.y),
                    egui::Vec2::new(node.width, node.height),
                );
                if rect.contains(scene_pos) {
                    return Some(node.id.clone());
                }
            }
        }
        None
    }

    fn edge_at_scene_pos(&self, scene_pos: egui::Pos2) -> Option<usize> {
        let layout = self.layout_result.as_ref()?;
        let threshold = 8.0;
        for (i, edge) in layout.edges.iter().enumerate() {
            if edge.points.len() < 2 {
                continue;
            }
            for pair in edge.points.windows(2) {
                let dist = point_to_segment_distance(
                    scene_pos.x,
                    scene_pos.y,
                    pair[0][0],
                    pair[0][1],
                    pair[1][0],
                    pair[1][1],
                );
                if dist < threshold {
                    return Some(i);
                }
            }
        }
        None
    }

    fn auto_layout(&mut self) {
        self.push_undo();
        if let Some(sizes) = &self.node_sizes {
            if let Ok(result) = layout::compute_layout(&self.graph, Some(sizes)) {
                for node in &result.nodes {
                    self.manual_positions
                        .insert(node.id.clone(), (node.x, node.y));
                }
                let full = egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(result.total_width, result.total_height),
                );
                self.full_scene_rect = full;
                let view_w = 1024.0f32.min(result.total_width);
                let view_h = 768.0f32.min(result.total_height);
                self.scene_rect = egui::Rect::from_center_size(
                    full.center(),
                    egui::Vec2::new(view_w, view_h),
                );
                self.layout_result = Some(result);
            }
        } else {
            self.rebuild_layout();
        }
    }
}

const TOOLBAR_SHAPES: &[(&str, &[(NodeShape, &str)])] = &[
    (
        "Basic",
        &[
            (NodeShape::Rect, "Rect"),
            (NodeShape::Rounded, "Rounded"),
            (NodeShape::Diamond, "Diamond"),
            (NodeShape::Circle, "Circle"),
            (NodeShape::Oval, "Oval"),
            (NodeShape::Stadium, "Stadium"),
        ],
    ),
    (
        "Flow",
        &[
            (NodeShape::Hexagon, "Hexagon"),
            (NodeShape::Parallelogram, "Parallel"),
            (NodeShape::Trapezoid, "Trapezoid"),
            (NodeShape::TrapezoidAlt, "Trap Alt"),
            (NodeShape::Flag, "Flag"),
            (NodeShape::Step, "Step"),
        ],
    ),
    (
        "Data",
        &[
            (NodeShape::Cylinder, "Cylinder"),
            (NodeShape::Document, "Document"),
            (NodeShape::StoredData, "Stored"),
            (NodeShape::Page, "Page"),
            (NodeShape::Queue, "Queue"),
        ],
    ),
    (
        "Other",
        &[
            (NodeShape::Package, "Package"),
            (NodeShape::Callout, "Callout"),
            (NodeShape::Cloud, "Cloud"),
            (NodeShape::Person, "Person"),
            (NodeShape::Subroutine, "Subroutine"),
            (NodeShape::DoubleCircle, "Dbl Circle"),
        ],
    ),
];

pub fn render_editor_ui(state: &mut EditorState, ui: &mut egui::Ui) -> EditorAction {
    ui.painter()
        .rect_filled(ui.max_rect(), 0.0, egui::Color32::WHITE);

    let action = render_editor_menu(state, ui);

    let measured = renderer::measure_node_texts(ui, &Some(state.graph.clone()));
    if let Some(sizes) = measured {
        let needs_initial_layout =
            state.manual_positions.is_empty() && !state.graph.nodes.is_empty();
        if state.node_sizes.as_ref() != Some(&sizes) {
            state.node_sizes = Some(sizes);
            if needs_initial_layout {
                state.auto_layout();
                state.dirty = false;
            } else {
                state.rebuild_layout();
            }
        }
    }

    render_toolbar(state, ui);

    let dirty_before = state.dirty;
    render_properties_panel(state, ui);
    if state.dirty && !dirty_before {
        state.rebuild_layout();
    }

    render_canvas(state, ui);
    action
}

fn render_editor_menu(state: &mut EditorState, ui: &mut egui::Ui) -> EditorAction {
    let mut action = EditorAction::None;

    egui::Panel::top("editor_menu_bar")
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(192, 192, 192))
                .inner_margin(2.0)
                .stroke(egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgb(128, 128, 128),
                )),
        )
        .show_inside(ui, |ui| {
            ui.visuals_mut().override_text_color = Some(egui::Color32::BLACK);
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        ui.close();
                        *state = EditorState::new();
                    }
                    if ui.button("Open in Viewer...").clicked() {
                        ui.close();
                        let cwd = std::env::current_dir().unwrap_or_default();
                        let dialog = rfd::FileDialog::new()
                            .set_directory(&cwd)
                            .add_filter("Diagram files", &["mmd", "dsl", "d2"])
                            .add_filter("All files", &["*"]);
                        if let Some(path) = dialog.pick_file() {
                            if let Some(fmt) = parser::detect_format(&path) {
                                action = EditorAction::OpenFile(path, fmt);
                            }
                        }
                    }
                    if ui.button("Open for Editing...").clicked() {
                        ui.close();
                        let cwd = std::env::current_dir().unwrap_or_default();
                        let dialog = rfd::FileDialog::new()
                            .set_directory(&cwd)
                            .add_filter("Diagram files", &["mmd", "dsl", "d2"])
                            .add_filter("All files", &["*"]);
                        if let Some(path) = dialog.pick_file() {
                            if let Some(fmt) = parser::detect_format(&path) {
                                action = EditorAction::EditFile(path, fmt);
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Save").clicked() {
                        ui.close();
                        if let Some(path) = &state.file_path {
                            let text = serialize_for_path(path, &state.graph);
                            if let Err(e) = std::fs::write(path, &text) {
                                eprintln!("Save error: {e}");
                            } else {
                                state.dirty = false;
                            }
                        }
                    }
                    if ui.button("Save As...").clicked() {
                        ui.close();
                        let cwd = std::env::current_dir().unwrap_or_default();
                        let dialog = rfd::FileDialog::new()
                            .set_directory(&cwd)
                            .add_filter("Mermaid", &["mmd"])
                            .add_filter("D2", &["d2"])
                            .set_file_name("diagram.mmd");
                        if let Some(path) = dialog.save_file() {
                            let text = serialize_for_path(&path, &state.graph);
                            if let Err(e) = std::fs::write(&path, &text) {
                                eprintln!("Save error: {e}");
                            } else {
                                state.file_path = Some(path);
                                state.dirty = false;
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    let undo_label = if state.undo_stack.is_empty() {
                        "Undo"
                    } else {
                        "Undo (Ctrl+Z)"
                    };
                    if ui
                        .add_enabled(!state.undo_stack.is_empty(), egui::Button::new(undo_label))
                        .clicked()
                    {
                        ui.close();
                        state.undo();
                    }
                    let redo_label = if state.redo_stack.is_empty() {
                        "Redo"
                    } else {
                        "Redo (Ctrl+Shift+Z)"
                    };
                    if ui
                        .add_enabled(!state.redo_stack.is_empty(), egui::Button::new(redo_label))
                        .clicked()
                    {
                        ui.close();
                        state.redo();
                    }
                });
                ui.menu_button("Diagram", |ui| {
                    let current_dir = state.graph.direction;
                    let dir_label = |d: Direction| match d {
                        Direction::TD => "Top-Down",
                        Direction::TB => "Top-Bottom",
                        Direction::LR => "Left-Right",
                        Direction::RL => "Right-Left",
                        Direction::BT => "Bottom-Top",
                    };
                    ui.menu_button(
                        format!("Direction: {}", dir_label(current_dir)),
                        |ui| {
                            let dirs = [
                                Direction::TD,
                                Direction::LR,
                                Direction::RL,
                                Direction::BT,
                            ];
                            for d in dirs {
                                if ui
                                    .selectable_label(current_dir == d, dir_label(d))
                                    .clicked()
                                {
                                    state.push_undo();
                                    state.graph.direction = d;
                                    state.dirty = true;
                                    ui.close();
                                }
                            }
                        },
                    );
                });
            });
        });

    action
}

fn serialize_for_path(path: &std::path::Path, graph: &DiagramGraph) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("d2") => serializer::d2::serialize(graph),
        _ => serializer::mermaid::serialize(graph),
    }
}

fn color_edit_row(ui: &mut egui::Ui, label: &str, color: &mut Option<[u8; 3]>) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(11.0)
                .color(egui::Color32::from_rgb(80, 80, 80)),
        );
    });

    let mut enabled = color.is_some();
    let mut rgb = color.unwrap_or([200, 200, 200]);

    ui.horizontal(|ui| {
        if ui.checkbox(&mut enabled, "").changed() {
            changed = true;
            if enabled {
                *color = Some(rgb);
            } else {
                *color = None;
            }
        }
        if enabled {
            let preview = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            let (rect, _) = ui.allocate_exact_size(
                egui::Vec2::new(16.0, 16.0),
                egui::Sense::hover(),
            );
            ui.painter()
                .rect_filled(rect, 2.0, preview);
            ui.painter().rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 120, 120)),
                egui::StrokeKind::Outside,
            );
        }
    });

    if enabled {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (i, ch) in ["R", "G", "B"].iter().enumerate() {
                ui.label(egui::RichText::new(*ch).size(9.0).color(egui::Color32::GRAY));
                let mut val = rgb[i] as i32;
                let resp = ui.add(
                    egui::DragValue::new(&mut val)
                        .range(0..=255)
                        .speed(1)
                        .custom_formatter(|v, _| format!("{}", v as u8)),
                );
                if resp.changed() {
                    rgb[i] = val.clamp(0, 255) as u8;
                    *color = Some(rgb);
                    changed = true;
                }
            }
        });
    }
    ui.add_space(4.0);
    changed
}

fn shape_display_name(s: NodeShape) -> &'static str {
    for &(_, shapes) in TOOLBAR_SHAPES {
        for &(shape, name) in shapes {
            if shape == s {
                return name;
            }
        }
    }
    "Other"
}

fn text_tool_button(ui: &mut egui::Ui, label: &str, is_active: bool) -> egui::Response {
    let btn = egui::Button::new(
        egui::RichText::new(label)
            .size(10.0)
            .color(if is_active {
                egui::Color32::WHITE
            } else {
                egui::Color32::from_rgb(40, 40, 40)
            }),
    )
    .min_size(egui::Vec2::new(62.0, 24.0))
    .fill(if is_active {
        egui::Color32::from_rgb(70, 130, 200)
    } else {
        egui::Color32::from_rgb(255, 255, 255)
    })
    .stroke(egui::Stroke::new(
        1.0,
        if is_active {
            egui::Color32::from_rgb(50, 100, 170)
        } else {
            egui::Color32::from_rgb(180, 180, 180)
        },
    ));
    ui.add(btn)
}

fn shape_icon_button(
    ui: &mut egui::Ui,
    shape: NodeShape,
    name: &str,
    is_active: bool,
) -> egui::Response {
    let size = egui::Vec2::new(26.0, 21.0);
    let fill = if is_active {
        egui::Color32::from_rgb(70, 130, 200)
    } else {
        egui::Color32::from_rgb(255, 255, 255)
    };
    let stroke_color = if is_active {
        egui::Color32::from_rgb(50, 100, 170)
    } else {
        egui::Color32::from_rgb(180, 180, 180)
    };
    let icon_color = if is_active {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_rgb(60, 60, 60)
    };

    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        ui.painter()
            .rect_filled(rect, 3.0, fill);
        ui.painter().rect_stroke(
            rect,
            3.0,
            egui::Stroke::new(1.0, stroke_color),
            egui::StrokeKind::Outside,
        );
        let icon_stroke = egui::Stroke::new(1.2, icon_color);
        draw_shape_icon(ui.painter(), rect.shrink(4.0), shape, icon_stroke, icon_color);
    }
    response.on_hover_text(name)
}

fn draw_shape_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    shape: NodeShape,
    stroke: egui::Stroke,
    fill: egui::Color32,
) {
    let c = rect.center();
    let w = rect.width();
    let h = rect.height();
    let hw = w / 2.0;
    let hh = h / 2.0;

    match shape {
        NodeShape::Rect => {
            painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Outside);
        }
        NodeShape::Rounded => {
            painter.rect_stroke(rect, 4.0, stroke, egui::StrokeKind::Outside);
        }
        NodeShape::Diamond => {
            let pts = vec![
                egui::Pos2::new(c.x, c.y - hh),
                egui::Pos2::new(c.x + hw, c.y),
                egui::Pos2::new(c.x, c.y + hh),
                egui::Pos2::new(c.x - hw, c.y),
            ];
            painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
        }
        NodeShape::Circle | NodeShape::DoubleCircle => {
            let r = hw.min(hh);
            painter.circle_stroke(c, r, stroke);
            if shape == NodeShape::DoubleCircle {
                painter.circle_stroke(c, r - 3.0, stroke);
            }
        }
        NodeShape::Oval => {
            painter.rect_stroke(rect, hw, stroke, egui::StrokeKind::Outside);
        }
        NodeShape::Stadium => {
            painter.rect_stroke(rect, hh, stroke, egui::StrokeKind::Outside);
        }
        NodeShape::Hexagon => {
            let inset = hw * 0.3;
            let pts = vec![
                egui::Pos2::new(c.x - hw + inset, c.y - hh),
                egui::Pos2::new(c.x + hw - inset, c.y - hh),
                egui::Pos2::new(c.x + hw, c.y),
                egui::Pos2::new(c.x + hw - inset, c.y + hh),
                egui::Pos2::new(c.x - hw + inset, c.y + hh),
                egui::Pos2::new(c.x - hw, c.y),
            ];
            painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
        }
        NodeShape::Parallelogram => {
            let skew = hw * 0.3;
            let pts = vec![
                egui::Pos2::new(c.x - hw + skew, c.y - hh),
                egui::Pos2::new(c.x + hw, c.y - hh),
                egui::Pos2::new(c.x + hw - skew, c.y + hh),
                egui::Pos2::new(c.x - hw, c.y + hh),
            ];
            painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
        }
        NodeShape::Trapezoid => {
            let inset = hw * 0.25;
            let pts = vec![
                egui::Pos2::new(c.x - hw + inset, c.y - hh),
                egui::Pos2::new(c.x + hw - inset, c.y - hh),
                egui::Pos2::new(c.x + hw, c.y + hh),
                egui::Pos2::new(c.x - hw, c.y + hh),
            ];
            painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
        }
        NodeShape::TrapezoidAlt => {
            let inset = hw * 0.25;
            let pts = vec![
                egui::Pos2::new(c.x - hw, c.y - hh),
                egui::Pos2::new(c.x + hw, c.y - hh),
                egui::Pos2::new(c.x + hw - inset, c.y + hh),
                egui::Pos2::new(c.x - hw + inset, c.y + hh),
            ];
            painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
        }
        NodeShape::Flag => {
            let pts = vec![
                egui::Pos2::new(c.x - hw, c.y - hh),
                egui::Pos2::new(c.x + hw, c.y - hh),
                egui::Pos2::new(c.x + hw - hw * 0.25, c.y),
                egui::Pos2::new(c.x + hw, c.y + hh),
                egui::Pos2::new(c.x - hw, c.y + hh),
            ];
            painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
        }
        NodeShape::Step => {
            let notch = hw * 0.3;
            painter.line_segment(
                [egui::Pos2::new(c.x - hw, c.y - hh), egui::Pos2::new(c.x + hw - notch, c.y - hh)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x + hw - notch, c.y - hh), egui::Pos2::new(c.x + hw, c.y)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x + hw, c.y), egui::Pos2::new(c.x + hw - notch, c.y + hh)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x + hw - notch, c.y + hh), egui::Pos2::new(c.x - hw, c.y + hh)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x - hw, c.y + hh), egui::Pos2::new(c.x - hw, c.y - hh)],
                stroke,
            );
        }
        NodeShape::Cylinder => {
            let ey = hh * 0.3;
            painter.line_segment(
                [egui::Pos2::new(c.x - hw, c.y - hh + ey), egui::Pos2::new(c.x - hw, c.y + hh - ey)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x + hw, c.y - hh + ey), egui::Pos2::new(c.x + hw, c.y + hh - ey)],
                stroke,
            );
            let top_r = egui::Rect::from_center_size(
                egui::Pos2::new(c.x, c.y - hh + ey),
                egui::Vec2::new(w, ey * 2.0),
            );
            painter.rect_stroke(top_r, hw, stroke, egui::StrokeKind::Outside);
            let bot_c = egui::Pos2::new(c.x, c.y + hh - ey);
            painter.add(egui::Shape::CubicBezier(egui::epaint::CubicBezierShape::from_points_stroke(
                [
                    egui::Pos2::new(c.x - hw, bot_c.y),
                    egui::Pos2::new(c.x - hw, bot_c.y + ey * 1.5),
                    egui::Pos2::new(c.x + hw, bot_c.y + ey * 1.5),
                    egui::Pos2::new(c.x + hw, bot_c.y),
                ],
                false,
                egui::Color32::TRANSPARENT,
                stroke,
            )));
        }
        NodeShape::Document => {
            painter.line_segment(
                [egui::Pos2::new(c.x - hw, c.y - hh), egui::Pos2::new(c.x + hw, c.y - hh)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x + hw, c.y - hh), egui::Pos2::new(c.x + hw, c.y + hh * 0.6)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x - hw, c.y + hh * 0.6), egui::Pos2::new(c.x - hw, c.y - hh)],
                stroke,
            );
            painter.add(egui::Shape::CubicBezier(egui::epaint::CubicBezierShape::from_points_stroke(
                [
                    egui::Pos2::new(c.x + hw, c.y + hh * 0.6),
                    egui::Pos2::new(c.x + hw * 0.3, c.y + hh * 1.2),
                    egui::Pos2::new(c.x - hw * 0.3, c.y + hh * 0.2),
                    egui::Pos2::new(c.x - hw, c.y + hh * 0.6),
                ],
                false,
                egui::Color32::TRANSPARENT,
                stroke,
            )));
        }
        NodeShape::Page => {
            let fold = hw * 0.35;
            painter.line_segment(
                [egui::Pos2::new(c.x - hw, c.y - hh), egui::Pos2::new(c.x + hw - fold, c.y - hh)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x + hw - fold, c.y - hh), egui::Pos2::new(c.x + hw, c.y - hh + fold)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x + hw, c.y - hh + fold), egui::Pos2::new(c.x + hw, c.y + hh)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x + hw, c.y + hh), egui::Pos2::new(c.x - hw, c.y + hh)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x - hw, c.y + hh), egui::Pos2::new(c.x - hw, c.y - hh)],
                stroke,
            );
        }
        NodeShape::StoredData => {
            painter.line_segment(
                [egui::Pos2::new(c.x - hw * 0.7, c.y - hh), egui::Pos2::new(c.x + hw, c.y - hh)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x + hw, c.y - hh), egui::Pos2::new(c.x + hw, c.y + hh)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x + hw, c.y + hh), egui::Pos2::new(c.x - hw * 0.7, c.y + hh)],
                stroke,
            );
            painter.add(egui::Shape::CubicBezier(egui::epaint::CubicBezierShape::from_points_stroke(
                [
                    egui::Pos2::new(c.x - hw * 0.7, c.y - hh),
                    egui::Pos2::new(c.x - hw * 1.2, c.y - hh * 0.3),
                    egui::Pos2::new(c.x - hw * 1.2, c.y + hh * 0.3),
                    egui::Pos2::new(c.x - hw * 0.7, c.y + hh),
                ],
                false,
                egui::Color32::TRANSPARENT,
                stroke,
            )));
        }
        NodeShape::Queue => {
            let ey = hw * 0.35;
            painter.line_segment(
                [egui::Pos2::new(c.x - hw + ey, c.y - hh), egui::Pos2::new(c.x + hw - ey, c.y - hh)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x - hw + ey, c.y + hh), egui::Pos2::new(c.x + hw - ey, c.y + hh)],
                stroke,
            );
            painter.circle_stroke(egui::Pos2::new(c.x - hw + ey, c.y), hh, stroke);
            painter.circle_stroke(egui::Pos2::new(c.x + hw - ey, c.y), hh, stroke);
        }
        NodeShape::Package => {
            painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Outside);
            let tab_w = w * 0.4;
            let tab_h = h * 0.2;
            let tab = egui::Rect::from_min_size(
                egui::Pos2::new(rect.left(), rect.top() - tab_h),
                egui::Vec2::new(tab_w, tab_h),
            );
            painter.rect_stroke(tab, 0.0, stroke, egui::StrokeKind::Outside);
        }
        NodeShape::Callout => {
            painter.rect_stroke(
                rect.shrink2(egui::Vec2::new(0.0, hh * 0.15)),
                2.0,
                stroke,
                egui::StrokeKind::Outside,
            );
            let tail_x = c.x - hw * 0.3;
            painter.line_segment(
                [
                    egui::Pos2::new(tail_x, c.y + hh * 0.85),
                    egui::Pos2::new(tail_x - hw * 0.2, c.y + hh),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::Pos2::new(tail_x - hw * 0.2, c.y + hh),
                    egui::Pos2::new(tail_x + hw * 0.15, c.y + hh * 0.85),
                ],
                stroke,
            );
        }
        NodeShape::Cloud => {
            let r = hw.min(hh) * 0.9;
            painter.circle_stroke(c, r, stroke);
            painter.circle_stroke(egui::Pos2::new(c.x - r * 0.6, c.y + r * 0.2), r * 0.5, stroke);
            painter.circle_stroke(egui::Pos2::new(c.x + r * 0.5, c.y - r * 0.1), r * 0.55, stroke);
        }
        NodeShape::Person => {
            let head_r = hh * 0.3;
            painter.circle_stroke(egui::Pos2::new(c.x, c.y - hh + head_r), head_r, stroke);
            painter.line_segment(
                [egui::Pos2::new(c.x, c.y - hh + head_r * 2.0), egui::Pos2::new(c.x, c.y + hh * 0.3)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x - hw * 0.6, c.y - hh * 0.1), egui::Pos2::new(c.x + hw * 0.6, c.y - hh * 0.1)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x, c.y + hh * 0.3), egui::Pos2::new(c.x - hw * 0.4, c.y + hh)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x, c.y + hh * 0.3), egui::Pos2::new(c.x + hw * 0.4, c.y + hh)],
                stroke,
            );
        }
        NodeShape::Subroutine => {
            painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Outside);
            let inset = hw * 0.2;
            painter.line_segment(
                [egui::Pos2::new(c.x - hw + inset, c.y - hh), egui::Pos2::new(c.x - hw + inset, c.y + hh)],
                stroke,
            );
            painter.line_segment(
                [egui::Pos2::new(c.x + hw - inset, c.y - hh), egui::Pos2::new(c.x + hw - inset, c.y + hh)],
                stroke,
            );
        }
        _ => {
            painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Outside);
        }
    }
}

fn render_toolbar(state: &mut EditorState, ui: &mut egui::Ui) {
    egui::Panel::left("editor_toolbar")
        .resizable(false)
        .show_separator_line(false)
        .exact_size(72.0)
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(240, 240, 240))
                .inner_margin(4.0),
        )
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for &(group_name, shapes) in TOOLBAR_SHAPES {
                    ui.label(
                        egui::RichText::new(group_name)
                            .size(9.0)
                            .color(egui::Color32::from_rgb(120, 120, 120)),
                    );
                    ui.add_space(1.0);

                    let mut chunks = shapes.chunks(2);
                    while let Some(row) = chunks.next() {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            for &(shape, name) in row {
                                let is_active =
                                    state.interaction == InteractionState::PlacingNode { shape };
                                if shape_icon_button(ui, shape, name, is_active).clicked() {
                                    state.interaction = if is_active {
                                        InteractionState::Idle
                                    } else {
                                        InteractionState::PlacingNode { shape }
                                    };
                                }
                            }
                        });
                    }
                    ui.add_space(3.0);
                }

                ui.separator();
                ui.add_space(2.0);

                let edge_active = matches!(
                    state.interaction,
                    InteractionState::ConnectingEdge { .. }
                );
                if text_tool_button(ui, "Edge", edge_active).clicked() {
                    state.interaction = if edge_active {
                        InteractionState::Idle
                    } else {
                        InteractionState::ConnectingEdge {
                            source_id: String::new(),
                        }
                    };
                }

                ui.add_space(2.0);

                if text_tool_button(ui, "Auto Layout", false).clicked() {
                    state.auto_layout();
                }

                if state.selected_nodes.len() >= 2 {
                    ui.add_space(2.0);
                    if text_tool_button(ui, "Group", false).clicked() {
                        state.push_undo();
                        let next = state.graph.subgraphs.len() + 1;
                        let title = format!("Group {next}");
                        let node_ids: Vec<String> =
                            state.selected_nodes.iter().cloned().collect();
                        state.graph.subgraphs.push(SubgraphDef {
                            title,
                            node_ids,
                            grid_rows: None,
                            grid_columns: None,
                            grid_gap: None,
                        });
                        state.dirty = true;
                        state.rebuild_layout();
                    }
                }
            });
        });
}

fn render_properties_panel(state: &mut EditorState, ui: &mut egui::Ui) {
    if state.selected_nodes.is_empty() && state.selected_edge.is_none() {
        return;
    }

    egui::Panel::right("properties_panel")
        .resizable(false)
        .exact_size(180.0)
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(245, 245, 245))
                .inner_margin(8.0)
                .stroke(egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgb(200, 200, 200),
                )),
        )
        .show_inside(ui, |ui| {
            ui.visuals_mut().override_text_color = Some(egui::Color32::BLACK);
            ui.visuals_mut().extreme_bg_color = egui::Color32::WHITE;
            let wv = &mut ui.visuals_mut().widgets;
            wv.inactive.bg_fill = egui::Color32::WHITE;
            wv.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(180, 180, 180));
            wv.hovered.bg_fill = egui::Color32::WHITE;
            wv.hovered.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 120, 120));
            wv.active.bg_fill = egui::Color32::WHITE;
            wv.active.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 130, 200));

            if let Some(node_id) = state.selected_nodes.iter().next().cloned() {
                ui.label(
                    egui::RichText::new("Node")
                        .size(12.0)
                        .strong()
                        .color(egui::Color32::from_rgb(60, 60, 60)),
                );
                ui.add_space(4.0);

                let mut new_label: Option<String> = None;
                let mut new_shape: Option<NodeShape> = None;
                let mut needs_undo = false;

                if let Some(node) = state.graph.nodes.get(&node_id) {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Label:")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(80, 80, 80)),
                        );
                    });
                    let mut label = node.label.clone();
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut label)
                            .desired_width(160.0)
                            .font(egui::FontId::proportional(12.0)),
                    );
                    if response.gained_focus() {
                        needs_undo = true;
                    }
                    if response.changed() {
                        new_label = Some(label);
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Shape:")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(80, 80, 80)),
                        );
                    });

                    let current_shape = node.shape;

                    egui::ComboBox::from_id_salt("shape_combo")
                        .selected_text(shape_display_name(current_shape))
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            for &(_, shapes) in TOOLBAR_SHAPES {
                                for &(shape, name) in shapes {
                                    if ui
                                        .selectable_label(current_shape == shape, name)
                                        .clicked()
                                    {
                                        new_shape = Some(shape);
                                    }
                                }
                            }
                        });
                }

                let cur_style = state
                    .graph
                    .styles
                    .get(&node_id)
                    .cloned()
                    .unwrap_or_default();
                let mut new_style = cur_style.clone();
                let mut style_changed = false;

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                style_changed |= color_edit_row(ui, "Fill:", &mut new_style.fill);
                style_changed |= color_edit_row(ui, "Stroke:", &mut new_style.stroke);
                style_changed |= color_edit_row(ui, "Text:", &mut new_style.color);

                if style_changed {
                    needs_undo = true;
                }

                if needs_undo || new_shape.is_some() {
                    state.push_undo();
                }
                let mut changed = false;
                if let Some(label) = new_label {
                    if let Some(node) = state.graph.nodes.get_mut(&node_id) {
                        node.label = label;
                        changed = true;
                    }
                }
                if let Some(shape) = new_shape {
                    if let Some(node) = state.graph.nodes.get_mut(&node_id) {
                        node.shape = shape;
                        changed = true;
                    }
                }
                if style_changed {
                    state.graph.styles.insert(node_id.clone(), new_style);
                    changed = true;
                }
                if changed {
                    state.dirty = true;
                    state.rebuild_layout();
                }
            } else if let Some(edge_idx) = state.selected_edge {
                ui.label(
                    egui::RichText::new("Edge")
                        .size(12.0)
                        .strong()
                        .color(egui::Color32::from_rgb(60, 60, 60)),
                );
                ui.add_space(4.0);

                let mut new_label: Option<Option<String>> = None;
                let mut new_type: Option<EdgeType> = None;
                let mut needs_undo = false;

                if let Some(edge) = state.graph.edges.get(edge_idx) {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Label:")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(80, 80, 80)),
                        );
                    });
                    let mut label = edge.label.clone().unwrap_or_default();
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut label)
                            .desired_width(160.0)
                            .font(egui::FontId::proportional(12.0)),
                    );
                    if response.gained_focus() {
                        needs_undo = true;
                    }
                    if response.changed() {
                        new_label = Some(if label.is_empty() {
                            None
                        } else {
                            Some(label)
                        });
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Type:")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(80, 80, 80)),
                        );
                    });

                    let edge_type_name = |t: EdgeType| match t {
                        EdgeType::Arrow => "Arrow",
                        EdgeType::Line => "Line",
                        EdgeType::DottedArrow => "Dotted Arrow",
                        EdgeType::DottedLine => "Dotted Line",
                        EdgeType::ThickArrow => "Thick Arrow",
                        EdgeType::ThickLine => "Thick Line",
                        _ => "Other",
                    };

                    let current_type = edge.edge_type;
                    egui::ComboBox::from_id_salt("edge_type_combo")
                        .selected_text(edge_type_name(current_type))
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            let types = [
                                EdgeType::Arrow,
                                EdgeType::Line,
                                EdgeType::DottedArrow,
                                EdgeType::DottedLine,
                                EdgeType::ThickArrow,
                                EdgeType::ThickLine,
                            ];
                            for t in types {
                                if ui
                                    .selectable_label(
                                        current_type == t,
                                        edge_type_name(t),
                                    )
                                    .clicked()
                                {
                                    new_type = Some(t);
                                }
                            }
                        });
                }

                if needs_undo || new_type.is_some() {
                    state.push_undo();
                }
                let mut changed = false;
                if let Some(label) = new_label {
                    if let Some(edge) = state.graph.edges.get_mut(edge_idx) {
                        edge.label = label;
                        changed = true;
                    }
                }
                if let Some(t) = new_type {
                    if let Some(edge) = state.graph.edges.get_mut(edge_idx) {
                        edge.edge_type = t;
                        changed = true;
                    }
                }
                if changed {
                    state.dirty = true;
                    state.rebuild_layout();
                }
            }
        });
}

fn render_canvas(state: &mut EditorState, ui: &mut egui::Ui) {
    let canvas_rect = ui.available_rect_before_wrap();
    ui.painter()
        .rect_filled(canvas_rect, 0.0, egui::Color32::WHITE);

    if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        state.interaction = InteractionState::Idle;
        state.selected_nodes.clear();
        state.selected_edge = None;
    }

    let modifiers = ui.ctx().input(|i| i.modifiers);
    if modifiers.command && !modifiers.shift && ui.ctx().input(|i| i.key_pressed(egui::Key::Z)) {
        state.undo();
    }
    if modifiers.command && modifiers.shift && ui.ctx().input(|i| i.key_pressed(egui::Key::Z)) {
        state.redo();
    }
    if modifiers.command && ui.ctx().input(|i| i.key_pressed(egui::Key::S)) {
        if let Some(path) = &state.file_path {
            let text = serialize_for_path(path, &state.graph);
            if let Err(e) = std::fs::write(path, &text) {
                eprintln!("Save error: {e}");
            } else {
                state.dirty = false;
            }
        }
    }
    if modifiers.command && ui.ctx().input(|i| i.key_pressed(egui::Key::N)) {
        *state = EditorState::new();
    }
    if modifiers.command && !modifiers.shift && ui.ctx().input(|i| i.key_pressed(egui::Key::C)) {
        state.copy_selection();
    }
    if modifiers.command && !modifiers.shift && ui.ctx().input(|i| i.key_pressed(egui::Key::V)) {
        state.paste_clipboard();
    }

    if ui.ctx().input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
    {
        let has_selection = !state.selected_nodes.is_empty() || state.selected_edge.is_some();
        if has_selection {
            state.push_undo();
        }
        let to_remove: Vec<String> = state.selected_nodes.drain().collect();
        for id in &to_remove {
            state.graph.nodes.remove(id);
            state.manual_positions.remove(id);
            state
                .graph
                .edges
                .retain(|e| e.from != *id && e.to != *id);
        }
        if let Some(edge_idx) = state.selected_edge.take() {
            if edge_idx < state.graph.edges.len() {
                state.graph.edges.remove(edge_idx);
            }
        }
        if has_selection {
            state.dirty = true;
            state.rebuild_layout();
        }
    }

    let scene_rect_before = state.scene_rect;

    let selected = state.selected_nodes.clone();
    let selected_edge_idx = state.selected_edge;
    let layout_snap = state.layout_result.clone();
    let is_empty = state.graph.nodes.is_empty();

    let connecting_source: Option<(f32, f32)> =
        if let InteractionState::ConnectingEdge { ref source_id } = state.interaction {
            if !source_id.is_empty() {
                state.manual_positions.get(source_id).copied()
            } else {
                None
            }
        } else {
            None
        };

    let viewport_rect = ui.available_rect_before_wrap();

    let mouse_scene_pos = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|p| viewport_rect.contains(*p))
        .map(|p| screen_to_scene(p, viewport_rect, scene_rect_before));

    let hover_node_id = mouse_scene_pos.and_then(|pos| {
        if let Some(layout) = &layout_snap {
            for node in layout.nodes.iter().rev() {
                let rect = egui::Rect::from_center_size(
                    egui::Pos2::new(node.x, node.y),
                    egui::Vec2::new(node.width, node.height),
                );
                if rect.contains(pos) {
                    return Some(node.id.clone());
                }
            }
        }
        None
    });

    let is_connecting = matches!(
        state.interaction,
        InteractionState::ConnectingEdge { .. }
    );

    let _scene_response = egui::Scene::new()
        .zoom_range(0.05..=8.0)
        .show(ui, &mut state.scene_rect, |scene_ui| {
            scene_ui
                .painter()
                .rect_filled(scene_ui.clip_rect(), 0.0, egui::Color32::WHITE);

            draw_grid(scene_ui);

            if let Some(layout_result) = &layout_snap {
                renderer::render_diagram(scene_ui, layout_result, &[]);
                draw_selection_highlights_from(scene_ui, layout_result, &selected);

                if let Some(eidx) = selected_edge_idx {
                    if let Some(edge) = layout_result.edges.get(eidx) {
                        let stroke = egui::Stroke::new(3.0, egui::Color32::from_rgb(70, 130, 200));
                        for pair in edge.points.windows(2) {
                            scene_ui.painter().line_segment(
                                [
                                    egui::Pos2::new(pair[0][0], pair[0][1]),
                                    egui::Pos2::new(pair[1][0], pair[1][1]),
                                ],
                                stroke,
                            );
                        }
                    }
                }

                if let Some((sx, sy)) = connecting_source {
                    for node in &layout_result.nodes {
                        let rect = egui::Rect::from_center_size(
                            egui::Pos2::new(node.x, node.y),
                            egui::Vec2::new(node.width + 6.0, node.height + 6.0),
                        );
                        if (node.x - sx).abs() < 1.0 && (node.y - sy).abs() < 1.0 {
                            scene_ui.painter().rect_stroke(
                                rect,
                                4.0,
                                egui::Stroke::new(
                                    2.5,
                                    egui::Color32::from_rgb(50, 180, 80),
                                ),
                                egui::StrokeKind::Outside,
                            );
                        }
                    }

                    if let Some(mouse_pos) = mouse_scene_pos {
                        scene_ui.painter().line_segment(
                            [
                                egui::Pos2::new(sx, sy),
                                mouse_pos,
                            ],
                            egui::Stroke::new(
                                2.0,
                                egui::Color32::from_rgba_unmultiplied(50, 180, 80, 180),
                            ),
                        );
                    }
                }

                if is_connecting {
                    if let Some(ref hid) = hover_node_id {
                        for node in &layout_result.nodes {
                            if node.id == *hid {
                                let rect = egui::Rect::from_center_size(
                                    egui::Pos2::new(node.x, node.y),
                                    egui::Vec2::new(
                                        node.width + 6.0,
                                        node.height + 6.0,
                                    ),
                                );
                                scene_ui.painter().rect_stroke(
                                    rect,
                                    4.0,
                                    egui::Stroke::new(
                                        2.0,
                                        egui::Color32::from_rgb(200, 140, 40),
                                    ),
                                    egui::StrokeKind::Outside,
                                );
                            }
                        }
                    }
                }
            }

            if is_empty {
                let center = scene_ui.clip_rect().center();
                scene_ui.painter().text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    "Click a shape in the toolbar, then click here to place it",
                    egui::FontId::proportional(16.0),
                    egui::Color32::from_rgb(160, 160, 160),
                );
            }
        });

    if connecting_source.is_some() {
        ui.ctx().request_repaint();
    }

    let primary_clicked = ui.input(|i| i.pointer.primary_clicked());
    if primary_clicked {
        if let Some(pointer_pos) = ui.input(|i| i.pointer.interact_pos()) {
            if viewport_rect.contains(pointer_pos) {
                let scene_pos = screen_to_scene(pointer_pos, viewport_rect, scene_rect_before);
                handle_canvas_click(state, scene_pos);
            }
        }
    }

    let dragging = ui.input(|i| i.pointer.primary_down());
    let drag_delta = ui.input(|i| i.pointer.delta());
    if dragging && drag_delta.length() > 0.0 {
        if let InteractionState::DraggingNode { ref node_id } = state.interaction {
            let scale_x = viewport_rect.width() / scene_rect_before.width();
            let scale_y = viewport_rect.height() / scene_rect_before.height();
            let zoom = scale_x.min(scale_y);
            let scene_delta_x = drag_delta.x / zoom;
            let scene_delta_y = drag_delta.y / zoom;
            if let Some(pos) = state.manual_positions.get_mut(node_id) {
                let snapped = snap_to_grid(pos.0 + scene_delta_x, pos.1 + scene_delta_y);
                *pos = snapped;
                state.dirty = true;
                state.rebuild_layout();
            }
        }
    }

    if !dragging {
        if let InteractionState::DraggingNode { .. } = state.interaction {
            state.interaction = InteractionState::Idle;
        }
    }
}

fn screen_to_scene(
    screen_pos: egui::Pos2,
    viewport: egui::Rect,
    scene_rect: egui::Rect,
) -> egui::Pos2 {
    let scale_x = viewport.width() / scene_rect.width();
    let scale_y = viewport.height() / scene_rect.height();
    let scale = scale_x.min(scale_y);
    let center_screen = viewport.center();
    let center_scene = scene_rect.center();
    egui::Pos2::new(
        (screen_pos.x - center_screen.x) / scale + center_scene.x,
        (screen_pos.y - center_screen.y) / scale + center_scene.y,
    )
}

fn handle_canvas_click(state: &mut EditorState, scene_pos: egui::Pos2) {
    match state.interaction.clone() {
        InteractionState::PlacingNode { shape } => {
            state.place_node(shape, scene_pos);
        }
        InteractionState::ConnectingEdge { ref source_id } if source_id.is_empty() => {
            if let Some(node_id) = state.node_at_scene_pos(scene_pos) {
                state.interaction = InteractionState::ConnectingEdge { source_id: node_id };
            }
        }
        InteractionState::ConnectingEdge { ref source_id } => {
            if let Some(target_id) = state.node_at_scene_pos(scene_pos) {
                if target_id != *source_id {
                    let from = source_id.clone();
                    state.add_edge(from, target_id);
                }
            }
            state.interaction = InteractionState::ConnectingEdge {
                source_id: String::new(),
            };
        }
        InteractionState::Idle | InteractionState::DraggingNode { .. } => {
            if let Some(node_id) = state.node_at_scene_pos(scene_pos) {
                state.selected_nodes.clear();
                state.selected_nodes.insert(node_id.clone());
                state.selected_edge = None;
                state.interaction = InteractionState::DraggingNode { node_id };
            } else if let Some(edge_idx) = state.edge_at_scene_pos(scene_pos) {
                state.selected_nodes.clear();
                state.selected_edge = Some(edge_idx);
            } else {
                state.selected_nodes.clear();
                state.selected_edge = None;
            }
        }
    }
}

const GRID_SPACING: f32 = 50.0;

fn snap_to_grid(x: f32, y: f32) -> (f32, f32) {
    (
        (x / GRID_SPACING).round() * GRID_SPACING,
        (y / GRID_SPACING).round() * GRID_SPACING,
    )
}

fn point_to_segment_distance(
    px: f32,
    py: f32,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-6 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let closest_x = ax + t * dx;
    let closest_y = ay + t * dy;
    ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt()
}

fn draw_grid(ui: &mut egui::Ui) {
    let clip = ui.clip_rect();
    let grid_spacing = GRID_SPACING;
    let grid_color = egui::Color32::from_rgb(235, 235, 235);
    let stroke = egui::Stroke::new(0.5, grid_color);

    let start_x = (clip.left() / grid_spacing).floor() as i32;
    let end_x = (clip.right() / grid_spacing).ceil() as i32;
    let start_y = (clip.top() / grid_spacing).floor() as i32;
    let end_y = (clip.bottom() / grid_spacing).ceil() as i32;

    for ix in start_x..=end_x {
        let x = ix as f32 * grid_spacing;
        ui.painter().line_segment(
            [
                egui::Pos2::new(x, clip.top()),
                egui::Pos2::new(x, clip.bottom()),
            ],
            stroke,
        );
    }
    for iy in start_y..=end_y {
        let y = iy as f32 * grid_spacing;
        ui.painter().line_segment(
            [
                egui::Pos2::new(clip.left(), y),
                egui::Pos2::new(clip.right(), y),
            ],
            stroke,
        );
    }
}

fn draw_selection_highlights_from(
    ui: &mut egui::Ui,
    layout: &layout::LayoutResult,
    selected: &HashSet<String>,
) {
    for node in &layout.nodes {
        if selected.contains(&node.id) {
            let rect = egui::Rect::from_center_size(
                egui::Pos2::new(node.x, node.y),
                egui::Vec2::new(node.width + 6.0, node.height + 6.0),
            );
            ui.painter().rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(70, 130, 200)),
                egui::StrokeKind::Outside,
            );
        }
    }
}
