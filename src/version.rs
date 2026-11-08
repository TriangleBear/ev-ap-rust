//! The version rules of the release workflow.
//!
//! Ported from `scripts/ci_build.py` of the Python app, so the Rust repo
//! bumps its version exactly like the Python one:
//!
//! | The merge came from | The version is bumped |
//! |---|---|
//! | `major/*` | major (minor and patch reset) |
//! | `feature/*` or `feat/*` | minor (patch reset) |
//! | anything else | patch |
//!
//! The rules live in Rust — and not in the workflow — so they can be tested.

/// The marker between the version and the release notes in the output of the
/// `version` example.
pub const NOTES_MARKER: &str = "---RELEASE NOTES---";

/// The version that is used when the repository has no tag yet.
pub const FIRST_VERSION: &str = "v0.0.0";

/// The branch a merge came from.
///
/// The merge message of a GitHub merge commit looks like
/// `Merge pull request #12 from TriangleBear/feature/foo`, so the branch is
/// the last part of the `from` clause.
///
/// Falls back to `dev/unknown`, which bumps the patch — the same fallback the
/// Python app uses.
pub fn parse_source_branch(merge_message: &str) -> String {
    merge_message
        .split_once("from ")
        .and_then(|(_, rest)| rest.split_whitespace().next())
        // `owner/branch` -> `branch`
        .and_then(|token| token.split_once('/').map(|(_, branch)| branch.to_string()))
        .unwrap_or_else(|| "dev/unknown".to_string())
}

/// Bump the version of a tag for a branch.
///
/// The tag may carry a `v` prefix, with or without one.
pub fn bump_version(tag: &str, branch: &str) -> String {
    let tag = tag.trim().trim_start_matches('v');
    let mut parts = tag.split('.').map(|part| part.parse::<u32>().unwrap_or(0));

    let mut major = parts.next().unwrap_or(0);
    let mut minor = parts.next().unwrap_or(0);
    let mut patch = parts.next().unwrap_or(0);

    if branch.starts_with("major/") {
        major += 1;
        minor = 0;
        patch = 0;
    } else if branch.starts_with("feature/") || branch.starts_with("feat/") {
        minor += 1;
        patch = 0;
    } else {
        patch += 1;
    }

    format!("v{major}.{minor}.{patch}")
}

/// The version and the release notes, as the `version` example prints them.
pub fn describe(tag: &str, merge_message: &str, notes: &str) -> String {
    let branch = parse_source_branch(merge_message);
    let version = bump_version(tag, &branch);
    format!("{version}\n{NOTES_MARKER}\n{notes}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_major_branch_bumps_the_major_version() {
        assert_eq!(bump_version("v1.2.3", "major/something"), "v2.0.0");
    }

    #[test]
    fn a_feature_branch_bumps_the_minor_version() {
        assert_eq!(bump_version("v1.2.3", "feature/something"), "v1.3.0");
        assert_eq!(bump_version("v1.2.3", "feat/something"), "v1.3.0");
    }

    #[test]
    fn every_other_branch_bumps_the_patch_version() {
        assert_eq!(bump_version("v1.2.3", "dev/something"), "v1.2.4");
        assert_eq!(bump_version("v1.2.3", "fix/something"), "v1.2.4");
        assert_eq!(bump_version("v1.2.3", "patch/something"), "v1.2.4");
        assert_eq!(bump_version("v1.2.3", "dev/unknown"), "v1.2.4");
    }

    #[test]
    fn the_tag_may_carry_a_v_prefix() {
        assert_eq!(bump_version("1.2.3", "dev/x"), "v1.2.4");
        assert_eq!(bump_version("v1.2.3", "dev/x"), "v1.2.4");
    }

    #[test]
    fn the_branch_is_taken_from_the_merge_message() {
        assert_eq!(
            parse_source_branch("Merge pull request #12 from TriangleBear/feature/foo"),
            "feature/foo"
        );
        assert_eq!(
            parse_source_branch("Merge pull request #12 from TriangleBear/major/foo"),
            "major/foo"
        );
        assert_eq!(
            parse_source_branch("Merge pull request #12 from TriangleBear/dev/foo"),
            "dev/foo"
        );
    }

    #[test]
    fn a_merge_message_without_a_branch_falls_back_to_dev_unknown() {
        assert_eq!(parse_source_branch("chore: something"), "dev/unknown");
        assert_eq!(
            bump_version("v1.2.3", &parse_source_branch("chore: x")),
            "v1.2.4"
        );
    }

    #[test]
    fn the_output_has_the_version_and_the_notes() {
        let output = describe("v1.2.3", "Merge from owner/feature/foo", "- a change");
        let (version, notes) = output.split_once(NOTES_MARKER).expect("the marker");
        assert_eq!(version.trim(), "v1.3.0");
        assert_eq!(notes.trim(), "- a change");
    }
}
