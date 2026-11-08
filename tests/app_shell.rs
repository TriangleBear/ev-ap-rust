//! Tests for the application shell.
//!
//! The shell is built without a window: `eframe::App::update` needs a real
//! context, but everything the shell owns — the navigation, the lazy view
//! creation, the background initialisation and the dialog flow — can be
//! exercised headlessly with an `egui::Context`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ev_ap::app::{AppShell, InitStage};
use ev_ap::config::DbMode;
use ev_ap::db::{Backend, Database};
use ev_ap::ui::message_box::MessageBoxResponse;
use ev_ap::views::View;
use tempfile::TempDir;

/// A shell on top of a database in a temporary directory.
fn setup() -> (TempDir, AppShell) {
    let dir = TempDir::new().expect("temp dir");
    let db = Arc::new(
        Database::with_sqlite_path(DbMode::Sqlite, "", &dir.path().join("Ev_Ap.db"))
            .expect("router"),
    );
    (dir, AppShell::new(db))
}

/// Wait until the background initialisation has finished.
fn wait_for_initialization(shell: &mut AppShell) {
    let ctx = egui::Context::default();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !shell.is_initialized() && Instant::now() < deadline {
        shell.poll_initialization(&ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    shell.poll_initialization(&ctx);
}

// ── Navigation ──────────────────────────────────────────────────────────

#[test]
fn the_sidebar_has_the_seven_python_navigation_buttons() {
    let labels: Vec<&str> = View::all().iter().map(|view| view.label()).collect();
    assert_eq!(
        labels,
        vec!["Home", "Events", "Members", "Reports", "Settings", "Help", "About"],
        "the sidebar must match the Python navigation"
    );
}

#[test]
fn the_shell_starts_on_the_home_view() {
    let (_dir, shell) = setup();
    assert_eq!(shell.current_view(), View::Home);
}

#[test]
fn navigating_switches_the_view() {
    let (_dir, mut shell) = setup();
    for view in View::all() {
        shell.navigate(view);
        assert_eq!(shell.current_view(), view);
    }
}

// ── Lazy view initialisation ────────────────────────────────────────────

#[test]
fn a_view_is_created_only_when_it_is_opened() {
    let mut views = ev_ap::views::ViewSet::new();
    assert_eq!(
        views.initialized_count(),
        0,
        "no view may exist before the first navigation"
    );

    views.get_mut(View::Home);
    assert_eq!(views.initialized_count(), 1);

    views.get_mut(View::Home);
    assert_eq!(
        views.initialized_count(),
        1,
        "opening the same view again must not create a second one"
    );

    views.get_mut(View::Settings);
    assert_eq!(views.initialized_count(), 2);
}

#[test]
fn every_view_can_be_created() {
    let mut views = ev_ap::views::ViewSet::new();
    for view in View::all() {
        views.get_mut(view);
    }
    assert_eq!(views.initialized_count(), 7);
}

// ── Background initialisation ───────────────────────────────────────────

#[test]
fn the_database_is_initialised_in_the_background() {
    let (_dir, mut shell) = setup();

    assert!(
        !shell.is_initialized(),
        "the initialisation must not block the constructor"
    );

    wait_for_initialization(&mut shell);

    assert!(shell.is_initialized());
    assert_eq!(*shell.init_stage(), InitStage::Done);
    assert!(shell.is_ready());
}

#[test]
fn the_initialisation_creates_the_members_table() {
    let (dir, mut shell) = setup();
    wait_for_initialization(&mut shell);

    let path = dir.path().join("Ev_Ap.db");
    assert!(path.exists(), "the database file must have been created");

    let db = ev_ap::db::sqlite::SqliteBackend::with_path(&path).expect("open");
    assert!(db.table_exists("Members").expect("table exists"));
}

#[test]
fn the_init_stage_reports_progress() {
    assert_eq!(InitStage::Connecting.status(), "Connecting to database...");
    assert_eq!(
        InitStage::CreatingTables.status(),
        "Creating database tables..."
    );
    assert_eq!(InitStage::Done.status(), "Ready");
    assert!(InitStage::Connecting.progress() < InitStage::CreatingTables.progress());
    assert!(InitStage::CreatingTables.progress() < InitStage::Done.progress());
    assert_eq!(InitStage::Done.progress(), 1.0);
}

#[test]
fn a_failed_initialisation_is_reported() {
    let mut stage = InitStage::Failed("disk on fire".to_string());
    assert_eq!(stage.status(), "Error: disk on fire");
    assert_eq!(stage.progress(), 1.0);
    stage = InitStage::Done;
    assert_eq!(stage.status(), "Ready");
}

// ── Status label ────────────────────────────────────────────────────────

#[test]
fn the_status_label_names_the_active_backend() {
    let (_dir, shell) = setup();
    assert_eq!(shell.status_text(), "Connected to SQLite");
}

// ── Message box flow ────────────────────────────────────────────────────

#[test]
fn a_message_box_is_opened_and_answered() {
    let (_dir, mut shell) = setup();

    assert!(shell.dialog().is_none());
    shell.show_message("RFID Scan", "RFID recorded");
    let dialog = shell.dialog().expect("the dialog is open");
    assert_eq!(dialog.title, "RFID Scan");
    assert_eq!(dialog.message, "RFID recorded");

    // The answer arrives after the button was pressed.
    shell.dialog_mut().expect("dialog").response = Some(MessageBoxResponse::Ok);
    shell.collect_dialog_response();

    assert!(shell.dialog().is_none(), "the dialog must close");
    assert_eq!(shell.take_dialog_response(), Some(MessageBoxResponse::Ok));
    assert_eq!(
        shell.take_dialog_response(),
        None,
        "the answer is taken once"
    );
}

#[test]
fn a_confirmation_dialog_has_a_yes_and_a_no_button() {
    let (_dir, mut shell) = setup();

    shell.ask_confirmation("Create Event", "Are you sure?", "Yes", "No");
    let dialog = shell.dialog().expect("the dialog is open");
    assert_eq!(dialog.buttons.len(), 2);
    assert_eq!(dialog.buttons[0].1, MessageBoxResponse::Yes);
    assert_eq!(dialog.buttons[1].1, MessageBoxResponse::No);

    shell.dialog_mut().expect("dialog").response = Some(MessageBoxResponse::No);
    shell.collect_dialog_response();
    assert_eq!(shell.take_dialog_response(), Some(MessageBoxResponse::No));
}

// ── Preloaded data ──────────────────────────────────────────────────────

#[test]
fn the_preloaded_data_is_reloaded_on_demand() {
    let (_dir, mut shell) = setup();
    wait_for_initialization(&mut shell);

    shell.db.create_event_table("Event1").expect("create");
    shell.reload_preloaded_data();

    assert!(
        shell.tables_list.contains(&"Event1".to_string()),
        "the event list must be reloaded"
    );
}

// ── Theme ───────────────────────────────────────────────────────────────

#[test]
fn the_theme_is_applied_to_the_egui_context() {
    use ev_ap::theme::ThemePreference;

    let ctx = egui::Context::default();

    ThemePreference::Dark.apply(&ctx);
    assert_eq!(
        ctx.style().visuals.text_color(),
        egui::Visuals::dark().text_color(),
        "the dark theme must be applied"
    );

    ThemePreference::Light.apply(&ctx);
    assert_eq!(
        ctx.style().visuals.text_color(),
        egui::Visuals::light().text_color(),
        "the light theme must be applied"
    );
}

#[test]
fn the_theme_colours_follow_the_python_theme_manager() {
    use ev_ap::theme::ThemePreference;

    let ctx = egui::Context::default();

    assert_eq!(
        ThemePreference::Light.text_color(&ctx),
        egui::Color32::BLACK,
        "light mode uses black text"
    );
    assert_eq!(
        ThemePreference::Dark.text_color(&ctx),
        egui::Color32::WHITE,
        "dark mode uses white text"
    );
    assert_eq!(
        ThemePreference::Light.sub_text_color(&ctx),
        egui::Color32::from_gray(51),
        "light mode uses gray20 for the subdued text"
    );
    assert_eq!(
        ThemePreference::Dark.sub_text_color(&ctx),
        egui::Color32::from_gray(204),
        "dark mode uses gray80 for the subdued text"
    );
}
