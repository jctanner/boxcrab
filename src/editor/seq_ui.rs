//! Editor UI for sequence diagrams: toolbar, canvas interaction and
//! properties panel. The data model operations live in `crate::seq_model`.

use super::*;
use crate::layout::sequence as seq_layout;
use crate::seq_model as sm;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SeqTool {
    #[default]
    Select,
    Participant,
    Actor,
    /// Drag between two lifelines.
    Message,
    /// Click a lifeline.
    SelfMessage,
    /// Click a lifeline.
    Note,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SeqDrag {
    Message { from: String, slot: usize },
    Participant { id: String },
    /// Dragging a message up or down the timeline.
    MoveMessage { slot: usize, press: egui::Pos2 },
    /// Dragging a note: vertically along the timeline, horizontally to
    /// attach it to another participant.
    MoveNote { id: String, press: egui::Pos2 },
    /// Dragging one end of the selected message onto another lifeline.
    Endpoint { slot: usize, is_from: bool },
}

/// Slots moved by a vertical drag of `dy` scene units.
fn slot_delta(dy: f32) -> isize {
    (dy / (seq_layout::slot_y(1) - seq_layout::slot_y(0))).round() as isize
}

/// Move the timeline entry at `from` to `to` by successive adjacent swaps.
fn move_slot(state: &mut EditorState, from: usize, to: usize) {
    let mut cur = from;
    while cur != to {
        let next = if to > cur { cur + 1 } else { cur - 1 };
        sm::swap_slots(&mut state.graph, cur, next);
        cur = next;
    }
}

/// Hit-test geometry derived from the current layout.
struct Geo {
    /// (id, x, box top, box bottom, half width), sorted left to right.
    parts: Vec<(String, f32, f32, f32, f32)>,
    n: usize,
    life_top: f32,
    life_bottom: f32,
}

impl Geo {
    fn from_state(state: &EditorState) -> Geo {
        let n = state.graph.edges.len();
        let (life_top, life_bottom) = seq_layout::lifeline_range(n);
        let mut parts = Vec::new();
        if let Some(layout) = &state.layout_result {
            for node in &layout.nodes {
                if state.graph.nodes.contains_key(&node.id) && !sm::is_note(&node.id) {
                    parts.push((
                        node.id.clone(),
                        node.x,
                        node.y - node.height / 2.0,
                        node.y + node.height / 2.0,
                        node.width / 2.0,
                    ));
                }
            }
        }
        parts.sort_by(|a, b| a.1.total_cmp(&b.1));
        Geo { parts, n, life_top, life_bottom }
    }

    fn x_of(&self, id: &str) -> Option<f32> {
        self.parts.iter().find(|p| p.0 == id).map(|p| p.1)
    }

    fn box_at(&self, pos: egui::Pos2) -> Option<&str> {
        self.parts
            .iter()
            .find(|p| (pos.x - p.1).abs() <= p.4 && pos.y >= p.2 && pos.y <= p.3)
            .map(|p| p.0.as_str())
    }

    /// A participant whose box or lifeline is under `pos`.
    fn lifeline_at(&self, pos: egui::Pos2) -> Option<&str> {
        if let Some(id) = self.box_at(pos) {
            return Some(id);
        }
        self.parts
            .iter()
            .find(|p| (pos.x - p.1).abs() <= 14.0 && pos.y >= self.life_top && pos.y <= self.life_bottom)
            .map(|p| p.0.as_str())
    }

    fn nearest_to_x(&self, x: f32, max_dist: f32) -> Option<&str> {
        self.parts
            .iter()
            .map(|p| (p, (p.1 - x).abs()))
            .filter(|(_, d)| *d <= max_dist)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(p, _)| p.0.as_str())
    }

    /// Where a participant dropped at `x` would land among the others.
    fn index_for_x(&self, x: f32, skip: Option<&str>) -> usize {
        self.parts
            .iter()
            .filter(|p| Some(p.0.as_str()) != skip)
            .filter(|p| p.1 < x)
            .count()
    }
}

pub fn render(state: &mut EditorState, ui: &mut egui::Ui) {
    render_toolbar(state, ui);
    render_panel(state, ui);
    render_canvas(state, ui);
}

fn finish_edit(state: &mut EditorState) {
    state.dirty = true;
    state.rebuild_layout();
}

// ---------------------------------------------------------------------------
// Toolbar
// ---------------------------------------------------------------------------

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
            ui.label(
                egui::RichText::new("Sequence")
                    .size(9.0)
                    .color(egui::Color32::from_rgb(120, 120, 120)),
            );
            let tools = [
                (SeqTool::Select, "Select"),
                (SeqTool::Participant, "Participant"),
                (SeqTool::Actor, "Actor"),
                (SeqTool::Message, "Message"),
                (SeqTool::SelfMessage, "Self msg"),
                (SeqTool::Note, "Note"),
            ];
            for (tool, name) in tools {
                let active = state.seq_tool == tool;
                if text_tool_button(ui, name, active).clicked() {
                    state.seq_tool = if active { SeqTool::Select } else { tool };
                    state.seq_drag = None;
                }
                ui.add_space(2.0);
            }

            ui.separator();
            ui.label(
                egui::RichText::new("Arrow")
                    .size(9.0)
                    .color(egui::Color32::from_rgb(120, 120, 120)),
            );
            let current = state.seq_arrow.min(sm::ARROW_STYLES.len() - 1);
            egui::ComboBox::from_id_salt("seq_arrow_style")
                .selected_text(sm::ARROW_STYLES[current].0.rsplit(' ').next().unwrap_or(""))
                .width(54.0)
                .show_ui(ui, |ui| {
                    for (i, (name, _, _)) in sm::ARROW_STYLES.iter().enumerate() {
                        if ui.selectable_label(current == i, *name).clicked() {
                            state.seq_arrow = i;
                        }
                    }
                });

            ui.add_space(6.0);
            let hint = match state.seq_tool {
                SeqTool::Select => "Click to select. Drag to move: participants reorder, messages and notes move along the timeline.",
                SeqTool::Participant | SeqTool::Actor => "Click the canvas to add.",
                SeqTool::Message => "Drag from one lifeline to another.",
                SeqTool::SelfMessage => "Click a lifeline.",
                SeqTool::Note => "Click a lifeline.",
            };
            ui.label(
                egui::RichText::new(hint)
                    .size(9.0)
                    .color(egui::Color32::from_rgb(120, 120, 120)),
            );
        });
}

