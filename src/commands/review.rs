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

    // Load config (used later when TUI is wired up)
    let _config = LazyprConfig::load(repo_root)?;

    // Compute diff
    let diff = provider
        .diff(&base, "HEAD")
        .context("failed to compute diff")?;

    if cli.json {
        // JSON output mode
        let json =
            serde_json::to_string_pretty(&diff).context("failed to serialize diff to JSON")?;
        println!("{}", json);
        return Ok(());
    }

    // TUI mode — for now, print summary until TUI is wired in Step 6
    println!("lazypr review: {} vs HEAD", base);
    println!("  {} files changed", diff.summary.total_files);
    println!(
        "  +{} -{}",
        diff.summary.total_additions, diff.summary.total_deletions
    );
    println!(
        "  Estimated review time: {} min",
        diff.summary.estimated_review_minutes
    );
    println!();
    for file in &diff.files {
        println!(
            "  {:?} {} (+{} -{})",
            file.status, file.path, file.stats.additions, file.stats.deletions
        );
    }
    println!();
    println!("(TUI coming in Phase 0 Step 6)");

    Ok(())
}
