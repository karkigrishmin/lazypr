#![allow(dead_code)]

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;

use super::store::store_path;
use crate::core::{ReviewRound, ReviewSession};

/// Replace `/` with `__` so branch names are safe for use as filenames.
pub fn sanitize_branch_name(branch: &str) -> String {
    branch.replace('/', "__")
}

/// Return the path to the session JSON file for a given branch.
pub fn session_path(repo_root: &Path, branch: &str) -> PathBuf {
    store_path(repo_root)
        .join("reviews")
        .join(format!("{}.json", sanitize_branch_name(branch)))
}

/// Load a review session for a branch. Returns `None` when no session file exists.
pub fn load_session(repo_root: &Path, branch: &str) -> Result<Option<ReviewSession>> {
    let path = session_path(repo_root, branch);
    if !path.exists() {
        return Ok(None);
    }
    let contents =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let session: ReviewSession =
        serde_json::from_str(&contents).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(session))
}

/// Save a review session to its branch-specific JSON file, creating parent dirs if needed.
pub fn save_session(repo_root: &Path, session: &ReviewSession) -> Result<()> {
    let path = session_path(repo_root, &session.branch);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(session).context("serialising session to JSON")?;
    std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Push a new `ReviewRound` with an incremented version, current timestamp, and empty state.
pub fn start_new_round(session: &mut ReviewSession, sha: &str) {
    let version = session.reviews.last().map_or(1, |r| r.version + 1);
    session.reviews.push(ReviewRound {
        version,
        sha: sha.to_owned(),
        timestamp: Utc::now(),
        files_viewed: vec![],
        notes_count: 0,
    });
}

/// Add a file path to the latest round's `files_viewed` if not already present.
pub fn mark_file_viewed(session: &mut ReviewSession, file_path: &str) {
    if let Some(round) = session.reviews.last_mut() {
        if !round.files_viewed.iter().any(|f| f == file_path) {
            round.files_viewed.push(file_path.to_owned());
        }
    }
}

/// Return a reference to the most recent review round, if any.
pub fn latest_round(session: &ReviewSession) -> Option<&ReviewRound> {
    session.reviews.last()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn sanitize_branch_replaces_slashes() {
        assert_eq!(sanitize_branch_name("feature/foo/bar"), "feature__foo__bar");
        assert_eq!(sanitize_branch_name("main"), "main");
        assert_eq!(sanitize_branch_name("fix/bug-123"), "fix__bug-123");
    }

    #[test]
    fn load_returns_none_when_no_file() {
        let tmp = TempDir::new().unwrap();
        crate::state::store::init_store(tmp.path()).unwrap();
        let result = load_session(tmp.path(), "main").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn save_then_load_round_trip() {
        let tmp = TempDir::new().unwrap();
        crate::state::store::init_store(tmp.path()).unwrap();
        let mut session = ReviewSession {
            branch: "feature/x".into(),
            reviews: vec![],
        };
        start_new_round(&mut session, "abc123");
        save_session(tmp.path(), &session).unwrap();
        let loaded = load_session(tmp.path(), "feature/x").unwrap().unwrap();
        assert_eq!(loaded.branch, "feature/x");
        assert_eq!(loaded.reviews.len(), 1);
        assert_eq!(loaded.reviews[0].sha, "abc123");
    }

    #[test]
    fn start_new_round_increments_version() {
        let mut session = ReviewSession {
            branch: "main".into(),
            reviews: vec![],
        };
        start_new_round(&mut session, "sha1");
        start_new_round(&mut session, "sha2");
        assert_eq!(session.reviews[0].version, 1);
        assert_eq!(session.reviews[1].version, 2);
    }

    #[test]
    fn mark_file_viewed_is_idempotent() {
        let mut session = ReviewSession {
            branch: "main".into(),
            reviews: vec![],
        };
        start_new_round(&mut session, "sha1");
        mark_file_viewed(&mut session, "src/main.rs");
        mark_file_viewed(&mut session, "src/main.rs");
        assert_eq!(latest_round(&session).unwrap().files_viewed.len(), 1);
    }

    #[test]
    fn latest_round_returns_most_recent() {
        let mut session = ReviewSession {
            branch: "main".into(),
            reviews: vec![],
        };
        start_new_round(&mut session, "sha1");
        start_new_round(&mut session, "sha2");
        assert_eq!(latest_round(&session).unwrap().sha, "sha2");
    }
}
