//! Navigation state machine (E09-S01). Pure and I/O-free: [`App::handle_key`] takes a
//! `crossterm` key event and returns the next state, so it is unit-testable without a real
//! terminal - `main.rs` is the only place that actually reads a `crossterm` event stream.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// One destination in the shell. `Atlas` (E09-S02), `Explain` (E09-S03), and `Plan` (E09-S04)
/// all render real, engine-derived content when `data::EngineData` carries it, and an explicit
/// "nothing loaded" state otherwise - never a fabricated action standing in for missing data.
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
        description: "select artifact (Explain / Plan screens)",
    },
    KeyBinding {
        keys: "c",
        description: "confirm plan (2 presses if irreversible)",
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

/// What the Plan screen's confirmation logic needs to know about the currently selected
/// artifact, computed by [`crate::data::EngineData::plan_context`]. Plain `bool`s only - `App`
/// stays free of `cancellai_policy`/`cancellai_model` types by design (`lib.rs`'s own doc), the
/// same reason `explain_selected` is reduced modulo the real list length only at render/context
/// time, never stored here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlanContext {
    /// The selected artifact's policy outcome is `Recommended` - there is something to confirm
    /// at all. `false` for `ObservationOnly`/`NotEvaluated`, or when nothing is loaded.
    pub can_confirm: bool,
    /// ...and its reversibility is `Irreversible` (AC2: "irreversible actions receive stronger
    /// confirmation than quarantine").
    pub requires_strong_confirmation: bool,
}

