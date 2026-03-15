use anyhow::Result;

use crate::cli::Cli;

/// Run the `notes` command.
///
/// Coming soon: CLI interface for managing review notes — list, export,
/// and clear notes for a branch. Notes can already be added via the TUI (press 'n').
pub fn run(_cli: &Cli) -> Result<()> {
    println!("Coming soon: CLI note management (use 'n' key in review TUI to add notes)");
    Ok(())
}
