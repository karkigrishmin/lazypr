use anyhow::{Context, Result};

use crate::cli::Cli;
use crate::commands::resolve::parse_repo_files;
use crate::core::git::{detect_base_branch, DiffProvider, Git2DiffProvider};
use crate::core::graph::dependency::DependencyGraph;
use crate::core::splitter::algorithm::generate_split_plan;
use crate::core::splitter::executor::{execute_split, ExecuteOptions};
use crate::core::splitter::validator::validate_plan;
use crate::state::{init_store, LazyprConfig};

pub fn run(cli: &Cli, dry_run: bool, prefix: String, execute: bool) -> Result<()> {
    let provider = Git2DiffProvider::open().context("failed to open git repository")?;
    let repo = provider.repo();
    let repo_root = repo
        .workdir()
        .context("bare repositories not supported")?
        .to_path_buf();
    init_store(&repo_root)?;
    let config = LazyprConfig::load(&repo_root)?;

    let base = match &cli.base {
        Some(b) => b.clone(),
        None => detect_base_branch(repo)
            .context("failed to detect base branch — use --base to specify one")?,
    };

    // Compute diff
    let mut diff = provider
        .diff(&base, "HEAD")
        .context("failed to compute diff")?;
    crate::core::differ::pipeline::analyze(&mut diff, &config.review);

    // Parse repo files and build dependency graph
    let parsed = parse_repo_files(&repo_root).context("failed to parse repository files")?;
    let graph = DependencyGraph::build(&parsed);

    // Generate split plan
    let plan = generate_split_plan(&diff.files, &graph, &config.split);

    // Validate
    let issues = validate_plan(&plan, &graph);

    if cli.json {
        // JSON output: include plan and validation issues
        let output = serde_json::json!({
            "plan": plan,
            "validation_issues": issues.iter().map(|i| {
                serde_json::json!({
                    "group_index": i.group_index,
                    "file": i.file,
                    "missing_dep": i.missing_dep,
                    "message": i.message,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    // Print plan summary
    println!(
        "Split plan: {} groups, {} skipped files\n",
        plan.groups.len(),
        plan.skipped_files.len()
    );

    for group in &plan.groups {
        let deps_str = if group.depends_on.is_empty() {
            String::new()
        } else {
            let deps: Vec<String> = group
                .depends_on
                .iter()
                .map(|d| (d + 1).to_string())
                .collect();
            format!(" [depends on: {}]", deps.join(", "))
        };
        println!(
            "  Group {}: {} ({} files, {} lines){}",
            group.index + 1,
            group.name,
            group.stats.total_files,
            group.stats.logic_lines,
            deps_str,
        );
        for file in &group.files {
            println!("    {}", file);
        }
        println!();
    }

    if !plan.skipped_files.is_empty() {
        println!("Skipped: {}", plan.skipped_files.join(", "));
        println!();
    }

    if !plan.warnings.is_empty() {
        println!("Warnings:");
        for w in &plan.warnings {
            println!("  {}", w);
        }
        println!();
    }

    if !issues.is_empty() {
        println!("Validation issues ({}):", issues.len());
        for issue in &issues {
            println!(
                "  Group {}: {} depends on {} — {}",
                issue.group_index + 1,
                issue.file,
                issue.missing_dep,
                issue.message
            );
        }
        println!();
    } else {
        println!("Validation: OK");
    }

    // Execute if requested
    if execute || dry_run {
        let options = ExecuteOptions {
            base_branch: base,
            branch_prefix: prefix,
            dry_run,
        };
        let result = execute_split(&plan, repo, &options)?;

        if dry_run {
            println!("\nDry run — branches that would be created:");
        } else {
            println!("\nBranches created:");
        }
        for branch in &result.branches {
            println!("  {} ({} files)", branch.branch_name, branch.file_count);
        }
    }

    Ok(())
}
