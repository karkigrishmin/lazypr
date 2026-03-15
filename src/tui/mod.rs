use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;

use crate::core::DiffResult;
use crate::state::LazyprConfig;

use self::app::App;

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

/// Run the TUI application with the given diff data and configuration.
pub fn run(diff: DiffResult, config: LazyprConfig) -> Result<()> {
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
    let mut app = App::new(diff, config);
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
fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| app.render(frame))?;

        if crossterm::event::poll(std::time::Duration::from_millis(50))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                let action = app.handle_key(key);
                if action == event::Action::Quit {
                    return Ok(());
                }
            }
        }
    }
}
