//! Tests for `ev_ap_config.json` handling.
//!
//! The Python app stores the config in the working directory, so these tests
//! use an explicit path instead of the process CWD to stay parallel-safe.

use std::fs;

use ev_ap::config::{Config, DbMode};
use tempfile::TempDir;

#[test]
fn a_missing_config_is_created_with_the_python_defaults() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ev_ap_config.json");

    let config = Config::load_from(&path);

    assert_eq!(config.db_mode, DbMode::Sqlite);
    assert_eq!(config.gsheet_api_url, "");
    assert_eq!(config.gsheet_api_key, "");
    assert!(path.exists(), "the config file should have been created");
}

#[test]
fn the_written_file_is_byte_compatible_with_the_python_app() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ev_ap_config.json");

    let config = Config::default();
    config.save_to(&path).expect("save");

    let raw = fs::read_to_string(&path).expect("read");
    assert_eq!(
        raw,
        "{\n  \"db_mode\": \"sqlite\",\n  \"gsheet_api_url\": \"\",\n  \"gsheet_api_key\": \"\"\n}"
    );
}

#[test]
fn the_mode_survives_a_save_load_round_trip() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ev_ap_config.json");

    let mut config = Config::default();
    config.set_db_mode(DbMode::Gsheet);
    config.set_gsheet_api_url("https://script.google.com/macros/s/ABC/exec");
    config.save_to(&path).expect("save");

    let reloaded = Config::load_from(&path);
    assert_eq!(reloaded.db_mode, DbMode::Gsheet);
    assert_eq!(
        reloaded.gsheet_api_url,
        "https://script.google.com/macros/s/ABC/exec"
    );
}

#[test]
fn unknown_keys_are_preserved() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ev_ap_config.json");

    fs::write(
        &path,
        "{\n  \"db_mode\": \"sqlite\",\n  \"gsheet_api_url\": \"\",\n  \"gsheet_api_key\": \"\",\n  \"future_option\": 42\n}",
    )
    .expect("write");

    let mut config = Config::load_from(&path);
    assert_eq!(
        config.extra.get("future_option").and_then(|v| v.as_i64()),
        Some(42)
    );

    config.set_db_mode(DbMode::Gsheet);
    config.save_to(&path).expect("save");

    let raw = fs::read_to_string(&path).expect("read");
    assert!(
        raw.contains("future_option"),
        "unknown keys must not be dropped on save"
    );
}

#[test]
fn a_corrupt_config_falls_back_to_the_defaults() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ev_ap_config.json");

    fs::write(&path, "this is not json").expect("write");

    let config = Config::load_from(&path);
    assert_eq!(config.db_mode, DbMode::Sqlite);
    assert_eq!(config.gsheet_api_url, "");
}

#[test]
fn a_config_missing_keys_is_still_readable() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ev_ap_config.json");

    fs::write(&path, "{}").expect("write");

    let config = Config::load_from(&path);
    assert_eq!(config.db_mode, DbMode::Sqlite);
    assert_eq!(config.gsheet_api_url, "");
    assert_eq!(config.gsheet_api_key, "");
}
