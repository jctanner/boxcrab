mod diagram;
mod layout;
mod parser;
mod renderer;
mod theme;
mod watcher;

use clap::Parser as ClapParser;
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

#[derive(ClapParser)]
#[command(name = "boxcrab", about = "A native diagram viewer and editor")]
struct Cli {
    /// Path to a diagram file (.mmd, .dsl, .d2)
    file: PathBuf,

    /// Export diagram to PNG instead of opening viewer
    #[arg(long)]
    export: Option<PathBuf>,

    /// Scale factor for PNG export (default: 2 for crisp output)
    #[arg(long, default_value = "2")]
    scale: f32,

    /// View index to render (0-based, for multi-view formats like Structurizr DSL)
    #[arg(long, default_value = "0")]
    view: usize,
}

struct BoxcrabApp {
    file_path: PathBuf,
    format: parser::DiagramFormat,
    view_index: usize,
    graph: Option<diagram::DiagramGraph>,
    layout_result: Option<layout::LayoutResult>,
    parse_error: Option<String>,
    scene_rect: egui::Rect,
    full_scene_rect: egui::Rect,
    watcher_rx: mpsc::Receiver<()>,
    _watcher: notify::RecommendedWatcher,
    last_reload: Instant,
    needs_layout: bool,
    node_sizes: Option<std::collections::HashMap<String, egui::Vec2>>,
    workspace: Option<parser::structurizr::ast::Workspace>,
    view_history: Vec<usize>,
    drillable_ids: Vec<String>,
}

impl BoxcrabApp {
    fn new(file_path: PathBuf, format: parser::DiagramFormat, view_index: usize, ctx: &egui::Context) -> Self {
        let source = std::fs::read_to_string(&file_path).unwrap_or_default();

        let (graph, parse_error, workspace) = match format {
            parser::DiagramFormat::Structurizr => {
                match parser::structurizr::parse_workspace_v2(&source) {
                    Ok(ws) => {
                        match parser::structurizr::to_diagram_graph(&ws, view_index) {
                            Ok(g) => (Some(g), None, Some(ws)),
                            Err(e) => (None, Some(e.to_string()), Some(ws)),
                        }
                    }
                    Err(e) => (None, Some(e.to_string()), None),
                }
            }
            _ => {
                match parser::parse(&source, format, view_index, file_path.parent()) {
                    Ok(g) => (Some(g), None, None),
                    Err(e) => (None, Some(e.to_string()), None),
                }
            }
        };

        let native_ppp = ctx.pixels_per_point();
        ctx.set_pixels_per_point((native_ppp * 2.0).max(2.0));

        let (watcher, watcher_rx) = watcher::spawn_watcher(&file_path, ctx.clone());

        let drillable_ids = Self::compute_drillable_ids(&graph, &workspace, view_index);

        let mut app = Self {
            file_path,
            format,
            view_index,
            graph,
            layout_result: None,
            parse_error,
            scene_rect: egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(2000.0, 2000.0),
            ),
            full_scene_rect: egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(2000.0, 2000.0),
            ),
            watcher_rx,
            _watcher: watcher,
            last_reload: Instant::now(),
            needs_layout: true,
            node_sizes: None,
            workspace,
            view_history: Vec::new(),
            drillable_ids,
        };
        app.do_layout();
        app
    }

    fn reload_file(&mut self) {
        let source = std::fs::read_to_string(&self.file_path).unwrap_or_default();
        match self.format {
            parser::DiagramFormat::Structurizr => {
                match parser::structurizr::parse_workspace_v2(&source) {
                    Ok(ws) => {
                        match parser::structurizr::to_diagram_graph(&ws, self.view_index) {
                            Ok(g) => {
                                self.graph = Some(g);
                                self.parse_error = None;
                            }
                            Err(e) => {
                                self.parse_error = Some(e.to_string());
                            }
                        }
                        self.workspace = Some(ws);
                    }
                    Err(e) => {
                        self.parse_error = Some(e.to_string());
                    }
                }
            }
            _ => {
                match parser::parse(&source, self.format, self.view_index, self.file_path.parent()) {
                    Ok(g) => {
                        self.graph = Some(g);
                        self.parse_error = None;
                    }
                    Err(e) => {
                        self.parse_error = Some(e.to_string());
                    }
                }
            }
        }
        self.node_sizes = None;
        self.needs_layout = true;
        self.drillable_ids = Self::compute_drillable_ids(&self.graph, &self.workspace, self.view_index);
    }

    fn do_layout(&mut self) {
        if let Some(graph) = &self.graph {
            let sizes = self.node_sizes.as_ref();
            match layout::compute_layout(graph, sizes) {
                Ok(result) => {
                    let full = egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(result.total_width, result.total_height),
                    );
                    self.full_scene_rect = full;
                    // Start zoomed in at ~1:1 scale, centered on the diagram
                    let view_w = 1024.0f32.min(result.total_width);
                    let view_h = 768.0f32.min(result.total_height);
                    self.scene_rect = egui::Rect::from_center_size(
                        full.center(),
                        egui::Vec2::new(view_w, view_h),
                    );
                    self.layout_result = Some(result);
                    self.needs_layout = false;
                }
                Err(e) => {
                    self.parse_error = Some(format!("Layout error: {e}"));
                }
            }
        }
    }

    fn check_file_updates(&mut self) {
        let mut changed = false;
        while self.watcher_rx.try_recv().is_ok() {
            changed = true;
        }
        if changed && self.last_reload.elapsed().as_millis() > 100 {
            self.last_reload = Instant::now();
            self.reload_file();
            self.do_layout();
        }
    }

    fn compute_drillable_ids(
        graph: &Option<diagram::DiagramGraph>,
        workspace: &Option<parser::structurizr::ast::Workspace>,
        view_index: usize,
    ) -> Vec<String> {
        let mut ids = Vec::new();
        if let (Some(g), Some(ws)) = (graph, workspace) {
            for node_id in g.nodes.keys() {
                if parser::structurizr::find_view_for_element(ws, node_id, view_index).is_some() {
                    ids.push(node_id.clone());
                }
            }
        }
        ids
    }

    fn switch_view(&mut self, new_index: usize) {
        if new_index == self.view_index {
            return;
        }
        if let Some(ws) = &self.workspace {
            match parser::structurizr::to_diagram_graph(ws, new_index) {
                Ok(g) => {
                    self.view_history.push(self.view_index);
                    self.view_index = new_index;
                    self.graph = Some(g);
                    self.parse_error = None;
                    self.node_sizes = None;
                    self.needs_layout = true;
                    self.drillable_ids =
                        Self::compute_drillable_ids(&self.graph, &self.workspace, self.view_index);
                    self.do_layout();
                }
                Err(e) => {
                    self.parse_error = Some(e.to_string());
                }
            }
        }
    }

    fn navigate_back(&mut self) {
        if let Some(prev_index) = self.view_history.pop() {
            if let Some(ws) = &self.workspace {
                match parser::structurizr::to_diagram_graph(ws, prev_index) {
                    Ok(g) => {
                        self.view_index = prev_index;
                        self.graph = Some(g);
                        self.parse_error = None;
                        self.node_sizes = None;
                        self.needs_layout = true;
                        self.drillable_ids =
                            Self::compute_drillable_ids(&self.graph, &self.workspace, self.view_index);
                        self.do_layout();
                    }
                    Err(e) => {
                        self.parse_error = Some(e.to_string());
                    }
                }
            }
        }
    }
}

