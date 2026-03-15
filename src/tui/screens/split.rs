use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::core::types::SplitPlan;
use crate::tui::theme::Theme;
use crate::tui::widgets::StatusBarWidget;

use super::{Action, Screen};

/// Which pane is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitFocus {
    GroupList,
    FileList,
}

/// Interactive split plan viewer/editor.
pub struct SplitScreen {
    plan: Option<SplitPlan>,
    selected_group: usize,
    selected_file: usize,
    focus: SplitFocus,
    scroll_offset: usize,
}

impl SplitScreen {
    /// Create a new split screen with an optional split plan.
    pub fn new(plan: Option<SplitPlan>) -> Self {
        Self {
            plan,
            selected_group: 0,
            selected_file: 0,
            focus: SplitFocus::GroupList,
            scroll_offset: 0,
        }
    }
}

impl Screen for SplitScreen {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        if self.plan.is_none() {
            return Action::None;
        }

        match key.code {
            KeyCode::Tab => {
                self.focus = match self.focus {
                    SplitFocus::GroupList => SplitFocus::FileList,
                    SplitFocus::FileList => SplitFocus::GroupList,
                };
                self.selected_file = 0;
                Action::SwitchPane
            }
            KeyCode::Char('j') | KeyCode::Down => {
                match self.focus {
                    SplitFocus::GroupList => {
                        if let Some(plan) = &self.plan {
                            if !plan.groups.is_empty() {
                                self.selected_group = (self.selected_group + 1) % plan.groups.len();
                                self.selected_file = 0;
                            }
                        }
                    }
                    SplitFocus::FileList => {
                        if let Some(plan) = &self.plan {
                            if let Some(group) = plan.groups.get(self.selected_group) {
                                if !group.files.is_empty() {
                                    self.selected_file =
                                        (self.selected_file + 1) % group.files.len();
                                }
                            }
                        }
                    }
                }
                Action::NavigateDown
            }
            KeyCode::Char('k') | KeyCode::Up => {
                match self.focus {
                    SplitFocus::GroupList => {
                        if let Some(plan) = &self.plan {
                            if !plan.groups.is_empty() {
                                self.selected_group = if self.selected_group == 0 {
                                    plan.groups.len() - 1
                                } else {
                                    self.selected_group - 1
                                };
                                self.selected_file = 0;
                            }
                        }
                    }
                    SplitFocus::FileList => {
                        if let Some(plan) = &self.plan {
                            if let Some(group) = plan.groups.get(self.selected_group) {
                                if !group.files.is_empty() {
                                    self.selected_file = if self.selected_file == 0 {
                                        group.files.len() - 1
                                    } else {
                                        self.selected_file - 1
                                    };
                                }
                            }
                        }
                    }
                }
                Action::NavigateUp
            }
            _ => Action::None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        match &self.plan {
            None => {
                let block = Block::default()
                    .title(" Split Plan ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border));

                let text = Paragraph::new(Line::from(Span::styled(
                    "Run `lazypr split` to generate a plan",
                    Style::default()
                        .fg(theme.muted)
                        .add_modifier(Modifier::ITALIC),
                )))
                .alignment(Alignment::Center)
                .block(block);

                frame.render_widget(text, area);
            }
            Some(plan) => {
                // Layout: content area + status bar at bottom
                let main_layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(area);

                let content_area = main_layout[0];
                let status_area = main_layout[1];

                // Two panes: 40% group list | 60% group detail + file list
                let panes = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                    .split(content_area);

                // --- Left pane: Group List ---
                let left_border_style = if self.focus == SplitFocus::GroupList {
                    Style::default().fg(theme.primary)
                } else {
                    Style::default().fg(theme.border)
                };

                let left_block = Block::default()
                    .title(" Groups ")
                    .borders(Borders::ALL)
                    .border_style(left_border_style);

                let inner_left = left_block.inner(panes[0]);
                let visible_height = inner_left.height as usize;

                // Adjust scroll offset to keep selected group visible
                let scroll_offset = {
                    let mut offset = self.scroll_offset;
                    if self.selected_group < offset {
                        offset = self.selected_group;
                    }
                    if self.selected_group >= offset + visible_height {
                        offset = self
                            .selected_group
                            .saturating_sub(visible_height.saturating_sub(1));
                    }
                    offset
                };

                let group_items: Vec<ListItem> = plan
                    .groups
                    .iter()
                    .enumerate()
                    .skip(scroll_offset)
                    .take(visible_height)
                    .map(|(i, group)| {
                        let mut label = format!(
                            "{}. {} ({} files, {} lines)",
                            i + 1,
                            group.name,
                            group.stats.total_files,
                            group.stats.logic_lines,
                        );

                        if !group.depends_on.is_empty() {
                            let deps: Vec<String> = group
                                .depends_on
                                .iter()
                                .map(|d| format!("{}", d + 1))
                                .collect();
                            label.push_str(&format!(" [deps: {}]", deps.join(", ")));
                        }

                        let style = if i == self.selected_group {
                            Style::default().bg(theme.highlight).fg(theme.fg)
                        } else {
                            Style::default().fg(theme.fg)
                        };

                        ListItem::new(Line::from(Span::styled(label, style)))
                    })
                    .collect();

                let group_list = List::new(group_items).block(left_block);
                frame.render_widget(group_list, panes[0]);

                // --- Right pane: Group Detail + File List ---
                let right_border_style = if self.focus == SplitFocus::FileList {
                    Style::default().fg(theme.primary)
                } else {
                    Style::default().fg(theme.border)
                };

                if let Some(group) = plan.groups.get(self.selected_group) {
                    let right_title =
                        format!(" Group {}: {} ", self.selected_group + 1, group.name);

                    let right_block = Block::default()
                        .title(right_title)
                        .borders(Borders::ALL)
                        .border_style(right_border_style);

                    // Split right pane: file list + stats/deps info at bottom
                    let right_inner = right_block.inner(panes[1]);

                    // Calculate lines needed for stats and deps info
                    let stats_lines: u16 = 1; // "+X / -Y | logic: Z"
                    let deps_lines: u16 = if group.depends_on.is_empty() { 0 } else { 1 };
                    let info_lines = stats_lines + deps_lines;

                    let right_layout = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(info_lines)])
                        .split(right_inner);

                    // Render the outer block first
                    frame.render_widget(right_block, panes[1]);

                    // File list
                    let file_items: Vec<ListItem> = group
                        .files
                        .iter()
                        .enumerate()
                        .map(|(i, file_path)| {
                            let style =
                                if self.focus == SplitFocus::FileList && i == self.selected_file {
                                    Style::default().bg(theme.highlight).fg(theme.fg)
                                } else {
                                    Style::default().fg(theme.fg)
                                };

                            ListItem::new(Line::from(Span::styled(file_path.clone(), style)))
                        })
                        .collect();

                    let file_list = List::new(file_items);
                    frame.render_widget(file_list, right_layout[0]);

                    // Stats summary
                    let mut info_spans: Vec<Span> = vec![
                        Span::styled(
                            format!("+{}", group.stats.total_additions),
                            Style::default().fg(theme.success),
                        ),
                        Span::styled(" / ", Style::default().fg(theme.muted)),
                        Span::styled(
                            format!("-{}", group.stats.total_deletions),
                            Style::default().fg(theme.error),
                        ),
                        Span::styled(" | logic: ", Style::default().fg(theme.muted)),
                        Span::styled(
                            format!("{}", group.stats.logic_lines),
                            Style::default().fg(theme.info),
                        ),
                    ];

                    if !group.depends_on.is_empty() {
                        let deps: Vec<String> = group
                            .depends_on
                            .iter()
                            .map(|d| format!("{}", d + 1))
                            .collect();
                        info_spans.push(Span::styled(
                            format!(" | depends on: {}", deps.join(", ")),
                            Style::default().fg(theme.muted),
                        ));
                    }

                    let info_paragraph =
                        Paragraph::new(Line::from(info_spans)).alignment(Alignment::Left);
                    frame.render_widget(info_paragraph, right_layout[1]);
                } else {
                    // No group selected (shouldn't happen but handle gracefully)
                    let right_block = Block::default()
                        .title(" Group Detail ")
                        .borders(Borders::ALL)
                        .border_style(right_border_style);

                    frame.render_widget(right_block, panes[1]);
                }

                // Status bar
                let bindings = vec![("j/k", "nav"), ("Tab", "switch pane"), ("q", "quit")];
                StatusBarWidget::new(&bindings).render(frame, status_area, theme);
            }
        }
    }
}
