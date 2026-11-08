//! Business-logic managers: event creation, member registration and the
//! event table / RFID scan flow.
//!
//! The Python managers build Tkinter windows directly. In the Rust port the
//! managers hold **only** the logic and the message texts, and the views
//! (Phase 5 and 6) drive them — which is what makes the whole flow testable
//! without a window.

pub mod event_manager;
pub mod export;
pub mod member_manager;
pub mod messages;
pub mod rfid_cache;
pub mod table_manager;

pub use event_manager::EventManager;
pub use member_manager::MemberManager;
pub use messages::Message;
pub use rfid_cache::{RfidCache, DEDUP_WINDOW_SECONDS};
pub use table_manager::{ScanResult, TableManager};
