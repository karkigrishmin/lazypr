use std::path::Path;

use anyhow::Result;

use crate::core::ReviewNote;

/// Load notes for a branch (stub -- returns empty vec).
pub fn load_notes(_repo_root: &Path, _branch: &str) -> Result<Vec<ReviewNote>> {
    Ok(Vec::new())
}

/// Save a note (stub -- no-op for now).
pub fn save_note(_repo_root: &Path, _note: &ReviewNote) -> Result<()> {
    Ok(())
}
