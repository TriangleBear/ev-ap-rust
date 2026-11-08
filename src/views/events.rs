//! The events view.
//!
//! Ported from `App/views/events_view.py`: the event list with a "View
//! Attendance" and a "Delete Event" button per event, and the confirmation
//! dialog before an event is deleted.

use super::{AppView, ViewContext};
use crate::managers::messages;
use crate::ui::attendance_window::AttendanceWindow;
use crate::ui::form::{FormField, FormModal, FormOutcome};
use crate::ui::message_box::MessageBoxResponse;

/// The event list.
pub struct EventsView {
    /// The form that creates an event.
    pub create_form: Option<FormModal>,
    /// The event whose deletion is waiting for an answer.
    pub pending_delete: Option<String>,
}

impl Default for EventsView {
    fn default() -> Self {
        EventsView::new()
    }
}

impl EventsView {
    /// A new events view.
    pub fn new() -> Self {
        EventsView {
            create_form: None,
            pending_delete: None,
        }
    }
}

impl AppView for EventsView {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut ViewContext<'_>) {
        ui.heading("Events Management");
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Create New Event").clicked() {
                self.create_form = Some(FormModal::new(
                    "Create Event Name",
                    vec![FormField::text(
                        "Event Name",
                        "Enter the name for the new event",
                    )],
                ));
            }
            if ui.button("Refresh").clicked() {
                ctx.request_refresh();
            }
        });

        ui.add_space(8.0);

        let events = ctx.events.list_events().unwrap_or_default();

        if events.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.add_space(40.0);
                ui.label("No events found. Use the 'Create New Event' button to add one.");
            });
        } else {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for event in &events {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(event).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Delete Event").clicked() {
                                self.pending_delete = Some(event.clone());
                                ctx.ask_confirmation(
                                    "Delete Event",
                                    format!("Are you sure you want to delete the event '{event}'?"),
                                    "Yes",
                                    "No",
                                );
                            }
                            if ui.button("View Attendance").clicked() {
                                *ctx.attendance = Some(AttendanceWindow::new(event));
                            }
                        });
                    });
                    ui.separator();
                }
            });
        }

        // ── The answer to the delete confirmation ──────────────────────
        if let Some(response) = ctx.response {
            let event = self.pending_delete.take();
            match (event, response) {
                (Some(event), MessageBoxResponse::Yes) => match ctx.events.delete_event(&event) {
                    Ok(()) => {
                        ctx.show_message(
                            "Delete Event",
                            format!("Event '{event}' deleted successfully."),
                        );
                        ctx.request_refresh();
                    }
                    Err(message) => ctx.show_message(message.title, message.text),
                },
                (Some(event), _) => {
                    ctx.show_message(
                        "Delete Event Error",
                        format!("Error deleting event '{event}'."),
                    );
                }
                (None, MessageBoxResponse::Yes) => {}
                (None, _) => {}
            }
        }

        // ── The create event form ──────────────────────────────────────
        if let Some(form) = self.create_form.as_mut() {
            match form.show(ui.ctx()) {
                FormOutcome::Submitted => {
                    let name = form.value(0);
                    self.create_form = None;
                    match ctx.events.create_event(&name) {
                        Ok(_) => {
                            ctx.show_message(messages::EVENT_CREATION, messages::EVENT_CREATED);
                            ctx.request_refresh();
                        }
                        Err(message) => ctx.show_message(message.title, message.text),
                    }
                }
                FormOutcome::Cancelled => self.create_form = None,
                FormOutcome::Idle => {}
            }
        }
    }
}
