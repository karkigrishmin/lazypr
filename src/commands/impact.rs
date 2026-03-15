use anyhow::{Context, Result};

use crate::cli::Cli;
use crate::commands::resolve::parse_repo_files;
use crate::core::graph::dependency::DependencyGraph;
use crate::core::graph::impact::{analyze_impact, ImpactSeverity};

/// Run the `impact` command — show dependency impact for a file.
pub fn run(cli: &Cli, file: &str) -> Result<()> {
    // Open git repo to get repo_root
    let provider =
        crate::core::git::Git2DiffProvider::open().context("failed to open git repository")?;
    let repo_root = provider
        .repo()
        .workdir()
        .context("bare repositories not supported")?
        .to_path_buf();

    // Parse all repo files and build dependency graph
    let parsed_files = parse_repo_files(&repo_root).context("failed to parse repository files")?;
    let graph = DependencyGraph::build(&parsed_files);

    // Run impact analysis (max_depth 5 for reasonable output)
    let result = analyze_impact(&graph, &[file.to_string()], 5);

    if cli.json {
        let json = serde_json::json!({
            "file": file,
            "direct_count": result.direct_count,
            "transitive_count": result.transitive_count,
            "total": result.impacted.len(),
            "impacted": result.impacted.iter().map(|e| {
                serde_json::json!({
                    "file": e.file,
                    "severity": match &e.severity {
                        ImpactSeverity::Direct => "direct".to_string(),
                        ImpactSeverity::Transitive { depth } => format!("transitive (depth {})", depth),
                    }
                })
            }).collect::<Vec<_>>()
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("Impact analysis for: {}\n", file);

        let direct: Vec<_> = result
            .impacted
            .iter()
            .filter(|e| e.severity == ImpactSeverity::Direct)
            .collect();
        let transitive: Vec<_> = result
            .impacted
            .iter()
            .filter(|e| matches!(e.severity, ImpactSeverity::Transitive { .. }))
            .collect();

        if direct.is_empty() && transitive.is_empty() {
            println!("No files depend on this file.");
            return Ok(());
        }

        if !direct.is_empty() {
            println!("Direct dependents ({}):", direct.len());
            for entry in &direct {
                println!("  {}", entry.file);
            }
            println!();
        }

        if !transitive.is_empty() {
            println!("Transitive dependents ({}):", transitive.len());
            for entry in &transitive {
                if let ImpactSeverity::Transitive { depth } = &entry.severity {
                    println!("  {} (depth: {})", entry.file, depth);
                }
            }
            println!();
        }

        println!("Total: {} files affected", result.impacted.len());
    }

    Ok(())
}
