use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use crate::core::differ::interdiff::InterDiffResult;
use crate::core::types::ChecklistItem;
use crate::core::{DiffFile, DiffResult, ReviewNote};
use crate::tui::theme::Theme;
use crate::tui::widgets::{
    ChecklistAction, ChecklistWidget, DiffViewWidget, FileTreeItem, FileTreeWidget,
    NoteEditorAction, NoteEditorWidget, ProgressBarWidget, StatusBarWidget,
};

use super::{Action, Screen};

/// Which pane is currently focused in the review screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    /// The left file list pane.
    FileTree,
    /// The right diff view pane.
    DiffView,
}

/// Context passed to the review screen for session state.
pub struct ReviewContext {
    /// Notes from the current review session.
    pub notes: Vec<ReviewNote>,
    /// Inter-diff result (comparing current diff with previous review round).
    pub interdiff: Option<InterDiffResult>,
    /// Paths of files already marked as viewed.
    pub viewed_files: Vec<String>,
    /// Repository root path (for saving notes to disk).
    pub repo_root: PathBuf,
    /// Branch name (for saving notes to disk).
    pub branch_name: String,
}

impl Default for ReviewContext {
    fn default() -> Self {
        Self {
            notes: Vec::new(),
            interdiff: None,
            viewed_files: Vec::new(),
            repo_root: PathBuf::new(),
            branch_name: String::new(),
        }
    }
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
    /// Last rendered diff pane area (for half-page scroll calculations).
    last_diff_area: Cell<Rect>,
    /// Set of original file indices that have been marked as viewed.
    viewed_files: HashSet<usize>,
    /// Review notes for the current session.
    notes: Vec<ReviewNote>,
    /// Active note editor popup (None when not editing).
    note_editor: Option<NoteEditorWidget>,
    /// Whether inter-diff mode is active (showing only changed files).
    interdiff_mode: bool,
    /// Inter-diff result for comparing review rounds.
    interdiff_result: Option<InterDiffResult>,
    /// Full (unfiltered) file list for restoring from inter-diff filter.
    full_files: Vec<DiffFile>,
    /// Repository root path.
    repo_root: PathBuf,
    /// Branch name.
    branch_name: String,
    /// Active checklist popup (None when not showing).
    checklist_widget: Option<ChecklistWidget>,
    /// Checklist state: file path -> checklist items.
    checklist_state: HashMap<String, Vec<ChecklistItem>>,
}

impl ReviewScreen {
    /// Create a new review screen from a diff result and review context.
    pub fn new(diff: &DiffResult, ctx: ReviewContext) -> Self {
        let file_tree_items: Vec<FileTreeItem> = diff
            .files
            .iter()
            .map(|f| FileTreeItem {
                status: f.status.as_char().to_string(),
                path: f.path.clone(),
                priority: f.priority.clone(),
                category: f.category.clone(),
                semantic_summary: None,
            })
            .collect();

        let file_tree = FileTreeWidget::new(file_tree_items);

        let diff_view = Self::build_diff_view_for_file(&diff.files, 0);

        // Map viewed file paths to indices
        let viewed_files: HashSet<usize> = diff
            .files
            .iter()
            .enumerate()
            .filter(|(_, f)| ctx.viewed_files.contains(&f.path))
            .map(|(i, _)| i)
            .collect();

        let mut screen = Self {
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
            last_diff_area: Cell::new(Rect::default()),
            viewed_files,
            notes: ctx.notes,
            note_editor: None,
            interdiff_mode: false,
            interdiff_result: ctx.interdiff,
            full_files: diff.files.clone(),
            repo_root: ctx.repo_root,
            branch_name: ctx.branch_name,
            checklist_widget: None,
            checklist_state: HashMap::new(),
        };

        // Set viewed indicators on file tree
        screen.file_tree.viewed = screen.viewed_files.clone();
        // Set note indicators for the initial file
        screen.update_note_indicators();

        screen
    }

    /// Whether the review screen is currently in search mode.
    pub fn is_search_mode(&self) -> bool {
        self.search_mode
    }

    /// Whether the note editor popup is currently active.
    pub fn is_note_editor_active(&self) -> bool {
        self.note_editor.is_some()
    }

