//! Entry point for the `Attendance_App` binary.
//!
//! The UI shell is wired up in later phases; for now the binary only reports
//! the build metadata so the packaging pipeline can be validated end to end.

use ev_ap::{APP_NAME, VERSION};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("{APP_NAME} {VERSION}");
    println!("{APP_NAME} {VERSION}");
}
