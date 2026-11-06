//! Tests for the Google Sheets (Apps Script) backend.
//!
//! The Apps Script cannot run in CI, so a local HTTP server impersonates it
//! (see `tests/support/mod.rs`). Every request the client makes is asserted,
//! and every error path of the Python client is reproduced.

mod support;

use std::collections::HashMap;

use ev_ap::config::DbMode;
use ev_ap::db::sheets::{SheetBackend, NOT_DEPLOYED_MESSAGE, NOT_FOUND_MESSAGE, REDIRECT_MESSAGE};
use ev_ap::db::{Backend, Database};
use serde_json::json;
use support::{CannedResponse, FakeAppsScript};

fn backend(server: &FakeAppsScript) -> SheetBackend {
    SheetBackend::new(&server.url).expect("build backend")
}

/// Build a server with the given routes.
fn server_with(routes: Vec<(&str, CannedResponse)>) -> FakeAppsScript {
    let map: HashMap<String, CannedResponse> = routes
        .into_iter()
        .map(|(action, response)| (action.to_string(), response))
        .collect();
    FakeAppsScript::start(map)
}

// ── Request shape ───────────────────────────────────────────────────────

#[test]
fn member_register_posts_addmember_with_the_python_payload() {
    let server = server_with(vec![(
        "addMember",
        CannedResponse::json(200, json!({"success": true}).to_string()),
    )]);
    let db = backend(&server);

    db.member_register("rfid-1", "M001", "Alice", "S001", "CS", "1")
        .expect("register");

    let request = server.last_request().expect("a request was sent");
    assert_eq!(request.method, "POST");
    assert_eq!(request.action(), "addMember");
    let body = request.json();
    assert_eq!(body["rfid"], "rfid-1");
    assert_eq!(body["memberid"], "M001");
    assert_eq!(body["name"], "Alice");
    assert_eq!(body["student_num"], "S001");
    assert_eq!(body["program"], "CS");
    assert_eq!(body["year"], "1");
    assert!(
        body["date_registered"].as_str().unwrap().len() == 10,
        "date_registered must be a yyyy-mm-dd string"
    );
}

#[test]
fn member_exists_posts_getmember_and_maps_the_member() {
    let server = server_with(vec![(
        "getMember",
        CannedResponse::json(
            200,
            json!({
                "success": true,
                "data": {
                    "rfid": "rfid-1",
                    "memberid": "M001",
                    "name": "Alice",
                    "student_num": "S001",
                    "program": "CS",
                    "year": "1",
                    "date_registered": "2026-01-01"
                }
            })
            .to_string(),
        ),
    )]);
    let db = backend(&server);

    let member = db.member_exists("rfid-1").expect("lookup").expect("found");
    assert_eq!(member.memberid, "M001");
    assert_eq!(member.name, "Alice");
    assert_eq!(member.student_num, "S001");

    let request = server.last_request().expect("a request was sent");
    assert_eq!(request.action(), "getMember");
    assert_eq!(request.json()["rfid"], "rfid-1");
}

#[test]
fn member_exists_returns_none_for_an_unknown_tag() {
    let server = server_with(vec![(
        "getMember",
        CannedResponse::json(200, json!({"success": true, "data": null}).to_string()),
    )]);
    let db = backend(&server);

    assert!(db.member_exists("nope").expect("lookup").is_none());
}

#[test]
fn list_tables_gets_listevents() {
    let server = server_with(vec![(
        "listEvents",
        CannedResponse::json(
            200,
            json!({"success": true, "data": ["Event1", "Event2"]}).to_string(),
        ),
    )]);
    let db = backend(&server);

    assert_eq!(
        db.list_tables().expect("list"),
        vec!["Event1".to_string(), "Event2".to_string()]
    );
    assert_eq!(server.last_request().unwrap().action(), "listEvents");
    assert_eq!(server.last_request().unwrap().method, "GET");
}

#[test]
fn create_event_table_normalises_the_event_name() {
    let server = server_with(vec![(
        "createEvent",
        CannedResponse::json(200, json!({"success": true}).to_string()),
    )]);
    let db = backend(&server);

    db.create_event_table("Test Event").expect("create");

    let request = server.last_request().expect("a request was sent");
    assert_eq!(request.action(), "createEvent");
    assert_eq!(
        request.json()["eventName"],
        "Test_Event",
        "the Apps Script replaces spaces with underscores itself — the client must agree"
    );
}

