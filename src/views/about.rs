//! The about view.
//!
//! Ported from `App/views/about_view.py`. The version comes from the build
//! time constant instead of a string the CI rewrote.

use super::{AppView, ViewContext};

/// The about page.
#[derive(Default)]
pub struct AboutView;

impl AboutView {
    /// A new about view.
    pub fn new() -> Self {
        AboutView
    }
}

impl AppView for AboutView {
    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut ViewContext<'_>) {
        ui.heading("About");
        ui.separator();
        ui.label(format!("{} {}", crate::APP_NAME, crate::VERSION));
        ui.label("Ev_Ap — Event Attendance Automation");
        ui.label("A Rust rewrite of the Python application.");
    }
}
