/// Blame popup widget (stub).
pub mod blame_popup;
/// Checklist popup widget for file-level review checklists.
pub mod checklist;
/// Diff view widget for rendering colored diff lines.
pub mod diff_view;
/// File tree widget for navigating changed files.
pub mod file_tree;
/// Hunk display widget (stub).
pub mod hunk;
/// Note editor popup widget for adding review notes.
pub mod note_editor;
/// Progress bar widget (stub).
pub mod progress_bar;
/// Status bar widget for displaying keybindings.
pub mod status_bar;

#[allow(unused_imports)]
pub use checklist::{ChecklistAction, ChecklistWidget};
pub use diff_view::DiffViewWidget;
pub use file_tree::{FileTreeItem, FileTreeWidget};
#[allow(unused_imports)]
pub use note_editor::{NoteEditorAction, NoteEditorWidget};
#[allow(unused_imports)]
pub use progress_bar::ProgressBarWidget;
pub use status_bar::StatusBarWidget;
