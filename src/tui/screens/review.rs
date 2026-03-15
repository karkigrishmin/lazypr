use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use crate::core::{DiffFile, DiffResult};
use crate::tui::theme::Theme;
use crate::tui::widgets::{DiffViewWidget, FileTreeItem, FileTreeWidget, StatusBarWidget};

use super::{Action, Screen};

/// Which pane is currently focused in the review screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    /// The left file list pane.
    FileTree,
    /// The right diff view pane.
    DiffView,
}

/// The primary review screen showing a file tree and diff view side-by-side.
pub struct ReviewScreen {
    active_pane: Pane,
    file_tree: FileTreeWidget,
    diff_view: DiffViewWidget,
    /// The diff files for rebuilding diff view on selection change.
    files: Vec<DiffFile>,
    /// Currently selected file index (mirrors file_tree selection).
    current_file_idx: usize,
    /// Set of file indices that have been skipped/reviewed.
    skipped_files: HashSet<usize>,
    /// Whether search mode is active.
    search_mode: bool,
    /// The search input widget.
    search_input: Input,
    /// Current search query string.
    search_query: String,
    /// Mapping from filtered indices to original file indices (when search is active).
    filtered_indices: Option<Vec<usize>>,
}

impl ReviewScreen {
    /// Create a new review screen from a diff result.
    pub fn new(diff: &DiffResult) -> Self {
        let file_tree_items: Vec<FileTreeItem> = diff
            .files
            .iter()
            .map(|f| FileTreeItem {
                status: f.status.as_char().to_string(),
                path: f.path.clone(),
                priority: f.priority.clone(),
                category: f.category.clone(),
            })
            .collect();

        let file_tree = FileTreeWidget::new(file_tree_items);

        let diff_view = Self::build_diff_view_for_file(&diff.files, 0);

        Self {
            active_pane: Pane::FileTree,
            file_tree,
            diff_view,
            files: diff.files.clone(),
            current_file_idx: 0,
            skipped_files: HashSet::new(),
            search_mode: false,
            search_input: Input::default(),
            search_query: String::new(),
            filtered_indices: None,
        }
    }

    /// Whether the review screen is currently in search mode.
    pub fn is_search_mode(&self) -> bool {
        self.search_mode
    }

    /// Build a DiffViewWidget for the file at the given index.
    fn build_diff_view_for_file(files: &[DiffFile], idx: usize) -> DiffViewWidget {
        if files.is_empty() || idx >= files.len() {
            DiffViewWidget::new(Vec::new())
        } else {
            let lines: Vec<(crate::core::LineKind, String)> = files[idx]
                .hunks
                .iter()
                .flat_map(|h| h.lines.iter())
                .map(|l| (l.kind.clone(), l.content.clone()))
                .collect();
            DiffViewWidget::new(lines)
        }
    }

    /// Rebuild the diff view if the file tree selection changed.
    fn sync_diff_view(&mut self) {
        if let Some(sel) = self.file_tree.selected() {
            // Map filtered selection back to original index if search is active
            let original_idx = if let Some(ref indices) = self.filtered_indices {
                if sel < indices.len() {
                    indices[sel]
                } else {
                    return;
                }
            } else {
                sel
            };

            if original_idx != self.current_file_idx {
                self.current_file_idx = original_idx;
                self.diff_view = Self::build_diff_view_for_file(&self.files, original_idx);
            }
        }
    }

    /// Apply the current search filter to the file tree.
    fn apply_search_filter(&mut self) {
        if self.search_query.is_empty() {
            // Restore full list
            self.restore_full_file_tree();
            return;
        }

        let query_lower = self.search_query.to_lowercase();
        let mut filtered_items = Vec::new();
        let mut indices = Vec::new();

        for (i, file) in self.files.iter().enumerate() {
            if file.path.to_lowercase().contains(&query_lower) {
                filtered_items.push(FileTreeItem {
                    status: file.status.as_char().to_string(),
                    path: file.path.clone(),
                    priority: file.priority.clone(),
                    category: file.category.clone(),
                });
                indices.push(i);
            }
        }

        // Rebuild skipped set for filtered view
        let mut filtered_skipped = HashSet::new();
        for (filtered_idx, &original_idx) in indices.iter().enumerate() {
            if self.skipped_files.contains(&original_idx) {
                filtered_skipped.insert(filtered_idx);
            }
        }

        self.file_tree = FileTreeWidget::new(filtered_items);
        self.file_tree.skipped = filtered_skipped;
        self.filtered_indices = Some(indices);
    }

    /// Restore the full (unfiltered) file tree.
    fn restore_full_file_tree(&mut self) {
        let file_tree_items: Vec<FileTreeItem> = self
            .files
            .iter()
            .map(|f| FileTreeItem {
                status: f.status.as_char().to_string(),
                path: f.path.clone(),
                priority: f.priority.clone(),
                category: f.category.clone(),
            })
            .collect();

        self.file_tree = FileTreeWidget::new(file_tree_items);
        self.file_tree.skipped = self.skipped_files.clone();
        self.filtered_indices = None;
    }
}

