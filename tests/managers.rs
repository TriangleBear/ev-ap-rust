//! Tests for the managers.
//!
//! These cover the logic that the Python managers used to hide inside Tkinter
//! callbacks: the registration flow, the event flow, the scan flow with its
//! 15 second de-duplication, the search filter and the export.

use std::sync::Arc;

use ev_ap::db::sqlite::SqliteBackend;
use ev_ap::db::{Row, SharedBackend};
use ev_ap::managers::export;
use ev_ap::managers::messages;
use ev_ap::managers::rfid_cache::{now_secs, RfidCache};
use ev_ap::managers::{EventManager, MemberManager, ScanResult, TableManager};
use tempfile::TempDir;

/// An initialised SQLite backend in a temporary directory.
fn setup() -> (TempDir, SharedBackend) {
    let dir = TempDir::new().expect("temp dir");
    let backend = SqliteBackend::with_path(dir.path().join("Ev_Ap.db")).expect("open db");
    backend.initialize_db().expect("initialize db");
    (dir, Arc::new(backend))
}

/// Create an event table and register a member that can be scanned.
fn prepare(db: &SharedBackend, table: &str, rfid: &str) {
    db.create_event_table(table).expect("create table");
    db.member_register(rfid, "M001", "Alice", "S001", "CS", "1")
        .expect("register");
}

fn row(cells: &[(&str, &str)]) -> Row {
    Row::new(
        cells
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect(),
    )
}

// ── Event manager ───────────────────────────────────────────────────────

#[test]
fn creating_an_event_creates_its_table_and_returns_the_stored_name() {
    let (_dir, db) = setup();
    let manager = EventManager::new(db);

    let stored = manager.create_event("Test Event").expect("create");

    assert_eq!(stored, "Test_Event");
    assert!(manager
        .list_events()
        .expect("list")
        .contains(&"Test_Event".to_string()));
}

#[test]
fn creating_an_event_without_a_name_is_refused() {
    let (_dir, db) = setup();
    let manager = EventManager::new(db);

    let error = manager.create_event("   ").expect_err("must fail");
    assert_eq!(error.title, messages::CREATE_EVENT);
    assert_eq!(error.text, messages::NO_EVENT_NAME);
}

#[test]
fn deleting_an_event_drops_its_table() {
    let (_dir, db) = setup();
    let manager = EventManager::new(db);
    manager.create_event("Doomed").expect("create");

    manager.delete_event("Doomed").expect("delete");

    assert!(!manager
        .list_events()
        .expect("list")
        .contains(&"Doomed".to_string()));
}

#[test]
fn the_created_message_matches_the_python_wording() {
    let message = EventManager::created_message();
    assert_eq!(message.title, messages::EVENT_CREATION);
    assert_eq!(message.text, messages::EVENT_CREATED);
}

// ── Member manager ──────────────────────────────────────────────────────

#[test]
fn registering_a_member_stores_it() {
    let (_dir, db) = setup();
    let manager = MemberManager::new(db);

    manager
        .register_member("rf-1", "M001", "Alice", "S001", "CS", "1")
        .expect("register");

    let member = manager
        .member_exists("rf-1")
        .expect("lookup")
        .expect("found");
    assert_eq!(member.memberid, "M001");
    assert_eq!(member.name, "Alice");
}

#[test]
fn registering_the_same_tag_twice_is_refused() {
    let (_dir, db) = setup();
    let manager = MemberManager::new(db);
    manager
        .register_member("rf-1", "M001", "Alice", "S001", "CS", "1")
        .expect("register");

    let error = manager
        .register_member("rf-1", "M002", "Bob", "S002", "CS", "2")
        .expect_err("must fail");

    assert_eq!(error.title, messages::MEMBER_REGISTRATION);
    assert_eq!(error.text, messages::MEMBER_ALREADY_EXISTS);
    assert_eq!(error.text, "Member already exists!");
}

#[test]
fn points_can_be_awarded_read_and_spent() {
    let (_dir, db) = setup();
    let manager = MemberManager::new(db);
    manager
        .register_member("rf-1", "M001", "Alice", "S001", "CS", "1")
        .expect("register");

    manager.add_points("rf-1", 25.0).expect("add");
    assert_eq!(manager.points("rf-1").expect("read"), 25.0);

    manager.redeem_points("rf-1", 10.0).expect("redeem");
    assert_eq!(manager.points("rf-1").expect("read"), 15.0);
}

