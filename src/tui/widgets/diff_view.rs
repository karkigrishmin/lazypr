use std::collections::HashSet;
use std::sync::LazyLock;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::core::{DiffLine, LineKind};
use crate::tui::theme::Theme;

/// Holds syntect state for syntax highlighting (loaded once, shared across all widgets).
struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme: syntect::highlighting::Theme,
}

static HIGHLIGHTER: LazyLock<SyntaxHighlighter> = LazyLock::new(|| {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();
    let theme = theme_set.themes["base16-ocean.dark"].clone();
    SyntaxHighlighter { syntax_set, theme }
});

/// Renders diff lines with color coding, line numbers, syntax highlighting, and scrolling.
pub struct DiffViewWidget {
    /// Lines with rich metadata.
    lines: Vec<DiffLine>,
    /// Current scroll offset.
    scroll_offset: usize,
    /// Cached highlighted spans for each line.
    highlighted_lines: Vec<Vec<Span<'static>>>,
    /// Cursor position (index into `lines` vec, not scroll-relative).
    cursor_line: usize,
    /// Line numbers that have notes attached.
    note_lines: HashSet<u32>,
}

/// Build syntax-highlighted spans for all lines, carrying parser state across lines.
fn highlight_lines(lines: &[DiffLine], extension: &str) -> Vec<Vec<Span<'static>>> {
    let hl = &*HIGHLIGHTER;
    let syntax = hl
        .syntax_set
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| hl.syntax_set.find_syntax_plain_text());

    let mut h = syntect::easy::HighlightLines::new(syntax, &hl.theme);

    lines
        .iter()
        .map(
            |line| match h.highlight_line(&line.content, &hl.syntax_set) {
                Ok(ranges) => ranges
                    .into_iter()
                    .map(|(style, text)| {
                        let fg =
                            Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                        Span::styled(text.to_string(), Style::default().fg(fg))
                    })
                    .collect(),
                Err(_) => vec![Span::raw(line.content.clone())],
            },
        )
        .collect()
}

impl DiffViewWidget {
    /// Create a new diff view widget from a list of diff lines.
    pub fn new(lines: Vec<DiffLine>, file_extension: String) -> Self {
        let highlighted_lines = highlight_lines(&lines, &file_extension);
        Self {
            lines,
            scroll_offset: 0,
            highlighted_lines,
            cursor_line: 0,
            note_lines: HashSet::new(),
        }
    }

    /// Move cursor down by one line (scroll_down delegates here).
    pub fn scroll_down(&mut self) {
        self.cursor_down();
    }

    /// Move cursor up by one line (scroll_up delegates here).
    pub fn scroll_up(&mut self) {
        self.cursor_up();
    }

    /// Move cursor down by one line.
    pub fn cursor_down(&mut self) {
        if self.cursor_line < self.lines.len().saturating_sub(1) {
            self.cursor_line += 1;
        }
    }

    /// Move cursor up by one line.
    pub fn cursor_up(&mut self) {
        self.cursor_line = self.cursor_line.saturating_sub(1);
    }

    /// Get the new_line_no at the cursor position (for attaching notes).
    pub fn cursor_line_no(&self) -> Option<u32> {
        self.lines.get(self.cursor_line).and_then(|l| l.new_line_no)
    }

    /// Set which line numbers have notes.
    pub fn set_note_lines(&mut self, lines: HashSet<u32>) {
        self.note_lines = lines;
    }

    /// Adjust scroll_offset to keep cursor_line visible within the given height.
    pub fn ensure_cursor_visible(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        if self.cursor_line < self.scroll_offset {
            self.scroll_offset = self.cursor_line;
        } else if self.cursor_line >= self.scroll_offset + visible_height {
            self.scroll_offset = self.cursor_line.saturating_sub(visible_height - 1);
        }
    }

    /// Scroll down by half a page.
    pub fn scroll_half_page_down(&mut self, visible_height: usize) {
        let half = visible_height / 2;
        let max = self.lines.len().saturating_sub(1);
        self.cursor_line = (self.cursor_line + half).min(max);
        self.scroll_offset = (self.scroll_offset + half).min(max);
    }

