//! The members view.
//!
//! Ported from `App/views/members_view.py`: the member list with the RFID,
//! Member ID, Name and Student Number columns.

use super::{AppView, ViewContext};
use crate::db::Backend;
use crate::managers::messages;
use crate::ui::form::{FormField, FormModal, FormOutcome};
use crate::ui::table::draw_table;

/// The columns of the member table.
pub const MEMBER_COLUMNS: &[&str] = &["rfid", "memberid", "name", "student_num"];

/// The member list.
pub struct MembersView {
    /// The registration form.
    pub register_form: Option<FormModal>,
    /// The search term.
    pub search: String,
}

impl Default for MembersView {
    fn default() -> Self {
        MembersView::new()
    }
}

impl MembersView {
    /// A new members view.
    pub fn new() -> Self {
        MembersView {
            register_form: None,
            search: String::new(),
        }
    }
}

impl AppView for MembersView {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut ViewContext<'_>) {
        ui.heading("Members Management");
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Register New Member").clicked() {
                self.register_form = Some(FormModal::new(
                    "Register New Member",
                    vec![
                        FormField::text("Member ID", "Enter member ID"),
                        FormField::text("Name", "Enter name"),
                        FormField::text("Student Number", "Enter student number"),
                        FormField::text("Program", "Enter program"),
                        FormField::text("Year", "Enter year"),
                        FormField::text("RFID", "Scan RFID"),
                    ],
                ));
            }
            if ui.button("Refresh").clicked() {
                ctx.request_refresh();
            }
            ui.separator();
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.search);
        });

        ui.add_space(8.0);

        let members = ctx.db.fetch_table_data("Members").unwrap_or_default();
        let filtered = crate::managers::TableManager::filter_rows(&members, &self.search);

        if members.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.add_space(40.0);
                ui.label("No members found. Use the 'Register New Member' button to add one.");
            });
        } else if filtered.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.add_space(40.0);
                ui.label("No members match the search.");
            });
        } else {
            egui::ScrollArea::vertical().show(ui, |ui| {
                draw_table(ui, &filtered, MEMBER_COLUMNS);
            });
        }

        // ── The registration form ──────────────────────────────────────
        if let Some(form) = self.register_form.as_mut() {
            match form.show(ui.ctx()) {
                FormOutcome::Submitted => {
                    let values = form.values();
                    let result = ctx.members.register_member(
                        &values[5], &values[0], &values[1], &values[2], &values[3], &values[4],
                    );
                    self.register_form = None;
                    match result {
                        Ok(()) => {
                            ctx.show_message(
                                messages::MEMBER_REGISTRATION,
                                messages::MEMBER_REGISTERED,
                            );
                            ctx.request_refresh();
                        }
                        Err(message) => ctx.show_message(message.title, message.text),
                    }
                }
                FormOutcome::Cancelled => self.register_form = None,
                FormOutcome::Idle => {}
            }
        }
    }
}
