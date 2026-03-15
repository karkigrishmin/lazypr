use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use crate::tui::theme::Theme;

/// Result of handling a key in the note editor.
#[allow(dead_code)]
pub enum NoteEditorAction {
    /// Enter pressed — contains the note content.
    Save(String),
    /// Esc pressed — cancel editing.
    Cancel,
    /// Key consumed (typing) — no action needed.
    Continue,
}

/// A popup widget for adding a review note to a specific file and line.
#[allow(dead_code)]
pub struct NoteEditorWidget {
    input: Input,
    file_path: String,
    line_no: Option<u32>,
}

#[allow(dead_code)]
impl NoteEditorWidget {
    /// Create a new note editor with empty input for the given file and line.
    pub fn new(file_path: String, line_no: Option<u32>) -> Self {
        Self {
            input: Input::default(),
            file_path,
            line_no,
        }
    }

    /// Handle a key event and return the resulting action.
    pub fn handle_key(&mut self, key: KeyEvent) -> NoteEditorAction {
        match key.code {
            KeyCode::Enter => NoteEditorAction::Save(self.input.value().to_string()),
            KeyCode::Esc => NoteEditorAction::Cancel,
            _ => {
                self.input.handle_event(&crossterm::event::Event::Key(key));
                NoteEditorAction::Continue
            }
        }
    }

    /// Render the note editor popup centered in the given area.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let popup_width = 60_u16.min(area.width * 80 / 100);
        let popup_height = 7_u16;
        let popup_area = centered_rect(popup_width, popup_height, area);

        // Clear the area behind the popup
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Add Note ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.primary))
            .style(Style::default().bg(theme.bg));

        let context = match self.line_no {
            Some(line) => format!("{}:{}", self.file_path, line),
            None => self.file_path.clone(),
        };

        let content = vec![
            Line::from(Span::styled(context, Style::default().fg(theme.muted))),
            Line::from(""),
            Line::from(Span::styled(
                self.input.value(),
                Style::default().fg(theme.fg),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Enter: save | Esc: cancel",
                Style::default().fg(theme.muted),
            )),
        ];

        let paragraph = Paragraph::new(content).block(block);
        frame.render_widget(paragraph, popup_area);

        // Set cursor position on the input line (line 3 inside the block = y + 3)
        let cursor_x = popup_area.x + self.input.visual_cursor() as u16 + 1;
        let cursor_y = popup_area.y + 3;
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
    }
}

/// Helper to create a centered rectangle of given width and height within `area`.
#[allow(dead_code)]
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
    fn esc_returns_cancel() {
        let mut editor = NoteEditorWidget::new("a.rs".into(), Some(10));
        assert!(matches!(
            editor.handle_key(key(KeyCode::Esc)),
            NoteEditorAction::Cancel
        ));
    }

    #[test]
    fn enter_returns_save_with_content() {
        let mut editor = NoteEditorWidget::new("a.rs".into(), Some(10));
        editor.handle_key(key(KeyCode::Char('h')));
        editor.handle_key(key(KeyCode::Char('i')));
        match editor.handle_key(key(KeyCode::Enter)) {
            NoteEditorAction::Save(content) => assert_eq!(content, "hi"),
            _ => panic!("expected Save"),
        }
    }

    #[test]
    fn typing_returns_continue() {
        let mut editor = NoteEditorWidget::new("a.rs".into(), Some(10));
        assert!(matches!(
            editor.handle_key(key(KeyCode::Char('x'))),
            NoteEditorAction::Continue
        ));
    }
}
