//! The home view.
//!
//! Ported from `App/views/home_view.py`. The Python view is a dashboard with
//! four cards: Event Attendance (with an event drop down), Register Member,
//! Create Event and a **disabled** Redeem Points card with the tooltip
//! "This feature is temporarily unavailable".

use super::{AppView, ViewContext};
use crate::managers::messages;
use crate::ui::form::{FormField, FormModal, FormOutcome};

/// The landing page.
pub struct HomeView {
    /// The event selected in the drop down.
    pub selected_event: Option<String>,
    /// The registration form.
    pub register_form: Option<FormModal>,
    /// The event creation form.
    pub create_event_form: Option<FormModal>,
}

impl Default for HomeView {
    fn default() -> Self {
        HomeView::new()
    }
}

impl HomeView {
    /// A new home view.
    pub fn new() -> Self {
        HomeView {
            selected_event: None,
            register_form: None,
            create_event_form: None,
        }
    }
}

impl AppView for HomeView {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut ViewContext<'_>) {
        ui.heading("Dashboard");
        ui.separator();

        let events = ctx.events.list_events().unwrap_or_default();

        // The dashboard is a two by two grid of cards.
        let egui_ctx = ui.ctx().clone();
        ui.columns(2, |columns| {
            // ── Event Attendance ───────────────────────────────────
            columns[0].vertical(|ui| {
                ui.heading("Event Attendance");
                ui.label("Scan RFID cards for event attendance");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let selected = self.selected_event.clone();
                    let label = match &selected {
                        Some(event) => event.clone(),
                        None if events.is_empty() => "No events available".to_string(),
                        None => events[0].clone(),
                    };
                    egui::ComboBox::from_id_salt("home_event")
                        .selected_text(label)
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for event in &events {
                                ui.selectable_value(
                                    &mut self.selected_event,
                                    Some(event.clone()),
                                    event,
                                );
                            }
                        });

                    if ui.button("Go").clicked() {
                        match self.selected_event.clone() {
                            Some(event) => {
                                let window =
                                    crate::ui::attendance_window::AttendanceWindow::new(&event);
                                *ctx.attendance = Some(window);
                            }
                            None => {
                                ctx.show_message(
                                    "Warning",
                                    "No event selected or no events available.",
                                );
                            }
                        }
                    }
                });
            });

            // ── Register Member ────────────────────────────────────
            columns[1].vertical(|ui| {
                ui.heading("Register Member");
                ui.label("Add new members to the database");
                ui.add_space(8.0);
                if ui.button("Register Member").clicked() {
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
            });
        });

        ui.columns(2, |columns| {
            // ── Create Event ───────────────────────────────────────
            columns[0].vertical(|ui| {
                ui.heading("Create Event");
                ui.label("Set up a new event for attendance tracking");
                ui.add_space(8.0);
                if ui.button("Create Event").clicked() {
                    self.create_event_form = Some(FormModal::new(
                        "Create Event Name",
                        vec![FormField::text(
                            "Event Name",
                            "Enter the name for the new event",
                        )],
                    ));
                }
            });

            // ── Redeem Points (disabled) ───────────────────────────
            let disabled_bg = ctx.theme.disabled_bg_color(&egui_ctx);
            let disabled_text = ctx.theme.disabled_text_color(&egui_ctx);
            columns[1].vertical(|ui| {
                egui::Frame::new()
                    .fill(disabled_bg)
                    .inner_margin(12.0)
                    .corner_radius(6.0)
                    .show(ui, |ui| {
                        ui.disable();
                        ui.colored_label(disabled_text, "Redeem Points");
                        ui.colored_label(disabled_text, "Process point redemptions for members");
                        ui.add_space(8.0);
                        let response = ui.button("Redeem Points");
                        response.on_hover_text("This feature is temporarily unavailable");
                    });
            });
        });

        // ── The registration form ─────────────────────────────────────
        if let Some(form) = self.register_form.as_mut() {
            match form.show(ui.ctx()) {
                FormOutcome::Submitted => {
                    let values = form.values();
                    let result = ctx.members.register_member(
                        &values[5], &values[0], &values[1], &values[2], &values[3], &values[4],
                    );
                    self.register_form = None;
                    match result {
                        Ok(()) => ctx.show_message(
                            messages::MEMBER_REGISTRATION,
                            messages::MEMBER_REGISTERED,
                        ),
                        Err(message) => ctx.show_message(message.title, message.text),
                    }
                }
                FormOutcome::Cancelled => self.register_form = None,
                FormOutcome::Idle => {}
            }
        }

        // ── The event creation form ───────────────────────────────────
        if let Some(form) = self.create_event_form.as_mut() {
            match form.show(ui.ctx()) {
                FormOutcome::Submitted => {
                    let name = form.value(0);
                    self.create_event_form = None;
                    match ctx.events.create_event(&name) {
                        Ok(created) => {
                            ctx.show_message(messages::EVENT_CREATION, messages::EVENT_CREATED);
                            self.selected_event = Some(created);
                            ctx.request_refresh();
                        }
                        Err(message) => ctx.show_message(message.title, message.text),
                    }
                }
                FormOutcome::Cancelled => self.create_event_form = None,
                FormOutcome::Idle => {}
            }
        }
    }
}
