//! The reports view.
//!
//! Ported from `App/views/reports_view.py`: an attendance report for one
//! event and a general data export, both of which write a CSV or an Excel
//! file.

use super::{AppView, ViewContext};
use crate::db::Backend;
use crate::managers::export;
use crate::ui::form::{FormField, FormModal, FormOutcome};

/// Which report is being prepared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportKind {
    Attendance,
    ExportData,
}

/// The reports page.
pub struct ReportsView {
    /// The form that selects the event to report on.
    pub selection: Option<FormModal>,
    /// What the form is for.
    pub kind: ReportKind,
}

impl Default for ReportsView {
    fn default() -> Self {
        ReportsView::new()
    }
}

impl ReportsView {
    /// A new reports view.
    pub fn new() -> Self {
        ReportsView {
            selection: None,
            kind: ReportKind::Attendance,
        }
    }
}

impl AppView for ReportsView {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut ViewContext<'_>) {
        ui.heading("Reports");
        ui.separator();

        let events = ctx.events.list_events().unwrap_or_default();

        // ── Attendance Report ─────────────────────────────────────────
        ui.vertical(|ui| {
            ui.heading("Attendance Report");
            ui.label("Generate a report of attendance for a specific event");
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Generate Report").clicked() {
                        if events.is_empty() {
                            ctx.show_message(
                                "Warning",
                                "No events available. Please create an event first.",
                            );
                        } else {
                            self.kind = ReportKind::Attendance;
                            self.selection = Some(
                                FormModal::new(
                                    "Select Event",
                                    vec![FormField::choice(
                                        "Select an event for attendance report:",
                                        events.clone(),
                                    )],
                                )
                                .with_submit_label("Generate Report"),
                            );
                        }
                    }
                });
            });
            ui.separator();
        });

        // ── Export Data ────────────────────────────────────────────────
        ui.vertical(|ui| {
            ui.heading("Export Data");
            ui.label("Export event or member data to CSV or Excel");
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Generate Report").clicked() {
                        let mut options = events.clone();
                        options.push("Members".to_string());
                        if options.is_empty() {
                            ctx.show_message("Warning", "No data available to export.");
                        } else {
                            self.kind = ReportKind::ExportData;
                            self.selection = Some(
                                FormModal::new(
                                    "Select Data to Export",
                                    vec![FormField::choice("Select data to export:", options)],
                                )
                                .with_submit_label("Export Data"),
                            );
                        }
                    }
                });
            });
        });

        // ── The selection form ─────────────────────────────────────────
        if let Some(form) = self.selection.as_mut() {
            match form.show(ui.ctx()) {
                FormOutcome::Submitted => {
                    let table = form.value(0);
                    self.selection = None;
                    self.generate(ctx, &table);
                }
                FormOutcome::Cancelled => self.selection = None,
                FormOutcome::Idle => {}
            }
        }
    }
}

impl ReportsView {
    /// Read the data and write the report.
    fn generate(&self, ctx: &mut ViewContext<'_>, table_name: &str) {
        let data = ctx.db.fetch_table_data(table_name).unwrap_or_default();
        if data.is_empty() {
            let message = match self.kind {
                ReportKind::Attendance => "No attendance data found for this event.".to_string(),
                ReportKind::ExportData => format!("No data found for {table_name}."),
            };
            ctx.show_message("No Data", message);
            return;
        }

        let Some(path) = export::save_dialog(&format!("{table_name}.csv")) else {
            return;
        };

        match export::write(&data, &path) {
            Ok(()) => {
                let message = match self.kind {
                    ReportKind::Attendance => {
                        "Attendance report generated successfully.".to_string()
                    }
                    ReportKind::ExportData => {
                        format!("{table_name} data exported successfully.")
                    }
                };
                ctx.show_message("Report Generated", message);
            }
            Err(e) => ctx.show_message("Export Error", e.to_string()),
        }
    }
}
