//! Ev_Ap — Event Attendance Automation.
//!
//! Rust rewrite of the Python/CustomTkinter application in
//! [`TriangleBear/Ev_Ap`](https://github.com/TriangleBear/Ev_Ap).
//!
//! The crate is split into a library (`ev_ap`) so that every layer can be
//! unit- and integration-tested without spinning up a window, plus a thin
//! binary (`Attendance_App`) that only bootstraps the UI.

pub mod app;
pub mod config;
pub mod db;
pub mod managers;
pub mod theme;
pub mod ui;
pub mod views;

/// Version of the application, injected at build time.
///
/// Replaces the CI workflow that used to rewrite a hardcoded version string
/// inside `about_view.py`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Name of the released binary artifact.
pub const APP_NAME: &str = "Attendance_App";
