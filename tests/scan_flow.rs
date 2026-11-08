//! Tests for the RFID scan flow.
//!
//! The scan is the heart of the app: a keyboard-wedge scanner types the tag
//! into the entry and presses Enter, the scan runs on a worker thread, and
//! the result is reported in a message box.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ev_ap::db::sqlite::SqliteBackend;
use ev_ap::db::Backend;
use ev_ap::managers::rfid_cache::now_secs;
use ev_ap::managers::{messages, ScanResult, TableManager};
use ev_ap::ui::attendance_window::AttendanceWindow;
use tempfile::TempDir;

/// An initialised backend with an event and a registered member.
fn setup() -> (TempDir, Arc<SqliteBackend>) {
    let dir = TempDir::new().expect("temp dir");
    let backend = SqliteBackend::with_path(dir.path().join("Ev_Ap.db")).expect("open db");
    backend.initialize_db().expect("initialize db");
    backend.create_event_table("Event1").expect("create event");
    backend
        .member_register("rf-1", "M001", "Alice", "S001", "CS", "1")
        .expect("register");
    (dir, Arc::new(backend))
}

/// Wait until the scan of the window finished.
fn wait_for_scan(window: &mut AttendanceWindow, manager: &TableManager) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while window.scanning && Instant::now() < deadline {
        window.collect_scan_result(manager);
        std::thread::sleep(Duration::from_millis(5));
    }
    window.collect_scan_result(manager);
}

// ── The worker thread ───────────────────────────────────────────────────

#[test]
fn a_scan_on_a_worker_thread_records_the_attendance() {
    let (_dir, backend) = setup();
    let manager = TableManager::new(backend.clone());

    let receiver = manager.spawn_scan("Event1", "rf-1");
    let result = receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("the scan must report a result");

    assert_eq!(result, ScanResult::Recorded);
    assert!(
        backend
            .member_attended_event("Event1", "rf-1")
            .expect("attended"),
        "the attendance must be written by the worker thread"
    );
}

#[test]
fn an_unknown_tag_reports_the_python_message() {
    let (_dir, backend) = setup();
    let manager = TableManager::new(backend.clone());

    let receiver = manager.spawn_scan("Event1", "unknown");
    let result = receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("the scan must report a result");

    match result {
        ScanResult::Failed(message) => assert_eq!(message, "RFID number not found"),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn the_second_scan_of_a_tag_reports_already_attended() {
    let (_dir, backend) = setup();
    let manager = TableManager::new(backend.clone());

    let first = manager
        .spawn_scan("Event1", "rf-1")
        .recv_timeout(Duration::from_secs(10))
        .expect("result");
    assert_eq!(first, ScanResult::Recorded);

    // Outside the de-duplication window the tag is scanned again and the
    // member is reported as having attended already.
    let second = manager
        .spawn_scan("Event1", "rf-1")
        .recv_timeout(Duration::from_secs(10))
        .expect("result");
    assert_eq!(
        second,
        ScanResult::Duplicate(
            "rf-1 was scanned within the last 15 seconds. Ignoring scan...".to_string()
        )
    );
}

// ── The window ──────────────────────────────────────────────────────────

#[test]
fn the_window_starts_a_scan_and_reports_the_result() {
    let (_dir, backend) = setup();
    let manager = TableManager::new(backend.clone());
    let mut window = AttendanceWindow::new("Event1");

    window.rfid_input = "rf-1".to_string();
    window.start_scan(&manager);

    assert!(window.scanning, "the scan must run in the background");

    wait_for_scan(&mut window, &manager);

    assert!(!window.scanning);
    let (title, text) = window.message.take().expect("a message");
    assert_eq!(title, messages::RFID_SCAN);
    assert_eq!(text, "RFID recorded");
    assert!(
        window.rfid_input.is_empty(),
        "the entry must be cleared after the scan"
    );
    assert_eq!(window.rows.len(), 1, "the table must be refreshed");
}

#[test]
fn the_window_reports_an_unknown_tag() {
    let (_dir, backend) = setup();
    let manager = TableManager::new(backend.clone());
    let mut window = AttendanceWindow::new("Event1");

    window.rfid_input = "unknown".to_string();
    window.start_scan(&manager);
    wait_for_scan(&mut window, &manager);

    let (title, text) = window.message.take().expect("a message");
    assert_eq!(title, messages::RFID_SCAN_ERROR);
    assert_eq!(text, "RFID number not found");
}

#[test]
fn the_window_reports_a_duplicate_without_starting_a_scan() {
    let (_dir, backend) = setup();
    let manager = TableManager::new(backend.clone());
    let mut window = AttendanceWindow::new("Event1");

    // The tag was just scanned.
    manager.rfid_cache().record("rf-1", now_secs());

    window.rfid_input = "rf-1".to_string();
    window.start_scan(&manager);

    assert!(
        !window.scanning,
        "a duplicate must be reported without starting a scan"
    );
    let (title, text) = window.message.take().expect("a message");
    assert_eq!(title, messages::RFID_SCAN);
    assert!(
        text.ends_with("was scanned within the last 15 seconds. Ignoring scan..."),
        "unexpected message: {text}"
    );
    assert_eq!(window.rfid_input, "", "the entry must be cleared");
    assert!(
        !backend
            .member_attended_event("Event1", "rf-1")
            .expect("attended"),
        "an ignored scan must not record anything"
    );
}

#[test]
fn an_empty_entry_does_not_start_a_scan() {
    let (_dir, backend) = setup();
    let manager = TableManager::new(backend.clone());
    let mut window = AttendanceWindow::new("Event1");

    window.rfid_input = "   ".to_string();
    window.start_scan(&manager);

    assert!(!window.scanning);
    assert!(window.message.is_none());
}

#[test]
fn the_window_reports_an_already_attended_member() {
    let (_dir, backend) = setup();
    let manager = TableManager::new(backend.clone());
    let mut window = AttendanceWindow::new("Event1");

    // The first scan records the attendance.
    window.rfid_input = "rf-1".to_string();
    window.start_scan(&manager);
    wait_for_scan(&mut window, &manager);
    window.message.take();

    // The second scan is outside the de-duplication window, so the member is
    // reported as having attended already.
    manager.rfid_cache().clear();
    window.rfid_input = "rf-1".to_string();
    window.start_scan(&manager);
    wait_for_scan(&mut window, &manager);

    let (title, text) = window.message.take().expect("a message");
    assert_eq!(title, messages::RFID_SCAN);
    assert_eq!(text, messages::ALREADY_ATTENDED);
}

// ── The focus ──────────────────────────────────────────────────────────

#[test]
fn the_rfid_entry_gets_the_focus_back_after_a_scan() {
    let (_dir, backend) = setup();
    let manager = TableManager::new(backend.clone());
    let ctx = egui::Context::default();

    let mut window = AttendanceWindow::new("Event1");
    window.rfid_input = "rf-1".to_string();
    window.start_scan(&manager);
    wait_for_scan(&mut window, &manager);
    window.message.take();

    // Draw the window once so it registers the id of its RFID entry, then
    // draw it again: it must ask for the focus back.
    let mut focused = false;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        window.show(ctx, &manager);
        focused = ctx.memory(|memory| memory.has_focus(window.rfid_id.unwrap()));
    });

    assert!(
        focused,
        "the RFID entry must have the focus so the next scan can be read"
    );
}