// ---------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------

fn save_current(state: &mut EditorState) {
    if let Some(path) = &state.file_path {
        let text = serialize_for_path(path, &state.graph);
        if let Err(e) = std::fs::write(path, &text) {
            eprintln!("Save error: {e}");
        } else {
            state.dirty = false;
        }
    }
}

fn delete_selection(state: &mut EditorState) {
    if state.selected_edge.is_none() && state.selected_nodes.is_empty() {
        return;
    }
    state.push_undo();
    if let Some(idx) = state.selected_edge.take() {
        sm::remove_slot(&mut state.graph, idx);
    }
    let ids: Vec<String> = state.selected_nodes.drain().collect();
    for id in ids {
        if sm::is_note(&id) {
            if let Some(slot) = sm::slot_of_note(&state.graph, &id) {
                sm::remove_slot(&mut state.graph, slot);
            }
        } else {
            sm::remove_participant(&mut state.graph, &id);
        }
    }
    finish_edit(state);
}

/// Index into the layout's edge list of the message at graph edge `idx`
/// (notes have no layout edge, so they are skipped).
fn layout_edge_index(state: &EditorState, idx: usize) -> Option<usize> {
    let e = state.graph.edges.get(idx)?;
    if sm::is_note_edge(e) {
        return None;
    }
    Some(state.graph.edges[..idx].iter().filter(|e| !sm::is_note_edge(e)).count())
}

fn message_at(state: &EditorState, pos: egui::Pos2) -> Option<usize> {
    let layout = state.layout_result.as_ref()?;
    let mut k = 0;
    for (gi, e) in state.graph.edges.iter().enumerate() {
        if sm::is_note_edge(e) {
            continue;
        }
        let le = layout.edges.get(k)?;
        k += 1;
        let poly = le.polyline();
        for pair in poly.windows(2) {
            let d = point_to_segment_distance(
                pos.x, pos.y, pair[0][0], pair[0][1], pair[1][0], pair[1][1],
            );
            if d < 8.0 {
                return Some(gi);
            }
        }
        // Clicking the label counts too.
        if let Some(lp) = le.label_pos {
            let half_w = le.label.as_deref().map_or(0.0, seq_layout::estimate_label_width) / 2.0;
            if le.label.is_some() && (pos.x - lp[0]).abs() < half_w && (pos.y - lp[1]).abs() < 8.0 {
                return Some(gi);
            }
        }
    }
    None
}

/// Scene positions of the two ends of the message at graph edge `idx`
/// (None for self messages, which can't be re-attached by dragging).
fn endpoints(state: &EditorState, idx: usize) -> Option<(egui::Pos2, egui::Pos2)> {
    let e = state.graph.edges.get(idx)?;
    if e.from == e.to {
        return None;
    }
    let k = layout_edge_index(state, idx)?;
    let le = state.layout_result.as_ref()?.edges.get(k)?;
    let a = le.points.first()?;
    let b = le.points.last()?;
    Some((egui::Pos2::new(a[0], a[1]), egui::Pos2::new(b[0], b[1])))
}

fn note_at(state: &EditorState, pos: egui::Pos2) -> Option<String> {
    let layout = state.layout_result.as_ref()?;
    layout
        .nodes
        .iter()
        .rev()
        .filter(|n| sm::is_note(&n.id) && state.graph.nodes.contains_key(&n.id))
        .find(|n| {
            egui::Rect::from_center_size(
                egui::Pos2::new(n.x, n.y),
                egui::Vec2::new(n.width, n.height),
            )
            .contains(pos)
        })
        .map(|n| n.id.clone())
}

fn handle_press(state: &mut EditorState, pos: egui::Pos2) {
    let geo = Geo::from_state(state);
    match state.seq_tool {
        SeqTool::Select => {
            let grabbed_end = state.selected_edge.and_then(|idx| {
                let (a, b) = endpoints(state, idx)?;
                if a.distance(pos) < 10.0 {
                    Some((idx, true))
                } else if b.distance(pos) < 10.0 {
                    Some((idx, false))
                } else {
                    None
                }
            });
            if let Some((slot, is_from)) = grabbed_end {
                state.seq_drag = Some(SeqDrag::Endpoint { slot, is_from });
            } else if let Some(id) = geo.box_at(pos).map(str::to_string) {
                state.selected_nodes.clear();
                state.selected_nodes.insert(id.clone());
                state.selected_edge = None;
                state.seq_drag = Some(SeqDrag::Participant { id });
            } else if let Some(id) = note_at(state, pos) {
                state.selected_nodes.clear();
                state.selected_nodes.insert(id.clone());
                state.selected_edge = None;
                state.seq_drag = Some(SeqDrag::MoveNote { id, press: pos });
            } else if let Some(idx) = message_at(state, pos) {
                state.selected_nodes.clear();
                state.selected_edge = Some(idx);
                state.seq_drag = Some(SeqDrag::MoveMessage { slot: idx, press: pos });
            } else {
                state.selected_nodes.clear();
                state.selected_edge = None;
            }
        }
        SeqTool::Participant | SeqTool::Actor => {
            let shape = if state.seq_tool == SeqTool::Actor {
                NodeShape::Person
            } else {
                NodeShape::Rect
            };
            let index = geo.index_for_x(pos.x, None);
            state.push_undo();
            let id = sm::add_participant(&mut state.graph, index, shape);
            state.selected_nodes.clear();
            state.selected_nodes.insert(id);
            state.selected_edge = None;
            finish_edit(state);
        }
        SeqTool::Message => {
            if let Some(from) = geo.lifeline_at(pos).map(str::to_string) {
                let slot = seq_layout::slot_at_y(pos.y, geo.n);
                state.seq_drag = Some(SeqDrag::Message { from, slot });
            }
        }
        SeqTool::SelfMessage => {
            if let Some(id) = geo.lifeline_at(pos).map(str::to_string) {
                let slot = seq_layout::slot_at_y(pos.y, geo.n);
                state.push_undo();
                sm::add_message(&mut state.graph, slot, &id, &id, state.seq_arrow);
                state.selected_nodes.clear();
                state.selected_edge = Some(slot);
                finish_edit(state);
            }
        }
        SeqTool::Note => {
            if let Some(id) = geo.lifeline_at(pos).map(str::to_string) {
                let slot = seq_layout::slot_at_y(pos.y, geo.n);
                state.push_undo();
                let note = sm::add_note(&mut state.graph, slot, &id, "right");
                state.selected_nodes.clear();
                state.selected_nodes.insert(note);
                state.selected_edge = None;
                finish_edit(state);
            }
        }
    }
}

