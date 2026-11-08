//! The events view.
//!
//! Ported from `App/views/events_view.py`. The content is filled in in Phase 5.

use super::{AppView, ViewContext};

/// The event list and the event creation flow.
#[derive(Default)]
pub struct EventsView;

impl EventsView {
    /// A new events view.
    pub fn new() -> Self {
        EventsView
    }
}

impl AppView for EventsView {
    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut ViewContext<'_>) {
        ui.heading("Events");
        ui.separator();
        ui.label("The events view is filled in in Phase 5.");
    }
}
