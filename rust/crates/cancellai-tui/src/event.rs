//! Thin `crossterm` event boundary (E09-S01). This is the one module allowed to block on real
//! terminal I/O; everything it produces is a plain [`AppEvent`] that [`crate::app::App`]
//! consumes without knowing `crossterm` exists, keeping `app`/`ui` testable without a tty.

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent};

/// What the event loop reacts to. `Tick` lets `main.rs` redraw on a timer even with no key
/// pressed (e.g. after a terminal resize is coalesced) without `app`/`ui` needing to know why.
pub enum AppEvent {
    Key(KeyEvent),
    Resize,
    Tick,
}

/// Block for up to `timeout` for the next terminal event, translating it to an [`AppEvent`].
/// Returns `Tick` on timeout rather than blocking forever, so the caller's loop always makes
/// progress.
pub fn next(timeout: Duration) -> io::Result<AppEvent> {
    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) => Ok(AppEvent::Key(key)),
            Event::Resize(_, _) => Ok(AppEvent::Resize),
            _ => Ok(AppEvent::Tick),
        }
    } else {
        Ok(AppEvent::Tick)
    }
}