fn handle_release(state: &mut EditorState, pos: Option<egui::Pos2>) {
    let Some(drag) = state.seq_drag.take() else { return };
    let Some(pos) = pos else { return };
    let geo = Geo::from_state(state);
    match drag {
        SeqDrag::Message { from, slot } => {
            if let Some(to) = geo.nearest_to_x(pos.x, 110.0).map(str::to_string) {
                if to != from {
                    state.push_undo();
                    sm::add_message(&mut state.graph, slot, &from, &to, state.seq_arrow);
                    state.selected_nodes.clear();
                    state.selected_edge = Some(slot);
                    finish_edit(state);
                }
            }
        }
        SeqDrag::MoveMessage { slot, press } => {
            let n = state.graph.edges.len();
            let target = (slot as isize + slot_delta(pos.y - press.y)).clamp(0, n as isize - 1) as usize;
            if target != slot {
                state.push_undo();
                move_slot(state, slot, target);
                state.selected_edge = Some(target);
                finish_edit(state);
            }
        }
        SeqDrag::MoveNote { id, press } => {
            let Some(slot) = sm::slot_of_note(&state.graph, &id) else { return };
            let n = state.graph.edges.len();
            let target = (slot as isize + slot_delta(pos.y - press.y)).clamp(0, n as isize - 1) as usize;
            let new_owner = if (pos.x - press.x).abs() > 30.0 {
                geo.nearest_to_x(pos.x, 110.0)
                    .map(str::to_string)
                    .filter(|p| *p != state.graph.edges[slot].from)
            } else {
                None
            };
            if target != slot || new_owner.is_some() {
                state.push_undo();
                if let Some(owner) = new_owner {
                    let old = sm::name_of(&state.graph.edges[slot].from).to_string();
                    let new_name = sm::name_of(&owner).to_string();
                    state.graph.edges[slot].from = owner;
                    if let Some(def) = state.graph.nodes.get_mut(&id) {
                        let mut info = sm::note_info(def);
                        match info.names.iter().position(|n| *n == old) {
                            Some(i) => info.names[i] = new_name,
                            None => info.names = vec![new_name],
                        }
                        sm::set_note_info(def, &info);
                    }
                }
                move_slot(state, slot, target);
                finish_edit(state);
            }
        }
        SeqDrag::Endpoint { slot, is_from } => {
            if let Some(target) = geo.nearest_to_x(pos.x, 110.0).map(str::to_string) {
                let e = &state.graph.edges[slot];
                let other = if is_from { &e.to } else { &e.from };
                let current = if is_from { &e.from } else { &e.to };
                if target != *other && target != *current {
                    state.push_undo();
                    let e = &mut state.graph.edges[slot];
                    if is_from {
                        e.from = target;
                    } else {
                        e.to = target;
                    }
                    finish_edit(state);
                }
            }
        }
        SeqDrag::Participant { id } => {
            let current = geo.parts.iter().position(|p| p.0 == id);
            let target = geo.index_for_x(pos.x, Some(&id));
            if current.is_some() && current != Some(target) {
                state.push_undo();
                let new_id = sm::move_participant(&mut state.graph, &id, target);
                state.selected_nodes.clear();
                state.selected_nodes.insert(new_id);
                finish_edit(state);
            }
        }
    }
}

