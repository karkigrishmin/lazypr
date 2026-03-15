use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::theme::Theme;

use super::{Action, Screen};

/// Stub screen for the split-plan view (coming in a later phase).
pub struct SplitScreen {
    _private: (),
}

impl SplitScreen {
    /// Create a new split screen stub.
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Screen for SplitScreen {
    fn handle_key(&mut self, _key: KeyEvent) -> Action {
        Action::None
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .title(" Split Plan ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let text = Paragraph::new(Line::from(Span::styled(
            "Coming in Phase 2",
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::ITALIC),
        )))
        .alignment(Alignment::Center)
        .block(block);

        frame.render_widget(text, area);
    }
}