    /// Whether the checklist popup is currently active.
    pub fn is_checklist_active(&self) -> bool {
        self.checklist_widget.is_some()
    }

    /// Get the set of viewed file indices (used for session lifecycle persistence).
    pub fn viewed_files(&self) -> &HashSet<usize> {
        &self.viewed_files
    }

    /// Get the review notes (used for session lifecycle persistence).
    pub fn notes(&self) -> &[ReviewNote] {
        &self.notes
    }

    /// Get the current file list (used for session lifecycle persistence).
    pub fn files(&self) -> &[DiffFile] {
        &self.files
    }

    /// Build a DiffViewWidget for the file at the given index.
    fn build_diff_view_for_file(files: &[DiffFile], idx: usize) -> DiffViewWidget {
        if files.is_empty() || idx >= files.len() {
            DiffViewWidget::new(Vec::new(), String::new())
        } else {
            let file = &files[idx];
            let lines = file.hunks.iter().flat_map(|h| h.lines.clone()).collect();

            let extension = std::path::Path::new(&file.path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string();

            DiffViewWidget::new(lines, extension)
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
                self.update_note_indicators();
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
                    semantic_summary: None,
                });
                indices.push(i);
            }
        }

        // Rebuild skipped set for filtered view
        let mut filtered_skipped = HashSet::new();
        let mut filtered_viewed = HashSet::new();
        for (filtered_idx, &original_idx) in indices.iter().enumerate() {
            if self.skipped_files.contains(&original_idx) {
                filtered_skipped.insert(filtered_idx);
            }
            if self.viewed_files.contains(&original_idx) {
                filtered_viewed.insert(filtered_idx);
            }
        }

        self.file_tree = FileTreeWidget::new(filtered_items);
        self.file_tree.skipped = filtered_skipped;
        self.file_tree.viewed = filtered_viewed;
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
                semantic_summary: None,
            })
            .collect();

        self.file_tree = FileTreeWidget::new(file_tree_items);
        self.file_tree.skipped = self.skipped_files.clone();
        self.file_tree.viewed = self.viewed_files.clone();
        self.filtered_indices = None;
    }

    /// Update the note line indicators on the diff view for the current file.
    fn update_note_indicators(&mut self) {
        if self.files.is_empty() || self.current_file_idx >= self.files.len() {
            self.diff_view.set_note_lines(HashSet::new());
            return;
        }

        let file_path = &self.files[self.current_file_idx].path;
        let note_lines: HashSet<u32> = crate::state::notes::notes_for_file(&self.notes, file_path)
            .iter()
            .filter_map(|n| n.line)
            .collect();
        self.diff_view.set_note_lines(note_lines);
    }

    /// Filter the file list to show only new + modified files from inter-diff.
    fn apply_interdiff_filter(&mut self) {
        if let Some(ref interdiff) = self.interdiff_result {
            let changed_paths: HashSet<&str> = interdiff
                .new_files
                .iter()
                .chain(interdiff.modified_files.iter())
                .map(|s| s.as_str())
                .collect();

            let filtered_files: Vec<DiffFile> = self
                .full_files
                .iter()
                .filter(|f| changed_paths.contains(f.path.as_str()))
                .cloned()
                .collect();

            self.files = filtered_files;
            self.current_file_idx = 0;
            self.restore_full_file_tree();
            self.diff_view = Self::build_diff_view_for_file(&self.files, 0);
            self.update_note_indicators();
        }
    }

    /// Restore the full file list from the saved copy.
    fn restore_from_interdiff(&mut self) {
        self.files = self.full_files.clone();
        self.current_file_idx = 0;
        self.restore_full_file_tree();
        self.diff_view = Self::build_diff_view_for_file(&self.files, 0);
        self.update_note_indicators();
    }

    /// Map a file tree selection index to the original file index.
    fn map_to_original_idx(&self, sel: usize) -> Option<usize> {
        if let Some(ref indices) = self.filtered_indices {
            indices.get(sel).copied()
        } else {
            Some(sel)
        }
    }
}

