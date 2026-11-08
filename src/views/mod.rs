//! The seven application views: home, events, members, reports, settings,
//! help and about.
//!
//! Ported from `App/views/`. The Python app imports a view the first time it
//! is opened (`MainApp.init_view`) and keeps it alive afterwards; the
//! [`ViewSet`] reproduces that lazy initialisation, so opening the window
//! costs nothing but the home view.

pub mod about;
pub mod events;
pub mod help;
pub mod home;
pub mod members;
pub mod reports;
pub mod settings;

use crate::db::Database;
use crate::managers::{EventManager, MemberManager, TableManager};
use crate::theme::ThemePreference;
use crate::ui::attendance_window::AttendanceWindow;
use crate::ui::message_box::{MessageBox, MessageBoxResponse};

use self::about::AboutView;
use self::events::EventsView;
use self::help::HelpView;
use self::home::HomeView;
use self::members::MembersView;
use self::reports::ReportsView;
use self::settings::SettingsView;

/// The pages of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum View {
    Home,
    Events,
    Members,
    Reports,
    Settings,
    Help,
    About,
}

impl View {
    /// The label of the navigation button.
    pub fn label(self) -> &'static str {
        match self {
            View::Home => "Home",
            View::Events => "Events",
            View::Members => "Members",
            View::Reports => "Reports",
            View::Settings => "Settings",
            View::Help => "Help",
            View::About => "About",
        }
    }

    /// Every view, in the order of the sidebar.
    pub fn all() -> [View; 7] {
        [
            View::Home,
            View::Events,
            View::Members,
            View::Reports,
            View::Settings,
            View::Help,
            View::About,
        ]
    }
}

/// What a view may reach while it is drawn.
pub struct ViewContext<'a> {
    /// The database router.
    pub db: &'a Database,
    /// The member manager.
    pub members: &'a MemberManager,
    /// The event manager.
    pub events: &'a EventManager,
    /// The table manager.
    pub tables: &'a TableManager,
    /// The appearance mode. A view may change it.
    pub theme: ThemePreference,
    /// The open message box. A view may put one here.
    pub dialog: &'a mut Option<MessageBox>,
    /// The answer to the message box that was closed in the previous frame.
    pub response: Option<MessageBoxResponse>,
    /// The attendance window. A view may open one.
    pub attendance: &'a mut Option<AttendanceWindow>,
    /// The view the user asked to go to, if any.
    pub request_view: Option<View>,
    /// The preloaded data has to be reloaded.
    pub refresh: bool,
}

impl ViewContext<'_> {
    /// Show a message box with a single "OK" button.
    pub fn show_message(&mut self, title: impl Into<String>, message: impl Into<String>) {
        *self.dialog = Some(MessageBox::info(title, message));
    }

    /// Show a message box with a "Yes" and a "No" button.
    pub fn ask_confirmation(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        yes_label: &str,
        no_label: &str,
    ) {
        *self.dialog = Some(MessageBox::confirm(title, message, yes_label, no_label));
    }

    /// Ask for the preloaded data to be reloaded.
    pub fn request_refresh(&mut self) {
        self.refresh = true;
    }
}

/// A page of the application.
pub trait AppView {
    /// Draw the page.
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut ViewContext<'_>);
}

/// Holds the views and creates them the first time they are opened.
pub struct ViewSet {
    home: Option<HomeView>,
    events: Option<EventsView>,
    members: Option<MembersView>,
    reports: Option<ReportsView>,
    settings: Option<SettingsView>,
    help: Option<HelpView>,
    about: Option<AboutView>,
}

impl Default for ViewSet {
    fn default() -> Self {
        ViewSet::new()
    }
}

impl ViewSet {
    /// An empty set — no view has been created yet.
    pub fn new() -> Self {
        ViewSet {
            home: None,
            events: None,
            members: None,
            reports: None,
            settings: None,
            help: None,
            about: None,
        }
    }

    /// The view, created on first use.
    pub fn get_mut(&mut self, view: View) -> &mut dyn AppView {
        match view {
            View::Home => self.home.get_or_insert_with(HomeView::new),
            View::Events => self.events.get_or_insert_with(EventsView::new),
            View::Members => self.members.get_or_insert_with(MembersView::new),
            View::Reports => self.reports.get_or_insert_with(ReportsView::new),
            View::Settings => self.settings.get_or_insert_with(SettingsView::new),
            View::Help => self.help.get_or_insert_with(HelpView::new),
            View::About => self.about.get_or_insert_with(AboutView::new),
        }
    }

    /// How many views have been created so far.
    pub fn initialized_count(&self) -> usize {
        [
            self.home.is_some(),
            self.events.is_some(),
            self.members.is_some(),
            self.reports.is_some(),
            self.settings.is_some(),
            self.help.is_some(),
            self.about.is_some(),
        ]
        .iter()
        .filter(|initialized| **initialized)
        .count()
    }
}
