use chrono::{DateTime, Utc};

/// Information about the author and commit for a single blamed line.
#[derive(Debug, Clone)]
pub struct BlameInfo {
    /// Author name.
    pub author: String,
    /// Author email.
    pub email: String,
    /// Timestamp of the commit.
    pub timestamp: DateTime<Utc>,
    /// Full SHA of the commit.
    pub commit_sha: String,
    /// Textual content of the blamed line.
    pub line_content: String,
}
