use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::core::LineKind;
use crate::tui::theme::Theme;

/// Renders diff lines with color coding and scrolling.
pub struct DiffViewWidget {
    /// Lines as (kind, content) pairs.
    lines: Vec<(LineKind, String)>,
    /// Current scroll offset.
    scroll_offset: usize,
}

impl DiffViewWidget {
    /// Create a new diff view widget from a list of diff lines.
    pub fn new(lines: Vec<(LineKind, String)>) -> Self {
        Self {
            lines,
            scroll_offset: 0,
        }
    }

    /// Scroll the view down by one line.
    pub fn scroll_down(&mut self) {
        if self.scroll_offset < self.lines.len().saturating_sub(1) {
            self.scroll_offset += 1;
        }
    }

    /// Scroll the view up by one line.
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Render the diff view into the given area.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, focused: bool) {
        let border_color = if focused { theme.primary } else { theme.border };

        let block = Block::default()
            .title(" Diff ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        let visible_height = inner.height as usize;

        let styled_lines: Vec<Line> = self
            .lines
            .iter()
            .skip(self.scroll_offset)
            .take(visible_height)
            .map(|(kind, content)| {
                let (prefix, color) = match kind {
                    LineKind::Added => ("+", theme.success),
                    LineKind::Removed => ("-", theme.error),
                    LineKind::Context => (" ", theme.fg),
                    LineKind::Moved => (">", theme.info),
                    LineKind::MovedEdited => ("~", theme.info),
                };
                Line::from(Span::styled(
                    format!("{prefix}{content}"),
                    Style::default().fg(color),
                ))
            })
            .collect();

        let paragraph = Paragraph::new(styled_lines).block(block);
        frame.render_widget(paragraph, area);
    }
}
