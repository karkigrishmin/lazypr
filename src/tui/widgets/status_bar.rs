use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::tui::theme::Theme;

/// Bottom status bar showing keybindings.
pub struct StatusBarWidget<'a> {
    /// Key-description pairs to display.
    bindings: &'a [(&'a str, &'a str)],
}

impl<'a> StatusBarWidget<'a> {
    /// Create a new status bar widget with the given keybinding descriptions.
    pub fn new(bindings: &'a [(&'a str, &'a str)]) -> Self {
        Self { bindings }
    }

    /// Render the status bar into the given area.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let mut spans: Vec<Span> = Vec::new();

        for (i, (key, desc)) in self.bindings.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    " | ",
                    Style::default().fg(theme.muted).bg(theme.status_bg),
                ));
            }
            spans.push(Span::styled(
                (*key).to_string(),
                Style::default()
                    .fg(theme.primary)
                    .bg(theme.status_bg)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(": {desc}"),
                Style::default().fg(theme.status_fg).bg(theme.status_bg),
            ));
        }

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line).style(Style::default().bg(theme.status_bg));
        frame.render_widget(paragraph, area);
    }
}
