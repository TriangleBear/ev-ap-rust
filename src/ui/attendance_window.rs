//! The event attendance window.
//!
//! The Python app opens a `1000x700` toplevel window (`TableManager
//! .create_table_window`) with a search field, the attendance table, a refresh
//! and an export button, and the RFID entry that records a scan.
//!
//! The RFID entry and the scan flow arrive in Phase 6; this phase delivers
//! the window with the table, the search and the export.

use crate::db::Row;
use crate::managers::export;
use crate::managers::TableManager;
use egui::Context;

use super::table::draw_table;

/// The columns of an attendance table.
pub const ATTENDANCE_COLUMNS: &[&str] = &["memberid", "name", "student_num", "attendance_time"];

/// The floating window that shows the attendance of one event.
pub struct AttendanceWindow {
    /// Is the window open?
    pub open: bool,
    /// The event the window shows.
    pub event: String,
    /// The rows of the table.
    pub rows: Vec<Row>,
    /// The search term.
    pub search: String,
    /// A message to show after an action, as `(title, text)`.
    pub message: Option<(String, String)>,
}

impl AttendanceWindow {
    /// A window for the given event.
    pub fn new(event: &str) -> Self {
        AttendanceWindow {
            open: true,
            event: event.to_string(),
            rows: Vec::new(),
            search: String::new(),
            message: None,
        }
    }

    /// Read the rows of the event.
    pub fn refresh(&mut self, manager: &TableManager) {
        self.rows = manager.fetch_rows(&self.event).unwrap_or_default();
    }

    /// The rows that match the search term.
    pub fn visible_rows(&self) -> Vec<Row> {
        TableManager::filter_rows(&self.rows, &self.search)
    }

    /// Draw the window.
    ///
    /// Returns a message to show in the application's message box, if an
    /// action produced one.
    pub fn show(&mut self, ctx: &Context, manager: &TableManager) -> Option<(String, String)> {
        let mut refresh = false;
        let mut export = false;

        // The state is taken out of `self` so the window closure does not have
        // to borrow the whole struct.
        let mut open = self.open;
        let mut search = std::mem::take(&mut self.search);
        let rows = self.visible_rows();
        let empty = self.rows.is_empty();

        egui::Window::new(format!("Event: {}", self.event))
            .open(&mut open)
            .default_width(900.0)
            .default_height(600.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Search Members:");
                    ui.text_edit_singleline(&mut search);
                    if ui.button("🔄 Refresh").clicked() {
                        refresh = true;
                    }
                    if ui.button("💾 Export").clicked() {
                        export = true;
                    }
                });

                ui.separator();

                if rows.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.add_space(40.0);
                        ui.label(if empty {
                            "No attendance recorded for this event yet."
                        } else {
                            "No members match the search."
                        });
                    });
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        draw_table(ui, &rows, ATTENDANCE_COLUMNS);
                    });
                }
            });

        self.open = open;
        self.search = search;

        if refresh {
            self.refresh(manager);
        }

        if export {
            let rows = self.visible_rows();
            if let Some(path) = export::save_dialog(&format!("{}.csv", self.event)) {
                return Some(match export::write(&rows, &path) {
                    Ok(()) => (
                        "Export".to_string(),
                        format!("{} exported successfully.", self.event),
                    ),
                    Err(e) => ("Export Error".to_string(), e.to_string()),
                });
            }
        }

        self.message.take()
    }
}
