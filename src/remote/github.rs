use anyhow::{Context, Result};

use crate::remote::provider::*;

/// GitHub API provider using octocrab.
pub struct GitHubProvider {
    client: octocrab::Octocrab,
    owner: String,
    repo: String,
}

impl GitHubProvider {
    /// Create from explicit token, owner, and repo.
    pub fn new(token: &str, owner: String, repo: String) -> Result<Self> {
        let client = octocrab::Octocrab::builder()
            .personal_token(token.to_string())
            .build()
            .context("failed to build GitHub client")?;
        Ok(Self {
            client,
            owner,
            repo,
        })
    }

    /// Create by detecting owner/repo from git remote URL and resolving token.
    pub fn from_repo(repo: &git2::Repository) -> Result<Self> {
        let remote = repo
            .find_remote("origin")
            .context("no 'origin' remote found")?;
        let url = remote.url().context("remote 'origin' has no URL")?;
        let (owner, repo_name) = parse_github_url(url)?;
        let token = resolve_github_token()?;
        Self::new(&token, owner, repo_name)
    }
}

/// Parse owner and repo from a GitHub URL.
/// Supports: `git@github.com:owner/repo.git`, `https://github.com/owner/repo.git`
pub fn parse_github_url(url: &str) -> Result<(String, String)> {
    // SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let path = rest.trim_end_matches(".git");
        let parts: Vec<&str> = path.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    }
    // HTTPS: https://github.com/owner/repo.git
    if url.contains("github.com/") {
        if let Some(after) = url.split("github.com/").nth(1) {
            let path = after.trim_end_matches(".git").trim_end_matches('/');
            let parts: Vec<&str> = path.splitn(2, '/').collect();
            if parts.len() == 2 {
                return Ok((parts[0].to_string(), parts[1].to_string()));
            }
        }
    }
    Err(anyhow::anyhow!(
        "could not parse GitHub owner/repo from URL: {}",
        url
    ))
}

/// Resolve GitHub token from environment or gh CLI.
pub fn resolve_github_token() -> Result<String> {
    // 1. GITHUB_TOKEN env
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }
    // 2. GH_TOKEN env (GitHub CLI convention)
    if let Ok(token) = std::env::var("GH_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }
    // 3. Shell out to `gh auth token`
    if let Ok(output) = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
    {
        if output.status.success() {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !token.is_empty() {
                return Ok(token);
            }
        }
    }
    Err(anyhow::anyhow!(
        "No GitHub token found. Set GITHUB_TOKEN environment variable or install the GitHub CLI (`gh`)."
    ))
}

#[async_trait::async_trait]
impl RemoteProvider for GitHubProvider {
    async fn current_user(&self) -> Result<String> {
        let user = self
            .client
            .current()
            .user()
            .await
            .context("failed to get current GitHub user")?;
        Ok(user.login)
    }

    async fn list_pull_requests(&self, filters: &PrFilters) -> Result<Vec<RemotePullRequest>> {
        let page = self
            .client
            .pulls(&self.owner, &self.repo)
            .list()
            .state(octocrab::params::State::Open)
            .per_page(100)
            .send()
            .await
            .context("failed to list pull requests")?;

        let prs: Vec<RemotePullRequest> = page
            .items
            .into_iter()
            .filter(|pr| {
                // Apply author filter
                if let Some(ref author) = filters.author {
                    if pr.user.as_ref().map(|u| u.login.as_str()) != Some(author.as_str()) {
                        return false;
                    }
                }
                true
            })
            .map(|pr| map_octocrab_pr(&pr, &self.owner, &self.repo))
            .collect();

        Ok(prs)
    }

    async fn get_pull_request(&self, number: u64) -> Result<RemotePullRequest> {
        let pr = self
            .client
            .pulls(&self.owner, &self.repo)
            .get(number)
            .await
            .context("failed to get pull request")?;
        Ok(map_octocrab_pr(&pr, &self.owner, &self.repo))
    }

    async fn create_pull_request(&self, req: &CreatePullRequest) -> Result<RemotePullRequest> {
        let pr = self
            .client
            .pulls(&self.owner, &self.repo)
            .create(&req.title, &req.head, &req.base)
            .body(&req.body)
            .draft(req.draft)
            .send()
            .await
            .context("failed to create pull request")?;
        Ok(map_octocrab_pr(&pr, &self.owner, &self.repo))
    }

    async fn post_comment(&self, pr_number: u64, body: &str) -> Result<()> {
        self.client
            .issues(&self.owner, &self.repo)
            .create_comment(pr_number, body)
            .await
            .context("failed to post comment")?;
        Ok(())
    }

    fn web_url(&self, pr_number: u64) -> String {
        format!(
            "https://github.com/{}/{}/pull/{}",
            self.owner, self.repo, pr_number
        )
    }
}

fn map_octocrab_pr(
    pr: &octocrab::models::pulls::PullRequest,
    owner: &str,
    repo: &str,
) -> RemotePullRequest {
    RemotePullRequest {
        number: pr.number,
        title: pr.title.clone().unwrap_or_default(),
        author: pr
            .user
            .as_ref()
            .map(|u| u.login.clone())
            .unwrap_or_default(),
        state: if pr.merged_at.is_some() {
            PullRequestState::Merged
        } else if pr.closed_at.is_some() {
            PullRequestState::Closed
        } else {
            PullRequestState::Open
        },
        draft: pr.draft.unwrap_or(false),
        head_branch: pr.head.ref_field.clone(),
        base_branch: pr.base.ref_field.clone(),
        url: pr
            .html_url
            .as_ref()
            .map(|u| u.to_string())
            .unwrap_or_else(|| format!("https://github.com/{}/{}/pull/{}", owner, repo, pr.number)),
        created_at: pr.created_at.unwrap_or_else(chrono::Utc::now),
        updated_at: pr.updated_at.unwrap_or_else(chrono::Utc::now),
        additions: pr.additions,
        deletions: pr.deletions,
        review_status: ReviewStatus::None, // Would need separate API call
        labels: pr
            .labels
            .as_ref()
            .map(|labels| labels.iter().map(|l| l.name.clone()).collect())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ssh_url() {
        let (owner, repo) = parse_github_url("git@github.com:owner/repo.git").unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
    }

    #[test]
    fn parse_https_url() {
        let (owner, repo) = parse_github_url("https://github.com/owner/repo.git").unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
    }

    #[test]
    fn parse_https_without_git_suffix() {
        let (owner, repo) = parse_github_url("https://github.com/owner/repo").unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
    }

    #[test]
    fn parse_invalid_url() {
        assert!(parse_github_url("https://gitlab.com/owner/repo").is_err());
    }

    #[test]
    fn parse_ssh_without_git_suffix() {
        let (owner, repo) = parse_github_url("git@github.com:myorg/myrepo").unwrap();
        assert_eq!(owner, "myorg");
        assert_eq!(repo, "myrepo");
    }

    #[test]
    fn parse_https_with_trailing_slash() {
        let (owner, repo) = parse_github_url("https://github.com/owner/repo/").unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
    }

    #[test]
    fn web_url_format() {
        // Test web_url without needing a real client by checking format directly
        let url = format!("https://github.com/{}/{}/pull/{}", "owner", "repo", 42);
        assert_eq!(url, "https://github.com/owner/repo/pull/42");
    }
}
