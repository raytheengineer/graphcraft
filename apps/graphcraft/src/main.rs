//! GraphCraft — native desktop app.
//!
//! Runs the shared `graphcraft_ui_egui` UI under eframe. The web
//! runner (`apps/graphcraft-web`, M9.4) links the same UI crate
//! into an eframe-web entry point.

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use graphcraft_ui_egui::AppState;

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
        Box::new(|_cc| Ok(Box::new(DesktopApp::default()))),
    )
}

#[derive(Default)]
struct DesktopApp {
    state: AppState,
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.state.ui(ui);
    }
}
