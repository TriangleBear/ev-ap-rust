//! Application shell: sidebar navigation, view routing, the background
//! database initialisation and the theme state.
//!
//! Ported from `App/rfid_app.py` (`MainApp`). The Python shell builds a
//! sidebar with seven navigation buttons, shows a loading screen while the
//! database is initialised in a background thread, and initialises each view
//! lazily the first time it is opened. All of that is reproduced here.

use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::config::DbMode;
use crate::db::{Backend, Database, Row};
use crate::managers::{EventManager, MemberManager, TableManager};
use crate::theme::ThemePreference;
use crate::ui::message_box::{MessageBox, MessageBoxResponse};
use crate::views::{View, ViewContext, ViewSet};

/// The width of the sidebar, in points.
pub const SIDEBAR_WIDTH: f32 = 200.0;

/// The title of the application window.
pub const WINDOW_TITLE: &str = "Events Attendance System";

/// The stage the background initialisation is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitStage {
    /// The connection is being opened.
    Connecting,
    /// The tables are being created.
    CreatingTables,
    /// Everything is ready.
    Done,
    /// The initialisation failed.
    Failed(String),
}

impl InitStage {
    /// The text shown under the progress bar.
    pub fn status(&self) -> String {
        match self {
            InitStage::Connecting => "Connecting to database...".to_string(),
            InitStage::CreatingTables => "Creating database tables...".to_string(),
            InitStage::Done => "Ready".to_string(),
            InitStage::Failed(error) => format!("Error: {error}"),
        }
    }

    /// The progress, between 0 and 1.
    pub fn progress(&self) -> f32 {
        match self {
            InitStage::Connecting => 0.1,
            InitStage::CreatingTables => 0.5,
            InitStage::Done => 1.0,
            InitStage::Failed(_) => 1.0,
        }
    }
}

/// Runs the database initialisation on a background thread.
///
/// The Python app starts a `threading.Thread` for the initialisation and
/// polls it from the UI thread; the Rust port does the same with a channel.
pub struct DatabaseInitializer {
    receiver: Receiver<InitStage>,
    handle: Option<JoinHandle<()>>,
    stage: InitStage,
}

impl DatabaseInitializer {
    /// Start the initialisation of the given database.
    pub fn start(db: Arc<Database>) -> Self {
        let (sender, receiver) = mpsc::channel();

        let handle = std::thread::spawn(move || {
            let _ = sender.send(InitStage::Connecting);

            // Let the first frame paint before the work starts.
            std::thread::sleep(Duration::from_millis(50));

            let _ = sender.send(InitStage::CreatingTables);
            if let Err(error) = db.initialize_db() {
                let _ = sender.send(InitStage::Failed(error.to_string()));
                return;
            }

            let _ = sender.send(InitStage::Done);
        });

        DatabaseInitializer {
            receiver,
            handle: Some(handle),
            stage: InitStage::Connecting,
        }
    }

    /// Drain the channel and apply the newest stage.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        while let Ok(stage) = self.receiver.try_recv() {
            self.stage = stage;
            changed = true;
        }

        if changed || !self.is_done() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// Is the initialisation finished — successfully or not?
    pub fn is_done(&self) -> bool {
        matches!(self.stage, InitStage::Done | InitStage::Failed(_))
    }

    /// The current stage.
    pub fn stage(&self) -> &InitStage {
        &self.stage
    }

    /// The error of a failed initialisation.
    pub fn error(&self) -> Option<&str> {
        match &self.stage {
            InitStage::Failed(error) => Some(error),
            _ => None,
        }
    }
}

impl Drop for DatabaseInitializer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The application shell.
pub struct AppShell {
    /// The database router.
    pub db: Arc<Database>,
    /// The member manager.
    pub members: MemberManager,
    /// The event manager.
    pub events: EventManager,
    /// The table manager.
    pub tables: TableManager,

    /// The appearance mode.
    pub theme: ThemePreference,
    /// The rows of the `Members` table, preloaded at start-up.
    pub preloaded_members: Vec<Row>,
    /// The names of the event tables, preloaded at start-up.
    pub tables_list: Vec<String>,

    views: ViewSet,
    current: View,
    dialog: Option<MessageBox>,
    pending_response: Option<MessageBoxResponse>,
    initializer: DatabaseInitializer,
}

impl AppShell {
    /// Build the shell around a database.
    pub fn new(db: Arc<Database>) -> Self {
        let backend: crate::db::SharedBackend = db.clone();
        let initializer = DatabaseInitializer::start(db.clone());

        AppShell {
            db,
            members: MemberManager::new(backend.clone()),
            events: EventManager::new(backend.clone()),
            tables: TableManager::new(backend),
            theme: ThemePreference::default(),
            preloaded_members: Vec::new(),
            tables_list: Vec::new(),
            views: ViewSet::new(),
            current: View::Home,
            dialog: None,
            pending_response: None,
            initializer,
        }
    }

    /// The view that is currently open.
    pub fn current_view(&self) -> View {
        self.current
    }

    /// Open another view.
    pub fn navigate(&mut self, view: View) {
        self.current = view;
    }

