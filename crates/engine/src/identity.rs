//! The build that produced a result (M6.P6b): version, commit, build date and profile.
//!
//! `kit --version` prints it, every envelope carries it as `build`, and the window's About
//! line can read it from [`build_identity`]. The commit and date come from two variables
//! read at compile time ([`COMMIT_ENV`], [`DATE_ENV`]), which `cargo xtask dist` and
//! `cargo xtask ui` set from git and the clock when they build; AGENTS.md allows no
//! `build.rs`. A build made without them (a plain `cargo build`, the CI gate, a checkout
//! without git) says `unknown` for each, never a guess. The profile is the compiler's
//! (`debug` when debug assertions are on, else `release`).

use serde::Serialize;

/// Compile-time variable naming the commit: a short hash, with `-dirty` when the tree had
/// uncommitted changes.
pub const COMMIT_ENV: &str = "KITKRAFT_BUILD_COMMIT";
/// Compile-time variable naming the build date (UTC, `yyyy-mm-dd`).
pub const DATE_ENV: &str = "KITKRAFT_BUILD_DATE";
/// What a missing value reads as.
pub const UNKNOWN: &str = "unknown";

/// Which build this is. Serializes as `{version, commit, date, profile}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BuildIdentity {
    /// The workspace version (`0.0.0`).
    pub version: &'static str,
    /// Short commit hash, `-dirty` appended for a tree with changes; `unknown` when the
    /// build was not made through xtask.
    pub commit: &'static str,
    /// Build date, UTC `yyyy-mm-dd`; `unknown` as for `commit`.
    pub date: &'static str,
    /// `debug` or `release`.
    pub profile: &'static str,
}

impl BuildIdentity {
    /// `0.0.0 (5d03d01, 2026-10-16, release)`, the text after `kit ` in `kit --version`.
    pub fn line(&self) -> String {
        format!(
            "{} ({}, {}, {})",
            self.version, self.commit, self.date, self.profile
        )
    }

    /// Whether this is a debug build.
    pub fn is_debug(&self) -> bool {
        self.profile == "debug"
    }
}

/// This build's identity.
pub fn build_identity() -> BuildIdentity {
    BuildIdentity {
        version: env!("CARGO_PKG_VERSION"),
        commit: known(option_env!("KITKRAFT_BUILD_COMMIT")),
        date: known(option_env!("KITKRAFT_BUILD_DATE")),
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
    }
}

/// `value` when it is set and not blank, else [`UNKNOWN`].
fn known(value: Option<&'static str>) -> &'static str {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(UNKNOWN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_names_version_commit_date_and_profile() {
        let id = build_identity();
        assert_eq!(id.version, env!("CARGO_PKG_VERSION"));
        assert!(!id.commit.is_empty() && !id.date.is_empty());
        assert_eq!(
            id.profile,
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        );
        assert_eq!(id.is_debug(), cfg!(debug_assertions));
        let line = id.line();
        assert!(line.starts_with(&format!("{} (", id.version)), "{line}");
        assert!(line.ends_with(&format!(", {})", id.profile)), "{line}");
        assert_eq!(known(None), UNKNOWN);
        assert_eq!(known(Some("  ")), UNKNOWN);
        assert_eq!(known(Some("5d03d01-dirty")), "5d03d01-dirty");
        let json = serde_json::to_value(id).unwrap();
        let keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        for k in ["version", "commit", "date", "profile"] {
            assert!(keys.contains(&k), "{k}");
        }
    }
}
