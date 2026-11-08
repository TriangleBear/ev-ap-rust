//! The reports view.
//!
//! Ported from `App/views/reports_view.py`. The content is filled in in Phase 5.

use super::{AppView, ViewContext};

/// The attendance reports.
#[derive(Default)]
pub struct ReportsView;

impl ReportsView {
    /// A new reports view.
    pub fn new() -> Self {
        ReportsView
    }
}

impl AppView for ReportsView {
    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut ViewContext<'_>) {
        ui.heading("Reports");
        ui.separator();
        ui.label("The reports view is filled in in Phase 5.");
    }
}