    /// The open message box, if there is one.
    pub fn dialog(&self) -> Option<&MessageBox> {
        self.dialog.as_ref()
    }

    /// The open message box, for tests and for views that need to answer it.
    pub fn dialog_mut(&mut self) -> Option<&mut MessageBox> {
        self.dialog.as_mut()
    }

    /// Drain the background initialisation.
    ///
    /// The shell calls this on every frame; it is public so the progress can
    /// be awaited in a test.
    pub fn poll_initialization(&mut self, ctx: &egui::Context) {
        self.initializer.poll(ctx);
    }

    /// Is the background initialisation finished?
    pub fn is_initialized(&self) -> bool {
        self.initializer.is_done()
    }

    /// The stage of the background initialisation.
    pub fn init_stage(&self) -> &InitStage {
        self.initializer.stage()
    }

    /// Show a message box.
    pub fn show_message(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.dialog = Some(MessageBox::info(title, message));
    }

    /// Show a message box with a Yes and a No button.
    pub fn ask_confirmation(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        yes_label: &str,
        no_label: &str,
    ) {
        self.dialog = Some(MessageBox::confirm(title, message, yes_label, no_label));
    }

    /// Is the database ready?
    pub fn is_ready(&self) -> bool {
        self.initializer.is_done() && self.initializer.error().is_none()
    }

    /// The text of the database status label in the sidebar.
    pub fn status_text(&self) -> &'static str {
        match self.db.mode() {
            DbMode::Sqlite => "Connected to SQLite",
            DbMode::Gsheet => "Connected to Google Sheets",
        }
    }

    /// Reload the preloaded data.
    ///
    /// The Python app preloads the members and the event list at start-up and
    /// refreshes the dropdown whenever an event is created.
    pub fn reload_preloaded_data(&mut self) {
        if let Ok(rows) = self.db.fetch_table_data("Members") {
            self.preloaded_members = rows;
        }
        if let Ok(tables) = self.db.list_tables() {
            self.tables_list = tables;
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.heading("Ev_Ap");
        });
        ui.separator();

        for view in [View::Home, View::Events, View::Members, View::Reports] {
            self.nav_button(ui, view);
        }

        ui.separator();

        for view in [View::Settings, View::Help, View::About] {
            self.nav_button(ui, view);
        }

        // The database status label.
        //
        // The Python app pins it to the bottom of the sidebar with
        // `pack(side="bottom")`. egui does not offer a "bottom of the panel"
        // layout that survives the panel's own layout, so the label sits
        // directly under the navigation here.
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(self.status_text())
                .color(self.theme.sub_text_color(ctx))
                .strong(),
        );
    }

    fn nav_button(&mut self, ui: &mut egui::Ui, view: View) {
        let selected = self.current == view;
        let response = ui.selectable_label(selected, view.label());
        if response.clicked() {
            self.current = view;
        }
    }

    fn loading_screen(&mut self, ui: &mut egui::Ui) {
        ui.centered_and_justified(|ui| {
            ui.add_space(80.0);
            ui.heading("Initializing Application...");
            ui.add_space(8.0);
            ui.label(self.initializer.stage().status());
            ui.add_space(16.0);
            let progress = self.initializer.stage().progress();
            ui.add(egui::ProgressBar::new(progress).desired_width(400.0));
        });
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        if !self.initializer.is_done() {
            self.loading_screen(ui);
            return;
        }

        if let Some(error) = self.initializer.error() {
            let error = error.to_string();
            ui.centered_and_justified(|ui| {
                ui.add_space(80.0);
                ui.heading("Database initialization failed");
                ui.label(error);
            });
            return;
        }

        let AppShell {
            db,
            members,
            events,
            tables,
            theme,
            views,
            current,
            dialog,
            pending_response,
            ..
        } = self;

        let mut context = ViewContext {
            db: db.as_ref(),
            members,
            events,
            tables,
            theme: *theme,
            dialog,
            response: pending_response.take(),
        };

        views.get_mut(*current).ui(ui, &mut context);

        *theme = context.theme;
    }

    fn render_dialog(&mut self, ctx: &egui::Context) {
        crate::ui::message_box::show(ctx, &mut self.dialog);
    }

    /// Move the answer of a closed message box into the pending response.
    pub fn collect_dialog_response(&mut self) {
        let response = self.dialog.as_ref().and_then(|dialog| dialog.response);
        if let Some(response) = response {
            self.dialog = None;
            self.pending_response = Some(response);
        }
    }

    /// The answer to the message box that was closed, if there was one.
    pub fn take_dialog_response(&mut self) -> Option<MessageBoxResponse> {
        self.pending_response.take()
    }
}

impl eframe::App for AppShell {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.theme.apply(ctx);
        self.initializer.poll(ctx);

        egui::SidePanel::left("sidebar")
            .exact_width(SIDEBAR_WIDTH)
            .resizable(false)
            .show(ctx, |ui| self.sidebar(ui, ctx));

        egui::CentralPanel::default().show(ctx, |ui| self.content(ui));

        self.render_dialog(ctx);
        self.collect_dialog_response();
    }
}
