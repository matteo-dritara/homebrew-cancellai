//! Terminal experience client binary (E09-S01). Thin by design: terminal init/teardown, the
//! event loop, and a panic hook that restores the terminal before the default handler runs -
//! everything else lives in `cancellai_tui`'s library modules, which are what this crate's
//! tests actually exercise (`lib.rs`'s own doc explains why).

use std::io::{self, Stdout, stdout};
use std::time::Duration;

use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use cancellai_tui::app::App;
use cancellai_tui::capability::{self, ProcessEnv};
use cancellai_tui::data::EngineData;
use cancellai_tui::event::{self, AppEvent};
use cancellai_tui::ui;

fn main() -> io::Result<()> {
    install_panic_hook();
    let mut terminal = init_terminal()?;
    let result = run(&mut terminal);
    restore_terminal()?;
    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    let capability = capability::detect(&ProcessEnv);
    let mut app = App::new();
    // No inventory scan is wired into the running binary yet (E09-S02/E09-S03's own evidence
    // packets: fixture-driven view-model correctness is each story's scope, live-scan wiring is
    // a separate, deferred change, matching E08-S04's identical deferral for the CLI). The Atlas
    // and Explain screens render their explicit "not loaded" states until a future story
    // assembles a real `Vec<ProviderResolution>` here and feeds it through
    // `cancellai_policy::atlas::summarize`/`cancellai_policy::explain::explain`.
    let data = EngineData::default();
    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, &app, capability, &data))?;
        if let AppEvent::Key(key) = event::next(Duration::from_millis(250))? {
            let plan_context = data.plan_context(app.explain_selected);
            app.handle_key(key, plan_context);
        }
    }
    Ok(())
}

fn init_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout()))
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)
}

/// A panic anywhere in `run` must not leave the user's terminal in raw/alternate-screen mode -
/// restore it first, then hand off to the default hook so the panic message still prints.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        default_hook(panic_info);
    }));
}
