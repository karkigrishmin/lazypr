use anyhow::Result;
use clap::Parser;

mod cli;
mod commands;
mod core;
mod remote;
mod state;
mod tui;
#[allow(dead_code)]
mod utils;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    match &cli.command {
        Some(cli::Commands::Review) | None => commands::review::run(&cli),
        Some(cli::Commands::Split {
            dry_run,
            prefix,
            execute,
            create_prs,
        }) => commands::split::run(&cli, *dry_run, prefix.clone(), *execute, *create_prs),
        Some(cli::Commands::Ghost) => commands::ghost::run(&cli),
        Some(cli::Commands::Impact { file }) => commands::impact::run(&cli, file),
        Some(cli::Commands::Inbox) => commands::inbox::run(&cli),
        Some(cli::Commands::Stats) => commands::stats::run(&cli),
        Some(cli::Commands::Notes) => commands::notes::run(&cli),
    }
}