#[test]
fn fetch_table_data_reads_members_and_attendees() {
    let server = server_with(vec![
        (
            "listMembers",
            CannedResponse::json(
                200,
                json!({
                    "success": true,
                    "data": [
                        {
                            "rfid": "rfid-1",
                            "memberid": "M001",
                            "name": "Alice",
                            "student_num": "S001",
                            "program": "CS",
                            "year": "1",
                            "date_registered": "2026-01-01"
                        }
                    ]
                })
                .to_string(),
            ),
        ),
        (
            "getAttendees",
            CannedResponse::json(
                200,
                json!({
                    "success": true,
                    "data": [
                        {
                            "rfid": "rfid-1",
                            "memberid": "M001",
                            "student_num": "S001",
                            "name": "Alice",
                            "attendance_time": "2026-01-01 10:00:00"
                        }
                    ]
                })
                .to_string(),
            ),
        ),
    ]);
    let db = backend(&server);

    let members = db.fetch_table_data("Members").expect("members");
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].get("memberid"), Some("M001"));
    assert_eq!(
        members[0].columns(),
        vec!["rfid", "memberid", "name", "student_num", "date_registered"],
        "the Members columns must match the SQLite backend"
    );

    let attendees = db.fetch_table_data("Event1").expect("attendees");
    assert_eq!(attendees.len(), 1);
    assert_eq!(
        attendees[0].get("attendance_time"),
        Some("2026-01-01 10:00:00")
    );
    assert_eq!(
        attendees[0].columns(),
        vec!["memberid", "name", "student_num", "attendance_time"]
    );
}

#[test]
fn scan_attendance_is_a_single_batched_call() {
    let server = server_with(vec![(
        "scanAttendance",
        CannedResponse::json(
            200,
            json!({"success": true, "attended": false, "attendanceTime": "2026-01-01 10:00:00"})
                .to_string(),
        ),
    )]);
    let db = backend(&server);

    let outcome = db.scan_attendance("Event1", "rfid-1").expect("scan");

    assert!(outcome.success);
    assert!(!outcome.attended);
    assert_eq!(server.request_count(), 1, "the scan must be one round trip");
    let request = server.last_request().unwrap();
    assert_eq!(request.action(), "scanAttendance");
    assert_eq!(request.json()["eventName"], "Event1");
    assert_eq!(request.json()["rfid"], "rfid-1");
}

#[test]
fn scan_attendance_reports_an_unknown_tag_with_the_python_message() {
    let server = server_with(vec![(
        "scanAttendance",
        CannedResponse::json(
            200,
            json!({"success": false, "error": "RFID number not found"}).to_string(),
        ),
    )]);
    let db = backend(&server);

    let outcome = db.scan_attendance("Event1", "unknown").expect("scan");
    assert!(!outcome.success);
    assert_eq!(
        outcome.error.as_deref(),
        Some("RFID number not found"),
        "the server message must reach the user untouched"
    );
}

#[test]
fn scan_attendance_reports_already_attended() {
    let server = server_with(vec![(
        "scanAttendance",
        CannedResponse::json(200, json!({"success": true, "attended": true}).to_string()),
    )]);
    let db = backend(&server);

    let outcome = db.scan_attendance("Event1", "rfid-1").expect("scan");
    assert!(outcome.success);
    assert!(outcome.attended);
}

#[test]
fn member_attended_event_uses_checkattended() {
    let server = server_with(vec![(
        "checkAttended",
        CannedResponse::json(200, json!({"success": true, "attended": true}).to_string()),
    )]);
    let db = backend(&server);

    assert!(db
        .member_attended_event("Event1", "rfid-1")
        .expect("attended"));
    assert_eq!(server.last_request().unwrap().action(), "checkAttended");
}

#[test]
fn delete_event_table_uses_deleteevent() {
    let server = server_with(vec![(
        "deleteEvent",
        CannedResponse::json(200, json!({"success": true}).to_string()),
    )]);
    let db = backend(&server);

    db.delete_event_table("Event1").expect("delete");
    assert_eq!(server.last_request().unwrap().action(), "deleteEvent");
    assert_eq!(server.last_request().unwrap().json()["eventName"], "Event1");
}

#[test]
fn points_calls_the_addpoints_and_redeempoints_actions() {
    // The Apps Script has no `addPoints`/`redeemPoints` handler yet, so the
    // script answers "Unknown action: addPoints". The client must send the
    // action and surface that message unchanged, so the backend works as soon
    // as the handler is added to the script.
    let server = FakeAppsScript::always(CannedResponse::json(
        200,
        json!({"success": false, "error": "Unknown action: addPoints"}).to_string(),
    ));
    let db = backend(&server);

    let error = db.add_points("rfid-1", 5.0).expect_err("must fail");
    assert_eq!(error.to_string(), "Unknown action: addPoints");
    assert_eq!(server.last_request().unwrap().action(), "addPoints");

    let error = db.redeem_points("rfid-1", 5.0).expect_err("must fail");
    assert_eq!(error.to_string(), "Unknown action: addPoints");
}

