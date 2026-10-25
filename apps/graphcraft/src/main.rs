//! GraphCraft — native desktop app.
//!
//! Runs the shared `graphcraft_ui_egui` UI under eframe. The web
//! runner (`apps/graphcraft-web`, M9.8) links the same UI crate
//! into an eframe-web entry point.

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use graphcraft_ui_egui::{AppState, FileService};
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

/// Native file dialogs via rfd.
struct RfdFileService;

impl FileService for RfdFileService {
    fn pick_open_graphcraft(&self) -> Result<Option<Vec<u8>>, String> {
        let picked: Option<PathBuf> = rfd::FileDialog::new()
            .set_title("Open GraphCraft project")
            .add_filter("GraphCraft project", &["graphcraft", "json"])
            .pick_file();
        match picked {
            None => Ok(None),
            Some(path) => std::fs::read(&path)
                .map(Some)
                .map_err(|e| format!("read {}: {e}", path.display())),
        }
    }

    fn pick_save_graphcraft(
        &self,
        default_name: &str,
        bytes: &[u8],
    ) -> Result<Option<String>, String> {
        let picked: Option<PathBuf> = rfd::FileDialog::new()
            .set_title("Save GraphCraft project")
            .set_file_name(format!("{default_name}.graphcraft"))
            .add_filter("GraphCraft project", &["graphcraft"])
            .save_file();
        match picked {
            None => Ok(None),
            Some(path) => std::fs::write(&path, bytes)
                .map(|()| Some(path.display().to_string()))
                .map_err(|e| format!("write {}: {e}", path.display())),
        }
    }
}
