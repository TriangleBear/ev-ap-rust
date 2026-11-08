//! RFID de-duplication cache.
//!
//! The Python app keeps `member_manager.rfid_cache`, a `dict` mapping an RFID
//! tag to the timestamp of its last **successful** scan, and ignores a rescan
//! of the same tag that arrives within 15 seconds. A keyboard-wedge scanner
//! fires the same tag twice in quick succession often enough that the guard is
//! user visible, so the behaviour is reproduced here byte for byte — including
//! the message text.
//!
//! The cache is only written after a *successful* attendance record, exactly
//! like the Python `_on_scan_complete` handler.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Length of the de-duplication window, in seconds.
pub const DEDUP_WINDOW_SECONDS: f64 = 15.0;

/// Current wall-clock time in seconds since the Unix epoch.
///
/// Mirrors Python's `time.time()`.
pub fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Maps an RFID tag to the timestamp of its last successful scan.
#[derive(Debug)]
pub struct RfidCache {
    entries: Mutex<HashMap<String, f64>>,
    window: f64,
}

impl Default for RfidCache {
    fn default() -> Self {
        RfidCache::new()
    }
}

impl RfidCache {
    /// A cache with the standard 15 second window.
    pub fn new() -> Self {
        RfidCache::with_window(DEDUP_WINDOW_SECONDS)
    }

    /// A cache with a custom window, in seconds.
    pub fn with_window(window: f64) -> Self {
        RfidCache {
            entries: Mutex::new(HashMap::new()),
            window,
        }
    }

    /// The de-duplication window in seconds.
    pub fn window(&self) -> f64 {
        self.window
    }

    /// Was this tag scanned within the window?
    pub fn is_duplicate(&self, rfid: &str, now: f64) -> bool {
        let entries = match self.entries.lock() {
            Ok(entries) => entries,
            Err(_) => return false,
        };
        match entries.get(rfid) {
            Some(last) => now - last < self.window,
            None => false,
        }
    }

    /// The message shown when a scan is ignored, or `None` when the scan is
    /// allowed through.
    ///
    /// The wording matches the Python messagebox exactly:
    /// `"<rfid> was scanned within the last 15 seconds. Ignoring scan..."`.
    pub fn duplicate_message(&self, rfid: &str, now: f64) -> Option<String> {
        if self.is_duplicate(rfid, now) {
            Some(format!(
                "{rfid} was scanned within the last {} seconds. Ignoring scan...",
                self.window
            ))
        } else {
            None
        }
    }

    /// Record a successful scan of this tag.
    pub fn record(&self, rfid: &str, now: f64) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(rfid.to_string(), now);
        }
    }

    /// Timestamp of the last recorded scan of this tag.
    pub fn last_seen(&self, rfid: &str) -> Option<f64> {
        self.entries.lock().ok()?.get(rfid).copied()
    }

    /// Number of tags currently held in the cache.
    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .map(|entries| entries.len())
            .unwrap_or(0)
    }

    /// Is the cache empty?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Forget every recorded scan.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_scan_is_never_a_duplicate() {
        let cache = RfidCache::new();
        assert!(!cache.is_duplicate("rf-1", 1000.0));
        assert!(cache.duplicate_message("rf-1", 1000.0).is_none());
    }

    #[test]
    fn a_rescan_inside_the_window_is_a_duplicate() {
        let cache = RfidCache::new();
        cache.record("rf-1", 1000.0);
        assert!(cache.is_duplicate("rf-1", 1005.0));
        assert!(cache.is_duplicate("rf-1", 1014.999));
    }

    #[test]
    fn a_rescan_after_the_window_is_allowed() {
        let cache = RfidCache::new();
        cache.record("rf-1", 1000.0);
        assert!(!cache.is_duplicate("rf-1", 1015.0));
        assert!(!cache.is_duplicate("rf-1", 1100.0));
    }

    #[test]
    fn the_message_matches_the_python_wording() {
        let cache = RfidCache::new();
        cache.record("rf-2", 1000.0);
        assert_eq!(
            cache.duplicate_message("rf-2", 1001.0).unwrap(),
            "rf-2 was scanned within the last 15 seconds. Ignoring scan..."
        );
    }

    #[test]
    fn tags_are_tracked_independently() {
        let cache = RfidCache::new();
        cache.record("rf-1", 1000.0);
        assert!(cache.is_duplicate("rf-1", 1001.0));
        assert!(!cache.is_duplicate("rf-2", 1001.0));
        assert_eq!(cache.len(), 1);
        cache.clear();
        assert!(cache.is_empty());
    }
}