#[test]
fn get_member_points_reads_the_points_field_of_the_member() {
    let server = server_with(vec![(
        "getMember",
        CannedResponse::json(
            200,
            json!({"success": true, "data": {"memberid": "M001", "name": "Alice", "points": 12.5}})
                .to_string(),
        ),
    )]);
    let db = backend(&server);

    assert_eq!(db.get_member_points("rfid-1").expect("points"), 12.5);
}

#[test]
fn get_member_points_is_zero_when_the_sheet_has_no_points_column() {
    let server = server_with(vec![(
        "getMember",
        CannedResponse::json(
            200,
            json!({"success": true, "data": {"memberid": "M001", "name": "Alice"}}).to_string(),
        ),
    )]);
    let db = backend(&server);

    assert_eq!(db.get_member_points("rfid-1").expect("points"), 0.0);
}

// ── Configuration flows ─────────────────────────────────────────────────

#[test]
fn get_config_reads_the_spreadsheet_configuration() {
    let server = server_with(vec![(
        "config",
        CannedResponse::json(
            200,
            json!({
                "success": true,
                "data": {
                    "spreadsheetId": "sheet-1",
                    "configured": true,
                    "spreadsheetName": "Ev_Ap Main",
                    "spreadsheetUrl": "https://docs.google.com/spreadsheets/d/sheet-1"
                }
            })
            .to_string(),
        ),
    )]);
    let db = backend(&server);

    let config = db.get_config().expect("config").expect("some config");
    assert_eq!(config.spreadsheet_id.as_deref(), Some("sheet-1"));
    assert!(config.configured);
    assert_eq!(config.spreadsheet_name.as_deref(), Some("Ev_Ap Main"));
    assert_eq!(server.last_request().unwrap().action(), "config");
}

#[test]
fn set_spreadsheet_sends_the_url_parameter() {
    let server = server_with(vec![(
        "setConfig",
        CannedResponse::json(200, json!({"success": true}).to_string()),
    )]);
    let db = backend(&server);

    assert!(db
        .set_spreadsheet("https://docs.google.com/spreadsheets/d/abc/edit")
        .expect("set"));

    let request = server.last_request().unwrap();
    assert_eq!(request.action(), "setConfig");
    assert_eq!(
        request.param("url").as_deref(),
        Some("https://docs.google.com/spreadsheets/d/abc/edit")
    );
}

#[test]
fn get_event_details_returns_the_registry_entry() {
    let server = server_with(vec![(
        "getEventDetails",
        CannedResponse::json(
            200,
            json!({
                "success": true,
                "data": {
                    "eventName": "Event1",
                    "spreadsheetId": "sheet-2",
                    "spreadsheetUrl": "https://docs.google.com/spreadsheets/d/sheet-2",
                    "createdDate": "2026-01-01 00:00:00",
                    "status": "active"
                }
            })
            .to_string(),
        ),
    )]);
    let db = backend(&server);

    let details = db
        .get_event_details("Event1")
        .expect("details")
        .expect("found");
    assert_eq!(details.event_name, "Event1");
    assert_eq!(details.spreadsheet_id, "sheet-2");
    assert_eq!(details.status, "active");
}

// ── Error paths ─────────────────────────────────────────────────────────

#[test]
fn a_missing_deployment_url_is_reported_exactly() {
    let db = SheetBackend::new("").expect("build backend");
    let error = db.list_tables().expect_err("must fail");
    assert_eq!(error.to_string(), "Google Sheets API URL not configured");
}

#[test]
fn the_error_message_constants_match_the_python_wording() {
    assert_eq!(
        NOT_FOUND_MESSAGE,
        "URL not found. Check the deployment URL."
    );
    assert_eq!(
        REDIRECT_MESSAGE,
        "The URL redirects. Use the final deployment URL (ends with /exec)."
    );
    assert_eq!(
        NOT_DEPLOYED_MESSAGE,
        "The script is not deployed as a Web App or is not publicly accessible. Go to Extensions → Apps Script → Deploy → New Deployment → Web App (Execute as: Me, Access: Anyone)."
    );
}

#[test]
fn a_404_is_reported_with_the_requests_style_text() {
    let server = FakeAppsScript::always(CannedResponse::json(404, "{}"));
    let db = backend(&server);

    let error = db.list_tables().expect_err("must fail");
    let message = error.to_string();
    assert!(
        message.starts_with("Request failed (HTTP 404): 404 Client Error: Not Found for url:"),
        "unexpected message: {message}"
    );
}

#[test]
fn a_500_is_reported_with_the_requests_style_text() {
    let server = FakeAppsScript::always(CannedResponse::json(500, "{}"));
    let db = backend(&server);

    let error = db.list_tables().expect_err("must fail");
    let message = error.to_string();
    assert!(
        message.starts_with(
            "Request failed (HTTP 500): 500 Server Error: Internal Server Error for url:"
        ),
        "unexpected message: {message}"
    );
}

