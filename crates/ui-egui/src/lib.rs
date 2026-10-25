//! L6 egui UI for GraphCraft.
//!
//! `AppState` is the live model: the open [`Project`], the
//! selected sheet, an optional modal, and a status line. All
//! mutations go through the project convenience methods or the
//! analysis / graph dispatchers.
//!
//! M9.1 painted the shell. M9.2 wires the add-sheet flows,
//! recompute, and delete. The native-painter graph scene lands in
//! M9.3; the eframe-web runner in M9.4; file dialogs in M9.5;
//! in-grid data editing in M9.6; the legend registry in M9.7.

use egui::{Color32, ComboBox, Modal, Panel, RichText, ScrollArea, TextEdit, Ui};
use graphcraft_analysis::{
    add_analysis_sheet, add_graph_sheet, recompute_all_stale, recompute_analysis_sheet,
    recompute_graph_sheet,
};
use graphcraft_format::{
    AnalysisSpec, Document, GraphColormap, GraphErrorBars, GraphSpec, Project, SheetBody, SheetId,
    SheetStatus,
};
use graphcraft_table::AnyTable;

/// Live state the UI maintains across frames.
pub struct AppState {
    pub project: Project,
    pub selected: Option<SheetId>,
    pub status_line: String,
    modal: ModalKind,
    /// Transient fields for the inline "configure this graph" form
    /// shown when a Placeholder graph is selected.
    configure: ConfigureGraphForm,
}

#[derive(Default)]
struct ConfigureGraphForm {
    for_sheet: Option<SheetId>,
    kind: GraphKindChoice,
    x: String,
    y: String,
    error_bars: GraphErrorBars,
    colormap: GraphColormap,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            project: Project::new("Untitled project"),
            selected: None,
            status_line: String::from("GraphCraft M9.3 — click + Data from CSV to begin."),
            modal: ModalKind::None,
            configure: ConfigureGraphForm::default(),
        }
    }
}

/// One of the add-sheet modals plus a `None` resting state.
#[derive(Default)]
enum ModalKind {
    #[default]
    None,
    AddData {
        title: String,
        csv: String,
        table_kind: TableKindChoice,
    },
    AddAnalysis {
        source: Option<SheetId>,
        kind: AnalysisKindChoice,
        a: String,
        b: String,
        x: String,
        y: String,
        col: String,
    },
    AddGraph {
        source: Option<SheetId>,
        kind: GraphKindChoice,
        x: String,
        y: String,
        error_bars: GraphErrorBars,
        colormap: GraphColormap,
    },
    AddLayout {
        title: String,
    },
}

/// Which table shape "Add data" should produce.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum TableKindChoice {
    #[default]
    Column,
    Xy,
}

impl TableKindChoice {
    const ALL: [Self; 2] = [Self::Column, Self::Xy];
    fn label(self) -> &'static str {
        match self {
            Self::Column => "Column (grouped values per column)",
            Self::Xy => "XY (first column is x, remaining columns are y datasets)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum AnalysisKindChoice {
    #[default]
    Descriptives,
    OneWayAnova,
    WelchT,
    Pearson,
    OlsRegression,
    ShapiroWilk,
}

impl AnalysisKindChoice {
    const ALL: [Self; 6] = [
        Self::Descriptives,
        Self::OneWayAnova,
        Self::WelchT,
        Self::Pearson,
        Self::OlsRegression,
        Self::ShapiroWilk,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Descriptives => "Descriptives",
            Self::OneWayAnova => "One-way ANOVA",
            Self::WelchT => "Welch's t-test",
            Self::Pearson => "Pearson correlation",
            Self::OlsRegression => "OLS regression",
            Self::ShapiroWilk => "Shapiro-Wilk normality",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum GraphKindChoice {
    #[default]
    Xy,
    Column,
    Heatmap,
}

impl GraphKindChoice {
    const ALL: [Self; 3] = [Self::Xy, Self::Column, Self::Heatmap];
    fn label(self) -> &'static str {
        match self {
            Self::Xy => "XY scatter",
            Self::Column => "Column plot",
            Self::Heatmap => "Heatmap",
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
        self.detail(ui);
        self.render_modal(ui);
    }

    // ---------------------------------------------------------------
    // Top bar.
    // ---------------------------------------------------------------

    fn top_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading("GraphCraft");
            ui.separator();
            ui.label("Project:");
            ui.add(TextEdit::singleline(&mut self.project.title).desired_width(220.0));
            ui.separator();
            if ui.button("New").clicked() {
                self.project = Project::new("Untitled project");
                self.selected = None;
                self.status_line = "new project".into();
            }
            if ui.button("Recompute all").clicked() {
                let n = recompute_all_stale(&mut self.project);
                self.status_line = format!("recomputed {n} stale sheets");
            }
            ui.add_space(16.0);
            ui.label(RichText::new(&self.status_line).weak());
        });
    }

