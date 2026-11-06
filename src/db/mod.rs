//! Database layer.
//!
//! [`Backend`] mirrors the `db_mode` routing that `DBActions` performs in the
//! Python app: every call is dispatched to either the local SQLite backend
//! ([`sqlite::SqliteBackend`]) or the Google Sheets (Apps Script) backend
//! ([`sheets::SheetBackend`], added in Phase 2).
//!
//! The façade deliberately mirrors the Python method names and return
//! semantics so the port can be checked line by line.

pub mod sheets;
pub mod sqlite;

use std::sync::{Arc, RwLock};

use crate::config::DbMode;

use self::sheets::SheetBackend;
use self::sqlite::SqliteBackend;

/// A member record, as returned by `member_exists`.
///
/// The Python version returns a `sqlite3.Row` limited to `memberid`, `name`
/// and `student_num`; the same three fields are used here.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Member {
    pub memberid: String,
    pub name: String,
    pub student_num: String,
}

/// One row of a table, as an ordered list of `(column, value)` pairs.
///
/// Ordered pairs keep the UI table renderer backend agnostic: SQLite rows and
/// Sheets dictionaries can both be turned into a `Row`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    pub cells: Vec<(String, String)>,
}

impl Row {
    /// Build a row from ordered `(column, value)` pairs.
    pub fn new(cells: Vec<(String, String)>) -> Self {
        Row { cells }
    }

    /// Look up a cell by column name.
    pub fn get(&self, column: &str) -> Option<&str> {
        self.cells
            .iter()
            .find(|(name, _)| name == column)
            .map(|(_, value)| value.as_str())
    }

    /// Column names of this row, in order.
    pub fn columns(&self) -> Vec<&str> {
        self.cells.iter().map(|(name, _)| name.as_str()).collect()
    }

    /// Cell values of this row, in order.
    pub fn values(&self) -> Vec<&str> {
        self.cells.iter().map(|(_, value)| value.as_str()).collect()
    }
}

/// Result of a batched attendance scan.
///
/// Mirrors the Python dictionary returned by `DBActions.scan_attendance()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOutcome {
    /// `false` when the tag is unknown or the write failed.
    pub success: bool,
    /// `true` when the member had already been recorded for this event.
    pub attended: bool,
    /// Human readable reason, mirroring the Python `error` key.
    pub error: Option<String>,
}

impl ScanOutcome {
    /// A successful scan that recorded a new attendance row.
    pub fn recorded() -> Self {
        ScanOutcome {
            success: true,
            attended: false,
            error: None,
        }
    }

    /// A successful scan for a member that was already recorded.
    pub fn already_attended() -> Self {
        ScanOutcome {
            success: true,
            attended: true,
            error: None,
        }
    }

    /// A failed scan carrying a reason.
    pub fn failed(error: impl Into<String>) -> Self {
        ScanOutcome {
            success: false,
            attended: false,
            error: Some(error.into()),
        }
    }
}

/// Errors raised by the data layer.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// The local database rejected the statement.
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The event name cannot be turned into a safe SQL identifier.
    #[error("invalid event name: {0}")]
    InvalidIdentifier(String),
    /// Filesystem problem (config file, database file).
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    /// The backend returned a failure.
    #[error("{0}")]
    Backend(String),
}

/// The data-layer façade.
///
/// Every method has a one-to-one counterpart in the Python `DBActions` class;
/// the doc comment on each method names it.
pub trait Backend {
    /// `DBActions.member_register` — insert a new member.
    fn member_register(
        &self,
        rfid: &str,
        memberid: &str,
        name: &str,
        student_num: &str,
        program: &str,
        year: &str,
    ) -> Result<(), DbError>;

    /// `DBActions.member_exists` — look up a member by RFID tag.
    fn member_exists(&self, rfid: &str) -> Result<Option<Member>, DbError>;

    /// `DBActions.list_tables` — list the event tables.
    fn list_tables(&self) -> Result<Vec<String>, DbError>;

    /// `DBActions.create_event_table` — create the table for an event.
    fn create_event_table(&self, table_name: &str) -> Result<(), DbError>;

    /// `DBActions.fetch_table_data` — read the rows of `Members` or an event table.
    fn fetch_table_data(&self, table_name: &str) -> Result<Vec<Row>, DbError>;

    /// `DBActions.attendance_member_event` — record attendance for a member.
    fn attendance_member_event(&self, table_name: &str, rfid: &str) -> Result<(), DbError>;

