use std::collections::HashSet;
use std::io;

use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use crate::core::{DiffFile, DiffResult, ReviewNote};
use crate::state::LazyprConfig;

use self::app::App;
use self::screens::ReviewContext;

/// State extracted from the TUI when the user quits.
pub struct ReviewFinalState {
    /// Indices of files the user marked as viewed during this session.
    pub viewed_files: HashSet<usize>,
    /// Review notes at the time the user quit.
    pub notes: Vec<ReviewNote>,
    /// The file list (used to map indices back to paths).
    pub files: Vec<DiffFile>,
}

/// Application state and main loop driver.
pub mod app;
/// TUI action/event types.
pub mod event;
/// Screen implementations (review, split, inbox, ghost, help).
pub mod screens;
/// Color theme definitions.
pub mod theme;
/// Reusable TUI widgets (file tree, diff view, status bar, etc.).
pub mod widgets;

/// Run the TUI application with the given diff data, configuration, and review context.
///
/// Returns the final state (viewed files, notes, file list) when the user quits.
pub fn run(diff: DiffResult, config: LazyprConfig, ctx: ReviewContext) -> Result<ReviewFinalState> {
    // Setup terminal
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to create terminal")?;

    // Install panic hook that restores terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));

    // Create and run app
    let mut app = App::new(diff, config, ctx);
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .context("failed to leave alternate screen")?;
    terminal.show_cursor().context("failed to show cursor")?;

    result
}

/// Main event loop: render, poll for events, dispatch to app.
///
/// Returns the final review state when the user triggers a quit action.
fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<ReviewFinalState> {
    loop {
        terminal.draw(|frame| app.render(frame))?;

        if crossterm::event::poll(std::time::Duration::from_millis(50))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                let action = app.handle_key(key);
                if action == event::Action::Quit {
                    let review = app.review_screen();
                    return Ok(ReviewFinalState {
                        viewed_files: review.viewed_files().clone(),
                        notes: review.notes().to_vec(),
                        files: review.files().to_vec(),
                    });
                }
            }
        }
    }
}
