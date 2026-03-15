use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};

use crate::core::types::FileChurn;

/// Summary information for a single git commit.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    /// Full SHA of the commit.
    pub sha: String,
    /// Author name.
    pub author: String,
    /// First line of the commit message.
    pub message: String,
    /// Timestamp of the commit.
    pub timestamp: DateTime<Utc>,
}

/// Compute churn statistics for a set of files.
/// Returns commit count and author count per file within `since_days` lookback.
#[allow(dead_code)]
pub fn compute_file_churn(
    repo: &git2::Repository,
    paths: &[String],
    since_days: u32,
) -> Result<HashMap<String, FileChurn>> {
    let path_set: HashSet<&str> = paths.iter().map(|s| s.as_str()).collect();

    // Per-file accumulators
    let mut commit_counts: HashMap<String, usize> = HashMap::new();
    let mut author_sets: HashMap<String, HashSet<String>> = HashMap::new();

    // Cutoff timestamp
    let cutoff = Utc::now() - Duration::days(i64::from(since_days));

    // Walk revlog from HEAD
    let mut revwalk = repo.revwalk().context("failed to create revwalk")?;
    revwalk
        .push_head()
        .context("failed to push HEAD to revwalk")?;
    revwalk
        .set_sorting(git2::Sort::TIME)
        .context("failed to set revwalk sorting")?;

    for oid_result in revwalk {
        let oid = oid_result.context("failed to get next oid from revwalk")?;
        let commit = repo
            .find_commit(oid)
            .with_context(|| format!("failed to find commit {}", oid))?;

        // Check time window
        let commit_time = commit.time();
        let commit_epoch = commit_time.seconds();
        let commit_dt = DateTime::from_timestamp(commit_epoch, 0).unwrap_or_default();
        if commit_dt < cutoff {
            break;
        }

        let author = commit.author().name().unwrap_or("unknown").to_string();

        // Get commit tree
        let commit_tree = commit.tree().context("commit has no tree")?;

        // Get parent tree (or empty tree for root commit)
        let parent_tree = if commit.parent_count() > 0 {
            let parent = commit.parent(0).context("failed to get parent commit")?;
            Some(parent.tree().context("parent has no tree")?)
        } else {
            None
        };

        // Diff between parent and commit
        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), None)
            .context("failed to diff trees")?;

        // Check each delta for matching paths
        for i in 0..diff.deltas().len() {
            let delta = diff.get_delta(i).expect("delta index in range");
            let new_path = delta
                .new_file()
                .path()
                .map(|p| p.to_string_lossy().to_string());
            let old_path = delta
                .old_file()
                .path()
                .map(|p| p.to_string_lossy().to_string());

            for file_path in [&new_path, &old_path].into_iter().flatten() {
                if path_set.contains(file_path.as_str()) {
                    *commit_counts.entry(file_path.clone()).or_default() += 1;
                    author_sets
                        .entry(file_path.clone())
                        .or_default()
                        .insert(author.clone());
                    break; // Don't double-count if old and new path are the same
                }
            }
        }
    }

    // Compute average commit count for risk multiplier
    let total_commits: usize = commit_counts.values().sum();
    let file_count = commit_counts.len().max(1);
    let avg_commits = total_commits as f64 / file_count as f64;

    // Build result
    let mut result = HashMap::new();
    for (path, count) in &commit_counts {
        let authors = author_sets.get(path).map_or(0, |s| s.len());
        let risk = if avg_commits > 0.0 {
            (*count as f64 / avg_commits).clamp(0.5, 3.0)
        } else {
            1.0
        };
        result.insert(
            path.clone(),
            FileChurn {
                path: path.clone(),
                commit_count: *count,
                author_count: authors,
                risk_multiplier: risk,
            },
        );
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, git2::Repository) {
        let dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();

        let sig = git2::Signature::now("Test", "test@test.com").unwrap();
        // Create initial commit with a file
        std::fs::write(dir.path().join("file.txt"), "initial").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        {
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap();
        }
        (dir, repo)
    }

    #[test]
    fn file_at_ref_returns_content() {
        let (dir, _repo) = create_test_repo();
        let provider = crate::core::git::Git2DiffProvider::open_at(dir.path()).unwrap();
        let content = provider.file_at_ref("HEAD", "file.txt").unwrap();
        assert_eq!(content, Some("initial".to_string()));
    }

    #[test]
    fn file_at_ref_nonexistent_returns_none() {
        let (dir, _repo) = create_test_repo();
        let provider = crate::core::git::Git2DiffProvider::open_at(dir.path()).unwrap();
        let content = provider.file_at_ref("HEAD", "nonexistent.txt").unwrap();
        assert!(content.is_none());
    }

    #[test]
    fn churn_counts_commits() {
        let (dir, repo) = create_test_repo();
        let sig = git2::Signature::now("Test", "test@test.com").unwrap();
        // Make a second commit modifying the file
        std::fs::write(dir.path().join("file.txt"), "modified").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "modify", &tree, &[&parent])
            .unwrap();

        let churn = compute_file_churn(&repo, &["file.txt".to_string()], 30).unwrap();
        assert!(churn.contains_key("file.txt"));
        assert!(churn["file.txt"].commit_count >= 1);
    }

    #[test]
    fn churn_tracks_multiple_authors() {
        let (dir, repo) = create_test_repo();

        // Second commit by a different author
        let sig2 = git2::Signature::now("Author2", "author2@test.com").unwrap();
        std::fs::write(dir.path().join("file.txt"), "changed by author2").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(
            Some("HEAD"),
            &sig2,
            &sig2,
            "author2 change",
            &tree,
            &[&parent],
        )
        .unwrap();

        let churn = compute_file_churn(&repo, &["file.txt".to_string()], 30).unwrap();
        assert_eq!(churn["file.txt"].author_count, 2);
    }

    #[test]
    fn churn_returns_empty_for_unmatched_paths() {
        let (_dir, repo) = create_test_repo();
        let churn = compute_file_churn(&repo, &["nonexistent.txt".to_string()], 30).unwrap();
        assert!(churn.is_empty());
    }
}
