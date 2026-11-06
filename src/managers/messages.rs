//! User-facing message texts.
//!
//! Every string here is copied verbatim from the Python app's `CTkMessagebox`
//! calls, so the Rust build shows the user exactly the same words as the
//! Python build. Keeping them in one module means a wording change is a
//! single edit and a single test.

/// A message ready to be shown in a dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// The dialog title.
    pub title: &'static str,
    /// The dialog body.
    pub text: String,
}

impl Message {
    /// Build a message.
    pub fn new(title: &'static str, text: impl Into<String>) -> Self {
        Message {
            title,
            text: text.into(),
        }
    }
}

// ── Member registration ─────────────────────────────────────────────────

/// Dialog title for member registration.
pub const MEMBER_REGISTRATION: &str = "Member Registration";
/// Shown when the RFID tag is already registered.
pub const MEMBER_ALREADY_EXISTS: &str = "Member already exists!";
/// Shown after a successful registration.
pub const MEMBER_REGISTERED: &str = "Member Registered!";
/// Shown when the member could not be written to the database.
pub const MEMBER_REGISTER_FAILED: &str =
    "Failed to register member. Check the database connection.";

// ── Event creation ──────────────────────────────────────────────────────

/// Dialog title for the event creation prompt.
pub const CREATE_EVENT: &str = "Create Event";
/// The confirmation prompt shown before creating an event.
pub const CREATE_EVENT_CONFIRM: &str = "Are you sure you want to create a new event?";
/// Shown when the event name field is empty.
pub const NO_EVENT_NAME: &str = "No event name provided.";
/// Dialog title for the event creation result.
pub const EVENT_CREATION: &str = "Event Creation";
/// Shown after an event was created.
pub const EVENT_CREATED: &str = "Event Created!";
/// Dialog title for an event creation failure.
pub const EVENT_CREATION_ERROR: &str = "Event Creation Error";

// ── RFID scan ───────────────────────────────────────────────────────────

/// Dialog title for a scan result.
pub const RFID_SCAN: &str = "RFID Scan";
/// Dialog title for a failed scan.
pub const RFID_SCAN_ERROR: &str = "RFID Scan Error";
/// Shown after an attendance was recorded.
pub const RFID_RECORDED: &str = "RFID recorded";
/// Shown when the member already attended this event.
pub const ALREADY_ATTENDED: &str = "Member has already attended this event.";

// ── Points ──────────────────────────────────────────────────────────────

/// Dialog title for the points redemption.
pub const POINTS_REDEMPTION: &str = "Redeem Points";
/// Shown when the member does not have enough points.
pub const NOT_ENOUGH_POINTS: &str = "Not enough points to redeem.";
/// Shown after points were redeemed.
pub const POINTS_REDEEMED: &str = "Points redeemed successfully!";

// ── Settings ────────────────────────────────────────────────────────────

/// Dialog title for the settings.
pub const SETTINGS: &str = "Settings";
/// Shown after the Sheets API URL was saved.
pub const GSHEET_URL_SAVED: &str = "Google App Script Web App URL saved.";
/// Shown when the URL field was empty.
pub const INVALID_URL: &str = "Please enter a valid URL.";
