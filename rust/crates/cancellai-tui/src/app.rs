//! Navigation state machine (E09-S01). Pure and I/O-free: [`App::handle_key`] takes a
//! `crossterm` key event and returns the next state, so it is unit-testable without a real
//! terminal - `main.rs` is the only place that actually reads a `crossterm` event stream.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// One destination in the shell. `Atlas` (E09-S02) and `Explain` (E09-S03) render real,
/// engine-derived content when `data::EngineData` carries it; `Plan` remains a placeholder
/// pending E09-S04. Nothing this story displays for a still-stubbed screen is a fabricated
/// action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Atlas,
    Explain,
    Plan,
}

/// Every screen, in the fixed order `Tab`/number-key navigation cycles through. One place to
/// extend when a future story adds a screen, rather than scattering the ordering across the
/// key reducer and the nav list widget.
pub const SCREENS: [Screen; 4] = [Screen::Home, Screen::Atlas, Screen::Explain, Screen::Plan];

impl Screen {
    pub fn title(self) -> &'static str {
        match self {
            Screen::Home => "Home",
            Screen::Atlas => "Atlas",
            Screen::Explain => "Explain",
            Screen::Plan => "Plan",
        }
    }

    fn index(self) -> usize {
        SCREENS
            .iter()
            .position(|screen| *screen == self)
            .expect("Screen is always in SCREENS")
    }
}

/// One entry in the footer's keybinding hints and the `?` help overlay - a single source so a
/// future story extends bindings in one place instead of duplicating literal strings.
pub struct KeyBinding {
    pub keys: &'static str,
    pub description: &'static str,
}

pub const KEY_BINDINGS: &[KeyBinding] = &[
    KeyBinding {
        keys: "Tab / Shift+Tab",
        description: "next / previous screen",
    },
    KeyBinding {
        keys: "1-4",
        description: "jump to screen",
    },
    KeyBinding {
        keys: "Up / Down",
        description: "select artifact (Explain screen)",
    },
    KeyBinding {
        keys: "?",
        description: "toggle this help",
    },
    KeyBinding {
        keys: "q / Esc",
        description: "quit",
    },
];

/// The whole shell's state. `should_quit` is checked by `main.rs`'s event loop after every
/// [`App::handle_key`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct App {
    pub screen: Screen,
    pub show_help: bool,
    pub should_quit: bool,
    /// Which artifact is highlighted on the Explain screen (E09-S03). Deliberately not bounded
    /// against the real explain-list length here - `App` has no knowledge of engine data by
    /// design (see `lib.rs`'s AC1 doc) - `ui::draw_explain_content` reduces it modulo the
    /// actual list length at render time, so an unbounded counter still produces correct
    /// wraparound selection without coupling this pure state machine to `data::EngineData`.
    pub explain_selected: usize,
}

impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::Home,
            show_help: false,
            should_quit: false,
            explain_selected: 0,
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance state for one key event. Pure: no I/O, no global state, so every branch is
    /// directly unit-testable.
    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = !self.show_help,
            KeyCode::Tab => self.go_to(self.next_screen()),
            KeyCode::BackTab => self.go_to(self.previous_screen()),
            KeyCode::Down => self.explain_selected = self.explain_selected.saturating_add(1),
            KeyCode::Up => self.explain_selected = self.explain_selected.saturating_sub(1),
            KeyCode::Char(digit @ '1'..='4') => {
                let index = digit as usize - '1' as usize;
                self.go_to(SCREENS[index]);
            }
            // crossterm reports Shift+Tab as BackTab on most backends, but some terminals
            // instead deliver Tab with the Shift modifier set - handle both rather than
            // silently dropping the reverse-navigation case on those terminals.
            KeyCode::Char('\t') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.go_to(self.previous_screen());
            }
            _ => {}
        }
    }

    fn go_to(&mut self, screen: Screen) {
        self.screen = screen;
    }

    fn next_screen(&self) -> Screen {
        SCREENS[(self.screen.index() + 1) % SCREENS.len()]
    }

    fn previous_screen(&self) -> Screen {
        SCREENS[(self.screen.index() + SCREENS.len() - 1) % SCREENS.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn starts_on_home_with_no_quit_or_help() {
        let app = App::new();
        assert_eq!(app.screen, Screen::Home);
        assert!(!app.show_help);
        assert!(!app.should_quit);
    }

    #[test]
    fn tab_cycles_forward_and_wraps() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.screen, Screen::Atlas);
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.screen, Screen::Plan);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(
            app.screen,
            Screen::Home,
            "must wrap back to the first screen"
        );
    }

    #[test]
    fn backtab_cycles_backward_and_wraps() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::BackTab));
        assert_eq!(
            app.screen,
            Screen::Plan,
            "must wrap to the last screen from Home"
        );
    }

    #[test]
    fn shift_tab_delivered_as_char_tab_also_navigates_backward() {
        let mut app = App::new();
        app.handle_key(KeyEvent::new(KeyCode::Char('\t'), KeyModifiers::SHIFT));
        assert_eq!(app.screen, Screen::Plan);
    }

    #[test]
    fn number_keys_jump_directly_to_a_screen() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char('3')));
        assert_eq!(app.screen, Screen::Explain);
    }

    #[test]
    fn question_mark_toggles_help() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char('?')));
        assert!(app.show_help);
        app.handle_key(key(KeyCode::Char('?')));
        assert!(!app.show_help);
    }

    #[test]
    fn q_and_esc_both_quit() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.should_quit);

        let mut app = App::new();
        app.handle_key(key(KeyCode::Esc));
        assert!(app.should_quit);
    }

    #[test]
    fn an_unbound_key_changes_nothing() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char('z')));
        assert_eq!(app, App::new());
    }

    #[test]
    fn down_advances_and_up_retreats_the_explain_selection() {
        let mut app = App::new();
        assert_eq!(app.explain_selected, 0);
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.explain_selected, 2);
        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.explain_selected, 1);
    }

    #[test]
    fn up_from_zero_saturates_rather_than_underflowing() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.explain_selected, 0);
    }
}
