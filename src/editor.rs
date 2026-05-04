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
}

#[derive(Debug, Clone, PartialEq)]
pub enum InteractionState {
    Idle,
    PlacingNode { shape: NodeShape },
    ConnectingEdge { source_id: String },
    DraggingNode { node_id: String },
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
        }
    }

    fn gen_node_id(&mut self) -> String {
        let id = format!("node_{}", self.next_node_id);
        self.next_node_id += 1;
        id
    }

    fn place_node(&mut self, shape: NodeShape, scene_pos: egui::Pos2) {
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
        self.manual_positions
            .insert(id, (scene_pos.x, scene_pos.y));
        self.dirty = true;
        self.rebuild_layout();
    }

    fn add_edge(&mut self, from: String, to: String) {
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

    fn auto_layout(&mut self) {
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

const TOOLBAR_SHAPES: &[(NodeShape, &str)] = &[
    (NodeShape::Rect, "Rect"),
    (NodeShape::Rounded, "Rounded"),
    (NodeShape::Diamond, "Diamond"),
    (NodeShape::Circle, "Circle"),
    (NodeShape::Hexagon, "Hexagon"),
    (NodeShape::Parallelogram, "Parallel"),
    (NodeShape::Stadium, "Stadium"),
    (NodeShape::Cylinder, "Cylinder"),
];

pub fn render_editor_ui(state: &mut EditorState, ui: &mut egui::Ui) -> EditorAction {
    let action = render_editor_menu(state, ui);

    let measured = renderer::measure_node_texts(ui, &Some(state.graph.clone()));
    if let Some(sizes) = measured {
        if state.node_sizes.as_ref() != Some(&sizes) {
            state.node_sizes = Some(sizes);
            state.rebuild_layout();
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
                    if ui.button("Open...").clicked() {
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
                    ui.separator();
                    if ui.button("Save").clicked() {
                        ui.close();
                        if let Some(path) = &state.file_path {
                            let text = serializer::mermaid::serialize(&state.graph);
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
                            .set_file_name("diagram.mmd");
                        if let Some(path) = dialog.save_file() {
                            let text = serializer::mermaid::serialize(&state.graph);
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

fn render_toolbar(state: &mut EditorState, ui: &mut egui::Ui) {
    egui::Panel::left("editor_toolbar")
        .resizable(false)
        .exact_size(80.0)
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(240, 240, 240))
                .inner_margin(4.0)
                .stroke(egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgb(200, 200, 200),
                )),
        )
        .show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Shapes")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(80, 80, 80)),
                );
                ui.add_space(4.0);

                for &(shape, name) in TOOLBAR_SHAPES {
                    let is_active = state.interaction == InteractionState::PlacingNode { shape };
                    let btn = egui::Button::new(
                        egui::RichText::new(name)
                            .size(11.0)
                            .color(if is_active {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::from_rgb(40, 40, 40)
                            }),
                    )
                    .min_size(egui::Vec2::new(72.0, 28.0))
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

                    if ui.add(btn).clicked() {
                        if is_active {
                            state.interaction = InteractionState::Idle;
                        } else {
                            state.interaction = InteractionState::PlacingNode { shape };
                        }
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                ui.label(
                    egui::RichText::new("Tools")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(80, 80, 80)),
                );
                ui.add_space(4.0);

                let edge_active = matches!(
                    state.interaction,
                    InteractionState::ConnectingEdge { .. }
                );
                let edge_btn = egui::Button::new(
                    egui::RichText::new("Edge")
                        .size(11.0)
                        .color(if edge_active {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_rgb(40, 40, 40)
                        }),
                )
                .min_size(egui::Vec2::new(72.0, 28.0))
                .fill(if edge_active {
                    egui::Color32::from_rgb(70, 130, 200)
                } else {
                    egui::Color32::from_rgb(255, 255, 255)
                })
                .stroke(egui::Stroke::new(
                    1.0,
                    if edge_active {
                        egui::Color32::from_rgb(50, 100, 170)
                    } else {
                        egui::Color32::from_rgb(180, 180, 180)
                    },
                ));

                if ui.add(edge_btn).clicked() {
                    state.interaction = if edge_active {
                        InteractionState::Idle
                    } else {
                        InteractionState::ConnectingEdge {
                            source_id: String::new(),
                        }
                    };
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Auto Layout")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(40, 40, 40)),
                        )
                        .min_size(egui::Vec2::new(72.0, 28.0))
                        .fill(egui::Color32::from_rgb(255, 255, 255))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(180, 180, 180),
                        )),
                    )
                    .clicked()
                {
                    state.auto_layout();
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

                if let Some(node) = state.graph.nodes.get_mut(&node_id) {
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
                    if response.changed() {
                        node.label = label;
                        state.dirty = true;
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
                    let shape_name = |s: NodeShape| match s {
                        NodeShape::Rect => "Rect",
                        NodeShape::Rounded => "Rounded",
                        NodeShape::Diamond => "Diamond",
                        NodeShape::Circle => "Circle",
                        NodeShape::Hexagon => "Hexagon",
                        NodeShape::Parallelogram => "Parallel",
                        NodeShape::Stadium => "Stadium",
                        NodeShape::Cylinder => "Cylinder",
                        _ => "Other",
                    };

                    egui::ComboBox::from_id_salt("shape_combo")
                        .selected_text(shape_name(current_shape))
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            for &(shape, name) in TOOLBAR_SHAPES {
                                if ui
                                    .selectable_label(current_shape == shape, name)
                                    .clicked()
                                {
                                    node.shape = shape;
                                    state.dirty = true;
                                }
                            }
                        });
                }
            } else if let Some(edge_idx) = state.selected_edge {
                ui.label(
                    egui::RichText::new("Edge")
                        .size(12.0)
                        .strong()
                        .color(egui::Color32::from_rgb(60, 60, 60)),
                );
                ui.add_space(4.0);

                if let Some(edge) = state.graph.edges.get_mut(edge_idx) {
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
                    if response.changed() {
                        edge.label = if label.is_empty() {
                            None
                        } else {
                            Some(label)
                        };
                        state.dirty = true;
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
                                    edge.edge_type = t;
                                    state.dirty = true;
                                }
                            }
                        });
                }
            }
        });
}

fn render_canvas(state: &mut EditorState, ui: &mut egui::Ui) {
    if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        state.interaction = InteractionState::Idle;
        state.selected_nodes.clear();
        state.selected_edge = None;
    }

    if ui.ctx().input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
    {
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
        if !to_remove.is_empty() {
            state.dirty = true;
            state.rebuild_layout();
        }
    }

    let scene_rect_before = state.scene_rect;

    let selected = state.selected_nodes.clone();
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
                pos.0 += scene_delta_x;
                pos.1 += scene_delta_y;
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
            } else {
                state.selected_nodes.clear();
                state.selected_edge = None;
            }
        }
    }
}

fn draw_grid(ui: &mut egui::Ui) {
    let clip = ui.clip_rect();
    let grid_spacing = 50.0;
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
