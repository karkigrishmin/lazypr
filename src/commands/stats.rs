use anyhow::Result;

use crate::cli::Cli;

/// Run the `stats` command.
///
/// Coming soon: Review analytics — time spent, files reviewed per session,
/// review velocity trends, and team review patterns.
pub fn run(_cli: &Cli) -> Result<()> {
    println!("Coming soon: review analytics and statistics");
    Ok(())
}
