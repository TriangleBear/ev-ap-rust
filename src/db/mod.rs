//! Database layer.
//!
//! [`Backend`] mirrors the `db_mode` routing that `DBActions` performs in the
//! Python app: every call is dispatched to either the local SQLite backend or
//! the Google Sheets (Apps Script) backend.
//!
//! Populated in Phase 1 (SQLite) and Phase 2 (Sheets).
