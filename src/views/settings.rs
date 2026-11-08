//! The settings view.
//!
//! Ported from `App/views/settings_view.py`: the appearance mode, the
//! database mode, the Google Apps Script Web App URL with its save and test
//! buttons, the spreadsheet URL and the "Apply & Restart Connection" button.

use super::{AppView, ViewContext};
use crate::config::{Config, DbMode};
use crate::managers::messages;

/// The settings page.
pub struct SettingsView {
    /// The configuration the page edits.
    pub config: Config,
    /// The Google Apps Script Web App URL shown in the entry.
    pub url_input: String,
    /// The Google Sheet URL or ID shown in the entry.
    pub sheet_input: String,
}

impl Default for SettingsView {
    fn default() -> Self {
        SettingsView::new()
    }
}

impl SettingsView {
    /// A new settings view, reading the configuration from disk.
    pub fn new() -> Self {
        let config = Config::load();
        let url_input = config.gsheet_api_url().to_string();
        SettingsView {
            config,
            url_input,
            sheet_input: String::new(),
        }
    }
}

impl AppView for SettingsView {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut ViewContext<'_>) {
        ui.heading("Settings");
        ui.separator();

        // ── Appearance ─────────────────────────────────────────────────
        ui.heading("Appearance Mode:");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("appearance_mode")
                .selected_text(ctx.theme.label())
                .show_ui(ui, |ui| {
                    for mode in crate::theme::ThemePreference::ALL {
                        ui.selectable_value(&mut ctx.theme, mode, mode.label());
                    }
                });
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        // ── Database ───────────────────────────────────────────────────
        ui.heading("Database Configuration");
        ui.label(
            "Choose your database backend. SQLite runs locally. Google Sheets syncs via a web API.",
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Database Mode:");
            let mut mode = self.config.db_mode;
            egui::ComboBox::from_id_salt("db_mode")
                .selected_text(match mode {
                    DbMode::Sqlite => "SQLite",
                    DbMode::Gsheet => "Google Sheets",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut mode, DbMode::Sqlite, "SQLite");
                    ui.selectable_value(&mut mode, DbMode::Gsheet, "Google Sheets");
                });
            if mode != self.config.db_mode {
                self.config.set_db_mode(mode);
                let _ = self.config.save();
                ctx.db.switch_mode(mode);
            }
        });

        ui.add_space(12.0);

        // The Google Sheets settings are only shown in Google Sheets mode.
        if self.config.db_mode == DbMode::Gsheet {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label("Google App Script Web App URL:");
                ui.label("Paste the deployment URL of your Google Apps Script web app.\nDeploy the script in 'apps-script/gsheet_api.gs' first.");
                ui.add_space(4.0);
                ui.text_edit_singleline(&mut self.url_input);

                ui.horizontal(|ui| {
                    if ui.button("Save API URL").clicked() {
                        let url = self.url_input.trim().to_string();
                        if url.is_empty() {
                            ctx.show_message(messages::SETTINGS, messages::INVALID_URL);
                        } else {
                            self.config.set_gsheet_api_url(&url);
                            let _ = self.config.save();
                            ctx.db.set_api_url(&url);
                            ctx.show_message(messages::SETTINGS, messages::GSHEET_URL_SAVED);
                        }
                    }
                    if ui.button("Test Connection").clicked() {
                        self.test_connection(ctx);
                    }
                });

                ui.add_space(8.0);
                ui.label("Google Sheet URL/ID (optional):");
                ui.label("Configure which Google Sheet to use. Paste the full Sheet URL or just the ID.\nOr run the Ev_Ap → Configure Spreadsheet menu inside the Apps Script editor.");
                ui.add_space(4.0);
                ui.text_edit_singleline(&mut self.sheet_input);

                if ui.button("Set Spreadsheet").clicked() {
                    self.set_spreadsheet(ctx);
                }
            });
        }

        ui.add_space(12.0);

        // ── Apply ──────────────────────────────────────────────────────
        if ui
            .add_sized(
                [250.0, 40.0],
                egui::Button::new(egui::RichText::new("Apply & Restart Connection").strong()),
            )
            .clicked()
        {
            self.apply(ctx);
        }
    }
}

impl SettingsView {
    /// Test the connection to the Apps Script Web App.
    fn test_connection(&self, ctx: &mut ViewContext<'_>) {
        let url = self.url_input.trim();
        if url.is_empty() {
            ctx.show_message(
                "Test Connection",
                "Please enter the Google App Script Web App URL first.",
            );
            return;
        }

        let sheets = ctx.db.sheets();
        let previous = sheets.api_url();
        sheets.set_api_url(url);

        let result = sheets.get_config();
        sheets.set_api_url(&previous);

        match result {
            Ok(Some(config)) => {
                let mut message = format!(
                    "Connected! API URL is valid.\n\nSpreadsheet ID: {}\nName: {}",
                    config.spreadsheet_id.as_deref().unwrap_or("(not set)"),
                    config.spreadsheet_name.as_deref().unwrap_or("(unknown)")
                );
                if let Some(url) = &config.spreadsheet_url {
                    message.push_str(&format!("\nURL: {url}"));
                }
                if let Some(error) = &config.error {
                    message.push_str(&format!("\n\nWarning: {error}"));
                }
                ctx.show_message("Test Connection", message);
            }
            Ok(None) => ctx.show_message(
                "Test Connection",
                "Connection failed: no response from API.",
            ),
            Err(e) => ctx.show_message("Test Connection", format!("Connection failed: {e}")),
        }
    }

    /// Point the Apps Script at a spreadsheet.
    fn set_spreadsheet(&self, ctx: &mut ViewContext<'_>) {
        let url = self.url_input.trim();
        if url.is_empty() {
            ctx.show_message(
                "Set Spreadsheet",
                "Please enter the Google App Script Web App URL first.",
            );
            return;
        }
        let sheet = self.sheet_input.trim();
        if sheet.is_empty() {
            ctx.show_message(
                "Set Spreadsheet",
                "Please enter the Google Sheet URL or ID.",
            );
            return;
        }

        let sheets = ctx.db.sheets();
        let previous = sheets.api_url();
        sheets.set_api_url(url);

        let result = sheets.set_spreadsheet(sheet);

        sheets.set_api_url(&previous);

        match result {
            Ok(true) => ctx.show_message("Set Spreadsheet", "Spreadsheet configured successfully!"),
            Ok(false) => ctx.show_message(
                "Set Spreadsheet",
                "Failed to configure spreadsheet. Check the URL/ID.",
            ),
            Err(e) => ctx.show_message("Set Spreadsheet", format!("Error: {e}")),
        }
    }

    /// Apply the settings.
    fn apply(&mut self, ctx: &mut ViewContext<'_>) {
        let mode = self.config.db_mode;
        if mode == DbMode::Gsheet {
            let url = self.url_input.trim();
            if url.is_empty() {
                ctx.show_message(
                    messages::SETTINGS,
                    "Please enter the Google App Script Web App URL first.",
                );
                return;
            }
            self.config.set_gsheet_api_url(url);
            let _ = self.config.save();
            ctx.db.set_api_url(url);
        }

        ctx.db.switch_mode(mode);

        let label = match mode {
            DbMode::Sqlite => "SQLite",
            DbMode::Gsheet => "Google Sheets",
        };
        ctx.show_message(
            "Database Mode",
            format!(
                "Database mode switched to {label}.\nRestart the app for full re-initialization."
            ),
        );
    }
}
