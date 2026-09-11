//! End-to-end shell test (E09-S01): drives `App` + `ui::draw` together through every screen in
//! sequence, the way a real key-by-key session would, rather than each module's own unit tests
//! in isolation.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use cancellai_tui::app::{App, Screen};
use cancellai_tui::capability::{ColorSupport, TerminalCapability};
use cancellai_tui::data::EngineData;
use cancellai_tui::ui;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn a_full_keyboard_session_reaches_every_screen_then_quits_cleanly() {
    let capability = TerminalCapability {
        color: ColorSupport::Basic,
        unicode: true,
    };
    let mut terminal = Terminal::new(TestBackend::new(60, 15)).expect("terminal");
    let mut app = App::new();
    let data = EngineData::default();

    let visited_in_order = [Screen::Home, Screen::Atlas, Screen::Explain, Screen::Plan];
    for expected in visited_in_order {
        assert_eq!(app.screen, expected);
        terminal
            .draw(|frame| ui::draw(frame, &app, capability, &data))
            .expect("draw must not panic");
        app.handle_key(key(KeyCode::Tab));
    }
    // Tab from Plan wraps back to Home rather than advancing past the last screen.
    assert_eq!(app.screen, Screen::Home);

    app.handle_key(key(KeyCode::Char('q')));
    assert!(
        app.should_quit,
        "q must terminate the session from any reachable screen"
    );
}

#[test]
fn help_overlay_opens_and_closes_without_losing_the_current_screen() {
    let capability = TerminalCapability {
        color: ColorSupport::Extended,
        unicode: true,
    };
    let mut terminal = Terminal::new(TestBackend::new(60, 15)).expect("terminal");
    let mut app = App::new();
    let data = EngineData::default();
    app.handle_key(key(KeyCode::Char('2')));
    assert_eq!(app.screen, Screen::Atlas);

    app.handle_key(key(KeyCode::Char('?')));
    assert!(app.show_help);
    terminal
        .draw(|frame| ui::draw(frame, &app, capability, &data))
        .expect("draw must not panic");
    assert_eq!(
        app.screen,
        Screen::Atlas,
        "opening help must not change the active screen"
    );

    app.handle_key(key(KeyCode::Char('?')));
    assert!(!app.show_help);
}

#[test]
fn every_ascii_no_color_frame_in_a_full_session_avoids_unicode_borders() {
    let capability = TerminalCapability::MINIMAL;
    let mut terminal = Terminal::new(TestBackend::new(60, 15)).expect("terminal");
    let mut app = App::new();
    let data = EngineData::default();
    for _ in 0..cancellai_tui::app::SCREENS.len() {
        terminal
            .draw(|frame| ui::draw(frame, &app, capability, &data))
            .expect("draw");
        let content: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            !content.contains('\u{2500}'),
            "ASCII capability must never render a Unicode border"
        );
        app.handle_key(key(KeyCode::Tab));
    }
}
