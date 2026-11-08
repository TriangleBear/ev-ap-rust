//! The members view.
//!
//! Ported from `App/views/members_view.py`. The content is filled in in Phase 5.

use super::{AppView, ViewContext};

/// The member table.
#[derive(Default)]
pub struct MembersView;

impl MembersView {
    /// A new members view.
    pub fn new() -> Self {
        MembersView
    }
}

impl AppView for MembersView {
    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut ViewContext<'_>) {
        ui.heading("Members");
        ui.separator();
        ui.label("The members view is filled in in Phase 5.");
    }
}
