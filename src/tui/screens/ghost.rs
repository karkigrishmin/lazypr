use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::core::types::{GhostCategory, GhostResult, GhostSeverity};
use crate::tui::theme::Theme;
use crate::tui::widgets::StatusBarWidget;

use super::{Action, Screen};

/// Screen displaying ghost analysis findings.
pub struct GhostScreen {
    result: Option<GhostResult>,
    selected: usize,
    scroll_offset: usize,
}

impl GhostScreen {
    /// Create a new ghost screen with optional analysis results.
    pub fn new(result: Option<GhostResult>) -> Self {
        Self {
            result,
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Format the category as a short uppercase label.
    fn category_label(category: &GhostCategory) -> &'static str {
        match category {
            GhostCategory::BrokenImport => "BROKEN_IMPORT",
            GhostCategory::MissingTest => "MISSING_TEST",
            GhostCategory::HighImpact { .. } => "HIGH_IMPACT",
        }
    }

    /// Return the severity icon character.
    fn severity_icon(severity: &GhostSeverity) -> &'static str {
        match severity {
            GhostSeverity::Error => "E",
            GhostSeverity::Warning => "W",
            GhostSeverity::Info => "I",
        }
    }

    /// Return the theme color for a severity level.
    fn severity_color(severity: &GhostSeverity, theme: &Theme) -> Color {
        match severity {
            GhostSeverity::Error => theme.error,
            GhostSeverity::Warning => theme.warning,
            GhostSeverity::Info => theme.info,
        }
    }
}

impl Screen for GhostScreen {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        let finding_count = self.result.as_ref().map(|r| r.findings.len()).unwrap_or(0);

        if finding_count == 0 {
            return Action::None;
        }

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected + 1 < finding_count {
                    self.selected += 1;
                }
                Action::NavigateDown
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                Action::NavigateUp
            }
            _ => Action::None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        match &self.result {
            None => {
                // No result — show placeholder message
                let block = Block::default()
                    .title(" Ghost Analysis ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border));

                let text = Paragraph::new(Line::from(Span::styled(
                    "Run `lazypr ghost` to analyze",
                    Style::default()
                        .fg(theme.muted)
                        .add_modifier(Modifier::ITALIC),
                )))
                .alignment(Alignment::Center)
                .block(block);

                frame.render_widget(text, area);
            }
            Some(result) => {
                // Layout: content area + status bar at bottom
                let main_layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(area);

                let content_area = main_layout[0];
                let status_area = main_layout[1];

                // Title with summary counts
                let title = format!(
                    " Ghost Analysis ({} errors, {} warnings, {} info) ",
                    result.error_count, result.warning_count, result.info_count
                );

                let block = Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border));

                if result.findings.is_empty() {
                    let text = Paragraph::new(Line::from(Span::styled(
                        "No findings — looking good!",
                        Style::default()
                            .fg(theme.success)
                            .add_modifier(Modifier::BOLD),
                    )))
                    .alignment(Alignment::Center)
                    .block(block);

                    frame.render_widget(text, content_area);
                } else {
                    // Calculate visible height (content area minus borders)
                    let inner = block.inner(content_area);
                    let visible_height = inner.height as usize;

                    // Adjust scroll offset to keep selected item visible
                    let scroll_offset = {
                        let mut offset = self.scroll_offset;
                        if self.selected < offset {
                            offset = self.selected;
                        }
                        if self.selected >= offset + visible_height {
                            offset = self
                                .selected
                                .saturating_sub(visible_height.saturating_sub(1));
                        }
                        offset
                    };

                    // Build list items for visible range
                    let items: Vec<ListItem> = result
                        .findings
                        .iter()
                        .enumerate()
                        .skip(scroll_offset)
                        .take(visible_height)
                        .map(|(i, finding)| {
                            let icon = Self::severity_icon(&finding.severity);
                            let color = Self::severity_color(&finding.severity, theme);
                            let category = Self::category_label(&finding.category);

                            let line = Line::from(vec![
                                Span::styled(
                                    format!("[{}]", icon),
                                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                                ),
                                Span::raw(" "),
                                Span::styled(format!("[{}]", category), Style::default().fg(color)),
                                Span::raw(" "),
                                Span::styled(&finding.file, Style::default().fg(theme.fg)),
                                Span::styled(" — ", Style::default().fg(theme.muted)),
                                Span::styled(&finding.message, Style::default().fg(theme.fg)),
                            ]);

                            if i == self.selected {
                                ListItem::new(line).style(Style::default().bg(theme.highlight))
                            } else {
                                ListItem::new(line)
                            }
                        })
                        .collect();

                    let list = List::new(items).block(block);
                    frame.render_widget(list, content_area);
                }

                // Status bar
                let bindings = vec![
                    ("q", "quit"),
                    ("?", "help"),
                    ("j/k", "navigate"),
                    ("1-4", "switch tab"),
                ];
                StatusBarWidget::new(&bindings).render(frame, status_area, theme);
            }
        }
    }
}
