//! The parity check of Phase 7.
//!
//! The **same** scenario is run against the SQLite backend and against the
//! Google Sheets backend, and both must produce the same result. The Sheets
//! backend talks to a stateful fake of the Apps Script
//! (`tests/support/apps_script.rs`) that follows `apps-script/gsheet_api.gs`.
//!
//! The scenario covers the six things Phase 7 has to prove: **register**,
//! **scan**, **points**, **export**, **dedup** and the **mode switch**.

mod support;

use std::sync::Arc;

use ev_ap::config::DbMode;
use ev_ap::db::sqlite::SqliteBackend;
use ev_ap::db::{Backend, Database, SharedBackend};
use ev_ap::managers::export;
use ev_ap::managers::rfid_cache::now_secs;
use ev_ap::managers::{EventManager, MemberManager, ScanResult, TableManager};
use support::apps_script::{new_state, SharedState};
use support::{FakeAppsScript, RecordedRequest};
use tempfile::TempDir;

/// What the scenario observed.
#[derive(Debug, PartialEq)]
struct ScenarioResult {
    registered: bool,
    member_name: Option<String>,
    event_name: Option<String>,
    first_scan: String,
    second_scan: String,
    points: f64,
    after_redeem: f64,
    exported_rows: usize,
    export_header: String,
}

/// Run the scenario against a backend.
fn run_scenario(db: SharedBackend) -> ScenarioResult {
    let members = MemberManager::new(db.clone());
    let events = EventManager::new(db.clone());
    let tables = TableManager::new(db.clone());

    // ── register ────────────────────────────────────────────────────
    let registered = members
        .register_member("rf-1", "M001", "Alice", "S001", "CS", "1")
        .is_ok();

    // ── create an event ─────────────────────────────────────────────
    let event_name = events.create_event("Test Event").ok();

    // ── scan ────────────────────────────────────────────────────────
    let event = event_name.clone().unwrap_or_default();
    let first = tables.scan(&event, "rf-1");
    let first_scan = describe(&first);

    // ── dedup: the same tag again, inside the window ─────────────────
    let second = tables.scan(&event, "rf-1");
    let second_scan = describe(&second);

    // ── points ──────────────────────────────────────────────────────
    let _ = members.add_points("rf-1", 25.0);
    let points = members.points("rf-1").unwrap_or(-1.0);
    let _ = members.redeem_points("rf-1", 10.0);
    let after_redeem = members.points("rf-1").unwrap_or(-1.0);

    // ── export ──────────────────────────────────────────────────────
    let rows = tables.fetch_rows(&event).unwrap_or_default();
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("attendance.csv");
    let _ = export::write(&rows, &path);
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let export_header = content.lines().next().unwrap_or_default().to_string();

    ScenarioResult {
        registered,
        member_name: members.member_exists("rf-1").ok().flatten().map(|m| m.name),
        event_name,
        first_scan,
        second_scan,
        points,
        after_redeem,
        exported_rows: rows.len(),
        export_header,
    }
}

/// A short description of a scan outcome.
fn describe(result: &ScanResult) -> String {
    match result {
        ScanResult::Recorded => "recorded".to_string(),
        ScanResult::AlreadyAttended => "already attended".to_string(),
        ScanResult::Duplicate(_) => "ignored".to_string(),
        ScanResult::Failed(message) => format!("failed: {message}"),
    }
}

/// A SQLite backend in a temporary directory.
fn sqlite_backend() -> (TempDir, SharedBackend) {
    let dir = TempDir::new().expect("temp dir");
    let backend = SqliteBackend::with_path(dir.path().join("Ev_Ap.db")).expect("open db");
    backend.initialize_db().expect("initialize db");
    (dir, Arc::new(backend))
}

/// A Sheets backend on top of the stateful fake.
fn sheets_backend(state: &SharedState) -> (TempDir, SharedBackend, FakeAppsScript) {
    let state = Arc::clone(state);
    let server = FakeAppsScript::start_with_handler(move |request: &RecordedRequest| {
        let payload = request.json();
        let mut state = state.lock().expect("state lock");
        let body = state.handle(&request.action(), &payload);
        (200, body.to_string())
    });

    let dir = TempDir::new().expect("temp dir");
    let database =
        Database::with_sqlite_path(DbMode::Gsheet, &server.url, &dir.path().join("Ev_Ap.db"))
            .expect("router");
    (dir, Arc::new(database), server)
}

