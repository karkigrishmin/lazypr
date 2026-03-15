use std::process;

use anyhow::{Context, Result};

use crate::cli::Cli;
use crate::commands::resolve::parse_repo_files;
use crate::core::analyzer::ghost::analyze_ghost;
use crate::core::git::{detect_base_branch, DiffProvider, Git2DiffProvider};
use crate::core::graph::dependency::DependencyGraph;
use crate::core::types::{GhostCategory, GhostSeverity};
use crate::state::LazyprConfig;

/// Run the `ghost` command — pre-push analysis to find issues before you push.
pub fn run(cli: &Cli) -> Result<()> {
    let provider = Git2DiffProvider::open().context("failed to open git repository")?;
    let repo = provider.repo();
    let repo_root = repo
        .workdir()
        .context("bare repositories not supported")?
        .to_path_buf();

    let base = match &cli.base {
        Some(b) => b.clone(),
        None => detect_base_branch(repo)
            .context("failed to detect base branch — use --base to specify one")?,
    };

    let config = LazyprConfig::load(&repo_root)?;

    // Compute diff
    let mut diff = provider
        .diff(&base, "HEAD")
        .context("failed to compute diff")?;
    crate::core::differ::pipeline::analyze(&mut diff, &config.review);

    // Parse all repo files
    let parsed_files = parse_repo_files(&repo_root).context("failed to parse repository files")?;

    // Build dependency graph
    let graph = DependencyGraph::build(&parsed_files);

    // Run ghost analysis
    let result = analyze_ghost(&diff.files, &parsed_files, &graph);

    if cli.json {
        let json =
            serde_json::to_string_pretty(&result).context("failed to serialize ghost result")?;
        println!("{}", json);
    } else {
        // Get current branch name for the header
        let branch = crate::core::git::current_branch(repo).unwrap_or_else(|_| "HEAD".to_string());
        println!("Ghost analysis: {} vs {}\n", branch, base);

        if result.findings.is_empty() {
            println!("No issues found. You're good to push!");
            return Ok(());
        }

        // Group by severity
        let errors: Vec<_> = result
            .findings
            .iter()
            .filter(|f| f.severity == GhostSeverity::Error)
            .collect();
        let warnings: Vec<_> = result
            .findings
            .iter()
            .filter(|f| f.severity == GhostSeverity::Warning)
            .collect();
        let infos: Vec<_> = result
            .findings
            .iter()
            .filter(|f| f.severity == GhostSeverity::Info)
            .collect();

        if !errors.is_empty() {
            println!("ERRORS ({}):", errors.len());
            for finding in &errors {
                let tag = match &finding.category {
                    GhostCategory::BrokenImport => "BROKEN_IMPORT",
                    GhostCategory::MissingTest => "MISSING_TEST",
                    GhostCategory::HighImpact { .. } => "HIGH_IMPACT",
                };
                println!("  [{}] {}", tag, finding.file);
                println!("    {}", finding.message);
            }
            println!();
        }

        if !warnings.is_empty() {
            println!("WARNINGS ({}):", warnings.len());
            for finding in &warnings {
                let tag = match &finding.category {
                    GhostCategory::BrokenImport => "BROKEN_IMPORT",
                    GhostCategory::MissingTest => "MISSING_TEST",
                    GhostCategory::HighImpact { .. } => "HIGH_IMPACT",
                };
                println!("  [{}] {} — {}", tag, finding.file, finding.message);
            }
            println!();
        }

        if !infos.is_empty() {
            println!("INFO ({}):", infos.len());
            for finding in &infos {
                let tag_str;
                let tag = match &finding.category {
                    GhostCategory::BrokenImport => "BROKEN_IMPORT",
                    GhostCategory::MissingTest => "MISSING_TEST",
                    GhostCategory::HighImpact { dependent_count } => {
                        tag_str = format!("HIGH_IMPACT({})", dependent_count);
                        &tag_str
                    }
                };
                println!("  [{}] {} — {}", tag, finding.file, finding.message);
            }
            println!();
        }

        println!(
            "Summary: {} errors, {} warnings, {} info",
            result.error_count, result.warning_count, result.info_count
        );

        // Exit with code 1 if errors found (useful for CI)
        if result.error_count > 0 {
            process::exit(1);
        }
    }

    Ok(())
}