fn render_canvas(state: &mut EditorState, ui: &mut egui::Ui) {
    let canvas_rect = ui.available_rect_before_wrap();
    ui.painter()
        .rect_filled(canvas_rect, 0.0, egui::Color32::WHITE);

    let ctx = ui.ctx().clone();
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.seq_tool = SeqTool::Select;
        state.seq_drag = None;
        state.selected_nodes.clear();
        state.selected_edge = None;
    }
    let modifiers = ctx.input(|i| i.modifiers);
    if modifiers.command && !modifiers.shift && ctx.input(|i| i.key_pressed(egui::Key::Z)) {
        state.undo();
    }
    if modifiers.command && modifiers.shift && ctx.input(|i| i.key_pressed(egui::Key::Z)) {
        state.redo();
    }
    if modifiers.command && ctx.input(|i| i.key_pressed(egui::Key::S)) {
        save_current(state);
    }
    let text_editing = ctx.memory(|m| m.focused().is_some());
    if !text_editing
        && ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
    {
        delete_selection(state);
    }

    let viewport_rect = ui.available_rect_before_wrap();
    let scene_rect_before = sync_scene_rect(state, viewport_rect);
    let mouse_scene_pos = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|p| viewport_rect.contains(*p))
        .map(|p| screen_to_scene(p, viewport_rect, scene_rect_before));

    let geo = Geo::from_state(state);
    let layout_snap = state.layout_result.clone();
    let selected = state.selected_nodes.clone();
    let selected_layout_edge = state
        .selected_edge
        .and_then(|idx| layout_edge_index(state, idx));
    let tool = state.seq_tool;
    let drag = state.seq_drag.clone();
    let state_graph = state.graph.clone();
    let endpoints_snap = match &drag {
        Some(SeqDrag::Endpoint { slot, .. }) => endpoints(state, *slot),
        _ => None,
    };
    let handles = state.selected_edge.and_then(|idx| endpoints(state, idx));
    let is_empty = state.graph.nodes.is_empty();
    let blue = egui::Color32::from_rgb(70, 130, 200);

    let pan_buttons = if drag.is_some() || tool != SeqTool::Select || !selected.is_empty() {
        egui::DragPanButtons::MIDDLE | egui::DragPanButtons::SECONDARY
    } else {
        egui::DragPanButtons::all()
    };

    egui::Scene::new()
        .zoom_range(0.05..=8.0)
        .drag_pan_buttons(pan_buttons)
        .show(ui, &mut state.scene_rect, |scene_ui| {
            scene_ui
                .painter()
                .rect_filled(scene_ui.clip_rect(), 0.0, egui::Color32::WHITE);

            if let Some(layout) = &layout_snap {
                renderer::render_diagram(scene_ui, layout, &[]);
                draw_selection_highlights_from(scene_ui, layout, &selected);

                if let Some(k) = selected_layout_edge {
                    if let Some(edge) = layout.edges.get(k) {
                        let poly = edge.polyline();
                        for pair in poly.windows(2) {
                            scene_ui.painter().line_segment(
                                [
                                    egui::Pos2::new(pair[0][0], pair[0][1]),
                                    egui::Pos2::new(pair[1][0], pair[1][1]),
                                ],
                                egui::Stroke::new(3.0_f32, blue),
                            );
                        }
                    }
                }
            }

            if let (Some((a, b)), None) = (handles, &drag) {
                for p in [a, b] {
                    scene_ui.painter().circle_filled(p, 5.0_f32, egui::Color32::WHITE);
                    scene_ui
                        .painter()
                        .circle_stroke(p, 5.0_f32, egui::Stroke::new(2.0_f32, blue));
                }
            }

            // Tool previews
            if let Some(mp) = mouse_scene_pos {
                match (&drag, tool) {
                    (Some(SeqDrag::Message { from, slot }), _) => {
                        if let Some(fx) = geo.x_of(from) {
                            let y = seq_layout::slot_y(*slot);
                            let target = geo.nearest_to_x(mp.x, 110.0).filter(|t| *t != from);
                            let end_x = target.and_then(|t| geo.x_of(t)).unwrap_or(mp.x);
                            scene_ui.painter().line_segment(
                                [egui::Pos2::new(fx, y), egui::Pos2::new(end_x, y)],
                                egui::Stroke::new(2.0_f32, blue),
                            );
                            if target.is_some() {
                                scene_ui
                                    .painter()
                                    .circle_filled(egui::Pos2::new(end_x, y), 5.0_f32, blue);
                            }
                        }
                    }
                    (Some(SeqDrag::MoveMessage { slot, press }), _) => {
                        let n = geo.n as isize;
                        let target =
                            (*slot as isize + slot_delta(mp.y - press.y)).clamp(0, n - 1) as usize;
                        let y = seq_layout::slot_y(target);
                        if let (Some(first), Some(last)) = (geo.parts.first(), geo.parts.last()) {
                            scene_ui.painter().line_segment(
                                [
                                    egui::Pos2::new(first.1 - 30.0, y),
                                    egui::Pos2::new(last.1 + 30.0, y),
                                ],
                                egui::Stroke::new(
                                    1.5_f32,
                                    egui::Color32::from_rgba_unmultiplied(70, 130, 200, 160),
                                ),
                            );
                        }
                    }
                    (Some(SeqDrag::MoveNote { id, press }), _) => {
                        let cur = sm::slot_of_note(&state_graph, id).unwrap_or(0);
                        let n = geo.n as isize;
                        let target =
                            (cur as isize + slot_delta(mp.y - press.y)).clamp(0, n - 1) as usize;
                        let y = seq_layout::slot_y(target);
                        if let Some(first) = geo.parts.first() {
                            scene_ui.painter().line_segment(
                                [
                                    egui::Pos2::new(first.1 - 30.0, y),
                                    egui::Pos2::new(geo.parts.last().map_or(first.1, |l| l.1) + 30.0, y),
                                ],
                                egui::Stroke::new(
                                    1.5_f32,
                                    egui::Color32::from_rgba_unmultiplied(70, 130, 200, 160),
                                ),
                            );
                        }
                        if (mp.x - press.x).abs() > 30.0 {
                            if let Some(x) = geo.nearest_to_x(mp.x, 110.0).and_then(|p| geo.x_of(p)) {
                                scene_ui
                                    .painter()
                                    .circle_filled(egui::Pos2::new(x, y), 5.0_f32, blue);
                            }
                        }
                    }
                    (Some(SeqDrag::Endpoint { slot, is_from }), _) => {
                        if let Some((a, b)) = endpoints_snap {
                            let (fixed, _) = if *is_from { (b, a) } else { (a, b) };
                            let target = geo.nearest_to_x(mp.x, 110.0);
                            let end_x = target.and_then(|t| geo.x_of(t)).unwrap_or(mp.x);
                            let y = fixed.y;
                            scene_ui.painter().line_segment(
                                [egui::Pos2::new(fixed.x, y), egui::Pos2::new(end_x, y)],
                                egui::Stroke::new(2.0_f32, blue),
                            );
                            if target.is_some() {
                                scene_ui
                                    .painter()
                                    .circle_filled(egui::Pos2::new(end_x, y), 5.0_f32, blue);
                            }
                        }
                        let _ = slot;
                    }
                    (Some(SeqDrag::Participant { id }), _) => {
                        if let Some(p) = geo.parts.iter().find(|p| p.0 == *id) {
                            let ghost = egui::Rect::from_min_max(
                                egui::Pos2::new(mp.x - p.4, p.2),
                                egui::Pos2::new(mp.x + p.4, p.3),
                            );
                            scene_ui.painter().rect_stroke(
                                ghost,
                                4.0_f32,
                                egui::Stroke::new(
                                    2.0_f32,
                                    egui::Color32::from_rgba_unmultiplied(70, 130, 200, 200),
                                ),
                                egui::StrokeKind::Outside,
                            );
                        }
                        let idx = geo.index_for_x(mp.x, Some(id));
                        let others: Vec<f32> = geo
                            .parts
                            .iter()
                            .filter(|p| p.0 != *id)
                            .map(|p| p.1)
                            .collect();
                        let x = match (idx.checked_sub(1).and_then(|i| others.get(i)), others.get(idx)) {
                            (Some(l), Some(r)) => (l + r) / 2.0,
                            (Some(l), None) => l + 100.0,
                            (None, Some(r)) => r - 100.0,
                            (None, None) => mp.x,
                        };
                        scene_ui.painter().line_segment(
                            [
                                egui::Pos2::new(x, geo.life_top - 40.0),
                                egui::Pos2::new(x, geo.life_bottom + 40.0),
                            ],
                            egui::Stroke::new(2.0_f32, blue),
                        );
                    }
                    (None, SeqTool::Message | SeqTool::SelfMessage | SeqTool::Note) => {
                        if let Some(id) = geo.lifeline_at(mp) {
                            if let Some(x) = geo.x_of(id) {
                                let y = seq_layout::slot_y(seq_layout::slot_at_y(mp.y, geo.n));
                                scene_ui
                                    .painter()
                                    .circle_filled(egui::Pos2::new(x, y), 5.0_f32, blue);
                            }
                        }
                    }
                    (None, SeqTool::Participant | SeqTool::Actor) => {
                        let idx = geo.index_for_x(mp.x, None);
                        let xs: Vec<f32> = geo.parts.iter().map(|p| p.1).collect();
                        let x = match (idx.checked_sub(1).and_then(|i| xs.get(i)), xs.get(idx)) {
                            (Some(l), Some(r)) => (l + r) / 2.0,
                            (Some(l), None) => l + 100.0,
                            (None, Some(r)) => r - 100.0,
                            (None, None) => mp.x,
                        };
                        scene_ui.painter().line_segment(
                            [
                                egui::Pos2::new(x, seq_layout::lifeline_range(0).0 - 40.0),
                                egui::Pos2::new(x, geo.life_bottom + 40.0),
                            ],
                            egui::Stroke::new(
                                1.5_f32,
                                egui::Color32::from_rgba_unmultiplied(70, 130, 200, 140),
                            ),
                        );
                    }
                    _ => {}
                }
            }

            if is_empty {
                scene_ui.painter().text(
                    scene_ui.clip_rect().center(),
                    egui::Align2::CENTER_CENTER,
                    "Pick Participant or Actor in the toolbar, then click here",
                    egui::FontId::proportional(16.0),
                    egui::Color32::from_rgb(160, 160, 160),
                );
            }
        });

    if state.seq_drag.is_some() || tool != SeqTool::Select {
        ui.ctx().request_repaint();
    }

    let primary_pressed = ui.input(|i| i.pointer.primary_pressed());
    if primary_pressed {
        if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
            if viewport_rect.contains(p) {
                let scene_pos = screen_to_scene(p, viewport_rect, scene_rect_before);
                handle_press(state, scene_pos);
            }
        }
    }

    if state.seq_drag.is_some() && !ui.input(|i| i.pointer.primary_down()) {
        let pos = ui
            .input(|i| i.pointer.latest_pos())
            .filter(|p| viewport_rect.contains(*p))
            .map(|p| screen_to_scene(p, viewport_rect, scene_rect_before));
        handle_release(state, pos);
    }
}