    /// Scroll up by half a page.
    pub fn scroll_half_page_up(&mut self, visible_height: usize) {
        let half = visible_height / 2;
        self.cursor_line = self.cursor_line.saturating_sub(half);
        self.scroll_offset = self.scroll_offset.saturating_sub(half);
    }

    /// Scroll to the very top.
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
        self.cursor_line = 0;
    }

    /// Scroll to the very bottom.
    pub fn scroll_to_bottom(&mut self) {
        let max = self.lines.len().saturating_sub(1);
        self.scroll_offset = max;
        self.cursor_line = max;
    }

    /// Return the visible height for the given area (inner area minus block borders).
    pub fn visible_height(&self, area: Rect) -> usize {
        let block = Block::default().title(" Diff ").borders(Borders::ALL);
        let inner = block.inner(area);
        inner.height as usize
    }

    /// Render the diff view into the given area.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, focused: bool) {
        let border_color = if focused { theme.primary } else { theme.border };

        let block = Block::default()
            .title(" Diff ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        let visible_height = inner.height as usize;

        // Auto-scroll to keep cursor visible (local calculation since render takes &self)
        let effective_scroll = if self.cursor_line < self.scroll_offset {
            self.cursor_line
        } else if visible_height > 0 && self.cursor_line >= self.scroll_offset + visible_height {
            self.cursor_line.saturating_sub(visible_height - 1)
        } else {
            self.scroll_offset
        };

        let styled_lines: Vec<Line> = self
            .lines
            .iter()
            .enumerate()
            .skip(effective_scroll)
            .take(visible_height)
            .map(|(idx, line)| {
                let (prefix, diff_color, modifier) = match line.kind {
                    LineKind::Added => ("+", theme.success, Modifier::empty()),
                    LineKind::Removed => ("-", theme.error, Modifier::empty()),
                    LineKind::Context => (" ", theme.fg, Modifier::empty()),
                    LineKind::Moved => (">", theme.info, Modifier::empty()),
                    LineKind::MovedEdited => ("~", theme.info, Modifier::ITALIC),
                };

                // Note indicator: show '*' after line numbers for lines with notes
                let note_indicator = if line
                    .new_line_no
                    .is_some_and(|n| self.note_lines.contains(&n))
                {
                    "*"
                } else {
                    " "
                };

                // Format line numbers: {old:>4} {new:>4}{note_indicator}
                let old_no = line
                    .old_line_no
                    .map(|n| format!("{n:>4}"))
                    .unwrap_or_else(|| "    ".to_string());
                let new_no = line
                    .new_line_no
                    .map(|n| format!("{n:>4}"))
                    .unwrap_or_else(|| "    ".to_string());

                // Gutter span (line numbers) in muted color
                let gutter = Span::styled(
                    format!("{old_no} {new_no}{note_indicator}"),
                    Style::default().fg(theme.muted),
                );

                // Prefix span in diff color (with modifier for MovedEdited)
                let prefix_span = Span::styled(
                    prefix.to_string(),
                    Style::default().fg(diff_color).add_modifier(modifier),
                );

                // Content: use syntax-highlighted spans if available
                let content_spans: Vec<Span> = if let Some(cached) = self.highlighted_lines.get(idx)
                {
                    cached.clone()
                } else {
                    vec![Span::styled(
                        line.content.clone(),
                        Style::default().fg(diff_color),
                    )]
                };

                let mut spans = Vec::with_capacity(2 + content_spans.len());
                spans.push(gutter);
                spans.push(prefix_span);
                spans.extend(content_spans);

                // Highlight cursor line
                if idx == self.cursor_line {
                    Line::from(spans).patch_style(Style::default().bg(theme.highlight))
                } else {
                    Line::from(spans)
                }
            })
            .collect();

        let paragraph = Paragraph::new(styled_lines).block(block);
        frame.render_widget(paragraph, area);
    }
}
