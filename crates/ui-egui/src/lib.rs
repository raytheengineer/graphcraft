//! L6 egui UI for GraphCraft.
//!
//! This crate holds *all* UI: the three-pane layout (top bar,
//! left sidebar of sheets, right detail), per-sheet detail
//! renderers, and the modal flows for adding sheets. It is shared
//! between the desktop binary (`apps/graphcraft`, running eframe
//! on native windowing) and the eframe-web runner that lands in a
//! later M9 slice (`apps/graphcraft-web`).
//!
//! The UI holds its own [`AppState`] containing the open
//! [`Project`] and a handful of transient fields. All mutations go
//! through the project convenience methods or the analysis /
//! graph dispatchers.
//!
//! M9.1 ships the shell and painted sidebar / detail skeletons.
//! Analyses, graphs, legend, and the modal flows wire up in
//! M9.2+.

use egui::{Color32, Panel, RichText, ScrollArea, Ui};
use graphcraft_format::{Project, SheetBody, SheetId, SheetStatus};

/// Live state the UI maintains across frames.
pub struct AppState {
    pub project: Project,
    pub selected: Option<SheetId>,
    pub status_line: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            project: Project::new("Untitled project"),
            selected: None,
            status_line: "GraphCraft M9.1 — eframe shell. Analyses / graphs wire up in M9.2+."
                .into(),
        }
    }
}

impl AppState {
    /// Draw one frame into the given root UI.
    pub fn ui(&mut self, ui: &mut Ui) {
        Panel::top("gc-top").show(ui, |ui| self.top_bar(ui));
        Panel::left("gc-sidebar")
            .resizable(true)
            .default_size(260.0)
            .size_range(220.0..=360.0)
            .show(ui, |ui| self.sidebar(ui));
        // The remaining space becomes the detail pane.
        self.detail(ui);
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading("GraphCraft");
            ui.separator();
            ui.label("Project:");
            ui.add(egui::TextEdit::singleline(&mut self.project.title).desired_width(220.0));
            ui.separator();
            if ui.button("New").clicked() {
                self.project = Project::new("Untitled project");
                self.selected = None;
                self.status_line = "new project".into();
            }
            if ui.button("Open…").clicked() {
                self.status_line = "Open file dialog — wired in M9.3".into();
            }
            if ui.button("Save…").clicked() {
                self.status_line = "Save file dialog — wired in M9.3".into();
            }
            ui.add_space(16.0);
            ui.label(RichText::new(&self.status_line).weak());
        });
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            section(ui, "Data");
            self.sheet_list(ui, |b| matches!(b, SheetBody::Data { .. }));
            if ui.small_button("+ Data from CSV").clicked() {
                self.status_line = "Add data from CSV — wired in M9.2".into();
            }

            ui.add_space(8.0);
            section(ui, "Analyses");
            self.sheet_list(ui, |b| matches!(b, SheetBody::Analysis { .. }));
            if ui.small_button("+ New analysis").clicked() {
                self.status_line = "Add analysis — wired in M9.2".into();
            }

            ui.add_space(8.0);
            section(ui, "Graphs");
            self.sheet_list(ui, |b| matches!(b, SheetBody::Graph { .. }));
            if ui.small_button("+ New graph").clicked() {
                self.status_line = "Add graph — wired in M9.2".into();
            }

            ui.add_space(8.0);
            section(ui, "Layouts");
            self.sheet_list(ui, |b| matches!(b, SheetBody::Layout { .. }));
            if ui.small_button("+ New layout").clicked() {
                self.status_line = "Add layout — wired in M9.5".into();
            }

            ui.add_space(8.0);
            section(ui, "Legend");
            ui.label(
                RichText::new("Shared dataset styling — M9.6 (allow, not force)")
                    .weak()
                    .small(),
            );
        });
    }

    fn sheet_list(&mut self, ui: &mut Ui, mut filter: impl FnMut(&SheetBody) -> bool) {
        // Collect first so we don't borrow project across the loop
        // body (we need to mutate self.selected).
        let rows: Vec<(SheetId, String, SheetStatus)> = self
            .project
            .sheets
            .iter()
            .filter(|s| filter(&s.body))
            .map(|s| (s.id, s.title.clone(), self.project.sheet_status(s.id)))
            .collect();
        for (id, title, status) in rows {
            let selected = self.selected == Some(id);
            ui.horizontal(|ui| {
                ui.label(status_dot(status));
                if ui.selectable_label(selected, &title).clicked() {
                    self.selected = Some(id);
                }
            });
        }
    }

    fn detail(&mut self, ui: &mut Ui) {
        let Some(id) = self.selected else {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(RichText::new("Select or add a sheet to begin.").weak());
            });
            return;
        };
        let Some(sheet) = self.project.get(id) else {
            self.selected = None;
            return;
        };
        let title = sheet.title.clone();
        let kind_label = match &sheet.body {
            SheetBody::Data { .. } => "data",
            SheetBody::Analysis { .. } => "analysis",
            SheetBody::Graph { .. } => "graph",
            SheetBody::Layout { .. } => "layout",
        };
        let status = self.project.sheet_status(id);

        ui.horizontal(|ui| {
            ui.heading(&title);
            ui.label(RichText::new(format!("({kind_label})")).weak());
            ui.label(status_dot(status));
        });
        ui.separator();

        match self.project.get(id).map(|s| &s.body) {
            Some(SheetBody::Data { .. }) => {
                ui.label("Data sheet — grid view lands in M9.2.");
            }
            Some(SheetBody::Analysis { result, .. }) => {
                let text = serde_json::to_string_pretty(result).unwrap_or_default();
                ScrollArea::vertical().show(ui, |ui| {
                    ui.monospace(text);
                });
            }
            Some(SheetBody::Graph { .. }) => {
                ui.label("Graph scene — native egui painter lands in M9.3.");
            }
            Some(SheetBody::Layout { items, .. }) => {
                ui.label(format!("{} items — composer lands in M9.5.", items.len()));
            }
            None => {}
        }
    }
}

fn section(ui: &mut Ui, title: &str) {
    ui.label(
        RichText::new(title)
            .small()
            .color(Color32::from_gray(140))
            .strong(),
    );
}

fn status_dot(status: SheetStatus) -> RichText {
    match status {
        SheetStatus::Fresh => RichText::new("●").color(Color32::from_rgb(44, 160, 44)),
        SheetStatus::Stale => RichText::new("●").color(Color32::from_rgb(215, 138, 0)),
        SheetStatus::Unknown => RichText::new("●").color(Color32::GRAY),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_state_default_is_untitled() {
        let s = AppState::default();
        assert_eq!(s.project.title, "Untitled project");
        assert!(s.selected.is_none());
    }
}
