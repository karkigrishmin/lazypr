use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::core::LineKind;
use crate::tui::theme::Theme;

/// A single line in the diff view with line number metadata.
#[derive(Debug, Clone)]
pub struct DiffViewLine {
    /// What kind of diff line this is.
    pub kind: LineKind,
    /// The textual content of the line.
    pub content: String,
    /// Line number in the old (base) file, if applicable.
    pub old_line_no: Option<u32>,
    /// Line number in the new (head) file, if applicable.
    pub new_line_no: Option<u32>,
}

/// Holds syntect state for syntax highlighting.
struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme: syntect::highlighting::Theme,
}

impl SyntaxHighlighter {
    fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-ocean.dark"].clone();
        Self { syntax_set, theme }
    }

    /// Highlight a line of code, returning a vec of ratatui Spans.
    /// The `extension` is used to find the appropriate syntax definition.
    fn highlight_line(&self, content: &str, extension: &str) -> Vec<Span<'static>> {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension(extension)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut h = syntect::easy::HighlightLines::new(syntax, &self.theme);
        match h.highlight_line(content, &self.syntax_set) {
            Ok(ranges) => ranges
                .into_iter()
                .map(|(style, text)| {
                    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                    Span::styled(text.to_string(), Style::default().fg(fg))
                })
                .collect(),
            Err(_) => vec![Span::raw(content.to_string())],
        }
    }
}

/// Cached syntax-highlighted output for lines in the diff view.
struct HighlightCache {
    /// File extension the cache was built for (for invalidation).
    _file_extension: String,
    /// Pre-highlighted spans for each line (by index).
    highlighted_lines: Vec<Vec<Span<'static>>>,
}

/// Renders diff lines with color coding, line numbers, syntax highlighting, and scrolling.
pub struct DiffViewWidget {
    /// Lines with rich metadata.
    lines: Vec<DiffViewLine>,
    /// Current scroll offset.
    scroll_offset: usize,
    /// Syntax highlighter instance (lazy-initialized).
    highlighter: SyntaxHighlighter,
    /// Cached highlighted lines.
    highlight_cache: Option<HighlightCache>,
    /// Current file extension for syntax detection.
    file_extension: String,
}

impl DiffViewWidget {
    /// Create a new diff view widget from a list of diff lines.
    pub fn new(lines: Vec<DiffViewLine>, file_extension: String) -> Self {
        let highlighter = SyntaxHighlighter::new();
        let mut widget = Self {
            lines,
            scroll_offset: 0,
            highlighter,
            highlight_cache: None,
            file_extension,
        };
        widget.rebuild_highlight_cache();
        widget
    }

    /// Rebuild the syntax highlight cache for the current lines/extension.
    fn rebuild_highlight_cache(&mut self) {
        let highlighted_lines: Vec<Vec<Span<'static>>> = self
            .lines
            .iter()
            .map(|line| {
                self.highlighter
                    .highlight_line(&line.content, &self.file_extension)
            })
            .collect();

        self.highlight_cache = Some(HighlightCache {
            _file_extension: self.file_extension.clone(),
            highlighted_lines,
        });
    }

    /// Scroll the view down by one line.
    pub fn scroll_down(&mut self) {
        if self.scroll_offset < self.lines.len().saturating_sub(1) {
            self.scroll_offset += 1;
        }
    }

    /// Scroll the view up by one line.
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scroll down by half a page.
    pub fn scroll_half_page_down(&mut self, visible_height: usize) {
        let half = visible_height / 2;
        let max = self.lines.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + half).min(max);
    }

    /// Scroll up by half a page.
    pub fn scroll_half_page_up(&mut self, visible_height: usize) {
        let half = visible_height / 2;
        self.scroll_offset = self.scroll_offset.saturating_sub(half);
    }

    /// Scroll to the very top.
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    /// Scroll to the very bottom.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.lines.len().saturating_sub(1);
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

        let cached_lines = self.highlight_cache.as_ref().map(|c| &c.highlighted_lines);

        let styled_lines: Vec<Line> = self
            .lines
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(visible_height)
            .map(|(idx, line)| {
                let (prefix, diff_color, modifier) = match line.kind {
                    LineKind::Added => ("+", theme.success, Modifier::empty()),
                    LineKind::Removed => ("-", theme.error, Modifier::empty()),
                    LineKind::Context => (" ", theme.fg, Modifier::empty()),
                    LineKind::Moved => (">", theme.info, Modifier::empty()),
                    LineKind::MovedEdited => ("~", theme.info, Modifier::ITALIC),
                };

                // Format line numbers: {old:>4} {new:>4}
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
                    format!("{old_no} {new_no} "),
                    Style::default().fg(theme.muted),
                );

                // Prefix span in diff color (with modifier for MovedEdited)
                let prefix_span = Span::styled(
                    prefix.to_string(),
                    Style::default().fg(diff_color).add_modifier(modifier),
                );

                // Content: use syntax-highlighted spans if available
                let content_spans: Vec<Span> =
                    if let Some(cached) = cached_lines.and_then(|c| c.get(idx)) {
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

                Line::from(spans)
            })
            .collect();

        let paragraph = Paragraph::new(styled_lines).block(block);
        frame.render_widget(paragraph, area);
    }
}