impl Screen for ReviewScreen {
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        // Note editor modal: intercept all keys when active
        if let Some(ref mut editor) = self.note_editor {
            match editor.handle_key(key) {
                NoteEditorAction::Save(content) => {
                    if !content.trim().is_empty() {
                        let file_path = self.files[self.current_file_idx].path.clone();
                        let line_no = self.diff_view.cursor_line_no();
                        crate::state::notes::add_note(
                            &mut self.notes,
                            &file_path,
                            line_no,
                            &content,
                        );
                        // Save notes to disk (best-effort)
                        let _ = crate::state::notes::save_notes(
                            &self.repo_root,
                            &self.branch_name,
                            &self.notes,
                        );
                        self.update_note_indicators();
                    }
                    self.note_editor = None;
                    return Action::None;
                }
                NoteEditorAction::Cancel => {
                    self.note_editor = None;
                    return Action::None;
                }
                NoteEditorAction::Continue => return Action::None,
            }
        }

        // Checklist modal: intercept all keys when active
        if self.checklist_widget.is_some() {
            // Pre-compute the original file index before taking the mutable borrow
            let original_idx = self.file_tree.selected().and_then(|sel| {
                let idx = self.map_to_original_idx(sel).unwrap_or(sel);
                if idx < self.files.len() {
                    Some(idx)
                } else {
                    None
                }
            });
            let file_path = original_idx.map(|idx| self.files[idx].path.clone());

            let checklist = self.checklist_widget.as_mut().unwrap();
            match checklist.handle_key(key) {
                ChecklistAction::Toggle(idx) => {
                    checklist.toggle(idx);
                    // Update checklist_state from the widget's items
                    if let Some(path) = file_path {
                        self.checklist_state
                            .insert(path, checklist.items().to_vec());
                    }
                    return Action::ToggleChecklist;
                }
                ChecklistAction::Close => {
                    self.checklist_widget = None;
                    return Action::None;
                }
                ChecklistAction::Continue => return Action::None,
            }
        }

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
                    Pane::DiffView => {
                        self.diff_view.scroll_down();
                        let vh = self.diff_view.visible_height(self.last_diff_area.get());
                        self.diff_view.ensure_cursor_visible(vh);
                    }
                }
                Action::NavigateDown
            }
            KeyCode::Char('k') | KeyCode::Up => {
                match self.active_pane {
                    Pane::FileTree => {
                        self.file_tree.previous();
                        self.sync_diff_view();
                    }
                    Pane::DiffView => {
                        self.diff_view.scroll_up();
                        let vh = self.diff_view.visible_height(self.last_diff_area.get());
                        self.diff_view.ensure_cursor_visible(vh);
                    }
                }
                Action::NavigateUp
            }
            KeyCode::Char('s') => {
                if self.active_pane == Pane::FileTree {
                    if let Some(sel) = self.file_tree.selected() {
                        // Map to original index if filtered
                        let original_idx = match self.map_to_original_idx(sel) {
                            Some(idx) => idx,
                            None => return Action::None,
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
            KeyCode::Char('v') => {
                if self.active_pane == Pane::FileTree {
                    if let Some(sel) = self.file_tree.selected() {
                        let original_idx = match self.map_to_original_idx(sel) {
                            Some(idx) => idx,
                            None => return Action::None,
                        };

                        // Toggle viewed
                        if self.viewed_files.contains(&original_idx) {
                            self.viewed_files.remove(&original_idx);
                            self.file_tree.viewed.remove(&sel);
                        } else {
                            self.viewed_files.insert(original_idx);
                            self.file_tree.viewed.insert(sel);
                        }
                        Action::MarkViewed
                    } else {
                        Action::None
                    }
                } else {
                    Action::None
                }
            }
            KeyCode::Char('n') => {
                if self.active_pane == Pane::DiffView && !self.files.is_empty() {
                    let file_path = self.files[self.current_file_idx].path.clone();
                    let line_no = self.diff_view.cursor_line_no();
                    self.note_editor = Some(NoteEditorWidget::new(file_path, line_no));
                    Action::OpenNoteEditor
                } else {
                    Action::None
                }
            }
            KeyCode::Char('i') => {
                if self.interdiff_result.is_some() {
                    self.interdiff_mode = !self.interdiff_mode;
                    if self.interdiff_mode {
                        self.apply_interdiff_filter();
                    } else {
                        self.restore_from_interdiff();
                    }
                    Action::ToggleInterDiff
                } else {
                    Action::None
                }
            }
            KeyCode::Char('c') => {
                if !self.files.is_empty() {
                    let original_idx = if let Some(sel) = self.file_tree.selected() {
                        self.map_to_original_idx(sel).unwrap_or(sel)
                    } else {
                        self.current_file_idx
                    };
                    if original_idx < self.files.len() {
                        let file_path = self.files[original_idx].path.clone();
                        if let Some(items) = self.checklist_state.get(&file_path) {
                            if !items.is_empty() {
                                self.checklist_widget =
                                    Some(ChecklistWidget::new(file_path, items.clone()));
                                return Action::ToggleChecklist;
                            }
                        }
                    }
                }
                Action::None
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
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.active_pane == Pane::DiffView {
                    let vh = self.diff_view.visible_height(self.last_diff_area.get());
                    self.diff_view.scroll_half_page_down(vh);
                    Action::ScrollDown
                } else {
                    Action::None
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.active_pane == Pane::DiffView {
                    let vh = self.diff_view.visible_height(self.last_diff_area.get());
                    self.diff_view.scroll_half_page_up(vh);
                    Action::ScrollUp
                } else {
                    Action::None
                }
            }
            KeyCode::Char('g') => {
                if self.active_pane == Pane::DiffView {
                    self.diff_view.scroll_to_top();
                    Action::ScrollUp
                } else {
                    Action::None
                }
            }
            KeyCode::Char('G') => {
                if self.active_pane == Pane::DiffView {
                    self.diff_view.scroll_to_bottom();
                    Action::ScrollDown
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

        // File tree pane: progress bar (1 line) + optional search + file tree
        let file_tree_focused = self.active_pane == Pane::FileTree;

        if self.search_mode {
            let file_pane_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(3),
                    Constraint::Min(1),
                ])
                .split(panes[0]);

            // Progress bar
            let progress = ProgressBarWidget::new(self.viewed_files.len(), self.files.len());
            progress.render(frame, file_pane_layout[0], theme);

            // Search input
            let search_block = Block::default()
                .title(" Search ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.primary));

            let search_text = Paragraph::new(Line::from(self.search_input.value()))
                .block(search_block)
                .style(Style::default().fg(theme.fg));
            frame.render_widget(search_text, file_pane_layout[1]);

            // Set cursor position in search input
            let cursor_x = file_pane_layout[1].x + self.search_input.visual_cursor() as u16 + 1;
            let cursor_y = file_pane_layout[1].y + 1;
            frame.set_cursor_position(Position::new(cursor_x, cursor_y));

            // File tree below search
            self.file_tree
                .render(frame, file_pane_layout[2], theme, file_tree_focused);
        } else {
            let file_pane_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(panes[0]);

            // Progress bar
            let progress = ProgressBarWidget::new(self.viewed_files.len(), self.files.len());
            progress.render(frame, file_pane_layout[0], theme);

            // File tree
            self.file_tree
                .render(frame, file_pane_layout[1], theme, file_tree_focused);
        }

        // Diff view pane
        let diff_view_focused = self.active_pane == Pane::DiffView;
        self.last_diff_area.set(panes[1]);
        self.diff_view
            .render(frame, panes[1], theme, diff_view_focused);

        // Status bar
        let bindings = vec![
            ("q", "quit"),
            ("?", "help"),
            ("Tab", "switch pane"),
            ("j/k", "navigate"),
            ("C-d/C-u", "half-page"),
            ("g/G", "top/bottom"),
            ("s", "skip"),
            ("v", "viewed"),
            ("n", "note"),
            ("c", "checklist"),
            ("i", "interdiff"),
            ("Enter", "select"),
            ("/", "search"),
        ];
        StatusBarWidget::new(&bindings).render(frame, status_area, theme);

        // Render note editor overlay on top if active
        if let Some(ref editor) = self.note_editor {
            editor.render(frame, area, theme);
        }

        // Render checklist overlay on top if active
        if let Some(ref checklist) = self.checklist_widget {
            checklist.render(frame, area, theme);
        }
    }
}