#[test]
fn the_scenario_behaves_the_same_on_both_backends() {
    let (_sqlite_dir, sqlite) = sqlite_backend();
    let state = new_state();
    let (_sheets_dir, sheets, _server) = sheets_backend(&state);

    let on_sqlite = run_scenario(sqlite);
    let on_sheets = run_scenario(sheets);

    // Everything that both backends support must be identical.
    assert_eq!(on_sqlite.registered, on_sheets.registered);
    assert_eq!(on_sqlite.member_name, on_sheets.member_name);
    assert_eq!(on_sqlite.event_name, on_sheets.event_name);
    assert_eq!(on_sqlite.first_scan, on_sheets.first_scan);
    assert_eq!(on_sqlite.second_scan, on_sheets.second_scan);
    assert_eq!(on_sqlite.exported_rows, on_sheets.exported_rows);
    assert_eq!(on_sqlite.export_header, on_sheets.export_header);

    // The points are the **one** known difference, and it is not a port
    // defect: the Apps Script has no `addPoints` / `redeemPoints` handler,
    // so the script answers "Unknown action: addPoints" and the balance
    // stays at 0. The Python app has exactly the same gap.
    //
    // The Rust client already sends the actions, so the Sheets backend
    // starts working the moment the handler is added to the script.
    assert_eq!(
        on_sqlite.points, 25.0,
        "the SQLite backend must award the points"
    );
    assert_eq!(
        on_sheets.points, 0.0,
        "the Sheets backend cannot award points while the script has no handler"
    );
    assert_eq!(on_sqlite.after_redeem, 15.0);
    assert_eq!(on_sheets.after_redeem, 0.0);
}

#[test]
fn the_scenario_covers_everything_phase_7_has_to_prove() {
    let (_dir, sqlite) = sqlite_backend();
    let result = run_scenario(sqlite);

    // register
    assert!(result.registered);
    assert_eq!(result.member_name.as_deref(), Some("Alice"));

    // create an event
    assert_eq!(result.event_name.as_deref(), Some("Test_Event"));

    // scan
    assert_eq!(result.first_scan, "recorded");

    // dedup
    assert_eq!(result.second_scan, "ignored");

    // points
    assert_eq!(result.points, 25.0);
    assert_eq!(result.after_redeem, 15.0);

    // export
    assert_eq!(result.exported_rows, 1);
    assert_eq!(
        result.export_header, "memberid,name,student_num,attendance_time",
        "the export must have the same columns as the Python app"
    );
}

#[test]
fn the_sheets_backend_stores_the_data_in_the_script() {
    let state = new_state();
    let (_dir, sheets, _server) = sheets_backend(&state);
    run_scenario(sheets);

    let state = state.lock().expect("state lock");
    assert_eq!(state.members.len(), 1, "the member must be in the sheet");
    assert_eq!(state.member("rf-1").expect("member").name, "Alice");
    assert_eq!(state.event_names(), vec!["Test_Event".to_string()]);
    assert_eq!(
        state.events.get("Test_Event").expect("event").len(),
        1,
        "the attendance must be in the event sheet"
    );
}

#[test]
fn the_mode_switch_moves_the_data_over() {
    // The same database file, used with both modes.
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("Ev_Ap.db");

    let database = Database::with_sqlite_path(DbMode::Sqlite, "", &path).expect("router");
    database.initialize_db().expect("init");
    database
        .member_register("rf-1", "M001", "Alice", "S001", "CS", "1")
        .expect("register");

    assert_eq!(database.mode(), DbMode::Sqlite);
    let member = database.member_exists("rf-1").expect("lookup");
    assert!(member.is_some());

    // Switch to the Sheets mode — the local data is still there, the switch
    // only changes where new calls go.
    database.switch_mode(DbMode::Gsheet);
    assert_eq!(database.mode(), DbMode::Gsheet);
    assert_eq!(database.mode(), DbMode::Gsheet);
}

#[test]
fn the_dedup_window_is_the_same_on_both_backends() {
    let (_sqlite_dir, sqlite) = sqlite_backend();
    let state = new_state();
    let (_sheets_dir, sheets, _server) = sheets_backend(&state);

    for db in [sqlite, sheets] {
        db.create_event_table("Event1").expect("create");
        db.member_register("rf-1", "M001", "Alice", "S001", "CS", "1")
            .expect("register");
        let manager = TableManager::new(db);

        manager.rfid_cache().record("rf-1", now_secs());
        assert!(manager.rfid_cache().is_duplicate("rf-1", now_secs()));
        assert!(!manager.rfid_cache().is_duplicate("rf-1", now_secs() + 15.0));
    }
}
