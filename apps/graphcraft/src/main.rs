//! GraphCraft — native desktop app.
//!
//! Runs the shared `graphcraft_ui_egui` UI under eframe. The web
//! runner (`apps/graphcraft-web`) links the same UI crate into an
//! eframe-web entry point.

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use graphcraft_ui_egui::{AppState, FileInbox, FileService, InboxEvent};
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("GraphCraft")
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "GraphCraft",
        options,
        Box::new(|_cc| {
            let state = AppState::new(Box::new(RfdFileService));
            Ok(Box::new(DesktopApp { state }))
        }),
    )
}

struct DesktopApp {
    state: AppState,
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.state.ui(ui);
    }
}

/// Native file dialogs via rfd. Sync — rfd's `pick_file` /
/// `save_file` block briefly on the UI thread, which is fine
/// for a native desktop app.
struct RfdFileService;

impl FileService for RfdFileService {
    fn open_project(&self, inbox: &FileInbox) {
        let picked: Option<PathBuf> = rfd::FileDialog::new()
            .set_title("Open GraphCraft project")
            .add_filter("GraphCraft project", &["graphcraft", "json"])
            .pick_file();
        let Some(path) = picked else { return };
        match std::fs::read(&path) {
            Ok(bytes) => inbox.push(InboxEvent::LoadGraphcraft {
                bytes,
                label: path.display().to_string(),
            }),
            Err(e) => {
                inbox.push(InboxEvent::Status(format!("read {}: {e}", path.display())));
            }
        }
    }

    fn save_project(&self, default_name: &str, bytes: Vec<u8>, inbox: &FileInbox) {
        let picked: Option<PathBuf> = rfd::FileDialog::new()
            .set_title("Save GraphCraft project")
            .set_file_name(format!("{default_name}.graphcraft"))
            .add_filter("GraphCraft project", &["graphcraft"])
            .save_file();
        let Some(path) = picked else { return };
        match std::fs::write(&path, &bytes) {
            Ok(()) => inbox.push(InboxEvent::Status(format!("saved to {}", path.display()))),
            Err(e) => inbox.push(InboxEvent::Status(format!("write {}: {e}", path.display()))),
        }
    }
}
