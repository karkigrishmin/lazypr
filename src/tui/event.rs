/// Actions that can be taken in the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Exit the application.
    Quit,
    /// Switch to a different screen.
    SwitchScreen(ActiveScreen),
    /// Switch focus between panes.
    SwitchPane,
    /// Move selection up.
    NavigateUp,
    /// Move selection down.
    NavigateDown,
    /// Scroll content up.
    ScrollUp,
    /// Scroll content down.
    ScrollDown,
    /// Toggle the help overlay.
    ToggleHelp,
    /// Skip/mark current file as reviewed.
    SkipFile,
    /// Select the current file (Enter key).
    SelectFile,
    /// Start file search (/ key). Phase 1: simple substring filter on file paths.
    StartSearch,
    /// Mark current file as viewed.
    MarkViewed,
    /// Open the note editor popup.
    OpenNoteEditor,
    /// Toggle inter-diff mode (show only changes since last review).
    ToggleInterDiff,
    /// Toggle the checklist popup.
    ToggleChecklist,
    /// No action taken.
    None,
}

/// The currently active screen in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveScreen {
    /// Diff review screen (default).
    Review,
    /// Split plan screen.
    Split,
    /// PR inbox screen.
    Inbox,
    /// Ghost diff screen.
    Ghost,
}
