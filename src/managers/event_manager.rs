//! Event creation and deletion.
//!
//! Ported from `App/managers/event_manager.py`. The Python manager opens
//! dialogs; here the logic is separated from the dialogs so it can be tested.

use crate::db::{DbError, SharedBackend};

use super::messages::{
    Message, CREATE_EVENT, EVENT_CREATED, EVENT_CREATION, EVENT_CREATION_ERROR, NO_EVENT_NAME,
};

/// Manages the lifecycle of events.
pub struct EventManager {
    db: SharedBackend,
}

impl EventManager {
    /// A manager on top of a backend.
    pub fn new(db: SharedBackend) -> Self {
        EventManager { db }
    }

    /// The names of the existing events.
    pub fn list_events(&self) -> Result<Vec<String>, DbError> {
        self.db.list_tables()
    }

    /// Create the table for a new event.
    ///
    /// Returns the stored name of the event, which is the normalised form
    /// (`"Test Event"` → `"Test_Event"`) — the name the dropdown shows.
    pub fn create_event(&self, event_name: &str) -> Result<String, Message> {
        let trimmed = event_name.trim();
        if trimmed.is_empty() {
            return Err(Message::new(CREATE_EVENT, NO_EVENT_NAME));
        }

        match self.db.create_event_table(trimmed) {
            Ok(()) => Ok(
                crate::db::normalize_table_name(trimmed).unwrap_or_else(|_| trimmed.to_string())
            ),
            Err(e) => {
                tracing::error!("failed to create event {trimmed}: {e}");
                Err(Message::new(
                    EVENT_CREATION_ERROR,
                    format!("Error creating event: {e}"),
                ))
            }
        }
    }

    /// Delete an event and its attendance table.
    ///
    /// For the Sheets backend this moves the event's spreadsheet to the trash
    /// and removes the registry entry; for SQLite it drops the table.
    pub fn delete_event(&self, event_name: &str) -> Result<(), Message> {
        self.db.delete_event_table(event_name).map_err(|e| {
            tracing::error!("failed to delete event {event_name}: {e}");
            Message::new(EVENT_CREATION_ERROR, format!("Error deleting event: {e}"))
        })
    }

    /// The message shown after a successful creation.
    pub fn created_message() -> Message {
        Message::new(EVENT_CREATION, EVENT_CREATED)
    }
}