#[test]
fn redeeming_more_points_than_the_balance_is_refused() {
    let (_dir, db) = setup();
    let manager = MemberManager::new(db);
    manager
        .register_member("rf-1", "M001", "Alice", "S001", "CS", "1")
        .expect("register");
    manager.add_points("rf-1", 5.0).expect("add");

    let error = manager.redeem_points("rf-1", 10.0).expect_err("must fail");
    assert_eq!(error.title, messages::POINTS_REDEMPTION);
    assert_eq!(error.text, messages::NOT_ENOUGH_POINTS);

    assert_eq!(
        manager.points("rf-1").expect("read"),
        5.0,
        "a refused redemption must not change the balance"
    );
}

// ── Table manager / scan flow ───────────────────────────────────────────

#[test]
fn a_scan_records_the_attendance() {
    let (_dir, db) = setup();
    prepare(&db, "Event1", "rf-1");
    let manager = TableManager::new(db);

    let result = manager.scan("Event1", "rf-1");

    assert_eq!(result, ScanResult::Recorded);
    assert_eq!(result.title(), messages::RFID_SCAN);
    assert_eq!(result.text(), "RFID recorded");
    assert!(result.is_recorded());
}

#[test]
fn scanning_an_unknown_tag_fails_with_the_python_message() {
    let (_dir, db) = setup();
    prepare(&db, "Event1", "rf-1");
    let manager = TableManager::new(db);

    let result = manager.scan("Event1", "unknown-tag");

    assert_eq!(result.title(), messages::RFID_SCAN_ERROR);
    assert_eq!(result.text(), "RFID number not found");
    assert!(!result.is_recorded());
}

#[test]
fn scanning_the_same_tag_twice_reports_already_attended() {
    let (_dir, db) = setup();
    prepare(&db, "Event1", "rf-1");
    // A zero length window simulates a scan that happens more than 15 seconds
    // after the first one, so the de-duplication does not kick in.
    let manager = TableManager::with_cache(db, Arc::new(RfidCache::with_window(0.0)));

    assert_eq!(manager.scan("Event1", "rf-1"), ScanResult::Recorded);
    assert_eq!(manager.scan("Event1", "rf-1"), ScanResult::AlreadyAttended);
}

#[test]
fn a_rescan_inside_the_window_is_ignored_before_the_duplicate_check() {
    let (_dir, db) = setup();
    prepare(&db, "Event1", "rf-1");
    let manager = TableManager::new(db);

    // The first scan is recorded and puts the tag into the cache.
    assert_eq!(manager.scan("Event1", "rf-1"), ScanResult::Recorded);
    // The second scan is inside the window, so the de-duplication wins over
    // the "already attended" answer.
    assert!(matches!(
        manager.scan("Event1", "rf-1"),
        ScanResult::Duplicate(_)
    ));
}

#[test]
fn a_quick_rescan_is_ignored_and_records_nothing() {
    let (_dir, db) = setup();
    prepare(&db, "Event1", "rf-1");
    let manager = TableManager::new(db);

    // Pretend the tag was just scanned.
    manager.rfid_cache().record("rf-1", now_secs());

    let result = manager.scan("Event1", "rf-1");

    assert_eq!(result.title(), messages::RFID_SCAN);
    assert!(
        result
            .text()
            .ends_with("was scanned within the last 15 seconds. Ignoring scan..."),
        "unexpected message: {}",
        result.text()
    );
    assert!(!result.is_recorded());
    assert!(
        !manager
            .fetch_rows("Event1")
            .expect("rows")
            .iter()
            .any(|row| row.get("rfid") == Some("rf-1")),
        "an ignored scan must not record attendance"
    );
}

#[test]
fn the_dedup_cache_is_only_written_after_a_successful_record() {
    let (_dir, db) = setup();
    prepare(&db, "Event1", "rf-1");
    let manager = TableManager::new(db);

    // A failed scan must not poison the cache.
    manager.scan("Event1", "unknown-tag");
    assert!(manager.rfid_cache().last_seen("unknown-tag").is_none());

    manager.scan("Event1", "rf-1");
    assert!(manager.rfid_cache().last_seen("rf-1").is_some());
}