    // ---------------------------------------------------------------
    // Sidebar.
    // ---------------------------------------------------------------

    fn sidebar(&mut self, ui: &mut Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            section(ui, "Data");
            self.sheet_list(ui, |b| matches!(b, SheetBody::Data { .. }));
            if ui.small_button("+ Data from CSV").clicked() {
                self.modal = ModalKind::AddData {
                    title: String::from("Imported data"),
                    csv: String::from("a,b\n1,10\n2,20\n3,30\n4,40\n5,50"),
                    table_kind: TableKindChoice::Column,
                };
            }

            ui.add_space(8.0);
            section(ui, "Analyses");
            self.sheet_list(ui, |b| matches!(b, SheetBody::Analysis { .. }));
            if ui.small_button("+ New analysis").clicked() {
                self.modal = ModalKind::AddAnalysis {
                    source: self.first_data_sheet(),
                    kind: AnalysisKindChoice::Descriptives,
                    a: String::new(),
                    b: String::new(),
                    x: String::new(),
                    y: String::new(),
                    col: String::new(),
                };
            }

            ui.add_space(8.0);
            section(ui, "Graphs");
            self.sheet_list(ui, |b| matches!(b, SheetBody::Graph { .. }));
            if ui.small_button("+ New graph").clicked() {
                self.modal = ModalKind::AddGraph {
                    source: self.first_data_sheet(),
                    kind: GraphKindChoice::Xy,
                    x: String::new(),
                    y: String::new(),
                    error_bars: GraphErrorBars::Sd,
                    colormap: GraphColormap::DivergingBlueRed,
                };
            }

            ui.add_space(8.0);
            section(ui, "Layouts");
            self.sheet_list(ui, |b| matches!(b, SheetBody::Layout { .. }));
            if ui.small_button("+ New layout").clicked() {
                self.modal = ModalKind::AddLayout {
                    title: String::from("Figure 1"),
                };
            }

            ui.add_space(8.0);
            section(ui, "Legend");
            ui.label(
                RichText::new("Shared dataset styling — wires up in M9.7")
                    .weak()
                    .small(),
            );
        });
    }

    fn first_data_sheet(&self) -> Option<SheetId> {
        self.project
            .sheets
            .iter()
            .find(|s| matches!(s.body, SheetBody::Data { .. }))
            .map(|s| s.id)
    }

    fn sheet_list(&mut self, ui: &mut Ui, mut filter: impl FnMut(&SheetBody) -> bool) {
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

    // ---------------------------------------------------------------
    // Detail pane.
    // ---------------------------------------------------------------

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
        let is_derived = matches!(
            sheet.body,
            SheetBody::Analysis { .. } | SheetBody::Graph { .. }
        );
        let status = self.project.sheet_status(id);

        ui.horizontal(|ui| {
            ui.heading(&title);
            ui.label(RichText::new(format!("({kind_label})")).weak());
            ui.label(status_dot(status));
        });
        let mut deleted = false;
        ui.horizontal(|ui| {
            if is_derived && ui.button("Recompute").clicked() {
                self.recompute_sheet(id);
            }
            if ui.button("Delete").clicked() {
                self.delete_sheet(id);
                deleted = true;
            }
        });
        if deleted {
            return;
        }
        ui.separator();

        match self.project.get(id).map(|s| &s.body) {
            Some(SheetBody::Data { table }) => {
                self.render_data_preview(ui, table);
            }
            Some(SheetBody::Analysis { result, .. }) => {
                let text = serde_json::to_string_pretty(result).unwrap_or_default();
                ScrollArea::vertical().show(ui, |ui| {
                    ui.monospace(text);
                });
            }
            Some(SheetBody::Graph { spec, .. }) => {
                if spec.is_placeholder() {
                    self.render_placeholder_graph(ui, id);
                } else {
                    ui.label("Graph rendering — native egui painter lands in M9.4.");
                }
            }
            Some(SheetBody::Layout { items, .. }) => {
                ui.label(format!("{} items — composer lands in M9.5.", items.len()));
            }
            None => {}
        }
    }

    fn render_placeholder_graph(&mut self, ui: &mut Ui, graph_id: SheetId) {
        // Reset the form when we switch to a different sheet.
        if self.configure.for_sheet != Some(graph_id) {
            self.configure = ConfigureGraphForm {
                for_sheet: Some(graph_id),
                ..Default::default()
            };
        }
        ui.label(
            RichText::new("This graph has no visualization yet. Pick a kind and fill the fields.")
                .weak(),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Kind");
            ComboBox::from_id_salt("placeholder-kind")
                .selected_text(self.configure.kind.label())
                .show_ui(ui, |ui| {
                    for k in GraphKindChoice::ALL {
                        ui.selectable_value(&mut self.configure.kind, k, k.label());
                    }
                });
        });
        match self.configure.kind {
            GraphKindChoice::Xy => {
                labeled(ui, "x column", &mut self.configure.x);
                labeled(ui, "y column", &mut self.configure.y);
            }
            GraphKindChoice::Column => {
                ui.label("Error bars");
                ComboBox::from_id_salt("placeholder-errorbars")
                    .selected_text(error_bars_label(self.configure.error_bars))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.configure.error_bars,
                            GraphErrorBars::Sd,
                            "SD",
                        );
                        ui.selectable_value(
                            &mut self.configure.error_bars,
                            GraphErrorBars::Sem,
                            "SEM",
                        );
                        ui.selectable_value(
                            &mut self.configure.error_bars,
                            GraphErrorBars::None,
                            "None",
                        );
                    });
            }
            GraphKindChoice::Heatmap => {
                ui.label("Colormap");
                ComboBox::from_id_salt("placeholder-cmap")
                    .selected_text(colormap_label(self.configure.colormap))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.configure.colormap,
                            GraphColormap::DivergingBlueRed,
                            "Blue → red",
                        );
                        ui.selectable_value(
                            &mut self.configure.colormap,
                            GraphColormap::Grayscale,
                            "Grayscale",
                        );
                        ui.selectable_value(
                            &mut self.configure.colormap,
                            GraphColormap::SequentialBlue,
                            "Sequential blue",
                        );
                    });
            }
        }
        ui.add_space(8.0);
        if ui.button("Apply").clicked() {
            let spec = graph_spec_from(
                self.configure.kind,
                &self.configure.x,
                &self.configure.y,
                self.configure.error_bars,
                self.configure.colormap,
            );
            if let Some(sheet) = self.project.get_mut(graph_id) {
                if let SheetBody::Graph { spec: existing, .. } = &mut sheet.body {
                    *existing = spec.clone();
                    sheet.title = spec.default_title();
                }
            }
            self.status_line = format!("configured graph #{}", graph_id.0);
            self.configure = ConfigureGraphForm::default();
        }
    }

    fn render_data_preview(&self, ui: &mut Ui, table: &graphcraft_table::AnyTable) {
        use graphcraft_table::AnyTable;
        let t: std::borrow::Cow<graphcraft_table::ColumnTable> = match table {
            AnyTable::Column(t) => std::borrow::Cow::Borrowed(t),
            AnyTable::Xy(xy) => std::borrow::Cow::Owned(xy_as_column_view(xy)),
            _ => {
                ui.label("(preview grid for this table kind lands in M9.6)");
                return;
            }
        };
        let t = t.as_ref();
        let n_rows = t.columns.iter().map(|c| c.values.len()).max().unwrap_or(0);
        let shown = n_rows.min(50);
        ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("data-grid")
                .striped(true)
                .spacing(egui::vec2(14.0, 2.0))
                .show(ui, |ui| {
                    for c in &t.columns {
                        ui.label(RichText::new(&c.title).strong());
                    }
                    ui.end_row();
                    for r in 0..shown {
                        for c in &t.columns {
                            let v = c.values.get(r).and_then(|v| v.as_ref());
                            let text = match v {
                                Some(v) => format!("{v}"),
                                None => String::new(),
                            };
                            ui.monospace(text);
                        }
                        ui.end_row();
                    }
                });
            if n_rows > shown {
                ui.label(
                    RichText::new(format!("… {} more rows", n_rows - shown))
                        .weak()
                        .small(),
                );
            }
        });
    }

    // ---------------------------------------------------------------
    // Mutations.
    // ---------------------------------------------------------------

    fn recompute_sheet(&mut self, id: SheetId) {
        let is_analysis = matches!(
            self.project.get(id).map(|s| &s.body),
            Some(SheetBody::Analysis { .. })
        );
        let result = if is_analysis {
            recompute_analysis_sheet(&mut self.project, id).map_err(|e| format!("{e}"))
        } else {
            recompute_graph_sheet(&mut self.project, id).map_err(|e| format!("{e}"))
        };
        match result {
            Ok(()) => self.status_line = format!("recomputed sheet #{}", id.0),
            Err(e) => self.status_line = format!("recompute failed: {e}"),
        }
    }

    fn delete_sheet(&mut self, id: SheetId) {
        self.project.sheets.retain(|s| s.id != id);
        if self.selected == Some(id) {
            self.selected = None;
        }
        self.status_line = format!("deleted sheet #{}", id.0);
    }

    // ---------------------------------------------------------------
    // Modal rendering.
    // ---------------------------------------------------------------

    fn render_modal(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        // Take ownership so modal UI code can borrow self without
        // conflicting with self.modal.
        let mut m = std::mem::take(&mut self.modal);
        let result = match &mut m {
            ModalKind::None => ModalResult::Keep,
            ModalKind::AddData {
                title,
                csv,
                table_kind,
            } => self.show_add_data_modal(&ctx, title, csv, table_kind),
            ModalKind::AddAnalysis {
                source,
                kind,
                a,
                b,
                x,
                y,
                col,
            } => self.show_add_analysis_modal(&ctx, source, kind, a, b, x, y, col),
            ModalKind::AddGraph {
                source,
                kind,
                x,
                y,
                error_bars,
                colormap,
            } => self.show_add_graph_modal(&ctx, source, kind, x, y, error_bars, colormap),
            ModalKind::AddLayout { title } => self.show_add_layout_modal(&ctx, title),
        };
        match result {
            ModalResult::Keep => self.modal = m,
            ModalResult::Close => self.modal = ModalKind::None,
        }
    }

    fn show_add_data_modal(
        &mut self,
        ctx: &egui::Context,
        title: &mut String,
        csv: &mut String,
        table_kind: &mut TableKindChoice,
    ) -> ModalResult {
        let mut outcome = ModalResult::Keep;
        Modal::new("gc-add-data".into()).show(ctx, |ui| {
            ui.set_min_width(480.0);
            ui.heading("Add data from CSV");
            ui.add_space(6.0);
            labeled(ui, "Title", title);
            ui.label("Table kind");
            for k in TableKindChoice::ALL {
                ui.radio_value(table_kind, k, k.label());
            }
            ui.label("CSV (first row is headers)");
            ui.add(TextEdit::multiline(csv).desired_rows(10).code_editor());
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "An empty graph sheet is created alongside the data so you can \
                     configure its visualization next.",
                )
                .weak()
                .small(),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    outcome = ModalResult::Close;
                }
                if ui.button("Add").clicked() {
                    outcome = self.commit_add_data(title, csv, *table_kind);
                }
            });
        });
        outcome
    }

    fn commit_add_data(&mut self, title: &str, csv: &str, kind: TableKindChoice) -> ModalResult {
        let column_table = match graphcraft_format::parse_csv_to_column_table(csv.as_bytes()) {
            Ok(t) => t,
            Err(e) => {
                self.status_line = format!("csv import failed: {e}");
                return ModalResult::Keep;
            }
        };
        let final_title = if title.is_empty() {
            "Imported CSV".to_string()
        } else {
            title.to_string()
        };
        let table: AnyTable = match kind {
            TableKindChoice::Column => AnyTable::Column(column_table),
            TableKindChoice::Xy => match graphcraft_format::csv_to_xy(column_table) {
                Ok(xy) => AnyTable::Xy(xy),
                Err(e) => {
                    self.status_line = format!("XY needs ≥ 2 columns: {e}");
                    return ModalResult::Keep;
                }
            },
        };
        let data_id = self.project.add_data_sheet(final_title.clone(), table);
        // Auto-create a Placeholder graph bound to the fresh data.
        let graph_id = graphcraft_analysis::add_graph_sheet(
            &mut self.project,
            data_id,
            GraphSpec::Placeholder,
        )
        .ok();
        self.selected = Some(data_id);
        self.status_line = match graph_id {
            Some(gid) => format!(
                "added #{} ({}): data + placeholder graph #{}",
                data_id.0, final_title, gid.0
            ),
            None => format!("added #{}: {}", data_id.0, final_title),
        };
        ModalResult::Close
    }

    #[allow(clippy::too_many_arguments)]
    fn show_add_analysis_modal(
        &mut self,
        ctx: &egui::Context,
        source: &mut Option<SheetId>,
        kind: &mut AnalysisKindChoice,
        a: &mut String,
        b: &mut String,
        x: &mut String,
        y: &mut String,
        col: &mut String,
    ) -> ModalResult {
        let mut outcome = ModalResult::Keep;
        Modal::new("gc-add-analysis".into()).show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.heading("Add analysis");
            ui.add_space(6.0);
            ui.label("Source data sheet");
            source_combo(ui, "ana-source", source, &self.project);
            ui.label("Kind");
            ComboBox::from_id_salt("ana-kind")
                .selected_text(kind.label())
                .show_ui(ui, |ui| {
                    for k in AnalysisKindChoice::ALL {
                        ui.selectable_value(kind, k, k.label());
                    }
                });
            match *kind {
                AnalysisKindChoice::Descriptives | AnalysisKindChoice::OneWayAnova => {}
                AnalysisKindChoice::WelchT | AnalysisKindChoice::Pearson => {
                    labeled(ui, "Column a (name or 0-based index)", a);
                    labeled(ui, "Column b", b);
                }
                AnalysisKindChoice::OlsRegression => {
                    labeled(ui, "x (predictor)", x);
                    labeled(ui, "y (response)", y);
                }
                AnalysisKindChoice::ShapiroWilk => {
                    labeled(ui, "Column", col);
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    outcome = ModalResult::Close;
                }
                let can_add = source.is_some();
                if ui.add_enabled(can_add, egui::Button::new("Add")).clicked() {
                    if let Some(src) = *source {
                        let spec = analysis_spec_from(*kind, a, b, x, y, col);
                        outcome = self.commit_add_analysis(src, spec);
                    }
                }
            });
        });
        outcome
    }

    fn commit_add_analysis(&mut self, src: SheetId, spec: AnalysisSpec) -> ModalResult {
        match add_analysis_sheet(&mut self.project, src, spec) {
            Ok(id) => {
                self.selected = Some(id);
                self.status_line = format!("added analysis #{}", id.0);
                ModalResult::Close
            }
            Err(e) => {
                self.status_line = format!("add analysis failed: {e}");
                ModalResult::Keep
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn show_add_graph_modal(
        &mut self,
        ctx: &egui::Context,
        source: &mut Option<SheetId>,
        kind: &mut GraphKindChoice,
        x: &mut String,
        y: &mut String,
        error_bars: &mut GraphErrorBars,
        colormap: &mut GraphColormap,
    ) -> ModalResult {
        let mut outcome = ModalResult::Keep;
        Modal::new("gc-add-graph".into()).show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.heading("Add graph");
            ui.add_space(6.0);
            ui.label("Source data sheet");
            source_combo(ui, "graph-source", source, &self.project);
            ui.label("Kind");
            ComboBox::from_id_salt("graph-kind")
                .selected_text(kind.label())
                .show_ui(ui, |ui| {
                    for k in GraphKindChoice::ALL {
                        ui.selectable_value(kind, k, k.label());
                    }
                });
            match *kind {
                GraphKindChoice::Xy => {
                    labeled(ui, "x column", x);
                    labeled(ui, "y column", y);
                }
                GraphKindChoice::Column => {
                    ui.label("Error bars");
                    ComboBox::from_id_salt("graph-errorbars")
                        .selected_text(error_bars_label(*error_bars))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(error_bars, GraphErrorBars::Sd, "SD");
                            ui.selectable_value(error_bars, GraphErrorBars::Sem, "SEM");
                            ui.selectable_value(error_bars, GraphErrorBars::None, "None");
                        });
                }
                GraphKindChoice::Heatmap => {
                    ui.label("Colormap");
                    ComboBox::from_id_salt("graph-cmap")
                        .selected_text(colormap_label(*colormap))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                colormap,
                                GraphColormap::DivergingBlueRed,
                                "Blue → red",
                            );
                            ui.selectable_value(colormap, GraphColormap::Grayscale, "Grayscale");
                            ui.selectable_value(
                                colormap,
                                GraphColormap::SequentialBlue,
                                "Sequential blue",
                            );
                        });
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    outcome = ModalResult::Close;
                }
                let can_add = source.is_some();
                if ui.add_enabled(can_add, egui::Button::new("Add")).clicked() {
                    if let Some(src) = *source {
                        let spec = graph_spec_from(*kind, x, y, *error_bars, *colormap);
                        outcome = self.commit_add_graph(src, spec);
                    }
                }
            });
        });
        outcome
    }

    fn commit_add_graph(&mut self, src: SheetId, spec: GraphSpec) -> ModalResult {
        match add_graph_sheet(&mut self.project, src, spec) {
            Ok(id) => {
                self.selected = Some(id);
                self.status_line = format!("added graph #{}", id.0);
                ModalResult::Close
            }
            Err(e) => {
                self.status_line = format!("add graph failed: {e}");
                ModalResult::Keep
            }
        }
    }

    fn show_add_layout_modal(&mut self, ctx: &egui::Context, title: &mut String) -> ModalResult {
        let mut outcome = ModalResult::Keep;
        Modal::new("gc-add-layout".into()).show(ctx, |ui| {
            ui.set_min_width(360.0);
            ui.heading("Add layout");
            ui.add_space(6.0);
            labeled(ui, "Title", title);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    outcome = ModalResult::Close;
                }
                if ui.button("Add").clicked() {
                    let id = self.project.add_layout_sheet(if title.is_empty() {
                        "Figure".to_string()
                    } else {
                        title.clone()
                    });
                    self.selected = Some(id);
                    self.status_line = format!("added layout #{}", id.0);
                    outcome = ModalResult::Close;
                }
            });
        });
        outcome
    }
}

