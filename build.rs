//! Injects the version into the crate.
//!
//! The release workflow computes the version from the branch name and passes
//! it in the `EV_AP_VERSION` environment variable, so **no source file has to
//! be edited** to change the version — the Python app had to rewrite a string
//! inside `about_view.py`.
//!
//! Without the variable the version of `Cargo.toml` is used, so a local
//! `cargo build` behaves as expected.

fn main() {
    let version =
        std::env::var("EV_AP_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());

    println!("cargo:rustc-env=EV_AP_VERSION={version}");
    println!("cargo:rerun-if-env-changed=EV_AP_VERSION");
}
