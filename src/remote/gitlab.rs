use anyhow::{bail, Result};

use crate::remote::provider::*;

/// GitLab remote provider (not yet implemented).
pub struct GitLabProvider;

impl GitLabProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RemoteProvider for GitLabProvider {
    async fn current_user(&self) -> Result<String> {
        bail!("GitLab integration is not yet implemented")
    }

    async fn list_pull_requests(&self, _filters: &PrFilters) -> Result<Vec<RemotePullRequest>> {
        bail!("GitLab integration is not yet implemented")
    }

    async fn get_pull_request(&self, _number: u64) -> Result<RemotePullRequest> {
        bail!("GitLab integration is not yet implemented")
    }

    async fn create_pull_request(&self, _req: &CreatePullRequest) -> Result<RemotePullRequest> {
        bail!("GitLab integration is not yet implemented")
    }

    async fn post_comment(&self, _pr_number: u64, _body: &str) -> Result<()> {
        bail!("GitLab integration is not yet implemented")
    }

    fn web_url(&self, _pr_number: u64) -> String {
        String::new()
    }
}
