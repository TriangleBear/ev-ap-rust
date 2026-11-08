//! The event table: scanning, searching and export.
//!
//! Ported from `App/managers/table_manager.py`. The Python manager builds a
//! Tkinter window; here the logic is separated from the widgets so it can be
//! tested, and the view in Phase 6 drives it.

use std::sync::Arc;

use crate::db::{DbError, Row, SharedBackend};

use super::messages::{ALREADY_ATTENDED, RFID_RECORDED, RFID_SCAN, RFID_SCAN_ERROR};
use super::rfid_cache::{now_secs, RfidCache};

/// The outcome of an RFID scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanResult {
    /// The same tag was scanned again inside the de-duplication window; the
    /// scan was ignored and nothing was written.
    Duplicate(String),
    /// The scan failed; the text is shown to the user.
    Failed(String),
    /// The member had already attended this event.
    AlreadyAttended,
    /// The attendance was recorded.
    Recorded,
}

impl ScanResult {
    /// The dialog title for this outcome.
    pub fn title(&self) -> &'static str {
        match self {
            ScanResult::Duplicate(_) => RFID_SCAN,
            ScanResult::Failed(_) => RFID_SCAN_ERROR,
            ScanResult::AlreadyAttended => RFID_SCAN,
            ScanResult::Recorded => RFID_SCAN,
        }
    }

    /// The dialog body for this outcome.
    pub fn text(&self) -> String {
        match self {
            ScanResult::Duplicate(message) => message.clone(),
            ScanResult::Failed(message) => message.clone(),
            ScanResult::AlreadyAttended => ALREADY_ATTENDED.to_string(),
            ScanResult::Recorded => RFID_RECORDED.to_string(),
        }
    }

    /// Was the attendance recorded?
    pub fn is_recorded(&self) -> bool {
        matches!(self, ScanResult::Recorded)
    }
}

/// Manages the event table window.
pub struct TableManager {
    db: SharedBackend,
    rfid_cache: Arc<RfidCache>,
}

impl TableManager {
    /// A manager on top of a backend.
    pub fn new(db: SharedBackend) -> Self {
        TableManager {
            db,
            rfid_cache: Arc::new(RfidCache::new()),
        }
    }

    /// A manager sharing an existing de-duplication cache with the member
    /// manager.
    pub fn with_cache(db: SharedBackend, rfid_cache: Arc<RfidCache>) -> Self {
        TableManager { db, rfid_cache }
    }

    /// The de-duplication cache.
    pub fn rfid_cache(&self) -> &Arc<RfidCache> {
        &self.rfid_cache
    }

    /// Record an attendance for a scanned tag.
    ///
    /// This is the Rust counterpart of `TableManager.rfid_scan_event` and its
    /// `_on_scan_complete` handler:
    ///
    /// 1. a tag seen within the de-duplication window is ignored,
    /// 2. an unknown tag is reported as a failure,
    /// 3. a member that already attended is reported as such,
    /// 4. otherwise the attendance is recorded and the tag is added to the
    ///    de-duplication cache — only after a *successful* record, exactly like
    ///    the Python code.
    pub fn scan(&self, table_name: &str, rfid: &str) -> ScanResult {
        let now = now_secs();

        if let Some(message) = self.rfid_cache.duplicate_message(rfid, now) {
            return ScanResult::Duplicate(message);
        }

        match self.db.scan_attendance(table_name, rfid) {
            Ok(outcome) if !outcome.success => {
                ScanResult::Failed(outcome.error.unwrap_or_else(|| "Unknown error".to_string()))
            }
            Ok(outcome) if outcome.attended => ScanResult::AlreadyAttended,
            Ok(_) => {
                self.rfid_cache.record(rfid, now);
                ScanResult::Recorded
            }
            Err(e) => {
                tracing::error!("scan failed for {rfid}: {e}");
                ScanResult::Failed(format!("Error: {e}"))
            }
        }
    }

    /// The rows of a table.
    pub fn fetch_rows(&self, table_name: &str) -> Result<Vec<Row>, DbError> {
        self.db.fetch_table_data(table_name)
    }

    /// Filter the rows by a search term.
    ///
    /// Mirrors the Python `search_table` handler, which matches the term
    /// against the **cell values** of every row, case insensitively.
    pub fn filter_rows(rows: &[Row], query: &str) -> Vec<Row> {
        let query = query.to_lowercase();
        if query.is_empty() {
            return rows.to_vec();
        }
        rows.iter()
            .filter(|row| {
                row.values()
                    .iter()
                    .any(|value| value.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    }

    /// The column widths of a table, in pixels.
    ///
    /// Mirrors the Python `calculate_column_widths`: the header length times
    /// eight, the longest cell length times seven plus twenty, clamped to
    /// between 100 and 400.
    pub fn column_widths(rows: &[Row]) -> Vec<usize> {
        let mut widths: Vec<usize> = rows
            .first()
            .map(|row| {
                row.columns()
                    .iter()
                    .map(|name| name.chars().count() * 8)
                    .collect()
            })
            .unwrap_or_default();

        for row in rows {
            for (index, value) in row.values().iter().enumerate() {
                if index < widths.len() {
                    let width = value.chars().count() * 7 + 20;
                    if width > widths[index] {
                        widths[index] = width;
                    }
                }
            }
        }

        widths
            .iter()
            .map(|width| (*width).clamp(100, 400))
            .collect()
    }
}
