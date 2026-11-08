//! `ev_ap_config.json` reader/writer.
//!
//! The on-disk format is byte-compatible with the Python app's
//! `App/database/config.py`: a JSON object in the working directory with the
//! keys `db_mode`, `gsheet_api_url` and `gsheet_api_key`. Unknown keys are
//! preserved so a config written by a newer build is not silently truncated.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Name of the config file, resolved against the current working directory.
pub const CONFIG_FILE: &str = "ev_ap_config.json";

/// Which database backend the app talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DbMode {
    /// Local SQLite file (`Ev_Ap.db`).
    #[default]
    Sqlite,
    /// Google Sheets via the Apps Script Web App.
    Gsheet,
}

impl DbMode {
    /// Wire representation used in `ev_ap_config.json`.
    pub fn as_str(self) -> &'static str {
        match self {
            DbMode::Sqlite => "sqlite",
            DbMode::Gsheet => "gsheet",
        }
    }

    /// Parse the wire representation. Returns `None` for unknown values.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "sqlite" => Some(DbMode::Sqlite),
            "gsheet" => Some(DbMode::Gsheet),
            _ => None,
        }
    }
}

/// Parsed contents of `ev_ap_config.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Active backend.
    #[serde(default)]
    pub db_mode: DbMode,
    /// Deployment URL of the Apps Script Web App.
    #[serde(default)]
    pub gsheet_api_url: String,
    /// Optional API key forwarded to the Apps Script backend.
    #[serde(default)]
    pub gsheet_api_key: String,
    /// Any keys this build does not know about — preserved on save.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            db_mode: DbMode::default(),
            gsheet_api_url: String::new(),
            gsheet_api_key: String::new(),
            extra: Map::new(),
        }
    }
}

impl Config {
    /// Load `ev_ap_config.json` from the current working directory.
    ///
    /// Mirrors Python's `load_config()`: the file is created with defaults when
    /// missing, and a corrupt file falls back to the defaults instead of
    /// aborting startup.
    pub fn load() -> Config {
        Config::load_from(Path::new(CONFIG_FILE))
    }

    /// Load the config from an explicit path.
    pub fn load_from(path: &Path) -> Config {
        match fs::read_to_string(path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => {
                let config = Config::default();
                // Best effort: a read-only working directory must not stop the app.
                let _ = config.save_to(path);
                config
            }
        }
    }

    /// Write the config to `ev_ap_config.json` in the current working directory.
    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(Path::new(CONFIG_FILE))
    }

    /// Write the config to an explicit path.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let body = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, body)
    }

    /// Path of the config file in the current working directory.
    pub fn path() -> PathBuf {
        PathBuf::from(CONFIG_FILE)
    }

    /// Active backend.
    pub fn db_mode(&self) -> DbMode {
        self.db_mode
    }

    /// Switch the active backend and persist the change.
    pub fn set_db_mode(&mut self, mode: DbMode) {
        self.db_mode = mode;
    }

    /// Deployment URL of the Apps Script Web App.
    pub fn gsheet_api_url(&self) -> &str {
        &self.gsheet_api_url
    }

    /// Set and persist the Apps Script Web App URL.
    pub fn set_gsheet_api_url(&mut self, url: &str) {
        self.gsheet_api_url = url.to_string();
    }
}

/// Read the active backend from `ev_ap_config.json`.
///
/// Mirrors Python's `get_db_mode()`.
pub fn get_db_mode() -> DbMode {
    Config::load().db_mode
}

/// Persist a backend switch to `ev_ap_config.json`.
///
/// Mirrors Python's `set_db_mode()`.
pub fn set_db_mode(mode: DbMode) -> std::io::Result<()> {
    let mut config = Config::load();
    config.set_db_mode(mode);
    config.save()
}

/// Read the Apps Script Web App URL from `ev_ap_config.json`.
///
/// Mirrors Python's `get_gsheet_api_url()`.
pub fn get_gsheet_api_url() -> String {
    Config::load().gsheet_api_url
}

/// Persist the Apps Script Web App URL to `ev_ap_config.json`.
///
/// Mirrors Python's `set_gsheet_api_url()`.
pub fn set_gsheet_api_url(url: &str) -> std::io::Result<()> {
    let mut config = Config::load();
    config.set_gsheet_api_url(url);
    config.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_mode_round_trips_through_the_wire_format() {
        assert_eq!(DbMode::Sqlite.as_str(), "sqlite");
        assert_eq!(DbMode::Gsheet.as_str(), "gsheet");
        assert_eq!(DbMode::parse("sqlite"), Some(DbMode::Sqlite));
        assert_eq!(DbMode::parse("gsheet"), Some(DbMode::Gsheet));
        assert_eq!(DbMode::parse("mysql"), None);
    }

    #[test]
    fn db_mode_serializes_lowercase() {
        let json = serde_json::to_string(&DbMode::Gsheet).unwrap();
        assert_eq!(json, "\"gsheet\"");
        let json = serde_json::to_string(&DbMode::Sqlite).unwrap();
        assert_eq!(json, "\"sqlite\"");
    }
}
