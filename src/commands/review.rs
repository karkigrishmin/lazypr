use anyhow::{Context, Result};

use crate::cli::Cli;
use crate::core::differ::interdiff::compute_interdiff;
use crate::core::git::{current_branch, detect_base_branch, DiffProvider, Git2DiffProvider};
use crate::core::ReviewSession;
use crate::state::notes::load_notes;
use crate::state::review_session::{latest_round, load_session, save_session, start_new_round};
use crate::state::{init_store, LazyprConfig};
use crate::tui::screens::ReviewContext;

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
        .context("bare repositories are not supported")?
        .to_path_buf();
    init_store(&repo_root)?;

    // Load config
    let config = LazyprConfig::load(&repo_root)?;

    // Get current branch name
    let branch = current_branch(repo)?;

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
        return Ok(());
    }

    // Load existing session or create new
    let mut session = load_session(&repo_root, &branch)?.unwrap_or(ReviewSession {
        branch: branch.clone(),
        reviews: vec![],
    });

    // Compute inter-diff if there's a previous review round
    let interdiff = if let Some(last) = latest_round(&session) {
        match provider.diff(&base, &last.sha) {
            Ok(old_diff) => Some(compute_interdiff(&old_diff, &diff)),
            Err(_) => None, // SHA no longer reachable — skip interdiff
        }
    } else {
        None
    };

    // Get previously viewed files from last round
    let viewed = latest_round(&session)
        .map(|r| r.files_viewed.clone())
        .unwrap_or_default();

    // Start a new review round with current HEAD SHA
    let head_sha = repo
        .revparse_single("HEAD")
        .context("failed to resolve HEAD")?
        .peel_to_commit()
        .context("HEAD is not a commit")?
        .id()
        .to_string();
    start_new_round(&mut session, &head_sha);

    // Load notes
    let notes = load_notes(&repo_root, &branch)?;

    // Build review context
    let ctx = ReviewContext {
        notes,
        interdiff,
        viewed_files: viewed,
        repo_root: repo_root.clone(),
        branch_name: branch.clone(),
    };

    // Run TUI — blocks until user quits
    let final_state = crate::tui::run(diff, config, ctx)?;

    // Persist session: update latest round with viewed files
    if let Some(round) = session.reviews.last_mut() {
        round.files_viewed = final_state
            .viewed_files
            .iter()
            .filter_map(|&idx| final_state.files.get(idx).map(|f| f.path.clone()))
            .collect();
        round.notes_count = final_state.notes.len();
    }
    save_session(&repo_root, &session)?;

    // Persist notes
    crate::state::notes::save_notes(&repo_root, &branch, &final_state.notes)?;

    Ok(())
}