impl eframe::App for BoxcrabApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.check_file_updates();

        if ui.ctx().input(|i| i.key_pressed(egui::Key::R)) {
            self.scene_rect = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(2000.0, 2000.0),
            );
        }

        if ui.ctx().input(|i| {
            i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::Escape)
        }) {
            self.navigate_back();
        }

        if self.needs_layout {
            self.do_layout();
        }

        let measured = renderer::measure_node_texts(ui, &self.graph);
        if let Some(sizes) = measured {
            if self.node_sizes.as_ref() != Some(&sizes) {
                self.node_sizes = Some(sizes);
                self.needs_layout = true;
            }
        }

        egui::Panel::top("menu_bar")
            .frame(egui::Frame::new()
                .fill(egui::Color32::from_rgb(192, 192, 192))
                .inner_margin(2.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(128, 128, 128))))
            .show_inside(ui, |ui| {
            ui.visuals_mut().override_text_color = Some(egui::Color32::BLACK);
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open...").clicked() {
                        ui.close();
                        let cwd = std::env::current_dir().unwrap_or_default();
                        let dialog = rfd::FileDialog::new()
                            .set_directory(&cwd)
                            .add_filter("Diagram files", &["mmd", "dsl", "d2"])
                            .add_filter("All files", &["*"]);
                        if let Some(path) = dialog.pick_file() {
                            if let Some(fmt) = parser::detect_format(&path) {
                                self.file_path = path;
                                self.format = fmt;
                                self.view_index = 0;
                                self.view_history.clear();
                                self.workspace = None;
                                self.reload_file();
                                self.do_layout();
                            } else {
                                self.parse_error = Some(format!(
                                    "Unsupported file type: {}",
                                    path.display()
                                ));
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Export to PNG...").clicked() {
                        ui.close();
                        if let Some(layout) = &self.layout_result {
                            let cwd = std::env::current_dir().unwrap_or_default();
                            let dialog = rfd::FileDialog::new()
                                .set_directory(&cwd)
                                .add_filter("PNG", &["png"])
                                .set_file_name("diagram.png");
                            if let Some(path) = dialog.save_file() {
                                if let Err(e) =
                                    renderer::export::export_png(layout, &path, 2.0)
                                {
                                    self.parse_error = Some(format!("Export error: {e}"));
                                }
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        let mut breadcrumb_target: Option<usize> = None;
        if !self.view_history.is_empty() {
            if let Some(ws) = &self.workspace {
                egui::Panel::top("breadcrumb_bar")
                    .frame(egui::Frame::new()
                        .fill(egui::Color32::from_rgb(192, 192, 192))
                        .inner_margin(4.0)
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(128, 128, 128))))
                    .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (i, &hist_index) in self.view_history.iter().enumerate() {
                            let label = parser::structurizr::view_label(ws, hist_index);
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(&label)
                                            .size(12.0)
                                            .color(egui::Color32::from_rgb(50, 100, 200)),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                breadcrumb_target = Some(i);
                            }
                            ui.label(
                                egui::RichText::new(">")
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(150, 150, 150)),
                            );
                        }
                        let current_label =
                            parser::structurizr::view_label(ws, self.view_index);
                        ui.label(
                            egui::RichText::new(current_label)
                                .size(12.0)
                                .strong()
                                .color(egui::Color32::from_rgb(30, 30, 30)),
                        );
                    });
                });
            }
        }

        if let Some(target_pos) = breadcrumb_target {
            let target_index = self.view_history[target_pos];
            self.view_history.truncate(target_pos);
            if let Some(ws) = &self.workspace {
                match parser::structurizr::to_diagram_graph(ws, target_index) {
                    Ok(g) => {
                        self.view_index = target_index;
                        self.graph = Some(g);
                        self.parse_error = None;
                        self.node_sizes = None;
                        self.needs_layout = true;
                        self.drillable_ids =
                            Self::compute_drillable_ids(&self.graph, &self.workspace, self.view_index);
                        self.do_layout();
                    }
                    Err(e) => {
                        self.parse_error = Some(e.to_string());
                    }
                }
            }
        }

        if let Some(err) = &self.parse_error {
            ui.colored_label(egui::Color32::from_rgb(220, 50, 50), err);
        }

        let drillable = self.drillable_ids.clone();
        let scene_rect_before = self.scene_rect;
        let viewport_rect = ui.available_rect_before_wrap();
        let scene_response = egui::Scene::new()
            .zoom_range(0.05..=8.0)
            .show(ui, &mut self.scene_rect, |scene_ui| {
                scene_ui.painter().rect_filled(
                    scene_ui.clip_rect(),
                    0.0,
                    egui::Color32::WHITE,
                );
                if let Some(layout_result) = &self.layout_result {
                    renderer::render_diagram(scene_ui, layout_result, &drillable);
                }
            });

        let primary_clicked = ui.input(|i| i.pointer.primary_clicked());
        if primary_clicked {
            eprintln!("[DEBUG] primary_clicked=true, drillable_ids={:?}", self.drillable_ids);
        }
        if primary_clicked && !drillable.is_empty() {
            if let Some(pointer_pos) = ui.input(|i| i.pointer.interact_pos()) {
                let vp_w = viewport_rect.width();
                let vp_h = viewport_rect.height();
                let scene_w = scene_rect_before.width();
                let scene_h = scene_rect_before.height();
                eprintln!("[DEBUG] pointer_pos={:?} viewport_rect={:?} scene_rect={:?}", pointer_pos, viewport_rect, scene_rect_before);
                if vp_w > 0.0 && vp_h > 0.0 {
                    let sx = (pointer_pos.x - viewport_rect.left()) / vp_w;
                    let sy = (pointer_pos.y - viewport_rect.top()) / vp_h;
                    let scene_x = scene_rect_before.left() + sx * scene_w;
                    let scene_y = scene_rect_before.top() + sy * scene_h;
                    let scene_pos = egui::Pos2::new(scene_x, scene_y);
                    eprintln!("[DEBUG] scene_pos={:?}", scene_pos);

                    if let Some(layout) = &self.layout_result {
                        for node in &layout.nodes {
                            if !drillable.contains(&node.id) {
                                continue;
                            }
                            let node_rect = egui::Rect::from_center_size(
                                egui::Pos2::new(node.x, node.y),
                                egui::Vec2::new(node.width, node.height),
                            );
                            eprintln!("[DEBUG] checking node={} node_rect={:?} contains={}", node.id, node_rect, node_rect.contains(scene_pos));
                            if node_rect.contains(scene_pos) {
                                if let Some(ws) = &self.workspace {
                                    if let Some(view_idx) =
                                        parser::structurizr::find_view_for_element(ws, &node.id, self.view_index)
                                    {
                                        eprintln!("[DEBUG] drilling into view_idx={}", view_idx);
                                        self.switch_view(view_idx);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let screen_rect = ui.ctx().content_rect();
        let ctrl_size = egui::Vec2::new(90.0, 90.0);
        let ctrl_pos = egui::Pos2::new(
            screen_rect.right() - ctrl_size.x - 16.0,
            screen_rect.bottom() - ctrl_size.y - 16.0,
        );

        let pan_step_x = self.scene_rect.width() * 0.15;
        let pan_step_y = self.scene_rect.height() * 0.15;

        egui::Area::new(egui::Id::new("nav_controls"))
            .fixed_pos(ctrl_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(240, 240, 240, 220))
                    .corner_radius(6.0)
                    .inner_margin(4.0)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 200)))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::Vec2::new(2.0, 2.0);
                        let btn_size = egui::Vec2::splat(28.0);
                        let nav_btn = |text: &str| {
                            egui::Button::new(
                                egui::RichText::new(text).size(14.0).color(egui::Color32::from_rgb(60, 60, 60))
                            )
                            .min_size(btn_size)
                            .fill(egui::Color32::from_rgb(250, 250, 250))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(180, 180, 180)))
                        };

                        ui.horizontal(|ui| {
                            ui.add_space(30.0);
                            if ui.add(nav_btn("^")).clicked() {
                                self.scene_rect = self.scene_rect.translate(egui::Vec2::new(0.0, -pan_step_y));
                            }
                            ui.add_space(2.0);
                            if ui.add(nav_btn("+")).clicked() {
                                let center = self.scene_rect.center();
                                let new_size = self.scene_rect.size() * 0.8;
                                self.scene_rect = egui::Rect::from_center_size(center, new_size);
                            }
                        });

                        ui.horizontal(|ui| {
                            if ui.add(nav_btn("<")).clicked() {
                                self.scene_rect = self.scene_rect.translate(egui::Vec2::new(-pan_step_x, 0.0));
                            }
                            if ui.add(nav_btn("R")).clicked() {
                                self.scene_rect = self.full_scene_rect;
                            }
                            if ui.add(nav_btn(">")).clicked() {
                                self.scene_rect = self.scene_rect.translate(egui::Vec2::new(pan_step_x, 0.0));
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.add_space(30.0);
                            if ui.add(nav_btn("v")).clicked() {
                                self.scene_rect = self.scene_rect.translate(egui::Vec2::new(0.0, pan_step_y));
                            }
                            ui.add_space(2.0);
                            if ui.add(nav_btn("-")).clicked() {
                                let center = self.scene_rect.center();
                                let new_size = self.scene_rect.size() * 1.25;
                                self.scene_rect = egui::Rect::from_center_size(center, new_size);
                            }
                        });
                    });
            });
    }
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    let format = parser::detect_format(&cli.file).unwrap_or_else(|| {
        eprintln!("Unsupported file type: {}", cli.file.display());
        std::process::exit(1);
    });

    if let Some(output) = cli.export {
        let source = std::fs::read_to_string(&cli.file).unwrap_or_else(|e| {
            eprintln!("Error reading {}: {e}", cli.file.display());
            std::process::exit(1);
        });
        let graph = parser::parse(&source, format, cli.view, cli.file.parent()).unwrap_or_else(|e| {
            eprintln!("Parse error: {e}");
            std::process::exit(1);
        });
        let layout_result = layout::compute_layout(&graph, None).unwrap_or_else(|e| {
            eprintln!("Layout error: {e}");
            std::process::exit(1);
        });
        renderer::export::export_png(&layout_result, &output, cli.scale).unwrap_or_else(|e| {
            eprintln!("Export error: {e}");
            std::process::exit(1);
        });
        println!("Exported to {}", output.display());
        return;
    }

    let file_name = cli
        .file
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "boxcrab".to_string());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("boxcrab — {file_name}"))
            .with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };

    eframe::run_native(
        "boxcrab",
        options,
        Box::new(move |cc| Ok(Box::new(BoxcrabApp::new(cli.file, format, cli.view, &cc.egui_ctx)))),
    )
    .expect("Failed to start eframe");
}