// ---------------------------------------------------------------------------
// Properties panel
// ---------------------------------------------------------------------------

enum Selection {
    Participant(String),
    Note(String),
    Message(usize),
}

fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(11.0)
            .color(egui::Color32::from_rgb(80, 80, 80)),
    );
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(12.0)
            .strong()
            .color(egui::Color32::from_rgb(60, 60, 60)),
    );
    ui.add_space(4.0);
}

/// Single-line text field bound to `value`. Returns true when edited; sets
/// `focus_gained` the first frame the field is focused (to snapshot undo).
fn text_field(ui: &mut egui::Ui, value: &mut String, focus_gained: &mut bool) -> bool {
    let response = ui.add(
        egui::TextEdit::singleline(value)
            .desired_width(160.0)
            .font(egui::FontId::proportional(12.0)),
    );
    if response.gained_focus() {
        *focus_gained = true;
    }
    response.changed()
}

fn render_panel(state: &mut EditorState, ui: &mut egui::Ui) {
    let selection = if let Some(id) = state.selected_nodes.iter().next().cloned() {
        if sm::is_note(&id) {
            Selection::Note(id)
        } else {
            Selection::Participant(id)
        }
    } else if let Some(idx) = state.selected_edge {
        Selection::Message(idx)
    } else {
        return;
    };

    egui::Panel::right("properties_panel")
        .resizable(false)
        .exact_size(180.0)
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(245, 245, 245))
                .inner_margin(8.0)
                .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(200, 200, 200))),
        )
        .show_inside(ui, |ui| {
            ui.visuals_mut().override_text_color = Some(egui::Color32::BLACK);
            ui.visuals_mut().extreme_bg_color = egui::Color32::WHITE;
            let wv = &mut ui.visuals_mut().widgets;
            wv.inactive.bg_fill = egui::Color32::WHITE;
            wv.inactive.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(180, 180, 180));
            wv.hovered.bg_fill = egui::Color32::WHITE;
            wv.hovered.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(120, 120, 120));
            wv.active.bg_fill = egui::Color32::WHITE;
            wv.active.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(70, 130, 200));

            match selection {
                Selection::Participant(id) => participant_panel(state, ui, &id),
                Selection::Note(id) => note_panel(state, ui, &id),
                Selection::Message(idx) => message_panel(state, ui, idx),
            }
        });
}

fn participant_panel(state: &mut EditorState, ui: &mut egui::Ui, id: &str) {
    let Some(def) = state.graph.nodes.get(id) else { return };
    heading(ui, "Participant");

    let mut label = def.label.clone();
    let mut shape = def.shape;
    let mut focus = false;
    field_label(ui, "Label:");
    let label_changed = text_field(ui, &mut label, &mut focus);

    ui.add_space(8.0);
    field_label(ui, "Kind:");
    let mut kind_changed = false;
    egui::ComboBox::from_id_salt("seq_kind")
        .selected_text(if shape == NodeShape::Person { "Actor" } else { "Participant" })
        .width(160.0)
        .show_ui(ui, |ui| {
            for (s, name) in [(NodeShape::Rect, "Participant"), (NodeShape::Person, "Actor")] {
                if ui.selectable_label(shape == s, name).clicked() {
                    shape = s;
                    kind_changed = true;
                }
            }
        });

    ui.add_space(8.0);
    let order = sm::participant_ids(&state.graph);
    let pos = order.iter().position(|p| p == id).unwrap_or(0);
    let mut move_to: Option<usize> = None;
    ui.horizontal(|ui| {
        if ui.add_enabled(pos > 0, egui::Button::new("< Left")).clicked() {
            move_to = Some(pos - 1);
        }
        if ui.add_enabled(pos + 1 < order.len(), egui::Button::new("Right >")).clicked() {
            move_to = Some(pos + 1);
        }
    });
    ui.add_space(8.0);
    let delete = ui.button("Delete participant").clicked();

    if focus || kind_changed || move_to.is_some() || delete {
        state.push_undo();
    }
    if label_changed || kind_changed {
        if let Some(def) = state.graph.nodes.get_mut(id) {
            if label_changed {
                def.label = label;
            }
            def.shape = shape;
        }
        finish_edit(state);
    }
    if let Some(target) = move_to {
        let new_id = sm::move_participant(&mut state.graph, id, target);
        state.selected_nodes.clear();
        state.selected_nodes.insert(new_id);
        finish_edit(state);
    }
    if delete {
        state.selected_nodes.clear();
        sm::remove_participant(&mut state.graph, id);
        finish_edit(state);
    }
}

