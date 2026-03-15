use ratatui::style::Color;

/// Color theme for the TUI.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Theme {
    /// Background color.
    pub bg: Color,
    /// Foreground (default text) color.
    pub fg: Color,
    /// Primary accent color.
    pub primary: Color,
    /// Secondary accent color.
    pub secondary: Color,
    /// Success color (green) -- used for added lines.
    pub success: Color,
    /// Error color (red) -- used for removed lines.
    pub error: Color,
    /// Warning color (yellow) -- used for scan priority.
    pub warning: Color,
    /// Info color (cyan) -- used for moved lines.
    pub info: Color,
    /// Muted / dim text color.
    pub muted: Color,
    /// Highlighted (selected) item background.
    pub highlight: Color,
    /// Border color.
    pub border: Color,
    /// Status bar background.
    pub status_bg: Color,
    /// Status bar foreground.
    pub status_fg: Color,
}

impl Theme {
    /// Dark theme suitable for terminals with dark backgrounds.
    pub fn dark() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::White,
            primary: Color::Cyan,
            secondary: Color::Blue,
            success: Color::Green,
            error: Color::Red,
            warning: Color::Yellow,
            info: Color::Cyan,
            muted: Color::DarkGray,
            highlight: Color::Rgb(40, 40, 60),
            border: Color::DarkGray,
            status_bg: Color::Rgb(30, 30, 50),
            status_fg: Color::White,
        }
    }

    /// Light theme suitable for terminals with light backgrounds.
    #[allow(dead_code)]
    pub fn light() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::Black,
            primary: Color::Blue,
            secondary: Color::DarkGray,
            success: Color::Rgb(0, 128, 0),
            error: Color::Rgb(180, 0, 0),
            warning: Color::Rgb(180, 120, 0),
            info: Color::Blue,
            muted: Color::Gray,
            highlight: Color::Rgb(220, 220, 240),
            border: Color::Gray,
            status_bg: Color::Rgb(230, 230, 240),
            status_fg: Color::Black,
        }
    }

    /// Detect terminal background and choose an appropriate theme.
    ///
    /// Terminal background detection is unreliable, so this defaults to dark.
    #[allow(dead_code)]
    pub fn detect() -> Self {
        Self::dark()
    }
}
