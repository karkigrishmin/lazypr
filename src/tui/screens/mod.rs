use crossterm::event::KeyEvent;
use ratatui::prelude::*;

use super::event::Action;
use super::theme::Theme;

/// Trait that all TUI screens implement.
pub trait Screen {
    /// Handle a key event, returning the resulting action.
    fn handle_key(&mut self, key: KeyEvent) -> Action;
    /// Render the screen into the given frame area.
    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme);
}

/// Ghost diff screen (stub).
pub mod ghost;
/// Help overlay popup.
pub mod help;
/// PR inbox screen (stub).
pub mod inbox;
/// Primary diff review screen.
pub mod review;
/// Split plan screen (stub).
pub mod split;

pub use ghost::GhostScreen;
pub use help::HelpOverlay;
pub use inbox::InboxScreen;
pub use review::{ReviewContext, ReviewScreen};
pub use split::SplitScreen;
