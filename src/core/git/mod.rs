pub mod blame;
pub mod branch;
pub mod diff;
pub mod log;
pub mod status;

use anyhow::Result;

use crate::core::types::DiffResult;

use self::blame::BlameInfo;

// Re-exports
pub use self::branch::{current_branch, detect_base_branch};
pub use self::diff::Git2DiffProvider;
pub use self::status::working_tree_clean;

/// Provides diff operations between two refs.
pub trait DiffProvider {
    /// Compute the diff between `base` and `head` refs.
    fn diff(&self, base: &str, head: &str) -> Result<DiffResult>;
}

/// Provides blame operations (stub for Phase 0).
pub trait BlameProvider {
    /// Retrieve blame information for a specific line in a file.
    fn blame(&self, path: &str, line: u32) -> Result<BlameInfo>;
}

/// Provides branch operations (stub for Phase 0).
pub trait BranchOperations {
    /// Create a new branch with the given name.
    fn create_branch(&self, name: &str) -> Result<()>;
    /// Check out an existing branch.
    fn checkout(&self, name: &str) -> Result<()>;
}
