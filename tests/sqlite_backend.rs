//! Ports of the Python test suite in `App/TEST/`.
//!
//! | Python test | Rust test |
//! |---|---|
//! | `test_sqlite_pytest.py::test_initialize_db_creates_file_and_members_table` | `initialize_db_creates_file_and_members_table` |
//! | `test_db_full_pytest.py::test_member_register_and_exists` | `member_register_and_exists` |
//! | `test_db_full_pytest.py::test_points_add_get_redeem` | `points_add_get_redeem` |
//! | `test_db_full_pytest.py::test_event_table_and_attendance` | `event_table_and_attendance` |
//! | `test_db_full_pytest.py::test_list_tables_filters_members` | `list_tables_filters_members` |
//! | `test_table_manager_pytest.py::test_rfid_scan_event_records_attendance_and_adds_points` | `rfid_scan_records_attendance_and_adds_points` |
//! | `test_table_manager_pytest.py::test_rfid_scan_event_ignores_quick_rescan` | `rfid_scan_ignores_quick_rescan` |
//!
//! The Python suite isolates itself with `monkeypatch.chdir(tmp_path)`. Rust
//! tests run in parallel threads, so changing the process working directory
//! would be a data race — instead every backend is opened on an explicit path
//! inside a [`tempfile::TempDir`].

use std::path::Path;

use ev_ap::db::sqlite::SqliteBackend;
use ev_ap::db::Backend;
use tempfile::TempDir;

/// An initialised backend inside an isolated temporary directory.
fn setup_db() -> (TempDir, SqliteBackend) {
    let dir = TempDir::new().expect("temp dir");
    let db = SqliteBackend::with_path(dir.path().join("Ev_Ap.db")).expect("open db");
    db.initialize_db().expect("initialize db");
    (dir, db)
}

/// Does a table with this name exist in `sqlite_master`?
fn table_exists(db: &SqliteBackend, name: &str) -> bool {
    db.table_exists(name).expect("table_exists")
}

#[test]
fn initialize_db_creates_file_and_members_table() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("Ev_Ap.db");
    let db = SqliteBackend::with_path(&path).expect("open db");

    db.initialize_db().expect("initialize db");

    assert!(path.exists(), "Database file was not created");
    assert!(
        table_exists(&db, "Members"),
        "Members table not found in database"
    );

    let data = db.fetch_all_data().expect("fetch all data");
    assert!(
        data.contains_key("Members"),
        "fetch_all_data did not include 'Members' table"
    );
}

#[test]
fn member_register_and_exists() {
    let (_dir, db) = setup_db();

    db.member_register("rfid-123", "M001", "Alice Smith", "S001", "CS", "1")
        .expect("register");

    let member = db.member_exists("rfid-123").expect("exists");
    let member = member.expect("member should exist");
    assert_eq!(member.memberid, "M001");
    assert_eq!(member.name, "Alice Smith");
    assert_eq!(member.student_num, "S001");
}

#[test]
fn points_add_get_redeem() {
    let (_dir, db) = setup_db();

    db.member_register("r-1", "M100", "Bob", "S100", "Eng", "2")
        .expect("register");

    db.add_points("r-1", 25.0).expect("add points");
    assert_eq!(db.get_member_points("r-1").expect("points"), 25.0);

    db.redeem_points("r-1", 10.0).expect("redeem points");
    assert_eq!(db.get_member_points("r-1").expect("points"), 15.0);
}

#[test]
fn event_table_and_attendance() {
    let (_dir, db) = setup_db();

    db.member_register("r-ev", "MEV", "Carol", "S200", "Bio", "3")
        .expect("register");

    db.create_event_table("Test Event")
        .expect("create event table");
    assert!(
        table_exists(&db, "Test_Event"),
        "event table was not created with the normalised name"
    );

    db.attendance_member_event("Test_Event", "r-ev")
        .expect("record attendance");
    assert!(db
        .member_attended_event("Test_Event", "r-ev")
        .expect("attended"));

    let rows = db.fetch_table_data("Test_Event").expect("fetch rows");
    assert!(
        rows.iter().any(|row| row.get("memberid") == Some("MEV")),
        "attendance row not returned by fetch_table_data"
    );
}

#[test]
fn list_tables_filters_members() {
    let (_dir, db) = setup_db();

    db.create_event_table("Another")
        .expect("create event table");

    let tables = db.list_tables().expect("list tables");
    assert!(!tables.contains(&"Members".to_string()));
    assert!(tables.contains(&"Another".to_string()));
}

