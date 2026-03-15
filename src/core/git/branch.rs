use anyhow::{Context, Result};

use crate::core::errors::GitError;

/// Get the current branch name (e.g. `"main"` or `"feature/foo"`).
///
/// Returns an error if HEAD is detached or cannot be resolved.
pub fn current_branch(repo: &git2::Repository) -> Result<String> {
    let head = repo.head().context("failed to resolve HEAD")?;
    let name = head.shorthand().ok_or_else(|| GitError::BranchNotFound {
        name: "HEAD".into(),
    })?;
    Ok(name.to_string())
}

/// Auto-detect the base branch for diff comparison.
///
/// Tries, in order: local `main`, local `master`, remote `origin/main`,
/// remote `origin/master`. Returns [`GitError::NoBaseBranch`] when none exist.
pub fn detect_base_branch(repo: &git2::Repository) -> Result<String> {
    let candidates = ["main", "master"];

    // Try local branches first
    for name in &candidates {
        if repo.find_branch(name, git2::BranchType::Local).is_ok() {
            return Ok(name.to_string());
        }
    }

    // Try remote branches
    let remote_candidates = ["origin/main", "origin/master"];
    for name in &remote_candidates {
        if repo.find_reference(&format!("refs/remotes/{name}")).is_ok() {
            return Ok(name.to_string());
        }
    }

    Err(GitError::NoBaseBranch.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a temp repo with an initial commit on "main".
    fn init_repo_with_main() -> (tempfile::TempDir, git2::Repository) {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();

        // Initial commit
        let initial_oid = {
            let file_path = dir.path().join("init.txt");
            fs::write(&file_path, "init").unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("init.txt")).unwrap();
            index.write().unwrap();
            let tree_oid = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            let sig = repo.signature().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap()
        };

        // Rename the default branch to "main" (git init may default to "master")
        // Create refs/heads/main pointing at the same commit
        repo.reference("refs/heads/main", initial_oid, true, "rename to main")
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();
        // Remove refs/heads/master if it exists
        if let Ok(mut master_ref) = repo.find_reference("refs/heads/master") {
            master_ref.delete().unwrap();
        }

        (dir, repo)
    }

    #[test]
    fn detect_base_branch_finds_main() {
        let (_dir, repo) = init_repo_with_main();
        let base = detect_base_branch(&repo).unwrap();
        assert_eq!(base, "main");
    }

    #[test]
    fn current_branch_returns_branch_name() {
        let (_dir, repo) = init_repo_with_main();
        let name = current_branch(&repo).unwrap();
        assert_eq!(name, "main");
    }

    #[test]
    fn current_branch_on_feature() {
        let (_dir, repo) = init_repo_with_main();

        // Create and checkout feature branch
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature/test", &head_commit, false).unwrap();
        repo.set_head("refs/heads/feature/test").unwrap();

        let name = current_branch(&repo).unwrap();
        assert_eq!(name, "feature/test");
    }
}
