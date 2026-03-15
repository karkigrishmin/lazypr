use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;

use crate::core::DiffResult;
use crate::tui::theme::Theme;
use crate::tui::widgets::{DiffViewWidget, FileTreeWidget, StatusBarWidget};

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
}

impl ReviewScreen {
    /// Create a new review screen from a diff result.
    pub fn new(diff: &DiffResult) -> Self {
        let file_paths: Vec<(String, String)> = diff
            .files
            .iter()
            .map(|f| {
                let status_char = match f.status {
                    crate::core::FileStatus::Added => "A",
                    crate::core::FileStatus::Modified => "M",
                    crate::core::FileStatus::Deleted => "D",
                    crate::core::FileStatus::Renamed => "R",
                };
                (status_char.to_string(), f.path.clone())
            })
            .collect();

        let file_tree = FileTreeWidget::new(file_paths);

        let diff_view = if diff.files.is_empty() {
            DiffViewWidget::new(Vec::new())
        } else {
            let lines: Vec<(crate::core::LineKind, String)> = diff.files[0]
                .hunks
                .iter()
                .flat_map(|h| h.lines.iter())
                .map(|l| (l.kind.clone(), l.content.clone()))
                .collect();
            DiffViewWidget::new(lines)
        };

        Self {
            active_pane: Pane::FileTree,
            file_tree,
            diff_view,
        }
    }
}

impl Screen for ReviewScreen {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
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
                    Pane::FileTree => self.file_tree.next(),
                    Pane::DiffView => self.diff_view.scroll_down(),
                }
                Action::NavigateDown
            }
            KeyCode::Char('k') | KeyCode::Up => {
                match self.active_pane {
                    Pane::FileTree => self.file_tree.previous(),
                    Pane::DiffView => self.diff_view.scroll_up(),
                }
                Action::NavigateUp
            }
            _ => Action::None,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        // Layout: file tree (30%) | diff view (70%), with status bar at bottom
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

        // File tree pane
        let file_tree_focused = self.active_pane == Pane::FileTree;
        self.file_tree
            .render(frame, panes[0], theme, file_tree_focused);

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
        ];
        StatusBarWidget::new(&bindings).render(frame, status_area, theme);
    }
}
