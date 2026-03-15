/// Blame popup widget (stub).
pub mod blame_popup;
/// Diff view widget for rendering colored diff lines.
pub mod diff_view;
/// File tree widget for navigating changed files.
pub mod file_tree;
/// Hunk display widget (stub).
pub mod hunk;
/// Note editor widget (stub).
pub mod note_editor;
/// Progress bar widget (stub).
pub mod progress_bar;
/// Status bar widget for displaying keybindings.
pub mod status_bar;

pub use diff_view::{DiffViewLine, DiffViewWidget};
pub use file_tree::{FileTreeItem, FileTreeWidget};
pub use status_bar::StatusBarWidget;
