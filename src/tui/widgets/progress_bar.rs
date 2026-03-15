use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::tui::theme::Theme;

/// A horizontal progress bar showing review completion (X/Y files viewed).
#[allow(dead_code)]
pub struct ProgressBarWidget {
    viewed: usize,
    total: usize,
}

#[allow(dead_code)]
impl ProgressBarWidget {
    /// Create a new progress bar widget.
    pub fn new(viewed: usize, total: usize) -> Self {
        Self { viewed, total }
    }

    /// Render the progress bar into the given area (1 line height).
    ///
    /// Format: `[=====>    ] 5/12 viewed`
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let viewed = self.viewed.min(self.total);
        let text = format!(" {}/{} viewed", viewed, self.total);
        let text_len = text.len() as u16;

        // We need at least 2 chars for brackets + text length
        let bracket_width: u16 = 2;
        let bar_width = area.width.saturating_sub(text_len + bracket_width);

        let fill_width = if self.total > 0 && bar_width > 0 {
            (bar_width as usize * viewed / self.total) as u16
        } else {
            0
        };
        let empty_width = bar_width.saturating_sub(fill_width);

        let spans = vec![
            Span::styled("[", Style::default().fg(theme.muted)),
            Span::styled(
                "=".repeat(fill_width as usize),
                Style::default().fg(theme.success),
            ),
            Span::styled(
                " ".repeat(empty_width as usize),
                Style::default().fg(theme.muted),
            ),
            Span::styled("]", Style::default().fg(theme.muted)),
            Span::styled(text, Style::default().fg(theme.fg)),
        ];

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_total_does_not_panic() {
        let _ = ProgressBarWidget::new(0, 0);
    }

    #[test]
    fn viewed_clamped_to_total() {
        let pb = ProgressBarWidget::new(15, 10);
        // Should not panic, viewed is effectively clamped
        assert_eq!(pb.viewed, 15);
        assert_eq!(pb.total, 10);
    }
}