fn note_panel(state: &mut EditorState, ui: &mut egui::Ui, id: &str) {
    let Some(def) = state.graph.nodes.get(id) else { return };
    heading(ui, "Note");

    let mut text = def.label.clone();
    let mut info = sm::note_info(def);
    let mut focus = false;
    field_label(ui, "Text:");
    let text_changed = text_field(ui, &mut text, &mut focus);

    ui.add_space(8.0);
    field_label(ui, "Position:");
    let mut info_changed = false;
    egui::ComboBox::from_id_salt("seq_note_pos")
        .selected_text(match info.position.as_str() {
            "left" => "Left of",
            "over" => "Over",
            _ => "Right of",
        })
        .width(160.0)
        .show_ui(ui, |ui| {
            for (p, name) in [("left", "Left of"), ("right", "Right of"), ("over", "Over")] {
                if ui.selectable_label(info.position == p, name).clicked() {
                    info.position = p.to_string();
                    info_changed = true;
                }
            }
        });

    // "Over" notes can span a second participant.
    let slot = sm::slot_of_note(&state.graph, id);
    let first = slot.map(|s| sm::name_of(&state.graph.edges[s].from).to_string());
    if info.position == "over" {
        ui.add_space(8.0);
        field_label(ui, "Also over:");
        let others: Vec<String> = sm::participant_ids(&state.graph)
            .iter()
            .map(|p| sm::name_of(p).to_string())
            .filter(|n| Some(n) != first.as_ref())
            .collect();
        let second = info.names.iter().find(|n| Some(*n) != first.as_ref()).cloned();
        egui::ComboBox::from_id_salt("seq_note_second")
            .selected_text(second.clone().unwrap_or_else(|| "(none)".to_string()))
            .width(160.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(second.is_none(), "(none)").clicked() {
                    info.names = first.iter().cloned().collect();
                    info_changed = true;
                }
                for n in &others {
                    if ui.selectable_label(second.as_ref() == Some(n), n).clicked() {
                        info.names = first.iter().cloned().chain(std::iter::once(n.clone())).collect();
                        info_changed = true;
                    }
                }
            });
    }

    ui.add_space(8.0);
    let mut move_dir: Option<isize> = None;
    ui.horizontal(|ui| {
        if ui.add_enabled(slot.is_some_and(|s| s > 0), egui::Button::new("Up")).clicked() {
            move_dir = Some(-1);
        }
        if ui
            .add_enabled(
                slot.is_some_and(|s| s + 1 < state.graph.edges.len()),
                egui::Button::new("Down"),
            )
            .clicked()
        {
            move_dir = Some(1);
        }
    });
    ui.add_space(8.0);
    let delete = ui.button("Delete note").clicked();

    if focus || info_changed || move_dir.is_some() || delete {
        state.push_undo();
    }
    if text_changed || info_changed {
        if let Some(def) = state.graph.nodes.get_mut(id) {
            if text_changed {
                def.label = text;
            }
            if info_changed {
                sm::set_note_info(def, &info);
            }
        }
        finish_edit(state);
    }
    if let (Some(dir), Some(s)) = (move_dir, slot) {
        let other = (s as isize + dir) as usize;
        sm::swap_slots(&mut state.graph, s, other);
        finish_edit(state);
    }
    if delete {
        if let Some(s) = slot {
            state.selected_nodes.clear();
            sm::remove_slot(&mut state.graph, s);
            finish_edit(state);
        }
    }
}

