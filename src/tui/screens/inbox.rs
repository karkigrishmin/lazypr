use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::remote::{PullRequestState, RemotePullRequest, ReviewStatus};
use crate::tui::theme::Theme;
use crate::tui::widgets::StatusBarWidget;

use super::{Action, Screen};

/// Which inbox section is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InboxSection {
    MyPrs,
    ReviewRequested,
}

/// Interactive PR inbox screen showing the user's PRs and review requests.
pub struct InboxScreen {
    my_prs: Vec<RemotePullRequest>,
    review_prs: Vec<RemotePullRequest>,
    selected: usize,
    section: InboxSection,
    scroll_offset: usize,
}

impl InboxScreen {
    /// Create a new inbox screen with the given PR lists.
    pub fn new(my_prs: Vec<RemotePullRequest>, review_prs: Vec<RemotePullRequest>) -> Self {
        Self {
            my_prs,
            review_prs,
            selected: 0,
            section: InboxSection::MyPrs,
            scroll_offset: 0,
        }
    }

    fn current_list(&self) -> &[RemotePullRequest] {
        match self.section {
            InboxSection::MyPrs => &self.my_prs,
            InboxSection::ReviewRequested => &self.review_prs,
        }
    }

    fn selected_pr(&self) -> Option<&RemotePullRequest> {
        self.current_list().get(self.selected)
    }

    /// Format a human-readable age string from a timestamp.
    fn format_age(dt: &chrono::DateTime<chrono::Utc>) -> String {
        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(*dt);

        let days = duration.num_days();
        if days > 0 {
            return format!("{days}d ago");
        }
        let hours = duration.num_hours();
        if hours > 0 {
            return format!("{hours}h ago");
        }
        let minutes = duration.num_minutes();
        if minutes > 0 {
            return format!("{minutes}m ago");
        }
        "just now".to_string()
    }
}

/// Open a URL in the system's default web browser.
fn open_in_browser(url: &str) {
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
}

impl Screen for InboxScreen {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        let list_len = self.current_list().len();

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if list_len > 0 {
                    self.selected = (self.selected + 1) % list_len;
                }
                Action::NavigateDown
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if list_len > 0 {
                    self.selected = if self.selected == 0 {
                        list_len - 1
                    } else {
                        self.selected - 1
                    };
                }
                Action::NavigateUp
            }
            KeyCode::Tab => {
                self.section = match self.section {
                    InboxSection::MyPrs => InboxSection::ReviewRequested,
                    InboxSection::ReviewRequested => InboxSection::MyPrs,
                };
                self.selected = 0;
                self.scroll_offset = 0;
                Action::SwitchPane
            }
            KeyCode::Char('o') | KeyCode::Enter => {
                if let Some(pr) = self.selected_pr() {
                    open_in_browser(&pr.url);
                }
                Action::OpenInBrowser
            }
            _ => Action::None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        // If both lists are empty, show a centered message.
        if self.my_prs.is_empty() && self.review_prs.is_empty() {
            let block = Block::default()
                .title(" PR Inbox ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border));

            let text = Paragraph::new(Line::from(Span::styled(
                "No remote configured. Set GITHUB_TOKEN and run `lazypr inbox`.",
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC),
            )))
            .alignment(Alignment::Center)
            .block(block);

            frame.render_widget(text, area);
            return;
        }

        // Layout: content area + status bar at bottom
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        let content_area = main_layout[0];
        let status_area = main_layout[1];

        // Two panes: 50% PR list | 50% PR detail
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content_area);

        // --- Left pane: PR list ---
        let section_title = match self.section {
            InboxSection::MyPrs => " Your PRs ",
            InboxSection::ReviewRequested => " Review Requested ",
        };

        let left_block = Block::default()
            .title(section_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.primary));

        let inner_left = left_block.inner(panes[0]);
        let visible_height = inner_left.height as usize;

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

        let current = self.current_list();
        let pr_items: Vec<ListItem> = current
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_height)
            .map(|(i, pr)| {
                let status_label = match pr.review_status {
                    ReviewStatus::Approved => "[approved]",
                    ReviewStatus::ChangesRequested => "[changes]",
                    ReviewStatus::Pending => "[pending]",
                    ReviewStatus::None => "[no review]",
                };

                let age = Self::format_age(&pr.updated_at);
                let label = format!("#{:<5} {}  {}  {}", pr.number, pr.title, status_label, age);

                let style = if i == self.selected {
                    // Selected item
                    if pr.draft {
                        Style::default().bg(theme.highlight).fg(theme.muted)
                    } else {
                        Style::default().bg(theme.highlight).fg(theme.fg)
                    }
                } else if pr.draft {
                    Style::default().fg(theme.muted)
                } else {
                    let status_color = match pr.review_status {
                        ReviewStatus::Approved => theme.success,
                        ReviewStatus::ChangesRequested => theme.error,
                        ReviewStatus::Pending => theme.warning,
                        ReviewStatus::None => theme.fg,
                    };
                    Style::default().fg(status_color)
                };

                ListItem::new(Line::from(Span::styled(label, style)))
            })
            .collect();

        let pr_list = List::new(pr_items).block(left_block);
        frame.render_widget(pr_list, panes[0]);

        // --- Right pane: PR detail ---
        let right_block = Block::default()
            .title(" PR Detail ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        if let Some(pr) = self.selected_pr() {
            let state_str = match pr.state {
                PullRequestState::Open => "Open",
                PullRequestState::Closed => "Closed",
                PullRequestState::Merged => "Merged",
            };

            let draft_str = if pr.draft { " (draft)" } else { "" };

            let labels_str = if pr.labels.is_empty() {
                String::from("none")
            } else {
                pr.labels.join(", ")
            };

            let lines = vec![
                Line::from(Span::styled(
                    &pr.title,
                    Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Author:  ", Style::default().fg(theme.muted)),
                    Span::styled(&pr.author, Style::default().fg(theme.fg)),
                ]),
                Line::from(vec![
                    Span::styled("State:   ", Style::default().fg(theme.muted)),
                    Span::styled(
                        format!("{state_str}{draft_str}"),
                        Style::default().fg(theme.fg),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Branch:  ", Style::default().fg(theme.muted)),
                    Span::styled(&pr.head_branch, Style::default().fg(theme.info)),
                    Span::styled(" -> ", Style::default().fg(theme.muted)),
                    Span::styled(&pr.base_branch, Style::default().fg(theme.info)),
                ]),
                Line::from(vec![
                    Span::styled("Labels:  ", Style::default().fg(theme.muted)),
                    Span::styled(labels_str, Style::default().fg(theme.fg)),
                ]),
                Line::from(vec![
                    Span::styled("URL:     ", Style::default().fg(theme.muted)),
                    Span::styled(&pr.url, Style::default().fg(theme.primary)),
                ]),
            ];

            let detail = Paragraph::new(lines).block(right_block);
            frame.render_widget(detail, panes[1]);
        } else {
            let detail = Paragraph::new(Line::from(Span::styled(
                "No PR selected",
                Style::default().fg(theme.muted),
            )))
            .block(right_block);
            frame.render_widget(detail, panes[1]);
        }

        // Status bar
        let bindings = vec![
            ("j/k", "nav"),
            ("Tab", "switch"),
            ("o", "open in browser"),
            ("q", "quit"),
        ];
        StatusBarWidget::new(&bindings).render(frame, status_area, theme);
    }
}
