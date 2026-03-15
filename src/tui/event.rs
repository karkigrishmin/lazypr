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