fn message_panel(state: &mut EditorState, ui: &mut egui::Ui, idx: usize) {
    let Some(edge) = state.graph.edges.get(idx) else { return };
    if sm::is_note_edge(edge) {
        return;
    }
    heading(ui, "Message");

    let n = state.graph.edges.len();
    let mut label = edge.label.clone().unwrap_or_default();
    let mut focus = false;
    field_label(ui, "Label:");
    let label_changed = text_field(ui, &mut label, &mut focus);

    ui.add_space(8.0);
    field_label(ui, "Arrow:");
    let current = sm::arrow_style_index(edge);
    let mut new_style: Option<usize> = None;
    egui::ComboBox::from_id_salt("seq_msg_arrow")
        .selected_text(current.map_or("Custom", |i| sm::ARROW_STYLES[i].0))
        .width(160.0)
        .show_ui(ui, |ui| {
            for (i, (name, _, _)) in sm::ARROW_STYLES.iter().enumerate() {
                if ui.selectable_label(current == Some(i), *name).clicked() {
                    new_style = Some(i);
                }
            }
        });

    let from_name = sm::name_of(&edge.from).to_string();
    let to_name = sm::name_of(&edge.to).to_string();
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(format!("{from_name}  →  {to_name}"))
            .size(11.0)
            .color(egui::Color32::from_rgb(80, 80, 80)),
    );
    let reverse = edge.from != edge.to && ui.button("Reverse direction").clicked();

    ui.add_space(8.0);
    let mut move_dir: Option<isize> = None;
    ui.horizontal(|ui| {
        if ui.add_enabled(idx > 0, egui::Button::new("Up")).clicked() {
            move_dir = Some(-1);
        }
        if ui.add_enabled(idx + 1 < n, egui::Button::new("Down")).clicked() {
            move_dir = Some(1);
        }
    });

    // Activation bar on the receiving participant, starting at this message.
    ui.add_space(8.0);
    field_label(ui, "Activation (target):");
    let target = edge.to.clone();
    let act_idx = state
        .graph
        .seq_activations
        .iter()
        .position(|a| a.0 == target && a.1 == idx);
    let toggle_act;
    let mut act_len: Option<usize> = None;
    match act_idx {
        None => {
            toggle_act = ui.button("Activate").clicked();
        }
        Some(ai) => {
            let (_, start, end) = state.graph.seq_activations[ai];
            let mut len = end.saturating_sub(start).max(1);
            ui.horizontal(|ui| {
                ui.label("Spans");
                if ui
                    .add(egui::DragValue::new(&mut len).range(1..=(n - start).max(1)))
                    .changed()
                {
                    act_len = Some(len);
                }
                ui.label("slot(s)");
            });
            toggle_act = ui.button("Remove activation").clicked();
        }
    }

    ui.add_space(8.0);
    let delete = ui.button("Delete message").clicked();

    if focus || new_style.is_some() || reverse || move_dir.is_some() || toggle_act || act_len.is_some() || delete
    {
        state.push_undo();
    }
    let mut changed = false;
    if label_changed {
        state.graph.edges[idx].label = if label.is_empty() { None } else { Some(label) };
        changed = true;
    }
    if let Some(i) = new_style {
        let (_, t, h) = sm::ARROW_STYLES[i];
        state.graph.edges[idx].edge_type = t;
        state.graph.edges[idx].dst_arrowhead = h;
        changed = true;
    }
    if reverse {
        let e = &mut state.graph.edges[idx];
        std::mem::swap(&mut e.from, &mut e.to);
        changed = true;
    }
    if let Some(dir) = move_dir {
        let other = (idx as isize + dir) as usize;
        sm::swap_slots(&mut state.graph, idx, other);
        state.selected_edge = Some(other);
        changed = true;
    }
    if toggle_act {
        match act_idx {
            Some(ai) => {
                state.graph.seq_activations.remove(ai);
            }
            None => state.graph.seq_activations.push((target.clone(), idx, (idx + 1).min(n))),
        }
        changed = true;
    }
    if let (Some(len), Some(ai)) = (act_len, act_idx) {
        let a = &mut state.graph.seq_activations[ai];
        a.2 = (a.1 + len).min(n);
        changed = true;
    }
    if delete {
        state.selected_edge = None;
        sm::remove_slot(&mut state.graph, idx);
        changed = true;
    }
    if changed {
        finish_edit(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_frame(ctx: &egui::Context, state: &mut EditorState, events: Vec<egui::Event>) {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(1200.0, 800.0),
        ));
        input.events = events;
        let mut session = crate::session::Session::default();
        let _ = ctx.run_ui(input, |ui| {
            let _ = render_editor_ui(state, ui, &mut session);
        });
    }

    fn ids(state: &EditorState) -> Vec<String> {
        sm::participant_ids(&state.graph)
            .iter()
            .map(|p| sm::name_of(p).to_string())
            .collect()
    }

    #[test]
    fn build_diagram_by_interaction() {
        let mut state = EditorState::new_sequence();

        // Add three participants (left to right).
        state.seq_tool = SeqTool::Participant;
        for x in [100.0, 300.0, 500.0] {
            handle_press(&mut state, egui::Pos2::new(x, 200.0));
        }
        assert_eq!(ids(&state), vec!["P1", "P2", "P3"]);

        // Insert one in the middle.
        let geo = Geo::from_state(&state);
        let mid = (geo.parts[0].1 + geo.parts[1].1) / 2.0;
        handle_press(&mut state, egui::Pos2::new(mid, 200.0));
        assert_eq!(ids(&state), vec!["P1", "P4", "P2", "P3"]);

        // Drag a message from P1 to P3.
        state.seq_tool = SeqTool::Message;
        let geo = Geo::from_state(&state);
        let p1 = geo.x_of(&sm::participant_ids(&state.graph)[0]).unwrap();
        let p3 = geo.x_of(&sm::participant_ids(&state.graph)[3]).unwrap();
        handle_press(&mut state, egui::Pos2::new(p1, seq_layout::slot_y(0)));
        assert!(state.seq_drag.is_some());
        handle_release(&mut state, Some(egui::Pos2::new(p3 + 5.0, seq_layout::slot_y(0))));
        assert_eq!(state.graph.edges.len(), 1);
        assert_eq!(sm::name_of(&state.graph.edges[0].from), "P1");
        assert_eq!(sm::name_of(&state.graph.edges[0].to), "P3");

        // Self message and a note lower down.
        state.seq_tool = SeqTool::SelfMessage;
        handle_press(&mut state, egui::Pos2::new(p1, seq_layout::slot_y(1)));
        state.seq_tool = SeqTool::Note;
        handle_press(&mut state, egui::Pos2::new(p3, seq_layout::slot_y(2)));
        assert_eq!(state.graph.edges.len(), 3);
        assert!(state.selected_nodes.iter().any(|n| sm::is_note(n)));

        // Select the first message by clicking its line.
        state.seq_tool = SeqTool::Select;
        let mid_x = (p1 + p3) / 2.0;
        handle_press(&mut state, egui::Pos2::new(mid_x, seq_layout::slot_y(0)));
        assert_eq!(state.selected_edge, Some(0));

        // Reorder: drag P1's box past everything else.
        let geo = Geo::from_state(&state);
        let (first, fx, top, bottom, _) = geo.parts[0].clone();
        handle_press(&mut state, egui::Pos2::new(fx, (top + bottom) / 2.0));
        assert!(matches!(state.seq_drag, Some(SeqDrag::Participant { .. })));
        handle_release(&mut state, Some(egui::Pos2::new(p3 + 300.0, 100.0)));
        assert_eq!(ids(&state).last().map(String::as_str), Some("P1"), "{first}");
        // Messages followed the participant to its new position.
        assert_eq!(sm::name_of(&state.graph.edges[0].from), "P1");

        // It serializes to valid Mermaid that parses back identically.
        let text = serializer::mermaid::serialize(&state.graph);
        assert!(text.starts_with("sequenceDiagram"), "{text}");
        let g2 = parser::parse(&text, parser::DiagramFormat::Mermaid, 0, None).unwrap();
        assert_eq!(g2.edges.len(), 3);
        assert_eq!(sm::participant_ids(&g2).len(), 4);

        // Delete + undo.
        state.selected_nodes.clear();
        state.selected_edge = Some(0);
        delete_selection(&mut state);
        assert_eq!(state.graph.edges.len(), 2);
        state.undo();
        assert_eq!(state.graph.edges.len(), 3);
    }

    #[test]
    fn renders_loaded_diagram_and_handles_pointer_events() {
        let src = "sequenceDiagram
    participant A as Alice
    participant B as Bob
    A->>+B: hello
    Note right of B: hmm
    B-->>-A: hi
    alt ok
        A->>B: again
    end
";
        let dir = std::env::temp_dir().join("boxcrab_seq_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.mmd");
        std::fs::write(&path, src).unwrap();
        let mut state = EditorState::from_file(path, parser::DiagramFormat::Mermaid).unwrap();
        assert_eq!(state.graph.diagram_type, DiagramType::Sequence);

        let ctx = egui::Context::default();
        // Several frames: text measurement, layout, scene fit.
        for _ in 0..4 {
            run_frame(&ctx, &mut state, vec![]);
        }
        let layout = state.layout_result.as_ref().expect("sequence layout built");
        assert!(layout.nodes.len() >= 4);
        assert!(state.scene_fitted);

        // Exercise every properties panel.
        let first = sm::participant_ids(&state.graph)[0].clone();
        let note = state.graph.nodes.keys().find(|k| sm::is_note(k)).cloned().unwrap();
        state.selected_nodes.insert(first);
        run_frame(&ctx, &mut state, vec![]);
        state.selected_nodes.clear();
        state.selected_nodes.insert(note);
        run_frame(&ctx, &mut state, vec![]);
        state.selected_nodes.clear();
        state.selected_edge = Some(0);
        run_frame(&ctx, &mut state, vec![]);
        state.selected_edge = None;

        // Click on a participant box through real pointer events: pick a
        // screen position by mapping the box center through the scene.
        let geo = Geo::from_state(&state);
        let (_, x, top, bottom, _) = geo.parts[0].clone();
        let _ = (x, top, bottom);
        run_frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(egui::Pos2::new(600.0, 400.0))],
        );
        run_frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerButton {
                pos: egui::Pos2::new(600.0, 400.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        run_frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerButton {
                pos: egui::Pos2::new(600.0, 400.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );

        // Save path round trip keeps the structure.
        let text = serializer::mermaid::serialize(&state.graph);
        let g2 = parser::parse(&text, parser::DiagramFormat::Mermaid, 0, None).unwrap();
        assert_eq!(g2.edges.len(), state.graph.edges.len());
        assert_eq!(g2.seq_activations.len(), state.graph.seq_activations.len());
        assert_eq!(g2.subgraphs.len(), 1);
    }

    fn two_msg_state() -> (EditorState, f32, f32) {
        let mut state = EditorState::new_sequence();
        state.seq_tool = SeqTool::Participant;
        handle_press(&mut state, egui::Pos2::new(100.0, 200.0));
        handle_press(&mut state, egui::Pos2::new(400.0, 200.0));
        handle_press(&mut state, egui::Pos2::new(800.0, 200.0));
        let geo = Geo::from_state(&state);
        let xs: Vec<f32> = geo.parts.iter().map(|p| p.1).collect();
        state.seq_tool = SeqTool::Message;
        for (slot, (a, b)) in [(0, (0, 1)), (1, (1, 2)), (2, (0, 2))] {
            handle_press(&mut state, egui::Pos2::new(xs[a], seq_layout::slot_y(slot)));
            handle_release(&mut state, Some(egui::Pos2::new(xs[b], seq_layout::slot_y(slot))));
        }
        state.seq_tool = SeqTool::Select;
        for (i, l) in ["first", "second", "third"].iter().enumerate() {
            state.graph.edges[i].label = Some(l.to_string());
        }
        state.rebuild_layout();
        (state, xs[0], xs[1])
    }

    fn labels(state: &EditorState) -> Vec<String> {
        state.graph.edges.iter().map(|e| e.label.clone().unwrap_or_default()).collect()
    }

    #[test]
    fn drag_message_down_reorders_timeline() {
        let (mut state, x0, x1) = two_msg_state();
        let mid = (x0 + x1) / 2.0;
        let y0 = seq_layout::slot_y(0);
        handle_press(&mut state, egui::Pos2::new(mid, y0));
        assert!(matches!(state.seq_drag, Some(SeqDrag::MoveMessage { slot: 0, .. })));
        handle_release(&mut state, Some(egui::Pos2::new(mid, y0 + 82.0)));
        assert_eq!(labels(&state), vec!["second", "third", "first"]);
        assert_eq!(state.selected_edge, Some(2));

        // A click without moving changes nothing.
        let y2 = seq_layout::slot_y(2);
        handle_press(&mut state, egui::Pos2::new(mid, y2));
        handle_release(&mut state, Some(egui::Pos2::new(mid, y2 + 3.0)));
        assert_eq!(labels(&state), vec!["second", "third", "first"]);
    }

    #[test]
    fn drag_endpoint_reattaches_message() {
        let (mut state, x0, x1) = two_msg_state();
        let x2 = Geo::from_state(&state).parts[2].1;
        // Select message 0 (P1 -> P2), then drag its head to P3.
        handle_press(&mut state, egui::Pos2::new((x0 + x1) / 2.0, seq_layout::slot_y(0)));
        handle_release(&mut state, Some(egui::Pos2::new((x0 + x1) / 2.0, seq_layout::slot_y(0))));
        assert_eq!(state.selected_edge, Some(0));
        let (_, head) = endpoints(&state, 0).unwrap();
        handle_press(&mut state, head);
        assert!(matches!(state.seq_drag, Some(SeqDrag::Endpoint { slot: 0, is_from: false })));
        handle_release(&mut state, Some(egui::Pos2::new(x2, head.y)));
        assert_eq!(sm::name_of(&state.graph.edges[0].to), "P3");
        assert_eq!(sm::name_of(&state.graph.edges[0].from), "P1");
    }

    #[test]
    fn drag_note_moves_and_reattaches() {
        let (mut state, _x0, _x1) = two_msg_state();
        state.seq_tool = SeqTool::Note;
        let geo = Geo::from_state(&state);
        let (p1, p3) = (geo.parts[0].1, geo.parts[2].1);
        handle_press(&mut state, egui::Pos2::new(p1, seq_layout::slot_y(3)));
        let note = state.selected_nodes.iter().next().cloned().unwrap();
        state.seq_tool = SeqTool::Select;
        assert_eq!(sm::slot_of_note(&state.graph, &note), Some(3));

        let layout = state.layout_result.clone().unwrap();
        let n = layout.nodes.iter().find(|n| n.id == note).unwrap();
        let press = egui::Pos2::new(n.x, n.y);
        handle_press(&mut state, press);
        assert!(matches!(state.seq_drag, Some(SeqDrag::MoveNote { .. })));
        handle_release(&mut state, Some(egui::Pos2::new(p3, press.y - 82.0)));
        assert_eq!(sm::slot_of_note(&state.graph, &note), Some(1));
        let owner = &state.graph.edges[1].from;
        assert_eq!(sm::name_of(owner), "P3");
        assert_eq!(sm::note_info(&state.graph.nodes[&note]).names, vec!["P3".to_string()]);
    }
}
