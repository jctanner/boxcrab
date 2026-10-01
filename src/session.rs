use eframe::egui;
use std::path::{Path, PathBuf};

const MAX_RECENT: usize = 10;

/// In-memory state shared by the viewer and editor menus: the folder the
/// last file dialog used and a most-recently-used file list.
#[derive(Default)]
pub struct Session {
    last_dir: Option<PathBuf>,
    recent: Vec<PathBuf>,
}

impl Session {
    /// Record a file that was opened or saved: remember its folder and move
    /// it to the front of the recent list.
    pub fn note(&mut self, path: &Path) {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            self.last_dir = Some(dir.to_path_buf());
        }
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(MAX_RECENT);
    }

    /// A file dialog starting in the last used folder (or the cwd).
    pub fn dialog(&self) -> rfd::FileDialog {
        let dir = self
            .last_dir
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        rfd::FileDialog::new().set_directory(dir)
    }

    /// "Recent Files" submenu. Returns the path the user picked, if any.
    pub fn recent_menu(&self, ui: &mut egui::Ui) -> Option<PathBuf> {
        let mut picked = None;
        ui.add_enabled_ui(!self.recent.is_empty(), |ui| {
            ui.menu_button("Recent Files", |ui| {
                for path in &self.recent {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string());
                    if ui
                        .button(name)
                        .on_hover_text(path.display().to_string())
                        .clicked()
                    {
                        picked = Some(path.clone());
                        ui.close();
                    }
                }
            });
        });
        picked
    }
}
