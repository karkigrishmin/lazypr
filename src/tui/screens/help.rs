use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::tui::theme::Theme;

use super::{Action, Screen};

/// A centered popup overlay that displays keybinding help.
pub struct HelpOverlay {
    _private: (),
}

impl HelpOverlay {
    /// Create a new help overlay.
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Screen for HelpOverlay {
    fn handle_key(&mut self, _key: KeyEvent) -> Action {
        // Any key dismisses the help overlay (handled by App)
        Action::ToggleHelp
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let help_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  q          ",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Quit", Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(
                    "  ?          ",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Toggle help", Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(
                    "  j/Down     ",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Navigate down", Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(
                    "  k/Up       ",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Navigate up", Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(
                    "  Tab        ",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Switch pane", Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(
                    "  1-4        ",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Switch screen", Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(
                    "  s          ",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Skip file", Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(
                    "  v          ",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Mark viewed", Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(
                    "  n          ",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Add note", Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(
                    "  i          ",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Inter-diff", Style::default().fg(theme.fg)),
            ]),
            Line::from(vec![
                Span::styled(
                    "  /          ",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Search files", Style::default().fg(theme.fg)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Press any key to close",
                Style::default().fg(theme.muted),
            )),
            Line::from(""),
        ];

        // Center the popup
        let popup_width = 35_u16;
        let popup_height = help_text.len() as u16 + 2; // +2 for borders
        let popup_area = centered_rect(popup_width, popup_height, area);

        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.primary))
            .style(Style::default().bg(theme.bg));

        // Clear the area behind the popup
        frame.render_widget(Clear, popup_area);

        let paragraph = Paragraph::new(help_text).block(block);
        frame.render_widget(paragraph, popup_area);
    }
}

/// Helper to create a centered rectangle of given width and height within `area`.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
