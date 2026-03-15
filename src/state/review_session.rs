use std::path::Path;

use anyhow::Result;

use crate::core::ReviewSession;

/// Load a review session for a branch (stub -- returns `None` for now).
pub fn load_session(_repo_root: &Path, _branch: &str) -> Result<Option<ReviewSession>> {
    Ok(None)
}

/// Save a review session (stub -- no-op for now).
pub fn save_session(_repo_root: &Path, _session: &ReviewSession) -> Result<()> {
    Ok(())
}
