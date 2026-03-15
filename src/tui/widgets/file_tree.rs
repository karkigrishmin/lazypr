use std::collections::HashSet;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::core::{FileCategory, ReviewPriority};
use crate::tui::theme::Theme;

/// A single item in the file tree widget.
pub struct FileTreeItem {
    /// File status icon (e.g. "A", "M", "D", "R").
    pub status: String,
    /// File path.
    pub path: String,
    /// Review priority tier.
    pub priority: ReviewPriority,
    /// File category (used by future phases for grouping/display).
    #[allow(dead_code)]
    pub category: FileCategory,
    /// Optional semantic diff summary (e.g. "+2fn ~1sig -1fn").
    pub semantic_summary: Option<String>,
}

/// A scrollable file list with status icons, priority indicators, and selection highlighting.
pub struct FileTreeWidget {
    /// Items in the file tree.
    pub items: Vec<FileTreeItem>,
    /// Currently selected index.
    selected: usize,
    /// Scroll offset for long lists.
    scroll_offset: usize,
    /// Indices of files that have been skipped/reviewed.
    pub skipped: HashSet<usize>,
    /// Indices of files that have been marked as viewed.
    pub viewed: HashSet<usize>,
}

impl FileTreeWidget {
    /// Create a new file tree widget from a list of file tree items.
    pub fn new(items: Vec<FileTreeItem>) -> Self {
        Self {
            items,
            selected: 0,
            scroll_offset: 0,
            skipped: HashSet::new(),
            viewed: HashSet::new(),
        }
    }

    /// Return the currently selected index, or `None` if the list is empty.
    pub fn selected(&self) -> Option<usize> {
        if self.items.is_empty() {
            None
        } else {
            Some(self.selected)
        }
    }

    /// Move selection to the next item (wraps around).
    pub fn next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    /// Move selection to the previous item (wraps around).
    pub fn previous(&mut self) {
        if !self.items.is_empty() {
            self.selected = if self.selected == 0 {
                self.items.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Render the file tree into the given area.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, focused: bool) {
        let border_color = if focused { theme.primary } else { theme.border };

        let block = Block::default()
            .title(" Files ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);

        // Adjust scroll offset to keep selected item visible
        let visible_height = inner.height as usize;
        let scroll = self.effective_scroll(visible_height);

        let items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .skip(scroll)
            .take(visible_height)
            .map(|(i, item)| {
                let priority_indicator = match item.priority {
                    ReviewPriority::Deep => "!!",
                    ReviewPriority::Scan => "! ",
                    ReviewPriority::Glance => ". ",
                    ReviewPriority::Skip => "  ",
                };

                let viewed_char = if self.viewed.contains(&i) {
                    "\u{2713}"
                } else {
                    " "
                };

                let main_content = format!(
                    "{viewed_char} {priority_indicator} [{status}] {path}",
                    status = item.status,
                    path = item.path
                );

                let is_skipped = self.skipped.contains(&i);

                let fg_color = if is_skipped {
                    theme.muted
                } else {
                    match item.priority {
                        ReviewPriority::Deep => theme.deep,
                        ReviewPriority::Scan => theme.scan,
                        ReviewPriority::Glance => theme.glance,
                        ReviewPriority::Skip => theme.skip_priority,
                    }
                };

                let bg = if i == self.selected {
                    Some(theme.highlight)
                } else {
                    None
                };

                let main_style = {
                    let mut s = Style::default().fg(fg_color);
                    if let Some(bg_color) = bg {
                        s = s.bg(bg_color);
                    }
                    if i == self.selected {
                        s = s.add_modifier(Modifier::BOLD);
                    }
                    s
                };

                let mut spans = vec![Span::styled(main_content, main_style)];

                if let Some(ref summary) = item.semantic_summary {
                    if !summary.is_empty() {
                        let summary_style = {
                            let mut s = Style::default().fg(theme.muted);
                            if let Some(bg_color) = bg {
                                s = s.bg(bg_color);
                            }
                            s
                        };
                        spans.push(Span::styled(format!("  {summary}"), summary_style));
                    }
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    }

    /// Calculate effective scroll offset keeping the selected item in view.
    fn effective_scroll(&self, visible_height: usize) -> usize {
        if visible_height == 0 {
            return 0;
        }
        let mut scroll = self.scroll_offset;
        if self.selected < scroll {
            scroll = self.selected;
        } else if self.selected >= scroll + visible_height {
            scroll = self.selected.saturating_sub(visible_height - 1);
        }
        scroll
    }
}
