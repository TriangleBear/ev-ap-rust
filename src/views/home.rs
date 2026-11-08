//! The home view.
//!
//! Ported from `App/views/home_view.py`. The content is filled in in Phase 5.

use super::{AppView, ViewContext};

/// The landing page.
#[derive(Default)]
pub struct HomeView;

impl HomeView {
    /// A new home view.
    pub fn new() -> Self {
        HomeView
    }
}

impl AppView for HomeView {
    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut ViewContext<'_>) {
        ui.heading("Home");
        ui.separator();
        ui.label("The home view is filled in in Phase 5.");
    }
}
