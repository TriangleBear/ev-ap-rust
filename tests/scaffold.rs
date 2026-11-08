use ev_ap::{APP_NAME, VERSION};

#[test]
fn version_is_injected_at_build_time() {
    // The release workflow passes the version in `EV_AP_VERSION`; a local
    // build falls back to the version of `Cargo.toml`.
    assert_eq!(VERSION, env!("EV_AP_VERSION"));
    assert!(!VERSION.is_empty());
}

#[test]
fn binary_name_is_stable_for_release_continuity() {
    assert_eq!(APP_NAME, "Attendance_App");
}

#[test]
fn crate_exposes_the_planned_module_tree() {
    // Compile-time proof that every planned module is wired into the crate.
    #[allow(unused_imports)]
    use ev_ap::{app, config, db, managers, theme, views};
}
