//! Pure rendering (E09-S01). [`draw`] takes a frame and the current [`App`]/
//! [`TerminalCapability`] and builds widgets - no I/O, so it renders identically against a real
//! terminal backend and against `ratatui::backend::TestBackend` in tests (this module's own
//! "snapshot/render tests").

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::{App, KEY_BINDINGS, SCREENS, Screen};
use crate::capability::{ColorSupport, TerminalCapability};
use crate::data::EngineData;
use crate::format::format_bytes;
use cancellai_policy::{AtlasSummary, ProjectBucketKey};

/// Below this size a real layout would clip or panic on narrow widgets; render a plain message
/// instead (AC3: graceful capability fallback also covers terminal *size*, not only color/
/// Unicode support).
const MIN_WIDTH: u16 = 24;
const MIN_HEIGHT: u16 = 6;

pub fn draw(frame: &mut Frame, app: &App, capability: TerminalCapability, data: &EngineData) {
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
    draw_content(frame, body[1], app, capability, border_set, data);

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

fn draw_content(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    capability: TerminalCapability,
    border_set: border::Set,
    data: &EngineData,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(border_set)
        .title(app.screen.title());

    if app.screen == Screen::Atlas {
        match &data.atlas {
            Some(summary) => draw_atlas_summary(frame, area, block, capability, summary),
            None => frame.render_widget(
                Paragraph::new("No inventory scan loaded yet.").block(block),
                area,
            ),
        }
        return;
    }

    let body = match app.screen {
        Screen::Home => "Keyboard-first navigation shell for cancellAI's Atlas engine query API.",
        Screen::Atlas => unreachable!("handled above"),
        Screen::Explain => "Coming in E09-S03: artifact explain view.",
        Screen::Plan => "Coming in E09-S04: plan review workflow.",
    };
    frame.render_widget(Paragraph::new(body).block(block), area);
}

/// AC1 ("Logical and estimated reclaimable values are visually distinct"): the two totals get
/// different labels always, plus different styling when color is available - distinctness never
/// depends on color alone. AC2 ("Unknown/incomplete scans are prominent and never hidden in
/// totals"): an incomplete scan gets its own highlighted line near the top, and the totals above
/// it are never suppressed or zeroed on its account (`atlas::summarize` already guarantees the
/// numbers themselves are never hidden; this only has to make the fact visible).
fn draw_atlas_summary(
    frame: &mut Frame,
    area: Rect,
    block: Block<'static>,
    capability: TerminalCapability,
    summary: &AtlasSummary,
) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::raw("Total footprint: "),
        Span::styled(
            format_bytes(summary.total_logical_bytes),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("   Estimated reclaimable: "),
        Span::styled(
            format_bytes(summary.total_reclaimable_bytes),
            reclaimable_style(capability),
        ),
    ]));

    if summary.any_incomplete {
        lines.push(Line::from(Span::styled(
            "! one or more scans are incomplete - totals may be undercounted",
            highlight_style(capability),
        )));
    }
    lines.push(Line::raw(""));

    lines.push(Line::from(Span::styled(
        "Providers:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for provider in &summary.providers {
        let status = if provider.scan_complete {
            String::new()
        } else {
            format!(
                " [incomplete: {}]",
                provider
                    .scan_incomplete_reason
                    .as_deref()
                    .unwrap_or("unknown reason")
            )
        };
        lines.push(Line::raw(format!(
            "  {}: {} logical, {} reclaimable, {} artifact(s){status}",
            provider.provider_id,
            format_bytes(provider.logical_bytes),
            format_bytes(provider.reclaimable_bytes),
            provider.artifact_count,
        )));
    }
    lines.push(Line::raw(""));

    lines.push(Line::from(Span::styled(
        "Projects:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for project in &summary.projects {
        let label = match project.key {
            ProjectBucketKey::Attributed(name) => name.to_string(),
            ProjectBucketKey::Unattributed => "Unattributed".to_string(),
        };
        lines.push(Line::raw(format!(
            "  {label}: {} logical, {} artifact(s)",
            format_bytes(project.logical_bytes),
            project.artifact_count,
        )));
    }
    lines.push(Line::raw(""));

    lines.push(Line::from(Span::styled(
        "Top contributors:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for (rank, contributor) in summary.top_contributors.iter().enumerate() {
        lines.push(Line::raw(format!(
            "  {}. {} ({}): {}",
            rank + 1,
            contributor.artifact_id,
            contributor.provider_id,
            format_bytes(contributor.logical_bytes),
        )));
    }

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn reclaimable_style(capability: TerminalCapability) -> Style {
    match capability.color {
        ColorSupport::None => Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ColorSupport::Basic | ColorSupport::Extended => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    }
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
        render_with_data(app, capability, width, height, &EngineData::default())
    }

    fn render_with_data(
        app: &App,
        capability: TerminalCapability,
        width: u16,
        height: u16,
        data: &EngineData,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| draw(frame, app, capability, data))
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
        app.screen = Screen::Explain;
        let content = render(
            &app,
            TerminalCapability {
                color: ColorSupport::Basic,
                unicode: true,
            },
            60,
            15,
        );
        assert!(content.contains("Coming in E09-S03"));
    }

    #[test]
    fn atlas_screen_with_no_data_loaded_shows_an_explicit_not_fabricated_placeholder() {
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
        assert!(content.contains("No inventory scan loaded yet"));
    }

    fn sample_summary() -> AtlasSummary<'static> {
        use cancellai_model::ArtifactId;
        use cancellai_policy::{ProjectTotals, ProviderTotals, TopContributor};
        // Test-only leak for a `&'static ArtifactId` (see Cargo.toml's dev-dependency comment):
        // `TopContributor` borrows from the source inventory in production, which this fixture
        // has no real inventory behind - one leaked value per test process is inconsequential.
        let artifact_id: &'static ArtifactId = Box::leak(Box::new(ArtifactId::new("claude-a")));
        AtlasSummary {
            total_logical_bytes: 3_145_728,
            total_reclaimable_bytes: 1_048_576,
            total_artifact_count: 2,
            any_incomplete: true,
            providers: vec![ProviderTotals {
                provider_id: "claude-code",
                logical_bytes: 3_145_728,
                reclaimable_bytes: 1_048_576,
                artifact_count: 2,
                scan_complete: false,
                scan_incomplete_reason: Some("permission denied".to_string()),
            }],
            projects: vec![ProjectTotals {
                key: ProjectBucketKey::Unattributed,
                logical_bytes: 3_145_728,
                reclaimable_bytes: 1_048_576,
                artifact_count: 2,
            }],
            top_contributors: vec![TopContributor {
                artifact_id,
                provider_id: "claude-code",
                logical_bytes: 3_145_728,
            }],
        }
    }

    #[test]
    fn atlas_screen_shows_logical_and_reclaimable_as_visually_distinct_labeled_values() {
        let mut app = App::new();
        app.screen = Screen::Atlas;
        let data = EngineData {
            atlas: Some(sample_summary()),
        };
        let content = render_with_data(
            &app,
            TerminalCapability {
                color: ColorSupport::Basic,
                unicode: true,
            },
            80,
            20,
            &data,
        );
        assert!(content.contains("Total footprint:"));
        assert!(content.contains("Estimated reclaimable:"));
        assert!(content.contains("3.00 MB"));
        assert!(content.contains("1.00 MB"));
    }

    #[test]
    fn atlas_screen_surfaces_an_incomplete_scan_prominently_without_hiding_the_totals() {
        let mut app = App::new();
        app.screen = Screen::Atlas;
        let data = EngineData {
            atlas: Some(sample_summary()),
        };
        let content = render_with_data(&app, TerminalCapability::MINIMAL, 80, 20, &data);
        assert!(content.contains("incomplete"));
        // AC2: the total is still the full 3 MB, not zeroed or withheld on account of the
        // incomplete provider scan.
        assert!(content.contains("3.00 MB"));
    }

    #[test]
    fn atlas_screen_shows_an_explicit_unattributed_project_bucket() {
        let mut app = App::new();
        app.screen = Screen::Atlas;
        let data = EngineData {
            atlas: Some(sample_summary()),
        };
        let content = render_with_data(
            &app,
            TerminalCapability {
                color: ColorSupport::Basic,
                unicode: true,
            },
            80,
            20,
            &data,
        );
        assert!(content.contains("Unattributed"));
    }
}