    /// `DBActions.scan_attendance` — batched existence + duplicate + record.
    fn scan_attendance(&self, table_name: &str, rfid: &str) -> Result<ScanOutcome, DbError>;

    /// `DBActions.get_member_name` — the member's display name.
    fn get_member_name(&self, rfid: &str) -> Result<Option<String>, DbError>;

    /// `DBActions.member_attended_event` — has this member attended this event?
    fn member_attended_event(&self, table_name: &str, rfid: &str) -> Result<bool, DbError>;

    /// `DBActions.add_points` — award points to a member.
    fn add_points(&self, rfid: &str, points: f64) -> Result<(), DbError>;

    /// `DBActions.get_member_points` — current point balance.
    fn get_member_points(&self, rfid: &str) -> Result<f64, DbError>;

    /// `DBActions.redeem_points` — spend points, only if the balance allows it.
    fn redeem_points(&self, rfid: &str, points: f64) -> Result<(), DbError>;

    /// `DBActions.delete_event_table` — drop an event table.
    fn delete_event_table(&self, table_name: &str) -> Result<(), DbError>;
}

/// A shared, thread-safe handle to a backend.
///
/// The Python app keeps a single global `DBActions` instance; this is the Rust
/// equivalent and is cheap to clone.
pub type SharedBackend = Arc<dyn Backend + Send + Sync>;

/// The router that dispatches every call to the active backend.
///
/// This is the Rust counterpart of the Python `Database` class in
/// `App/database/dblite.py`, which holds both backends and routes each call
/// according to `db_mode`.
pub struct Database {
    mode: RwLock<DbMode>,
    sqlite: SqliteBackend,
    sheets: SheetBackend,
}

impl Database {
    /// Create the router with an already selected backend.
    pub fn new(mode: DbMode, gsheet_api_url: &str) -> Result<Self, DbError> {
        Ok(Database {
            mode: RwLock::new(mode),
            sqlite: SqliteBackend::new()?,
            sheets: SheetBackend::new(gsheet_api_url)?,
        })
    }

    /// Create the router with an explicit SQLite database path.
    ///
    /// Used by the tests and by the parity tooling.
    pub fn with_sqlite_path(
        mode: DbMode,
        gsheet_api_url: &str,
        sqlite_path: &std::path::Path,
    ) -> Result<Self, DbError> {
        Ok(Database {
            mode: RwLock::new(mode),
            sqlite: SqliteBackend::with_path(sqlite_path)?,
            sheets: SheetBackend::new(gsheet_api_url)?,
        })
    }

    /// The active backend.
    pub fn mode(&self) -> DbMode {
        self.mode.read().map(|mode| *mode).unwrap_or_default()
    }

    /// Switch the active backend, mirroring Python's `Database.switch_mode`.
    pub fn switch_mode(&self, mode: DbMode) {
        if let Ok(mut current) = self.mode.write() {
            *current = mode;
        }
    }

    /// Point the Sheets backend at a deployment URL.
    pub fn set_api_url(&self, url: &str) {
        self.sheets.set_api_url(url);
    }

    /// The Sheets backend, for the settings flows (test connection, set
    /// spreadsheet, event details).
    pub fn sheets(&self) -> &SheetBackend {
        &self.sheets
    }

    /// The SQLite backend, for direct access.
    pub fn sqlite(&self) -> &SqliteBackend {
        &self.sqlite
    }

    /// Prepare the active backend — mirrors Python's `Database.initialize_db`.
    pub fn initialize_db(&self) -> Result<(), DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.initialize_db(),
            DbMode::Gsheet => Ok(()),
        }
    }

    /// Is the active backend usable?
    pub fn db_exists(&self) -> bool {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.db_exists(),
            DbMode::Gsheet => self.sheets.db_exists(),
        }
    }
}

impl Backend for Database {
    fn member_register(
        &self,
        rfid: &str,
        memberid: &str,
        name: &str,
        student_num: &str,
        program: &str,
        year: &str,
    ) -> Result<(), DbError> {
        match self.mode() {
            DbMode::Sqlite => {
                self.sqlite
                    .member_register(rfid, memberid, name, student_num, program, year)
            }
            DbMode::Gsheet => {
                self.sheets
                    .member_register(rfid, memberid, name, student_num, program, year)
            }
        }
    }

