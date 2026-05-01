use eframe::egui;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;

pub fn spawn_watcher(
    path: &Path,
    ctx: egui::Context,
) -> (RecommendedWatcher, mpsc::Receiver<()>) {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            if matches!(event.kind, EventKind::Modify(_)) {
                let _ = tx.send(());
                ctx.request_repaint();
            }
        }
    })
    .expect("Failed to create file watcher");

    watcher
        .watch(path, RecursiveMode::NonRecursive)
        .expect("Failed to watch file");

    (watcher, rx)
}