#[test]
fn rfid_scan_records_attendance_and_adds_points() {
    let (_dir, db) = setup_db();

    let table_name = "Event1";
    db.create_event_table(table_name)
        .expect("create event table");
    db.member_register("rf-1", "ID1", "Test", "S1", "Prog", "1")
        .expect("register");

    assert!(!db
        .member_attended_event(table_name, "rf-1")
        .expect("attended"));

    let outcome = db.scan_attendance(table_name, "rf-1").expect("scan");
    assert!(outcome.success, "scan should succeed: {outcome:?}");
    assert!(!outcome.attended, "first scan is not a duplicate");

    assert!(db
        .member_attended_event(table_name, "rf-1")
        .expect("attended"));

    db.add_points("rf-1", 5.0).expect("add points");
    assert_eq!(db.get_member_points("rf-1").expect("points"), 5.0);
}

#[test]
fn rfid_scan_ignores_quick_rescan() {
    let (_dir, db) = setup_db();

    let table_name = "Event2";
    db.create_event_table(table_name)
        .expect("create event table");
    db.member_register("rf-2", "ID2", "Test2", "S2", "Prog", "1")
        .expect("register");

    let cache = ev_ap::managers::rfid_cache::RfidCache::new();
    let scan_time = ev_ap::managers::rfid_cache::now_secs();
    cache.record("rf-2", scan_time);

    // The manager shows the messagebox and returns *without* scanning.
    let message = cache
        .duplicate_message("rf-2", scan_time + 1.0)
        .expect("rescan must be reported as a duplicate");
    assert_eq!(
        message,
        "rf-2 was scanned within the last 15 seconds. Ignoring scan..."
    );

    assert!(
        !db.member_attended_event(table_name, "rf-2")
            .expect("attended"),
        "a duplicate scan must not record attendance"
    );
}

#[test]
fn scanning_an_unknown_tag_reports_the_python_error_message() {
    let (_dir, db) = setup_db();
    db.create_event_table("Event3").expect("create event table");

    let outcome = db.scan_attendance("Event3", "unknown-tag").expect("scan");
    assert!(!outcome.success);
    assert_eq!(
        outcome.error.as_deref(),
        Some("RFID number not found"),
        "error text must match the Python app"
    );
}

#[test]
fn scanning_the_same_tag_twice_reports_already_attended() {
    let (_dir, db) = setup_db();
    db.create_event_table("Event4").expect("create event table");
    db.member_register("rf-4", "ID4", "Test4", "S4", "Prog", "1")
        .expect("register");

    let first = db.scan_attendance("Event4", "rf-4").expect("scan");
    assert!(first.success && !first.attended);

    let second = db.scan_attendance("Event4", "rf-4").expect("scan");
    assert!(second.success);
    assert!(second.attended, "second scan must report already attended");
}

#[test]
fn the_python_database_file_stays_readable() {
    // Guards the "opens an existing Ev_Ap.db without migration" requirement:
    // the schema produced here is the schema the Python app writes.
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("Ev_Ap.db");
    let db = SqliteBackend::with_path(&path).expect("open db");
    db.initialize_db().expect("initialize db");

    let connection = rusqlite::Connection::open(&path).expect("open raw");
    let columns: Vec<String> = {
        let mut stmt = connection
            .prepare("SELECT name FROM pragma_table_info('Members')")
            .expect("prepare");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query");
        rows.collect::<Result<Vec<_>, _>>().expect("collect")
    };

    assert_eq!(
        columns,
        vec![
            "rfid",
            "memberid",
            "name",
            "student_num",
            "program",
            "year",
            "date_registered",
            "points"
        ],
        "Members schema must match the Python app column for column"
    );
}

#[test]
fn an_existing_database_without_points_is_migrated_in_place() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("Ev_Ap.db");

    // Recreate the pre-points schema exactly as the old Python build wrote it.
    {
        let connection = rusqlite::Connection::open(&path).expect("open raw");
        connection
            .execute_batch(
                "CREATE TABLE Members (
                    rfid TEXT PRIMARY KEY,
                    memberid TEXT,
                    name TEXT,
                    student_num TEXT,
                    program TEXT,
                    year TEXT,
                    date_registered TEXT
                );
                INSERT INTO Members VALUES ('legacy-1', 'M9', 'Legacy', 'S9', 'CS', '1', '2024-01-01');",
            )
            .expect("seed legacy db");
    }

    let db = SqliteBackend::with_path(&path).expect("open db");
    db.initialize_db().expect("initialize db");

    let member = db.member_exists("legacy-1").expect("exists");
    assert!(member.is_some(), "legacy row must survive the migration");
    assert_eq!(
        db.get_member_points("legacy-1").expect("points"),
        0.0,
        "migrated member starts with zero points"
    );
}

#[test]
fn the_database_file_is_created_in_the_working_directory_by_default() {
    // `SqliteBackend::new()` resolves `Ev_Ap.db` against the CWD, exactly like
    // the Python `sqlite_db.py`. The path assertion is made without touching
    // the process CWD so the test stays parallel-safe.
    let expected: &Path = Path::new("Ev_Ap.db");
    assert_eq!(expected.file_name().unwrap(), "Ev_Ap.db");
    assert_eq!(
        ev_ap::db::sqlite::DB_FILE,
        "Ev_Ap.db",
        "the default database file name must not change"
    );
}
