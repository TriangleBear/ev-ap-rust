//! Tests for the views and the shared user interface components.
//!
//! The views draw into an `egui::Ui`, which needs a real context. The parts
//! that carry the behaviour — the form modal, the attendance window, the
//! column definitions and the message texts — are tested directly.

use std::sync::Arc;

use ev_ap::db::sqlite::SqliteBackend;
use ev_ap::db::Backend;
use ev_ap::managers::TableManager;
use ev_ap::ui::attendance_window::AttendanceWindow;
use ev_ap::ui::form::{FormField, FormModal, FormOutcome};
use ev_ap::views::about::CREATOR_URL;
use ev_ap::views::events::EventsView;
use ev_ap::views::help::HELP_TEXT;
use ev_ap::views::home::HomeView;
use ev_ap::views::members::{MembersView, MEMBER_COLUMNS};
use ev_ap::views::reports::ReportsView;
use ev_ap::views::settings::SettingsView;
use ev_ap::views::View;
use tempfile::TempDir;

/// An initialised backend in a temporary directory.
fn setup() -> (TempDir, Arc<SqliteBackend>) {
    let dir = TempDir::new().expect("temp dir");
    let backend = SqliteBackend::with_path(dir.path().join("Ev_Ap.db")).expect("open db");
    backend.initialize_db().expect("initialize db");
    (dir, Arc::new(backend))
}

// ── Column definitions ──────────────────────────────────────────────────

#[test]
fn the_member_columns_match_the_python_view() {
    assert_eq!(
        MEMBER_COLUMNS,
        &["rfid", "memberid", "name", "student_num"],
        "the members view shows the RFID, the member id, the name and the student number"
    );
}

#[test]
fn the_attendance_columns_match_the_python_view() {
    assert_eq!(
        ev_ap::ui::attendance_window::ATTENDANCE_COLUMNS,
        &["memberid", "name", "student_num", "attendance_time"]
    );
}

// ── The help text ───────────────────────────────────────────────────────

#[test]
fn the_help_text_is_the_python_text() {
    assert!(HELP_TEXT.starts_with("Welcome to the Events Attendance App!"));
    assert!(HELP_TEXT.contains("- Register new members"));
    assert!(HELP_TEXT.contains("- Create new events"));
    assert!(HELP_TEXT.contains("- Record attendance using RFID"));
    assert!(
        HELP_TEXT.contains("- Redeem points for members DISABLED"),
        "the disabled note must stay"
    );
    assert!(HELP_TEXT.contains("- Export event data to CSV or Excel"));
    assert!(HELP_TEXT.contains("For any assistance, please contact support."));
}

// ── The about view ──────────────────────────────────────────────────────

#[test]
fn the_about_view_links_to_the_author() {
    assert_eq!(CREATOR_URL, "https://github.com/TriangleBear");
}

// ── The form modal ──────────────────────────────────────────────────────

#[test]
fn a_form_starts_with_empty_text_fields() {
    let form = FormModal::new(
        "Register New Member",
        vec![
            FormField::text("Member ID", "Enter member ID"),
            FormField::text("Name", "Enter name"),
        ],
    );

    assert_eq!(form.values(), vec!["", ""]);
    assert_eq!(form.value(0), "");
}

#[test]
fn the_form_values_are_read_in_order() {
    let mut form = FormModal::new(
        "Register New Member",
        vec![
            FormField::text("Member ID", "Enter member ID"),
            FormField::text("Name", "Enter name"),
            FormField::text("RFID", "Scan RFID"),
        ],
    );

    if let FormField::Text { value, .. } = &mut form.fields[0] {
        *value = "M001".to_string();
    }
    if let FormField::Text { value, .. } = &mut form.fields[1] {
        *value = "Alice Smith".to_string();
    }
    if let FormField::Text { value, .. } = &mut form.fields[2] {
        *value = "rfid-1".to_string();
    }

    assert_eq!(form.values(), vec!["M001", "Alice Smith", "rfid-1"]);
    assert_eq!(form.value(1), "Alice Smith");
}

#[test]
fn a_choice_field_starts_with_the_first_option() {
    let form = FormModal::new(
        "Select Event",
        vec![FormField::choice(
            "Select an event for attendance report:",
            vec!["Event1".to_string(), "Event2".to_string()],
        )],
    );

    assert_eq!(form.value(0), "Event1");
}

