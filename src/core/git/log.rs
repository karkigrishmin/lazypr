use chrono::{DateTime, Utc};

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
