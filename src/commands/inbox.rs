use anyhow::{Context, Result};
use chrono::Utc;

use crate::cli::Cli;
use crate::core::git::Git2DiffProvider;
use crate::remote::{self, PrFilters, PullRequestState, RemotePullRequest, ReviewStatus};
use crate::state::{init_store, LazyprConfig};

/// Run the `inbox` command — PR dashboard showing PRs needing review.
pub fn run(cli: &Cli) -> Result<()> {
    let provider = Git2DiffProvider::open().context("failed to open git repository")?;
    let repo = provider.repo();
    let repo_root = repo
        .workdir()
        .context("bare repositories not supported")?
        .to_path_buf();
    init_store(&repo_root)?;
    let config = LazyprConfig::load(&repo_root)?;

    // Create tokio runtime first — octocrab's tower buffer requires a runtime context
    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    let _guard = rt.enter();

    // Detect remote provider (octocrab client needs tokio runtime context)
    let remote = remote::detect_provider(repo, &config.remote)?.context(
        "No remote provider detected. Ensure your repo has a GitHub remote and GITHUB_TOKEN is set.",
    )?;

    let (user, all_prs) = rt.block_on(async {
        let user = remote.current_user().await?;
        let prs = remote
            .list_pull_requests(&PrFilters {
                state: Some(PullRequestState::Open),
                ..Default::default()
            })
            .await?;
        Ok::<_, anyhow::Error>((user, prs))
    })?;

    // Separate into my PRs and review-requested
    let my_prs: Vec<&RemotePullRequest> = all_prs.iter().filter(|pr| pr.author == user).collect();
    let review_prs: Vec<&RemotePullRequest> =
        all_prs.iter().filter(|pr| pr.author != user).collect();

    // Cache result
    let cache_path = repo_root.join(".lazypr").join("cache").join("inbox.json");
    if let Ok(json) = serde_json::to_string_pretty(&all_prs) {
        let _ = std::fs::write(&cache_path, json);
    }

    if cli.json {
        let output = serde_json::json!({
            "user": user,
            "my_prs": my_prs,
            "review_prs": review_prs,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    // Pretty print
    println!("PR Inbox\n");

    if my_prs.is_empty() && review_prs.is_empty() {
        println!("No open pull requests found.");
        return Ok(());
    }

    if !my_prs.is_empty() {
        println!("Your PRs ({}):", my_prs.len());
        for pr in &my_prs {
            print_pr_line(pr);
        }
        println!();
    }

    if !review_prs.is_empty() {
        println!("Review Requested ({}):", review_prs.len());
        for pr in &review_prs {
            print_pr_line(pr);
        }
        println!();
    }

    Ok(())
}

fn print_pr_line(pr: &RemotePullRequest) {
    let status = match pr.review_status {
        ReviewStatus::Approved => "[approved]",
        ReviewStatus::ChangesRequested => "[changes]",
        ReviewStatus::Pending => "[pending]",
        ReviewStatus::None => "[no review]",
    };
    let draft = if pr.draft { " (draft)" } else { "" };
    let age = format_age(pr.updated_at);
    let changes = match (pr.additions, pr.deletions) {
        (Some(a), Some(d)) => format!("+{}/-{}", a, d),
        _ => String::new(),
    };
    println!(
        "  #{:<5} {:<40} {:>12} {:>10} {:>6}{}",
        pr.number,
        truncate(&pr.title, 40),
        status,
        changes,
        age,
        draft
    );
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

fn format_age(dt: chrono::DateTime<Utc>) -> String {
    let diff = Utc::now() - dt;
    if diff.num_hours() < 1 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h ago", diff.num_hours())
    } else {
        format!("{}d ago", diff.num_days())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_long_string() {
        let result = truncate("this is a very long string", 10);
        assert!(result.len() <= 10);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn format_age_minutes() {
        let dt = Utc::now() - Duration::minutes(30);
        let result = format_age(dt);
        assert!(result.contains("m ago"), "expected minutes: {}", result);
    }

    #[test]
    fn format_age_hours() {
        let dt = Utc::now() - Duration::hours(5);
        let result = format_age(dt);
        assert!(result.contains("h ago"), "expected hours: {}", result);
    }

    #[test]
    fn format_age_days() {
        let dt = Utc::now() - Duration::days(3);
        let result = format_age(dt);
        assert!(result.contains("d ago"), "expected days: {}", result);
    }

    #[test]
    fn print_pr_line_formats_correctly() {
        let pr = RemotePullRequest {
            number: 42,
            title: "feat: add inbox dashboard".to_string(),
            author: "testuser".to_string(),
            state: PullRequestState::Open,
            draft: false,
            head_branch: "feature/inbox".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/owner/repo/pull/42".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            additions: Some(100),
            deletions: Some(20),
            review_status: ReviewStatus::Approved,
            labels: vec!["enhancement".to_string()],
        };
        // Just ensure it doesn't panic
        print_pr_line(&pr);
    }

    #[test]
    fn print_pr_line_draft() {
        let pr = RemotePullRequest {
            number: 7,
            title: "wip: draft pr".to_string(),
            author: "user".to_string(),
            state: PullRequestState::Open,
            draft: true,
            head_branch: "draft-branch".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/owner/repo/pull/7".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            additions: None,
            deletions: None,
            review_status: ReviewStatus::None,
            labels: vec![],
        };
        // Just ensure it doesn't panic
        print_pr_line(&pr);
    }
}
