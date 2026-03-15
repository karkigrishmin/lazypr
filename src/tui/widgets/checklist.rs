use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::core::types::ChecklistItem;
use crate::tui::theme::Theme;

/// Action returned from the checklist widget.
pub enum ChecklistAction {
    /// Toggle the item at the given index.
    Toggle(usize),
    /// Close the checklist popup.
    Close,
    /// Key consumed, no action needed.
    Continue,
}

/// A popup widget displaying a review checklist for a file.
pub struct ChecklistWidget {
    items: Vec<ChecklistItem>,
    selected: usize,
    file_path: String,
}

impl ChecklistWidget {
    /// Create a new checklist widget for the given file.
    pub fn new(file_path: String, items: Vec<ChecklistItem>) -> Self {
        Self {
            items,
            selected: 0,
            file_path,
        }
    }

    /// Handle a key event and return the resulting action.
    pub fn handle_key(&mut self, key: KeyEvent) -> ChecklistAction {
        match key.code {
            KeyCode::Char(' ') | KeyCode::Enter => ChecklistAction::Toggle(self.selected),
            KeyCode::Esc => ChecklistAction::Close,
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.items.is_empty() {
                    self.selected = (self.selected + 1) % self.items.len();
                }
                ChecklistAction::Continue
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if !self.items.is_empty() {
                    self.selected = if self.selected == 0 {
                        self.items.len() - 1
                    } else {
                        self.selected - 1
                    };
                }
                ChecklistAction::Continue
            }
            _ => ChecklistAction::Continue,
        }
    }

    /// Get the checklist items.
    pub fn items(&self) -> &[ChecklistItem] {
        &self.items
    }

    /// Toggle the checked state of the item at the given index.
    pub fn toggle(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            item.checked = !item.checked;
        }
    }

    /// Render the checklist popup centered in the given area.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let popup_width = 60_u16.min(area.width * 80 / 100);
        let popup_height = (self.items.len() as u16 + 5).min(area.height);
        let popup_area = centered_rect(popup_width, popup_height, area);

        // Clear the area behind the popup
        frame.render_widget(Clear, popup_area);

        let title = format!(" Checklist: {} ", self.file_path);
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.primary))
            .style(Style::default().bg(theme.bg));

        let mut lines: Vec<Line> = Vec::new();

        for (i, item) in self.items.iter().enumerate() {
            let check_mark = if item.checked { "[x]" } else { "[ ]" };
            let text = format!("{} {}", check_mark, item.text);

            let fg = if item.checked {
                theme.success
            } else {
                theme.fg
            };

            let mut style = Style::default().fg(fg);
            if i == self.selected {
                style = style.bg(theme.highlight);
            }

            lines.push(Line::from(Span::styled(format!(" {text}"), style)));
        }

        // Add empty line and hint
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Space: toggle | Esc: close",
            Style::default().fg(theme.muted),
        )));

        let paragraph = Paragraph::new(lines).block(block);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn esc_returns_close() {
        let items = vec![ChecklistItem {
            text: "Test?".into(),
            checked: false,
            source_pattern: "**".into(),
        }];
        let mut w = ChecklistWidget::new("a.ts".into(), items);
        assert!(matches!(
            w.handle_key(key(KeyCode::Esc)),
            ChecklistAction::Close
        ));
    }

    #[test]
    fn space_returns_toggle() {
        let items = vec![ChecklistItem {
            text: "Test?".into(),
            checked: false,
            source_pattern: "**".into(),
        }];
        let mut w = ChecklistWidget::new("a.ts".into(), items);
        assert!(matches!(
            w.handle_key(key(KeyCode::Char(' '))),
            ChecklistAction::Toggle(0)
        ));
    }

    #[test]
    fn toggle_flips_checked() {
        let items = vec![ChecklistItem {
            text: "Test?".into(),
            checked: false,
            source_pattern: "**".into(),
        }];
        let mut w = ChecklistWidget::new("a.ts".into(), items);
        w.toggle(0);
        assert!(w.items()[0].checked);
        w.toggle(0);
        assert!(!w.items()[0].checked);
    }

    #[test]
    fn navigation_wraps_around() {
        let items = vec![
            ChecklistItem {
                text: "A".into(),
                checked: false,
                source_pattern: "**".into(),
            },
            ChecklistItem {
                text: "B".into(),
                checked: false,
                source_pattern: "**".into(),
            },
        ];
        let mut w = ChecklistWidget::new("a.ts".into(), items);
        // Start at 0, go up wraps to 1
        w.handle_key(key(KeyCode::Char('k')));
        assert!(matches!(
            w.handle_key(key(KeyCode::Char(' '))),
            ChecklistAction::Toggle(1)
        ));
    }

    #[test]
    fn enter_returns_toggle() {
        let items = vec![ChecklistItem {
            text: "Test?".into(),
            checked: false,
            source_pattern: "**".into(),
        }];
        let mut w = ChecklistWidget::new("a.ts".into(), items);
        assert!(matches!(
            w.handle_key(key(KeyCode::Enter)),
            ChecklistAction::Toggle(0)
        ));
    }
}