    fn member_exists(&self, rfid: &str) -> Result<Option<Member>, DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.member_exists(rfid),
            DbMode::Gsheet => self.sheets.member_exists(rfid),
        }
    }

    fn list_tables(&self) -> Result<Vec<String>, DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.list_tables(),
            DbMode::Gsheet => self.sheets.list_tables(),
        }
    }

    fn create_event_table(&self, table_name: &str) -> Result<(), DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.create_event_table(table_name),
            DbMode::Gsheet => self.sheets.create_event_table(table_name),
        }
    }

    fn fetch_table_data(&self, table_name: &str) -> Result<Vec<Row>, DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.fetch_table_data(table_name),
            DbMode::Gsheet => self.sheets.fetch_table_data(table_name),
        }
    }

    fn attendance_member_event(&self, table_name: &str, rfid: &str) -> Result<(), DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.attendance_member_event(table_name, rfid),
            DbMode::Gsheet => self.sheets.attendance_member_event(table_name, rfid),
        }
    }

    fn scan_attendance(&self, table_name: &str, rfid: &str) -> Result<ScanOutcome, DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.scan_attendance(table_name, rfid),
            DbMode::Gsheet => self.sheets.scan_attendance(table_name, rfid),
        }
    }

    fn get_member_name(&self, rfid: &str) -> Result<Option<String>, DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.get_member_name(rfid),
            DbMode::Gsheet => self.sheets.get_member_name(rfid),
        }
    }

    fn member_attended_event(&self, table_name: &str, rfid: &str) -> Result<bool, DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.member_attended_event(table_name, rfid),
            DbMode::Gsheet => self.sheets.member_attended_event(table_name, rfid),
        }
    }

    fn add_points(&self, rfid: &str, points: f64) -> Result<(), DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.add_points(rfid, points),
            DbMode::Gsheet => self.sheets.add_points(rfid, points),
        }
    }

    fn get_member_points(&self, rfid: &str) -> Result<f64, DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.get_member_points(rfid),
            DbMode::Gsheet => self.sheets.get_member_points(rfid),
        }
    }

    fn redeem_points(&self, rfid: &str, points: f64) -> Result<(), DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.redeem_points(rfid, points),
            DbMode::Gsheet => self.sheets.redeem_points(rfid, points),
        }
    }

    fn delete_event_table(&self, table_name: &str) -> Result<(), DbError> {
        match self.mode() {
            DbMode::Sqlite => self.sqlite.delete_event_table(table_name),
            DbMode::Gsheet => self.sheets.delete_event_table(table_name),
        }
    }
}

/// Turn an event name into a safe SQL identifier.
///
/// The Python app interpolates the event name straight into the SQL string.
/// That is an injection vector, so the Rust port validates instead:
/// spaces become underscores and everything outside `[A-Za-z0-9_]` is
/// rejected.
///
/// Mirrors the Python `table_name.replace(" ", "_")` normalisation.
pub fn normalize_table_name(name: &str) -> Result<String, DbError> {
    let normalized: String = name
        .trim()
        .chars()
        .map(|c| if c == ' ' { '_' } else { c })
        .collect();

    if normalized.is_empty() {
        return Err(DbError::InvalidIdentifier(name.to_string()));
    }

    if !normalized
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(DbError::InvalidIdentifier(name.to_string()));
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaces_become_underscores() {
        assert_eq!(normalize_table_name("Test Event").unwrap(), "Test_Event");
        assert_eq!(normalize_table_name("Another").unwrap(), "Another");
        assert_eq!(
            normalize_table_name("  Padded Name  ").unwrap(),
            "Padded_Name"
        );
    }

    #[test]
    fn unsafe_identifiers_are_rejected() {
        assert!(normalize_table_name("").is_err());
        assert!(normalize_table_name("   ").is_err());
        assert!(normalize_table_name("drop; DROP TABLE Members").is_err());
        assert!(normalize_table_name("table`name").is_err());
        assert!(normalize_table_name("na/me").is_err());
    }

    #[test]
    fn row_lookup_by_column_name() {
        let row = Row::new(vec![
            ("memberid".into(), "M001".into()),
            ("name".into(), "Alice Smith".into()),
        ]);
        assert_eq!(row.get("name"), Some("Alice Smith"));
        assert_eq!(row.get("missing"), None);
        assert_eq!(row.columns(), vec!["memberid", "name"]);
        assert_eq!(row.values(), vec!["M001", "Alice Smith"]);
    }
}
