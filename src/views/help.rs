//! The help view.
//!
//! Ported from `App/views/help_view.py`. The text is copied verbatim,
//! including the "DISABLED" note about the points redemption.

use super::{AppView, ViewContext};

/// The help text of the Python app.
pub const HELP_TEXT: &str = "Welcome to the Events Attendance App!

This application allows you to manage event attendance and member registrations.

Features:
- Register new members
- Create new events
- Record attendance using RFID
- Redeem points for members DISABLED
- Export event data to CSV or Excel

For any assistance, please contact support.";

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
        ui.add_space(8.0);
        ui.label(HELP_TEXT);
    }
}
