use clap::{Parser, Subcommand};

/// The lazygit of pull requests — fast, intelligent PR review in your terminal
#[derive(Parser, Debug)]
#[command(name = "lazypr", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Output as JSON instead of launching TUI
    #[arg(long, global = true)]
    pub json: bool,

    /// Override the base branch for comparison
    #[arg(long, global = true)]
    pub base: Option<String>,

    /// Enable verbose/debug output
    #[arg(long, short, global = true)]
    pub verbose: bool,
}

/// Available subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Review the current branch's changes
    Review,
    /// Split a large PR into smaller stacked PRs
    Split {
        /// Dry run: show the plan without creating branches
        #[arg(long)]
        dry_run: bool,
        /// Branch name prefix for generated stacked branches
        #[arg(long, default_value = "split")]
        prefix: String,
        /// Execute immediately without interactive TUI
        #[arg(long)]
        execute: bool,
        /// Create GitHub/GitLab PRs for each split branch (requires --execute)
        #[arg(long)]
        create_prs: bool,
    },
    /// Pre-push analysis — find issues before you push
    Ghost,
    /// Show dependency impact for a file
    Impact {
        /// The file to analyze
        file: String,
    },
    /// PR dashboard — track PRs needing review
    Inbox,
    /// Review analytics and statistics
    Stats,
    /// Manage private review notes
    Notes,
}
