use anyhow::{Context, Result};

use crate::core::errors::GitError;
use crate::core::git::{BranchOperations, Git2DiffProvider};

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

impl BranchOperations for Git2DiffProvider {
    fn create_branch(&self, name: &str) -> Result<()> {
        let head = self.repo().head().context("failed to get HEAD")?;
        let commit = head.peel_to_commit().context("HEAD is not a commit")?;
        self.repo()
            .branch(name, &commit, false)
            .with_context(|| format!("failed to create branch '{}'", name))?;
        Ok(())
    }

    fn checkout(&self, name: &str) -> Result<()> {
        let ref_name = format!("refs/heads/{}", name);
        self.repo()
            .set_head(&ref_name)
            .with_context(|| format!("failed to set HEAD to '{}'", name))?;
        self.repo()
            .checkout_head(Some(git2::build::CheckoutBuilder::default().safe()))
            .context("failed to checkout HEAD")?;
        Ok(())
    }
}

/// Additional branch helpers for the split executor.
impl Git2DiffProvider {
    /// Create a branch pointing at a specific commit.
    #[allow(dead_code)]
    pub fn create_branch_at(&self, name: &str, commit: &git2::Commit) -> Result<()> {
        self.repo()
            .branch(name, commit, false)
            .with_context(|| format!("failed to create branch '{}' at {}", name, commit.id()))?;
        Ok(())
    }
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

    #[test]
    fn create_and_checkout_branch() {
        let (dir, repo) = init_repo_with_main();
        let provider = Git2DiffProvider::open_at(dir.path()).unwrap();
        provider.create_branch("test-branch").unwrap();

        // Verify branch exists
        assert!(repo
            .find_branch("test-branch", git2::BranchType::Local)
            .is_ok());

        // Checkout and verify
        provider.checkout("test-branch").unwrap();
        let head = repo.head().unwrap();
        assert!(head.name().unwrap().contains("test-branch"));
    }

    #[test]
    fn duplicate_branch_errors() {
        let (dir, _repo) = init_repo_with_main();
        let provider = Git2DiffProvider::open_at(dir.path()).unwrap();
        provider.create_branch("test-branch").unwrap();
        assert!(provider.create_branch("test-branch").is_err());
    }

    #[test]
    fn checkout_nonexistent_branch_errors() {
        let (dir, _repo) = init_repo_with_main();
        let provider = Git2DiffProvider::open_at(dir.path()).unwrap();
        assert!(provider.checkout("does-not-exist").is_err());
    }

    #[test]
    fn create_branch_at_specific_commit() {
        let (dir, _repo) = init_repo_with_main();
        let provider = Git2DiffProvider::open_at(dir.path()).unwrap();
        let commit = provider.repo().head().unwrap().peel_to_commit().unwrap();
        let commit_id = commit.id();
        provider.create_branch_at("at-commit", &commit).unwrap();

        let branch = provider
            .repo()
            .find_branch("at-commit", git2::BranchType::Local)
            .unwrap();
        let branch_commit = branch.get().peel_to_commit().unwrap();
        assert_eq!(branch_commit.id(), commit_id);
    }
}
