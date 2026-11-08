//! The about view.
//!
//! Ported from `App/views/about_view.py`. The version comes from the build
//! time constant instead of a string the CI rewrote.

use super::{AppView, View, ViewContext};

/// The link to the author.
pub const CREATOR_URL: &str = "https://github.com/TriangleBear";

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
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut ViewContext<'_>) {
        ui.add_space(8.0);
        ui.heading("About This Application");
        ui.label(
            "This application is designed to manage event attendance and member registration.",
        );
        ui.add_space(12.0);
        ui.label("Created by:");

        ui.hyperlink_to("TriangleBear", CREATOR_URL);

        ui.add_space(12.0);
        ui.label(format!("Version: {}", crate::VERSION));

        ui.add_space(12.0);
        if ui.button("Close").clicked() {
            ctx.request_view = Some(View::Home);
        }
    }
}