#[must_use]
enum ModalResult {
    Keep,
    Close,
}

// ---------------------------------------------------------------
// Small UI primitives.
// ---------------------------------------------------------------

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

fn labeled(ui: &mut Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(TextEdit::singleline(value));
}

fn source_combo(ui: &mut Ui, id: &str, source: &mut Option<SheetId>, project: &Project) {
    let data_sheets: Vec<(SheetId, String)> = project
        .sheets
        .iter()
        .filter(|s| matches!(s.body, SheetBody::Data { .. }))
        .map(|s| (s.id, s.title.clone()))
        .collect();
    let selected_text = source
        .and_then(|id| data_sheets.iter().find(|(x, _)| *x == id))
        .map(|(id, t)| format!("#{}  {}", id.0, t))
        .unwrap_or_else(|| "(no data sheets)".to_string());
    ComboBox::from_id_salt(id)
        .selected_text(selected_text)
        .show_ui(ui, |ui| {
            for (sid, title) in &data_sheets {
                ui.selectable_value(source, Some(*sid), format!("#{}  {}", sid.0, title));
            }
        });
}

fn analysis_spec_from(
    kind: AnalysisKindChoice,
    a: &str,
    b: &str,
    x: &str,
    y: &str,
    col: &str,
) -> AnalysisSpec {
    match kind {
        AnalysisKindChoice::Descriptives => AnalysisSpec::Descriptives,
        AnalysisKindChoice::OneWayAnova => AnalysisSpec::OneWayAnova,
        AnalysisKindChoice::WelchT => AnalysisSpec::WelchT {
            a: a.to_string(),
            b: b.to_string(),
        },
        AnalysisKindChoice::Pearson => AnalysisSpec::Pearson {
            a: a.to_string(),
            b: b.to_string(),
        },
        AnalysisKindChoice::OlsRegression => AnalysisSpec::OlsRegression {
            x: x.to_string(),
            y: y.to_string(),
        },
        AnalysisKindChoice::ShapiroWilk => AnalysisSpec::ShapiroWilk {
            col: col.to_string(),
        },
    }
}