/// The whole shell's state. `should_quit` is checked by `main.rs`'s event loop after every
/// [`App::handle_key`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct App {
    pub screen: Screen,
    pub show_help: bool,
    pub should_quit: bool,
    /// Which artifact is highlighted on the Explain/Plan screens (E09-S03/E09-S04). Deliberately
    /// not bounded against the real explain-list length here - `App` has no knowledge of engine
    /// data by design (see `lib.rs`'s AC1 doc) - `ui::draw_explain_content`/`draw_plan_content`
    /// reduce it modulo the actual list length at render time, so an unbounded counter still
    /// produces correct wraparound selection without coupling this pure state machine to
    /// `data::EngineData`.
    pub explain_selected: usize,
    /// E09-S04: the currently selected artifact's irreversible action has received its first
    /// confirmation press and is waiting for the second (AC2's "stronger confirmation").
    pub plan_confirm_armed: bool,
    /// E09-S04: the currently selected artifact's plan has been fully confirmed - the TUI's own
    /// terminal review state. Never itself a mutation: see `data.rs`/`ui.rs`'s own docs for why
    /// nothing this crate does at this state actually executes anything.
    pub plan_confirmed: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::Home,
            show_help: false,
            should_quit: false,
            explain_selected: 0,
            plan_confirm_armed: false,
            plan_confirmed: false,
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance state for one key event. Pure: no I/O, no global state, so every branch is
    /// directly unit-testable. `plan` is the current Plan-screen confirmation context
    /// (`data::EngineData::plan_context`) - the only engine-derived fact this reducer ever sees,
    /// and only as plain `bool`s (`PlanContext`'s own doc).
    pub fn handle_key(&mut self, key: KeyEvent, plan: PlanContext) {
        // Any key but a second unmodified `c` cancels a pending irreversible-action
        // confirmation - one general rule rather than special-casing every other branch below.
        // Modifiers matter here: raw mode disables the terminal's own `ISIG` handling, so a
        // real terminal delivers Ctrl+C as `KeyCode::Char('c')` with `KeyModifiers::CONTROL`
        // rather than a signal - matching on the bare `KeyCode` alone would make the universal
        // "abort" keystroke complete an irreversible confirmation instead of cancelling it
        // (found during self-review, not the independent Codex review). `is_plain_c` is what both this disarm
        // rule and the confirm arm below key on, so the two can never drift apart.
        if self.plan_confirm_armed && !is_plain_c(key) {
            self.plan_confirm_armed = false;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = !self.show_help,
            KeyCode::Tab => {
                self.reset_plan_confirmation();
                self.go_to(self.next_screen());
            }
            KeyCode::BackTab => {
                self.reset_plan_confirmation();
                self.go_to(self.previous_screen());
            }
            KeyCode::Down => {
                self.reset_plan_confirmation();
                self.explain_selected = self.explain_selected.saturating_add(1);
            }
            KeyCode::Up => {
                self.reset_plan_confirmation();
                self.explain_selected = self.explain_selected.saturating_sub(1);
            }
            KeyCode::Char(digit @ '1'..='4') => {
                self.reset_plan_confirmation();
                let index = digit as usize - '1' as usize;
                self.go_to(SCREENS[index]);
            }
            // crossterm reports Shift+Tab as BackTab on most backends, but some terminals
            // instead deliver Tab with the Shift modifier set - handle both rather than
            // silently dropping the reverse-navigation case on those terminals.
            KeyCode::Char('\t') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.reset_plan_confirmation();
                self.go_to(self.previous_screen());
            }
            // AC2: one press confirms a non-irreversible recommendation; an irreversible one
            // needs a second press (the first only arms it - see the disarm rule above for how
            // any other key cancels that arming instead of letting it linger). Idempotent once
            // confirmed: a further `c` press is a no-op rather than re-arming, so repeatedly
            // pressing the same key never un-confirms and re-demands a second press.
            KeyCode::Char('c')
                if is_plain_c(key) && self.screen == Screen::Plan && plan.can_confirm =>
            {
                if self.plan_confirmed {
                    // already confirmed - nothing left for this key to do
                } else if plan.requires_strong_confirmation && !self.plan_confirm_armed {
                    self.plan_confirm_armed = true;
                } else {
                    self.plan_confirmed = true;
                    self.plan_confirm_armed = false;
                }
            }
            _ => {}
        }
    }

    fn reset_plan_confirmation(&mut self) {
        self.plan_confirm_armed = false;
        self.plan_confirmed = false;
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

/// A `c` press carrying no modifier at all - deliberately not just `key.code ==
/// KeyCode::Char('c')`, since that alone is also what a real terminal reports for Ctrl+C once
/// raw mode has disabled `ISIG` (see `handle_key`'s own comment on the arming/disarming rule
/// this backs).
fn is_plain_c(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.is_empty()
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
        app.handle_key(key(KeyCode::Tab), PlanContext::default());
        assert_eq!(app.screen, Screen::Atlas);
        app.handle_key(key(KeyCode::Tab), PlanContext::default());
        app.handle_key(key(KeyCode::Tab), PlanContext::default());
        assert_eq!(app.screen, Screen::Plan);
        app.handle_key(key(KeyCode::Tab), PlanContext::default());
        assert_eq!(
            app.screen,
            Screen::Home,
            "must wrap back to the first screen"
        );
    }

    #[test]
    fn backtab_cycles_backward_and_wraps() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::BackTab), PlanContext::default());
        assert_eq!(
            app.screen,
            Screen::Plan,
            "must wrap to the last screen from Home"
        );
    }

    #[test]
    fn shift_tab_delivered_as_char_tab_also_navigates_backward() {
        let mut app = App::new();
        app.handle_key(
            KeyEvent::new(KeyCode::Char('\t'), KeyModifiers::SHIFT),
            PlanContext::default(),
        );
        assert_eq!(app.screen, Screen::Plan);
    }

    #[test]
    fn number_keys_jump_directly_to_a_screen() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char('3')), PlanContext::default());
        assert_eq!(app.screen, Screen::Explain);
    }

    #[test]
    fn question_mark_toggles_help() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char('?')), PlanContext::default());
        assert!(app.show_help);
        app.handle_key(key(KeyCode::Char('?')), PlanContext::default());
        assert!(!app.show_help);
    }

    #[test]
    fn q_and_esc_both_quit() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char('q')), PlanContext::default());
        assert!(app.should_quit);

        let mut app = App::new();
        app.handle_key(key(KeyCode::Esc), PlanContext::default());
        assert!(app.should_quit);
    }

    #[test]
    fn an_unbound_key_changes_nothing() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char('z')), PlanContext::default());
        assert_eq!(app, App::new());
    }

    #[test]
    fn down_advances_and_up_retreats_the_explain_selection() {
        let mut app = App::new();
        assert_eq!(app.explain_selected, 0);
        app.handle_key(key(KeyCode::Down), PlanContext::default());
        app.handle_key(key(KeyCode::Down), PlanContext::default());
        assert_eq!(app.explain_selected, 2);
        app.handle_key(key(KeyCode::Up), PlanContext::default());
        assert_eq!(app.explain_selected, 1);
    }

    #[test]
    fn up_from_zero_saturates_rather_than_underflowing() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Up), PlanContext::default());
        assert_eq!(app.explain_selected, 0);
    }

    fn plan(can_confirm: bool, requires_strong_confirmation: bool) -> PlanContext {
        PlanContext {
            can_confirm,
            requires_strong_confirmation,
        }
    }

    fn on_plan_screen() -> App {
        let mut app = App::new();
        app.screen = Screen::Plan;
        app
    }

    #[test]
    fn a_single_c_press_confirms_a_non_irreversible_recommendation() {
        let mut app = on_plan_screen();
        app.handle_key(key(KeyCode::Char('c')), plan(true, false));
        assert!(
            app.plan_confirmed,
            "AC2: quarantine-tier needs only one press"
        );
        assert!(!app.plan_confirm_armed);
    }

    #[test]
    fn repeated_c_presses_after_confirmation_are_idempotent_not_a_re_arm() {
        let mut app = on_plan_screen();
        app.handle_key(key(KeyCode::Char('c')), plan(true, true));
        app.handle_key(key(KeyCode::Char('c')), plan(true, true));
        assert!(app.plan_confirmed);
        assert!(!app.plan_confirm_armed);
        // A third press must stay confirmed, never quietly un-confirm and re-arm.
        app.handle_key(key(KeyCode::Char('c')), plan(true, true));
        assert!(app.plan_confirmed);
        assert!(!app.plan_confirm_armed);
    }

    #[test]
    fn an_irreversible_recommendation_needs_two_c_presses() {
        let mut app = on_plan_screen();
        app.handle_key(key(KeyCode::Char('c')), plan(true, true));
        assert!(
            app.plan_confirm_armed && !app.plan_confirmed,
            "AC2: the first press on an irreversible action must only arm, not confirm"
        );
        app.handle_key(key(KeyCode::Char('c')), plan(true, true));
        assert!(app.plan_confirmed, "the second press must confirm");
        assert!(!app.plan_confirm_armed);
    }

    #[test]
    fn ctrl_c_never_completes_an_armed_irreversible_confirmation() {
        // Found during E09's self-review (not the independent Codex review): raw mode disables the terminal's own `ISIG`
        // handling, so a real terminal delivers Ctrl+C as `KeyCode::Char('c')` with
        // `KeyModifiers::CONTROL` - matching the bare `KeyCode` alone (the pre-fix behavior)
        // would let the universal "abort" gesture complete a pending irreversible confirmation
        // instead of cancelling it.
        let mut app = on_plan_screen();
        app.handle_key(key(KeyCode::Char('c')), plan(true, true));
        assert!(
            app.plan_confirm_armed,
            "the first plain press must still arm"
        );

        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        app.handle_key(ctrl_c, plan(true, true));
        assert!(
            !app.plan_confirmed,
            "Ctrl+C must never complete an irreversible confirmation"
        );
        assert!(
            !app.plan_confirm_armed,
            "Ctrl+C must cancel the pending confirmation, like any other non-plain-c key"
        );
    }

    #[test]
    fn any_key_other_than_c_cancels_an_armed_irreversible_confirmation() {
        let mut app = on_plan_screen();
        app.handle_key(key(KeyCode::Char('c')), plan(true, true));
        assert!(app.plan_confirm_armed);
        app.handle_key(key(KeyCode::Char('?')), plan(true, true));
        assert!(
            !app.plan_confirm_armed,
            "an unrelated key must cancel the pending confirmation, not leave it armed"
        );
        assert!(!app.plan_confirmed, "cancelling must never confirm");
    }

    #[test]
    fn c_does_nothing_when_the_selection_cannot_be_confirmed() {
        let mut app = on_plan_screen();
        app.handle_key(key(KeyCode::Char('c')), plan(false, false));
        assert!(!app.plan_confirmed);
        assert!(!app.plan_confirm_armed);
    }

    #[test]
    fn c_does_nothing_off_the_plan_screen_even_if_the_context_says_it_could_confirm() {
        let mut app = App::new();
        assert_eq!(app.screen, Screen::Home);
        app.handle_key(key(KeyCode::Char('c')), plan(true, true));
        assert!(!app.plan_confirmed);
        assert!(!app.plan_confirm_armed);
    }

    #[test]
    fn changing_the_selected_artifact_resets_a_pending_or_completed_confirmation() {
        let mut app = on_plan_screen();
        app.handle_key(key(KeyCode::Char('c')), plan(true, false));
        assert!(app.plan_confirmed);
        app.handle_key(key(KeyCode::Down), plan(true, false));
        assert!(
            !app.plan_confirmed,
            "selecting a different artifact must never carry over a confirmation for the old one"
        );
    }

    #[test]
    fn leaving_the_plan_screen_resets_a_pending_or_completed_confirmation() {
        let mut app = on_plan_screen();
        app.handle_key(key(KeyCode::Char('c')), plan(true, true));
        assert!(app.plan_confirm_armed);
        app.handle_key(key(KeyCode::Tab), plan(true, true));
        assert!(!app.plan_confirm_armed);
        assert!(!app.plan_confirmed);
    }
}
