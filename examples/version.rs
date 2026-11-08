//! Print the version and the release notes for the release workflow.
//!
//! The counterpart of `scripts/ci_build.py` in the Python app. The workflow
//! calls it and reads the version from the first line and the notes from
//! behind the marker.
//!
//! ```text
//! cargo run --quiet --example version -- "Merge pull request #1 from owner/feature/foo"
//! ```
//!
//! Without an argument the current merge message is used.

use ev_ap::version::{bump_version, parse_source_branch, NOTES_MARKER};

fn main() {
    let merge_message = std::env::args().nth(1).unwrap_or_else(|| {
        std::process::Command::new("git")
            .args(["log", "-1", "--format=%s"])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_string()
    });

    let tag = std::process::Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .unwrap_or_else(|| ev_ap::version::FIRST_VERSION.to_string());

    let branch = parse_source_branch(&merge_message);
    let version = bump_version(&tag, &branch);

    println!("{version}");
    println!("{NOTES_MARKER}");
    println!("{}", release_notes());
}

/// The commits since the previous tag.
fn release_notes() -> String {
    let previous = std::process::Command::new("git")
        .args(["describe", "--tags", "--abbrev=0", "HEAD~1"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty());

    let Some(previous) = previous else {
        return "Initial release.".to_string();
    };

    let notes = std::process::Command::new("git")
        .args([
            "log",
            &format!("{previous}..HEAD"),
            "--pretty=format:- %s (%h)",
            "--no-merges",
        ])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string();

    if notes.is_empty() {
        "No changes since last tag.".to_string()
    } else {
        notes
    }
}