impl Screen for ReviewScreen {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        // Search mode: intercept keys for the search input
        if self.search_mode {
            match key.code {
                KeyCode::Esc => {
                    // Exit search mode, restore full list
                    self.search_mode = false;
                    self.search_query.clear();
                    self.search_input.reset();
                    self.restore_full_file_tree();
                    return Action::None;
                }
                KeyCode::Enter => {
                    // Exit search mode, keep filtered selection
                    self.search_mode = false;
                    // Sync the diff view to current selection
                    self.sync_diff_view();
                    return Action::SelectFile;
                }
                _ => {
                    // Forward key to the input widget
                    self.search_input
                        .handle_event(&crossterm::event::Event::Key(key));
                    self.search_query = self.search_input.value().to_string();
                    self.apply_search_filter();
                    return Action::None;
                }
            }
        }

        match key.code {
            KeyCode::Tab => {
                self.active_pane = match self.active_pane {
                    Pane::FileTree => Pane::DiffView,
                    Pane::DiffView => Pane::FileTree,
                };
                Action::SwitchPane
            }
            KeyCode::Char('j') | KeyCode::Down => {
                match self.active_pane {
                    Pane::FileTree => {
                        self.file_tree.next();
                        self.sync_diff_view();
                    }
                    Pane::DiffView => self.diff_view.scroll_down(),
                }
                Action::NavigateDown
            }
            KeyCode::Char('k') | KeyCode::Up => {
                match self.active_pane {
                    Pane::FileTree => {
                        self.file_tree.previous();
                        self.sync_diff_view();
                    }
                    Pane::DiffView => self.diff_view.scroll_up(),
                }
                Action::NavigateUp
            }
            KeyCode::Char('s') => {
                if self.active_pane == Pane::FileTree {
                    if let Some(sel) = self.file_tree.selected() {
                        // Map to original index if filtered
                        let original_idx = if let Some(ref indices) = self.filtered_indices {
                            if sel < indices.len() {
                                indices[sel]
                            } else {
                                return Action::None;
                            }
                        } else {
                            sel
                        };

                        // Toggle skip
                        if self.skipped_files.contains(&original_idx) {
                            self.skipped_files.remove(&original_idx);
                            self.file_tree.skipped.remove(&sel);
                        } else {
                            self.skipped_files.insert(original_idx);
                            self.file_tree.skipped.insert(sel);
                        }
                        Action::SkipFile
                    } else {
                        Action::None
                    }
                } else {
                    Action::None
                }
            }
            KeyCode::Enter => {
                if self.active_pane == Pane::FileTree {
                    self.sync_diff_view();
                    self.active_pane = Pane::DiffView;
                    Action::SelectFile
                } else {
                    Action::None
                }
            }
            KeyCode::Char('/') => {
                if self.active_pane == Pane::FileTree {
                    self.search_mode = true;
                    self.search_input.reset();
                    self.search_query.clear();
                    Action::StartSearch
                } else {
                    Action::None
                }
            }
            _ => Action::None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        // Layout: file tree (30%) | diff view (70%), with status bar at bottom
        // If search mode is active, add a search input line above the file tree
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        let content_area = main_layout[0];
        let status_area = main_layout[1];

        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(content_area);

        // File tree pane (with optional search input)
        let file_tree_focused = self.active_pane == Pane::FileTree;

        if self.search_mode {
            let file_tree_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .split(panes[0]);

            // Search input
            let search_block = Block::default()
                .title(" Search ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.primary));

            let search_text = Paragraph::new(Line::from(self.search_input.value()))
                .block(search_block)
                .style(Style::default().fg(theme.fg));
            frame.render_widget(search_text, file_tree_layout[0]);

            // Set cursor position in search input
            let cursor_x = file_tree_layout[0].x + self.search_input.visual_cursor() as u16 + 1;
            let cursor_y = file_tree_layout[0].y + 1;
            frame.set_cursor_position(Position::new(cursor_x, cursor_y));

            // File tree below search
            self.file_tree
                .render(frame, file_tree_layout[1], theme, file_tree_focused);
        } else {
            self.file_tree
                .render(frame, panes[0], theme, file_tree_focused);
        }

        // Diff view pane
        let diff_view_focused = self.active_pane == Pane::DiffView;
        self.diff_view
            .render(frame, panes[1], theme, diff_view_focused);

        // Status bar
        let bindings = vec![
            ("q", "quit"),
            ("?", "help"),
            ("Tab", "switch pane"),
            ("j/k", "navigate"),
            ("s", "skip"),
            ("Enter", "select"),
            ("/", "search"),
        ];
        StatusBarWidget::new(&bindings).render(frame, status_area, theme);
    }
}
