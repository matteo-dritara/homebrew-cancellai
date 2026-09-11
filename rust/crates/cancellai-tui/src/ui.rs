//! Pure rendering (E09-S01). [`draw`] takes a frame and the current [`App`]/
//! [`TerminalCapability`] and builds widgets - no I/O, so it renders identically against a real
//! terminal backend and against `ratatui::backend::TestBackend` in tests (this module's own
//! "snapshot/render tests").

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::{App, KEY_BINDINGS, SCREENS, Screen};
use crate::capability::{ColorSupport, TerminalCapability};

/// Below this size a real layout would clip or panic on narrow widgets; render a plain message
/// instead (AC3: graceful capability fallback also covers terminal *size*, not only color/
/// Unicode support).
const MIN_WIDTH: u16 = 24;
const MIN_HEIGHT: u16 = 6;

pub fn draw(frame: &mut Frame, app: &App, capability: TerminalCapability) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        frame.render_widget(Paragraph::new("terminal too small"), area);
        return;
    }

    let border_set = if capability.unicode {
        border::PLAIN
    } else {
        border::Set {
            top_left: "+",
            top_right: "+",
            bottom_left: "+",
            bottom_right: "+",
            vertical_left: "|",
            vertical_right: "|",
            horizontal_top: "-",
            horizontal_bottom: "-",
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);

    draw_title_bar(frame, chunks[0], border_set);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(16), Constraint::Min(1)])
        .split(chunks[1]);
    draw_nav(frame, body[0], app, capability, border_set);
    draw_content(frame, body[1], app, border_set);

    draw_footer(frame, chunks[2], app, border_set);

    if app.show_help {
        draw_help_overlay(frame, area, border_set);
    }
}

fn highlight_style(capability: TerminalCapability) -> Style {
    match capability.color {
        ColorSupport::None => Style::default().add_modifier(Modifier::REVERSED),
        ColorSupport::Basic | ColorSupport::Extended => {
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        }
    }
}

fn draw_title_bar(frame: &mut Frame, area: Rect, border_set: border::Set) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(border_set)
        .title("cancellAI Atlas");
    frame.render_widget(block, area);
}

fn draw_nav(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    capability: TerminalCapability,
    border_set: border::Set,
) {
    let items: Vec<ListItem> = SCREENS
        .iter()
        .map(|screen| {
            let style = if *screen == app.screen {
                highlight_style(capability)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(screen.title(), style)))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(border_set)
            .title("Screens"),
    );
    frame.render_widget(list, area);
}

fn draw_content(frame: &mut Frame, area: Rect, app: &App, border_set: border::Set) {
    let body = match app.screen {
        Screen::Home => "Keyboard-first navigation shell for cancellAI's Atlas engine query API.",
        Screen::Atlas => "Coming in E09-S02: machine and project atlas.",
        Screen::Explain => "Coming in E09-S03: artifact explain view.",
        Screen::Plan => "Coming in E09-S04: plan review workflow.",
    };
    let paragraph = Paragraph::new(body).block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(border_set)
            .title(app.screen.title()),
    );
    frame.render_widget(paragraph, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App, border_set: border::Set) {
    let hint = if app.show_help {
        "press ? to close help"
    } else {
        "Tab: next  1-4: jump  ?: help  q: quit"
    };
    let paragraph = Paragraph::new(hint).block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(border_set),
    );
    frame.render_widget(paragraph, area);
}

fn draw_help_overlay(frame: &mut Frame, area: Rect, border_set: border::Set) {
    let lines: Vec<Line> = KEY_BINDINGS
        .iter()
        .map(|binding| Line::from(format!("{:<16} {}", binding.keys, binding.description)))
        .collect();
    // Sized from the longest line rather than a fixed guess, so a future keybinding with a
    // longer description is never silently clipped (the exact bug a fixed `40` produced for
    // "Tab / Shift+Tab" + "next / previous screen").
    let content_width = lines.iter().map(|line| line.width()).max().unwrap_or(0) as u16;
    let width = (content_width + 2).min(area.width);
    let height = (KEY_BINDINGS.len() as u16 + 2).min(area.height);
    let overlay = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(border_set)
            .title("Keys"),
    );
    frame.render_widget(paragraph, overlay);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render(app: &App, capability: TerminalCapability, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| draw(frame, app, capability))
            .expect("draw");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .concat()
    }

    #[test]
    fn home_screen_shows_its_title_and_nav_entries() {
        let content = render(
            &App::new(),
            TerminalCapability {
                color: ColorSupport::Basic,
                unicode: true,
            },
            60,
            15,
        );
        assert!(content.contains("cancellAI Atlas"));
        assert!(content.contains("Home"));
        assert!(content.contains("Atlas"));
        assert!(content.contains("Explain"));
        assert!(content.contains("Plan"));
    }

    #[test]
    fn ascii_capability_never_emits_unicode_box_drawing_glyphs() {
        let content = render(
            &App::new(),
            TerminalCapability {
                color: ColorSupport::None,
                unicode: false,
            },
            60,
            15,
        );
        for glyph in [
            "\u{2500}", "\u{2502}", "\u{250c}", "\u{2510}", "\u{2514}", "\u{2518}",
        ] {
            assert!(
                !content.contains(glyph),
                "ASCII fallback must not render {glyph:?}"
            );
        }
        assert!(content.contains('+'));
    }

    #[test]
    fn a_too_small_terminal_renders_a_message_instead_of_panicking() {
        // Wide enough for the whole message to fit on ratatui's single unwrapped line (an
        // extreme 5x2 area is covered separately below, where the message is unavoidably
        // clipped but must still not panic).
        let content = render(&App::new(), TerminalCapability::MINIMAL, 20, 5);
        assert!(content.contains("too small"));
    }

    #[test]
    fn an_extremely_small_terminal_does_not_panic() {
        render(&App::new(), TerminalCapability::MINIMAL, 5, 2);
    }

    #[test]
    fn help_overlay_lists_every_keybinding_when_toggled() {
        let mut app = App::new();
        app.show_help = true;
        let content = render(
            &app,
            TerminalCapability {
                color: ColorSupport::Basic,
                unicode: true,
            },
            60,
            15,
        );
        for binding in KEY_BINDINGS {
            assert!(
                content.contains(binding.description),
                "missing hint for {}",
                binding.keys
            );
        }
    }

    #[test]
    fn stub_screens_show_a_coming_soon_placeholder_not_fabricated_data() {
        let mut app = App::new();
        app.screen = Screen::Atlas;
        let content = render(
            &app,
            TerminalCapability {
                color: ColorSupport::Basic,
                unicode: true,
            },
            60,
            15,
        );
        assert!(content.contains("Coming in E09-S02"));
    }
}
