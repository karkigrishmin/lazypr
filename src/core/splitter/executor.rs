#![allow(dead_code)]

use anyhow::{Context, Result};

use crate::core::git::current_branch;
use crate::core::types::SplitPlan;

/// Options for executing a split plan.
pub struct ExecuteOptions {
    /// Base branch name (e.g. "main").
    pub base_branch: String,
    /// Prefix for generated branch names (e.g. "split").
    pub branch_prefix: String,
    /// Whether to do a dry run (no branches created).
    pub dry_run: bool,
}

/// Result of executing one group.
pub struct GroupExecutionResult {
    /// Index of the group.
    pub group_index: usize,
    /// Name of the created branch.
    pub branch_name: String,
    /// Number of files in this group.
    pub file_count: usize,
}

/// Result of executing the full split plan.
pub struct ExecutionResult {
    /// Results for each group.
    pub branches: Vec<GroupExecutionResult>,
    /// The branch we were on before execution.
    pub original_branch: String,
}

/// Sanitize a group name for use in a branch name.
///
/// Replaces non-alphanumeric characters (except `-`, `_`, `/`) with `-`
/// and trims leading/trailing dashes.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '/' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Execute a split plan by creating stacked branches.
///
/// For each group in order:
/// 1. Determine parent branch (from `depends_on`, or `base_branch`)
/// 2. Create branch `{prefix}/part-{N:02}-{sanitized_name}` from parent
/// 3. After all groups: restore original branch
///
/// In `dry_run` mode, computes branch names but creates nothing.
pub fn execute_split(
    plan: &SplitPlan,
    repo: &git2::Repository,
    options: &ExecuteOptions,
) -> Result<ExecutionResult> {
    let original_branch = current_branch(repo)?;
    let mut branches: Vec<GroupExecutionResult> = Vec::new();

    for group in &plan.groups {
        let sanitized = sanitize_name(&group.name);
        let branch_name = format!(
            "{}/part-{:02}-{}",
            options.branch_prefix,
            group.index + 1,
            sanitized
        );

        if !options.dry_run {
            // Determine parent: use last depends_on branch, or base_branch
            let parent_branch = if let Some(&dep_idx) = group.depends_on.last() {
                branches
                    .get(dep_idx)
                    .map(|r: &GroupExecutionResult| r.branch_name.clone())
                    .unwrap_or_else(|| options.base_branch.clone())
            } else {
                options.base_branch.clone()
            };

            // Resolve parent commit
            let parent_ref = format!("refs/heads/{}", parent_branch);
            let parent_commit = repo
                .find_reference(&parent_ref)
                .with_context(|| format!("failed to find parent branch '{}'", parent_branch))?
                .peel_to_commit()
                .with_context(|| {
                    format!("failed to peel parent branch '{}' to commit", parent_branch)
                })?;

            // Create branch from parent
            repo.branch(&branch_name, &parent_commit, false)
                .with_context(|| format!("failed to create branch '{}'", branch_name))?;
        }

        branches.push(GroupExecutionResult {
            group_index: group.index,
            branch_name,
            file_count: group.files.len(),
        });
    }

    // Restore original branch (on dry run this is a no-op since we didn't switch)
    if !options.dry_run {
        let ref_name = format!("refs/heads/{}", original_branch);
        repo.set_head(&ref_name)
            .context("failed to restore original branch")?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().safe()))
            .context("failed to checkout original branch")?;
    }

    Ok(ExecutionResult {
        branches,
        original_branch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{GroupStats, SplitGroup, SplitPlan};
    use tempfile::TempDir;

    /// Create a temp repo with an initial commit on "main".
    fn make_test_repo() -> (TempDir, git2::Repository) {
        let dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let sig = git2::Signature::now("Test", "test@test.com").unwrap();
        // Create initial commit
        let initial_oid = {
            let tree_id = repo.index().unwrap().write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap()
        };
        // Ensure refs/heads/main exists and HEAD points to it
        repo.reference("refs/heads/main", initial_oid, true, "init main")
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();
        // Remove refs/heads/master if it exists
        if let Ok(mut master_ref) = repo.find_reference("refs/heads/master") {
            master_ref.delete().unwrap();
        }
        (dir, repo)
    }

    fn make_plan(num_groups: usize) -> SplitPlan {
        let groups = (0..num_groups)
            .map(|i| SplitGroup {
                index: i,
                name: format!("group-{}", i + 1),
                files: vec![format!("file{}.ts", i)],
                depends_on: if i > 0 { vec![i - 1] } else { vec![] },
                stats: GroupStats {
                    total_files: 1,
                    total_additions: 50,
                    total_deletions: 0,
                    logic_lines: 50,
                },
            })
            .collect();
        SplitPlan {
            groups,
            skipped_files: vec![],
            warnings: vec![],
        }
    }

    #[test]
    fn dry_run_no_branches() {
        let (_dir, repo) = make_test_repo();
        let plan = make_plan(3);
        let options = ExecuteOptions {
            base_branch: "main".to_string(),
            branch_prefix: "split".to_string(),
            dry_run: true,
        };
        let result = execute_split(&plan, &repo, &options).unwrap();
        assert_eq!(result.branches.len(), 3);
        assert_eq!(result.branches[0].branch_name, "split/part-01-group-1");
        assert_eq!(result.branches[1].branch_name, "split/part-02-group-2");
        assert_eq!(result.branches[2].branch_name, "split/part-03-group-3");
        // Verify no actual branches were created
        assert!(repo
            .find_branch("split/part-01-group-1", git2::BranchType::Local)
            .is_err());
    }

    #[test]
    fn creates_branches_in_order() {
        let (_dir, repo) = make_test_repo();
        let plan = make_plan(2);
        let options = ExecuteOptions {
            base_branch: "main".to_string(),
            branch_prefix: "split".to_string(),
            dry_run: false,
        };
        let result = execute_split(&plan, &repo, &options).unwrap();
        assert_eq!(result.branches.len(), 2);
        assert_eq!(result.branches[0].branch_name, "split/part-01-group-1");
        assert_eq!(result.branches[1].branch_name, "split/part-02-group-2");
        assert_eq!(result.branches[0].file_count, 1);
        assert_eq!(result.branches[1].file_count, 1);
        // Verify branches exist in the repo
        assert!(repo
            .find_branch("split/part-01-group-1", git2::BranchType::Local)
            .is_ok());
        assert!(repo
            .find_branch("split/part-02-group-2", git2::BranchType::Local)
            .is_ok());
    }

    #[test]
    fn dependent_branch_points_to_parent() {
        let (_dir, repo) = make_test_repo();
        // Group 1 depends on group 0
        let plan = make_plan(2);
        let options = ExecuteOptions {
            base_branch: "main".to_string(),
            branch_prefix: "split".to_string(),
            dry_run: false,
        };
        let result = execute_split(&plan, &repo, &options).unwrap();

        // Both branches should point to the same commit (since parent was created
        // from main and the second one depends on the first)
        let b0 = repo
            .find_branch(&result.branches[0].branch_name, git2::BranchType::Local)
            .unwrap();
        let b1 = repo
            .find_branch(&result.branches[1].branch_name, git2::BranchType::Local)
            .unwrap();
        let c0 = b0.get().peel_to_commit().unwrap();
        let c1 = b1.get().peel_to_commit().unwrap();
        assert_eq!(c0.id(), c1.id());
    }

    #[test]
    fn restores_original_branch() {
        let (_dir, repo) = make_test_repo();
        let original = current_branch(&repo).unwrap();

        let plan = make_plan(1);
        let options = ExecuteOptions {
            base_branch: "main".to_string(),
            branch_prefix: "split".to_string(),
            dry_run: false,
        };
        let result = execute_split(&plan, &repo, &options).unwrap();
        assert_eq!(result.original_branch, original);

        // Current branch should still be the original
        let after = current_branch(&repo).unwrap();
        assert_eq!(after, original);
    }

    #[test]
    fn sanitize_name_works() {
        assert_eq!(sanitize_name("core/types"), "core/types");
        assert_eq!(sanitize_name("my-group"), "my-group");
        assert_eq!(sanitize_name("under_score"), "under_score");
        assert_eq!(sanitize_name("has spaces"), "has-spaces");
        assert_eq!(sanitize_name("special!@#chars"), "special---chars");
        assert_eq!(sanitize_name("-leading-dash"), "leading-dash");
        assert_eq!(sanitize_name("trailing-dash-"), "trailing-dash");
    }

    #[test]
    fn dry_run_preserves_group_metadata() {
        let (_dir, repo) = make_test_repo();
        let plan = make_plan(2);
        let options = ExecuteOptions {
            base_branch: "main".to_string(),
            branch_prefix: "my-prefix".to_string(),
            dry_run: true,
        };
        let result = execute_split(&plan, &repo, &options).unwrap();
        assert_eq!(result.branches[0].group_index, 0);
        assert_eq!(result.branches[1].group_index, 1);
        assert_eq!(result.branches[0].branch_name, "my-prefix/part-01-group-1");
        assert_eq!(result.branches[1].branch_name, "my-prefix/part-02-group-2");
    }
}
