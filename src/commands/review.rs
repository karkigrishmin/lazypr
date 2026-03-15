use anyhow::{Context, Result};

use crate::cli::Cli;
use crate::core::git::{detect_base_branch, DiffProvider, Git2DiffProvider};
use crate::state::{init_store, LazyprConfig};

/// Run the `review` command (also the default when no subcommand is given).
pub fn run(cli: &Cli) -> Result<()> {
    // Open git repo
    let provider = Git2DiffProvider::open()
        .context("failed to open git repository — are you in a git repo?")?;

    let repo = provider.repo();

    // Detect or use specified base branch
    let base = match &cli.base {
        Some(b) => b.clone(),
        None => detect_base_branch(repo)
            .context("failed to detect base branch — use --base to specify one")?,
    };

    // Initialize state directory
    let repo_root = repo
        .workdir()
        .context("bare repositories are not supported")?;
    init_store(repo_root)?;

    // Load config
    let config = LazyprConfig::load(repo_root)?;

    // Compute diff
    let mut diff = provider
        .diff(&base, "HEAD")
        .context("failed to compute diff")?;
    crate::core::differ::pipeline::analyze(&mut diff, &config.review);

    if cli.json {
        // JSON output mode
        let json =
            serde_json::to_string_pretty(&diff).context("failed to serialize diff to JSON")?;
        println!("{}", json);
    } else {
        // Launch TUI
        crate::tui::run(diff, config)?;
    }

    Ok(())
}
