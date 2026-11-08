//! The settings view.
//!
//! Ported from `App/views/settings_view.py`. The content is filled in in
//! Phase 5; the appearance mode control is already wired up here because it
//! belongs to the theme handling of the shell.

use super::{AppView, ViewContext};

/// The settings page.
#[derive(Default)]
pub struct SettingsView;

impl SettingsView {
    /// A new settings view.
    pub fn new() -> Self {
        SettingsView
    }
}

impl AppView for SettingsView {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut ViewContext<'_>) {
        ui.heading("Settings");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Appearance Mode:");
            egui::ComboBox::from_id_salt("appearance_mode")
                .selected_text(ctx.theme.label())
                .show_ui(ui, |ui| {
                    for mode in crate::theme::ThemePreference::ALL {
                        ui.selectable_value(&mut ctx.theme, mode, mode.label());
                    }
                });
        });

        ui.add_space(12.0);
        ui.label("The rest of the settings view is filled in in Phase 5.");
    }
}
