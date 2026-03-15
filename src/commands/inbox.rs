use anyhow::Result;

use crate::cli::Cli;

/// Run the `inbox` command.
///
/// Coming in Phase 5: PR dashboard showing PRs needing review,
/// your PRs awaiting review, and review status tracking via GitHub/GitLab API.
pub fn run(_cli: &Cli) -> Result<()> {
    println!("Coming soon: PR inbox dashboard (Phase 5 — remote integration)");
    Ok(())
}