#[test]
fn a_form_that_is_not_interacted_with_stays_idle() {
    let ctx = egui::Context::default();
    let mut form = FormModal::new("Test", vec![FormField::text("Name", "Enter name")]);
    let mut outcome = FormOutcome::Idle;

    // egui only allows drawing inside `Context::run`.
    ctx.run(egui::RawInput::default(), |ctx| {
        outcome = form.show(ctx);
    });

    assert_eq!(outcome, FormOutcome::Idle);
}

// ── The attendance window ───────────────────────────────────────────────

#[test]
fn the_attendance_window_reads_the_rows_of_the_event() {
    let (_dir, backend) = setup();
    backend.create_event_table("Event1").expect("create");
    backend
        .member_register("rf-1", "M001", "Alice", "S001", "CS", "1")
        .expect("register");
    backend
        .attendance_member_event("Event1", "rf-1")
        .expect("attendance");

    let manager = TableManager::new(backend.clone());
    let mut window = AttendanceWindow::new("Event1");
    window.refresh(&manager);

    assert_eq!(window.rows.len(), 1);
    assert_eq!(window.rows[0].get("name"), Some("Alice"));
}

#[test]
fn the_attendance_window_filters_the_rows() {
    let (_dir, backend) = setup();
    backend.create_event_table("Event1").expect("create");
    backend
        .member_register("rf-1", "M001", "Alice", "S001", "CS", "1")
        .expect("register");
    backend
        .member_register("rf-2", "M002", "Bob", "S002", "CS", "1")
        .expect("register");
    backend
        .attendance_member_event("Event1", "rf-1")
        .expect("attendance");
    backend
        .attendance_member_event("Event1", "rf-2")
        .expect("attendance");

    let manager = TableManager::new(backend.clone());
    let mut window = AttendanceWindow::new("Event1");
    window.refresh(&manager);

    assert_eq!(window.visible_rows().len(), 2);

    window.search = "alice".to_string();
    let filtered = window.visible_rows();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].get("name"), Some("Alice"));

    window.search = "nobody".to_string();
    assert!(window.visible_rows().is_empty());
}

#[test]
fn the_attendance_window_exports_the_visible_rows() {
    let (_dir, backend) = setup();
    backend.create_event_table("Event1").expect("create");
    backend
        .member_register("rf-1", "M001", "Alice", "S001", "CS", "1")
        .expect("register");
    backend
        .attendance_member_event("Event1", "rf-1")
        .expect("attendance");

    let manager = TableManager::new(backend.clone());
    let mut window = AttendanceWindow::new("Event1");
    window.refresh(&manager);

    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("attendance.csv");
    ev_ap::managers::export::write(&window.visible_rows(), &path).expect("export");

    let content = std::fs::read_to_string(&path).expect("read");
    assert!(content.starts_with("memberid,name,student_num,attendance_time\n"));
    assert!(content.contains("M001,Alice"));
}

// ── The views exist and can be created ──────────────────────────────────

#[test]
fn every_view_can_be_created() {
    let _home = HomeView::new();
    let _events = EventsView::new();
    let _members = MembersView::new();
    let _reports = ReportsView::new();
    let _settings = SettingsView::new();
}

#[test]
fn the_view_labels_are_the_python_navigation() {
    let labels: Vec<&str> = View::all().iter().map(|view| view.label()).collect();
    assert_eq!(
        labels,
        vec!["Home", "Events", "Members", "Reports", "Settings", "Help", "About"]
    );
}

// ── The messages ────────────────────────────────────────────────────────

#[test]
fn the_message_texts_are_the_python_texts() {
    use ev_ap::managers::messages;

    assert_eq!(messages::MEMBER_ALREADY_EXISTS, "Member already exists!");
    assert_eq!(messages::MEMBER_REGISTERED, "Member Registered!");
    assert_eq!(
        messages::MEMBER_REGISTER_FAILED,
        "Failed to register member. Check the database connection."
    );
    assert_eq!(messages::NO_EVENT_NAME, "No event name provided.");
    assert_eq!(messages::EVENT_CREATED, "Event Created!");
    assert_eq!(messages::RFID_RECORDED, "RFID recorded");
    assert_eq!(
        messages::ALREADY_ATTENDED,
        "Member has already attended this event."
    );
    assert_eq!(
        messages::CREATE_EVENT_CONFIRM,
        "Are you sure you want to create a new event?"
    );
}
