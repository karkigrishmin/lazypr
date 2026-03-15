/// GitHub remote provider.
pub mod github;
/// GitLab remote provider (stub).
pub mod gitlab;
/// Remote provider trait and PR types.
pub mod provider;

pub use provider::{
    CreatePullRequest, PrFilters, PullRequestState, RemoteProvider, RemotePullRequest, ReviewStatus,
};
