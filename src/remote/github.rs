use anyhow::{bail, Result};

use crate::remote::provider::*;

/// GitHub remote provider.
pub struct GitHubProvider;

impl GitHubProvider {
    /// Create a GitHub provider by extracting owner/repo from the git remote URL.
    pub fn from_repo(_repo: &git2::Repository) -> Result<Self> {
        // Full implementation will be added in Task 2.
        Ok(Self)
    }
}

#[async_trait::async_trait]
impl RemoteProvider for GitHubProvider {
    async fn current_user(&self) -> Result<String> {
        bail!("GitHub provider not yet fully implemented")
    }

    async fn list_pull_requests(&self, _filters: &PrFilters) -> Result<Vec<RemotePullRequest>> {
        bail!("GitHub provider not yet fully implemented")
    }

    async fn get_pull_request(&self, _number: u64) -> Result<RemotePullRequest> {
        bail!("GitHub provider not yet fully implemented")
    }

    async fn create_pull_request(&self, _req: &CreatePullRequest) -> Result<RemotePullRequest> {
        bail!("GitHub provider not yet fully implemented")
    }

    async fn post_comment(&self, _pr_number: u64, _body: &str) -> Result<()> {
        bail!("GitHub provider not yet fully implemented")
    }

    fn web_url(&self, _pr_number: u64) -> String {
        String::new()
    }
}
