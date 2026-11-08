//! Entry point for the `Attendance_App` binary.
//!
//! The window is opened at 1100×650, the same size as the Python app's
//! `MainApp` (`self.root.geometry("1100x650")`).

use std::sync::Arc;

use ev_ap::app::{AppShell, WINDOW_TITLE};
use ev_ap::config::Config;
use ev_ap::db::Database;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = Config::load();
    tracing::info!(
        "starting {WINDOW_TITLE} in {} mode",
        config.db_mode().as_str()
    );

    let db = Arc::new(
        Database::new(config.db_mode(), config.gsheet_api_url()).map_err(|e| -> eframe::Error {
            eframe::Error::AppCreation(format!("could not open the database: {e}").into())
        })?,
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size([1100.0, 650.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        WINDOW_TITLE,
        options,
        Box::new(|_cc| Ok(Box::new(AppShell::new(db)))),
    )
}
