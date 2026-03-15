use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;

use crate::core::DiffResult;
use crate::state::LazyprConfig;

use super::event::{Action, ActiveScreen};
use super::screens::{GhostScreen, HelpOverlay, InboxScreen, ReviewScreen, Screen, SplitScreen};
use super::theme::Theme;

/// Top-level TUI application state.
pub struct App {
    /// The currently active screen tab.
    pub active_screen: ActiveScreen,
    /// Whether the help overlay is currently shown.
    pub show_help: bool,
    /// The diff data being reviewed.
    pub diff: DiffResult,
    /// Application configuration.
    pub config: LazyprConfig,
    /// The active color theme.
    pub theme: Theme,
    review_screen: ReviewScreen,
    split_screen: SplitScreen,
    inbox_screen: InboxScreen,
    ghost_screen: GhostScreen,
    help_overlay: HelpOverlay,
}

impl App {
    /// Create a new application with the given diff data and configuration.
    pub fn new(diff: DiffResult, config: LazyprConfig) -> Self {
        let theme = Theme::dark();
        let review_screen = ReviewScreen::new(&diff);
        Self {
            active_screen: ActiveScreen::Review,
            show_help: false,
            diff,
            config,
            theme,
            review_screen,
            split_screen: SplitScreen::new(),
            inbox_screen: InboxScreen::new(),
            ghost_screen: GhostScreen::new(),
            help_overlay: HelpOverlay::new(),
        }
    }

    /// Process a key event and return the resulting action.
    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        // Global keys that work regardless of screen
        match key.code {
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Action::Quit;
            }
            KeyCode::Char('?') => {
                self.show_help = !self.show_help;
                return Action::ToggleHelp;
            }
            KeyCode::Char('1') => {
                self.active_screen = ActiveScreen::Review;
                return Action::SwitchScreen(ActiveScreen::Review);
            }
            KeyCode::Char('2') => {
                self.active_screen = ActiveScreen::Split;
                return Action::SwitchScreen(ActiveScreen::Split);
            }
            KeyCode::Char('3') => {
                self.active_screen = ActiveScreen::Inbox;
                return Action::SwitchScreen(ActiveScreen::Inbox);
            }
            KeyCode::Char('4') => {
                self.active_screen = ActiveScreen::Ghost;
                return Action::SwitchScreen(ActiveScreen::Ghost);
            }
            _ => {}
        }

        // If help is shown, any other key dismisses it
        if self.show_help {
            self.show_help = false;
            return Action::ToggleHelp;
        }

        // Delegate to active screen
        match self.active_screen {
            ActiveScreen::Review => self.review_screen.handle_key(key),
            ActiveScreen::Split => self.split_screen.handle_key(key),
            ActiveScreen::Inbox => self.inbox_screen.handle_key(key),
            ActiveScreen::Ghost => self.ghost_screen.handle_key(key),
        }
    }

    /// Render the current screen (and any overlays) to the frame.
    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        // Render active screen
        match self.active_screen {
            ActiveScreen::Review => self.review_screen.render(frame, area, &self.theme),
            ActiveScreen::Split => self.split_screen.render(frame, area, &self.theme),
            ActiveScreen::Inbox => self.inbox_screen.render(frame, area, &self.theme),
            ActiveScreen::Ghost => self.ghost_screen.render(frame, area, &self.theme),
        }

        // Render help overlay on top if shown
        if self.show_help {
            self.help_overlay.render(frame, area, &self.theme);
        }
    }
}