fn graph_spec_from(
    kind: GraphKindChoice,
    x: &str,
    y: &str,
    error_bars: GraphErrorBars,
    colormap: GraphColormap,
) -> GraphSpec {
    match kind {
        GraphKindChoice::Xy => GraphSpec::Xy {
            x: x.to_string(),
            y: y.to_string(),
            title: None,
        },
        GraphKindChoice::Column => GraphSpec::Column {
            title: None,
            error_bars,
            y_title: None,
        },
        GraphKindChoice::Heatmap => GraphSpec::Heatmap {
            title: None,
            colormap,
        },
    }
}

fn error_bars_label(e: GraphErrorBars) -> &'static str {
    match e {
        GraphErrorBars::Sd => "SD",
        GraphErrorBars::Sem => "SEM",
        GraphErrorBars::None => "None",
    }
}

/// Treat an `XyTable` as a flat column view for preview — one
/// column for the shared x, one per Y subcolumn in order.
fn xy_as_column_view(xy: &graphcraft_table::XyTable) -> graphcraft_table::ColumnTable {
    use graphcraft_table::{Column, ColumnTable};
    let mut columns = Vec::with_capacity(1 + xy.datasets.len() * 2);
    columns.push(Column {
        title: if xy.x_title.is_empty() {
            "x".to_string()
        } else {
            xy.x_title.clone()
        },
        values: xy.x.clone(),
    });
    for ds in &xy.datasets {
        for sub in &ds.subcolumns {
            let title = if ds.subcolumns.len() == 1 {
                ds.title.clone()
            } else {
                format!("{} / {}", ds.title, sub.title)
            };
            columns.push(Column {
                title,
                values: sub.values.clone(),
            });
        }
    }
    ColumnTable {
        title: xy.title.clone(),
        row_titles: Vec::new(),
        columns,
    }
}

fn colormap_label(c: GraphColormap) -> &'static str {
    match c {
        GraphColormap::DivergingBlueRed => "Blue → red",
        GraphColormap::Grayscale => "Grayscale",
        GraphColormap::SequentialBlue => "Sequential blue",
    }
}

// Silence the "unused Document" import when we add Save/Open in
// M9.5; mentioning it here keeps the module header docs honest
// about what's wired.
#[allow(dead_code)]
fn _ensure_document_in_scope(doc: &Document) -> &Project {
    &doc.project
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

    #[test]
    fn commit_add_data_appends_sheet() {
        let mut s = AppState::default();
        let _ = s.commit_add_data("test", "a,b\n1,2\n3,4\n", TableKindChoice::Column);
        assert_eq!(s.project.data_sheet_count(), 1);
        assert!(s.selected.is_some());
    }

    #[test]
    fn commit_add_analysis_requires_source() {
        let mut s = AppState::default();
        let _ = s.commit_add_data("d", "x,y\n1,2\n2,4\n3,6\n", TableKindChoice::Column);
        let src = s.first_data_sheet().unwrap();
        let _ = s.commit_add_analysis(src, AnalysisSpec::Descriptives);
        assert_eq!(s.project.analysis_sheet_count(), 1);
    }
}
