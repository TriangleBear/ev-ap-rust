//! The event attendance window.
//!
//! The Python app opens a `1000x700` toplevel window (`TableManager
//! .create_table_window`) with a search field, the attendance table, a refresh
//! and an export button, and the RFID entry that records a scan.
//!
//! The scan is the heart of the app: the tag is typed into the entry by a
//! keyboard-wedge scanner and sent with the Enter key, the scan runs on a
//! worker thread, and the result is reported in a message box.

use std::sync::mpsc::Receiver;

use crate::db::Row;
use crate::managers::export;
use crate::managers::messages;
use crate::managers::rfid_cache::now_secs;
use crate::managers::{ScanResult, TableManager};
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
    /// A message to show in the application's message box.
    pub message: Option<(String, String)>,
    /// The RFID that was scanned.
    pub rfid_input: String,
    /// Is a scan running?
    pub scanning: bool,
    /// The receiver of the running scan.
    receiver: Option<Receiver<ScanResult>>,
    /// The id of the RFID entry, so the focus can be given back to it.
    pub rfid_id: Option<egui::Id>,
    /// The focus has to be moved back to the RFID entry.
    pending_focus: bool,
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
            rfid_input: String::new(),
            scanning: false,
            receiver: None,
            rfid_id: None,
            pending_focus: false,
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

    /// Start a scan for the RFID in the entry.
    ///
    /// The de-duplication is checked **before** the scan starts, so that the
    /// "ignoring scan" message appears right away instead of after a spinner
    /// — exactly like the Python `rfid_scan_event`.
    pub fn start_scan(&mut self, manager: &TableManager) {
        let rfid = self.rfid_input.trim().to_string();
        if rfid.is_empty() {
            return;
        }

        if let Some(message) = manager.rfid_cache().duplicate_message(&rfid, now_secs()) {
            self.message = Some((messages::RFID_SCAN.to_string(), message));
            self.rfid_input.clear();
            self.pending_focus = true;
            return;
        }

        self.scanning = true;
        self.receiver = Some(manager.spawn_scan(&self.event, &rfid));
    }

    /// Collect the result of a running scan.
    ///
    /// Public so the flow can be driven without a window.
    pub fn collect_scan_result(&mut self, manager: &TableManager) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        let Ok(result) = receiver.try_recv() else {
            return;
        };

        self.receiver = None;
        self.scanning = false;
        self.rfid_input.clear();
        self.pending_focus = true;

        match result {
            ScanResult::Recorded => {
                self.message = Some((
                    messages::RFID_SCAN.to_string(),
                    messages::RFID_RECORDED.to_string(),
                ));
                self.refresh(manager);
            }
            ScanResult::AlreadyAttended => {
                self.message = Some((
                    messages::RFID_SCAN.to_string(),
                    messages::ALREADY_ATTENDED.to_string(),
                ));
            }
            ScanResult::Duplicate(message) => {
                self.message = Some((messages::RFID_SCAN.to_string(), message));
            }
            ScanResult::Failed(message) => {
                self.message = Some((messages::RFID_SCAN_ERROR.to_string(), message));
            }
        }
    }

    /// Draw the window.
    ///
    /// Returns a message to show in the application's message box, if an
    /// action produced one.
    pub fn show(&mut self, ctx: &Context, manager: &TableManager) -> Option<(String, String)> {
        self.collect_scan_result(manager);

        let mut refresh = false;
        let mut export = false;
        let mut submit = false;

        // The state is taken out of `self` so the window closure does not have
        // to borrow the whole struct.
        let mut open = self.open;
        let mut search = std::mem::take(&mut self.search);
        let mut rfid_input = std::mem::take(&mut self.rfid_input);
        let rows = self.visible_rows();
        let empty = self.rows.is_empty();
        let scanning = self.scanning;

        egui::Window::new(format!("Event: {}", self.event))
            .open(&mut open)
            .default_width(760.0)
            .default_height(460.0)
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

                // The table area has a fixed height so the window stays
                // compact and the RFID entry below it is always reachable.
                ui.set_height(260.0);

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

                ui.separator();

                // ── The RFID entry ────────────────────────────────────
                ui.label("Scan RFID Card:");
                let response = ui.add_enabled(
                    !scanning,
                    egui::TextEdit::singleline(&mut rfid_input)
                        .hint_text("Place RFID card here to record attendance...")
                        .desired_width(f32::INFINITY),
                );
                self.rfid_id = Some(response.id);

                // The scanner sends the tag and presses Enter.
                let enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
                if enter && response.has_focus() && !rfid_input.trim().is_empty() {
                    submit = true;
                }

                if scanning {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Checking member and recording attendance...");
                    });
                }
            });

        self.open = open;
        self.search = search;
        self.rfid_input = rfid_input;

        if submit {
            self.start_scan(manager);
        }

        if refresh {
            self.refresh(manager);
        }

        // Give the focus back to the RFID entry, so the next scan can be read
        // straight away.
        if self.pending_focus {
            if let Some(id) = self.rfid_id {
                ctx.memory_mut(|memory| memory.request_focus(id));
            }
            self.pending_focus = false;
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
