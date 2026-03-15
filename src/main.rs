use anyhow::Result;
use clap::Parser;

mod cli;
#[allow(dead_code, unused_imports)]
mod commands;
#[allow(dead_code, unused_imports)]
mod core;
#[allow(dead_code, unused_imports)]
mod remote;
#[allow(dead_code, unused_imports)]
mod state;
#[allow(dead_code, unused_imports)]
mod tui;
#[allow(dead_code, unused_imports)]
mod utils;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    match &cli.command {
        Some(cli::Commands::Review) | None => commands::review::run(&cli),
        Some(cli::Commands::Split) => commands::split::run(&cli),
        Some(cli::Commands::Ghost) => commands::ghost::run(&cli),
        Some(cli::Commands::Impact { file }) => commands::impact::run(&cli, file),
        Some(cli::Commands::Inbox) => commands::inbox::run(&cli),
        Some(cli::Commands::Stats) => commands::stats::run(&cli),
        Some(cli::Commands::Notes) => commands::notes::run(&cli),
    }
}
