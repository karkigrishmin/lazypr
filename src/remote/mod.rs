/// GitHub remote provider.
pub mod github;
/// GitLab remote provider (stub).
pub mod gitlab;
/// Remote provider trait and PR types.
pub mod provider;

#[allow(unused_imports)]
pub use provider::{
    CreatePullRequest, PrFilters, PullRequestState, RemoteProvider, RemotePullRequest, ReviewStatus,
};

use anyhow::Result;

use crate::state::config::RemoteConfig;

/// Detect and construct the appropriate remote provider for this repository.
/// Returns `None` if no remote is configured or the host is unrecognised.
pub fn detect_provider(
    repo: &git2::Repository,
    config: &RemoteConfig,
) -> Result<Option<Box<dyn RemoteProvider>>> {
    let remote = match repo.find_remote(&config.remote_name) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    let url = match remote.url() {
        Some(u) => u.to_string(),
        None => return Ok(None),
    };

    if url.contains("github.com") || config.provider.as_deref() == Some("github") {
        let provider = github::GitHubProvider::from_repo(repo)?;
        Ok(Some(Box::new(provider)))
    } else if url.contains("gitlab") || config.provider.as_deref() == Some("gitlab") {
        Ok(Some(Box::new(gitlab::GitLabProvider::new())))
    } else {
        Ok(None)
    }
}
