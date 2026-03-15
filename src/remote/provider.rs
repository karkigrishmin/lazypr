use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Pull Request types
// ---------------------------------------------------------------------------

/// State of a pull request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PullRequestState {
    /// Open and accepting reviews.
    Open,
    /// Closed without merging.
    Closed,
    /// Merged into the base branch.
    Merged,
}

/// Review status summary for a pull request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewStatus {
    /// No reviews yet.
    None,
    /// Reviews requested but not completed.
    Pending,
    /// At least one approval, no changes requested.
    Approved,
    /// At least one reviewer requested changes.
    ChangesRequested,
}

/// A pull request from a remote provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotePullRequest {
    /// PR number.
    pub number: u64,
    /// PR title.
    pub title: String,
    /// Author login name.
    pub author: String,
    /// Current state.
    pub state: PullRequestState,
    /// Whether this is a draft PR.
    pub draft: bool,
    /// Head (source) branch name.
    pub head_branch: String,
    /// Base (target) branch name.
    pub base_branch: String,
    /// Web URL for the PR.
    pub url: String,
    /// When the PR was created.
    pub created_at: DateTime<Utc>,
    /// When the PR was last updated.
    pub updated_at: DateTime<Utc>,
    /// Number of lines added (if available).
    pub additions: Option<u64>,
    /// Number of lines deleted (if available).
    pub deletions: Option<u64>,
    /// Aggregate review status.
    pub review_status: ReviewStatus,
    /// Labels applied to the PR.
    pub labels: Vec<String>,
}

/// Request to create a new pull request.
#[derive(Debug, Clone)]
pub struct CreatePullRequest {
    /// PR title.
    pub title: String,
    /// PR body / description.
    pub body: String,
    /// Head (source) branch name.
    pub head: String,
    /// Base (target) branch name.
    pub base: String,
    /// Whether to create as a draft.
    pub draft: bool,
}

/// Filters for listing pull requests.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct PrFilters {
    /// Filter by author login.
    pub author: Option<String>,
    /// Filter by requested reviewer.
    pub reviewer: Option<String>,
    /// Filter by state.
    pub state: Option<PullRequestState>,
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Trait abstracting remote providers (GitHub, GitLab).
#[allow(dead_code)]
#[async_trait::async_trait]
pub trait RemoteProvider: Send + Sync {
    /// Get the authenticated user's login name.
    async fn current_user(&self) -> Result<String>;

    /// List pull requests matching the given filters.
    async fn list_pull_requests(&self, filters: &PrFilters) -> Result<Vec<RemotePullRequest>>;

    /// Get a single pull request by number.
    async fn get_pull_request(&self, number: u64) -> Result<RemotePullRequest>;

    /// Create a new pull request.
    async fn create_pull_request(&self, req: &CreatePullRequest) -> Result<RemotePullRequest>;

    /// Post a comment on a pull request.
    async fn post_comment(&self, pr_number: u64, body: &str) -> Result<()>;

    /// Get the web URL for a pull request.
    fn web_url(&self, pr_number: u64) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_pull_request() {
        let pr = RemotePullRequest {
            number: 42,
            title: "feat: add feature".to_string(),
            author: "user".to_string(),
            state: PullRequestState::Open,
            draft: false,
            head_branch: "feature/x".to_string(),
            base_branch: "main".to_string(),
            url: "https://github.com/owner/repo/pull/42".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            additions: Some(100),
            deletions: Some(20),
            review_status: ReviewStatus::Approved,
            labels: vec!["enhancement".to_string()],
        };
        let json = serde_json::to_string(&pr).unwrap();
        let back: RemotePullRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.number, 42);
        assert_eq!(back.state, PullRequestState::Open);
        assert_eq!(back.review_status, ReviewStatus::Approved);
    }

    #[test]
    fn pr_filters_default_is_empty() {
        let f = PrFilters::default();
        assert!(f.author.is_none());
        assert!(f.reviewer.is_none());
        assert!(f.state.is_none());
    }
}
