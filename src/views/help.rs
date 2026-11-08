//! The help view.
//!
//! Ported from `App/views/help_view.py`. The content is filled in in Phase 5.

use super::{AppView, ViewContext};

/// The help page.
#[derive(Default)]
pub struct HelpView;

impl HelpView {
    /// A new help view.
    pub fn new() -> Self {
        HelpView
    }
}

impl AppView for HelpView {
    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut ViewContext<'_>) {
        ui.heading("Help");
        ui.separator();
        ui.label("The help view is filled in in Phase 5.");
    }
}