#[test]
fn a_non_json_response_shows_the_preview() {
    let server = FakeAppsScript::always(CannedResponse::text(
        200,
        "text/html",
        "<html><body>hello</body></html>",
    ));
    let db = backend(&server);

    let error = db.list_tables().expect_err("must fail");
    assert_eq!(
        error.to_string(),
        "API returned non-JSON (HTTP 200). Response preview: <html><body>hello</body></html>"
    );
}

#[test]
fn a_non_json_sign_in_page_is_reported_as_not_deployed() {
    let server = FakeAppsScript::always(CannedResponse::text(
        200,
        "text/html",
        "<html>accounts.google.com/signin</html>",
    ));
    let db = backend(&server);

    let error = db.list_tables().expect_err("must fail");
    assert_eq!(
        error.to_string(),
        format!("API returned non-JSON (HTTP 200). {NOT_DEPLOYED_MESSAGE}")
    );
}

#[test]
fn a_non_json_404_is_reported_as_a_request_failure() {
    // The Python client calls `raise_for_status()` *before* parsing the body,
    // so a 404 never reaches the "API returned non-JSON" branch — it is
    // reported as a request failure. The port keeps that ordering.
    let server = FakeAppsScript::always(CannedResponse::text(404, "text/html", "<html/>"));
    let db = backend(&server);

    let error = db.list_tables().expect_err("must fail");
    assert!(
        error
            .to_string()
            .starts_with("Request failed (HTTP 404): 404 Client Error: Not Found for url:"),
        "unexpected message: {error}"
    );
}

#[test]
fn a_non_json_302_is_reported_as_a_redirect() {
    let server = FakeAppsScript::always(CannedResponse::text(302, "text/html", "<html/>"));
    let db = backend(&server);

    let error = db.list_tables().expect_err("must fail");
    assert_eq!(
        error.to_string(),
        format!("API returned non-JSON (HTTP 302). {REDIRECT_MESSAGE}")
    );
}

#[test]
fn a_transport_failure_is_reported_without_a_status() {
    // Nothing is listening on this port.
    let db = SheetBackend::new("http://127.0.0.1:1/exec").expect("build backend");

    let error = db.list_tables().expect_err("must fail");
    assert!(
        error.to_string().starts_with("Request failed (HTTP N/A): "),
        "unexpected message: {error}"
    );
}

// ── Router ──────────────────────────────────────────────────────────────

#[test]
fn the_router_sends_calls_to_the_sheets_backend_in_gsheet_mode() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let server = server_with(vec![(
        "getMember",
        CannedResponse::json(
            200,
            json!({"success": true, "data": {"memberid": "M002", "name": "Bob", "student_num": "S2"}})
                .to_string(),
        ),
    )]);

    let db = Database::with_sqlite_path(DbMode::Gsheet, &server.url, &dir.path().join("Ev_Ap.db"))
        .expect("router");
    assert_eq!(db.mode(), DbMode::Gsheet);

    let member = db.member_exists("rfid-2").expect("lookup").expect("found");
    assert_eq!(member.memberid, "M002");
    assert_eq!(server.last_request().unwrap().action(), "getMember");
}

#[test]
fn the_router_sends_calls_to_sqlite_in_sqlite_mode() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let path = dir.path().join("Ev_Ap.db");

    let db = Database::with_sqlite_path(DbMode::Sqlite, "", &path).expect("router");
    db.initialize_db().expect("init");
    db.member_register("rfid-3", "M003", "Carol", "S3", "CS", "2")
        .expect("register");

    let member = db.member_exists("rfid-3").expect("lookup").expect("found");
    assert_eq!(member.memberid, "M003");
}

#[test]
fn switching_the_router_changes_the_target_backend() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let path = dir.path().join("Ev_Ap.db");
    let server = server_with(vec![(
        "listEvents",
        CannedResponse::json(
            200,
            json!({"success": true, "data": ["FromSheets"]}).to_string(),
        ),
    )]);

    let db = Database::with_sqlite_path(DbMode::Sqlite, &server.url, &path).expect("router");
    db.initialize_db().expect("init");
    db.create_event_table("FromSqlite").expect("create");
    assert!(db
        .list_tables()
        .expect("list")
        .contains(&"FromSqlite".to_string()));

    db.switch_mode(DbMode::Gsheet);
    assert_eq!(db.mode(), DbMode::Gsheet);
    assert!(db
        .list_tables()
        .expect("list")
        .contains(&"FromSheets".to_string()));

    db.switch_mode(DbMode::Sqlite);
    assert!(db
        .list_tables()
        .expect("list")
        .contains(&"FromSqlite".to_string()));
}