#[test]
fn the_member_and_table_managers_can_share_one_cache() {
    let (_dir, db) = setup();
    let cache = Arc::new(RfidCache::new());
    let members = MemberManager::with_cache(db.clone(), Arc::clone(&cache));
    let table = TableManager::with_cache(db, Arc::clone(&cache));

    assert!(Arc::ptr_eq(members.rfid_cache(), table.rfid_cache()));
}

// ── Search and layout ───────────────────────────────────────────────────

#[test]
fn the_search_filter_matches_cell_values_case_insensitively() {
    let rows = vec![
        row(&[("memberid", "M001"), ("name", "Alice Smith")]),
        row(&[("memberid", "M002"), ("name", "Bob Jones")]),
    ];

    let filtered = TableManager::filter_rows(&rows, "alice");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].get("name"), Some("Alice Smith"));

    let filtered = TableManager::filter_rows(&rows, "M002");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].get("name"), Some("Bob Jones"));

    assert_eq!(TableManager::filter_rows(&rows, "").len(), 2);
    assert_eq!(TableManager::filter_rows(&rows, "nobody").len(), 0);
}

#[test]
fn the_column_widths_are_clamped_like_the_python_ones() {
    let rows = vec![row(&[("memberid", "M001"), ("name", "Alice")])];

    let widths = TableManager::column_widths(&rows);
    assert_eq!(widths.len(), 2);
    assert!(widths.iter().all(|width| (100..=400).contains(width)));

    let wide = vec![row(&[("name", &"x".repeat(200))])];
    let widths = TableManager::column_widths(&wide);
    assert_eq!(widths[0], 400, "wide columns are clamped to 400");
}

// ── Export ──────────────────────────────────────────────────────────────

#[test]
fn the_csv_export_has_a_header_row_and_one_line_per_record() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("attendance.csv");
    let rows = vec![
        row(&[("memberid", "M001"), ("name", "Alice Smith")]),
        row(&[("memberid", "M002"), ("name", "Bob Jones")]),
    ];

    export::write_csv(&rows, &path).expect("write csv");

    let content = std::fs::read_to_string(&path).expect("read");
    assert_eq!(content, "memberid,name\nM001,Alice Smith\nM002,Bob Jones\n");
}

#[test]
fn the_csv_export_quotes_values_containing_the_separator() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("attendance.csv");
    let rows = vec![row(&[("name", "Smith, Alice")])];

    export::write_csv(&rows, &path).expect("write csv");

    let content = std::fs::read_to_string(&path).expect("read");
    assert_eq!(content, "name\n\"Smith, Alice\"\n");
}

#[test]
fn the_excel_export_writes_a_readable_workbook() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("attendance.xlsx");
    let rows = vec![
        row(&[("memberid", "M001"), ("name", "Alice Smith")]),
        row(&[("memberid", "M002"), ("name", "Bob Jones")]),
    ];

    export::write_xlsx(&rows, &path).expect("write xlsx");

    assert!(path.exists());
    assert!(std::fs::metadata(&path).expect("metadata").len() > 0);

    // A zip container — the same shape `pandas.to_excel` produces.
    let bytes = std::fs::read(&path).expect("read");
    assert_eq!(&bytes[0..2], b"PK");
}

#[test]
fn the_export_picks_the_format_from_the_extension() {
    let dir = TempDir::new().expect("temp dir");
    let rows = vec![row(&[("name", "Alice")])];

    let csv_path = dir.path().join("out.csv");
    export::write(&rows, &csv_path).expect("write");
    assert!(std::fs::read_to_string(&csv_path)
        .expect("read")
        .starts_with("name\n"));

    let xlsx_path = dir.path().join("out.xlsx");
    export::write(&rows, &xlsx_path).expect("write");
    assert_eq!(&std::fs::read(&xlsx_path).expect("read")[0..2], b"PK");
}

#[test]
fn exporting_nothing_still_writes_a_valid_file() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("empty.csv");

    export::write_csv(&[], &path).expect("write");

    assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
}
