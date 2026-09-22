//! OS-appropriate Guardian notifications with a terminal fallback (E15-S02,
//! `docs/architecture/GUARDIAN_MODEL.md`'s Guardian/pressure vocabulary; AC1/AC2 below).
//!
//! **AC1 ("notifications never include sensitive transcript/source content") holds by
//! construction, not by filtering.** [`NotificationKind`] carries no `String`/`PathBuf` field at
//! all - every variant's payload is a closed enum (`PressureState`, `AnomalySeverity`) or a
//! plain count, so there is no field a caller could ever populate with a path, a transcript
//! excerpt, or provider content, mistakenly or otherwise. This is the same "imports no
//! sensitive-capable type" argument `pressure`/`baseline` already make for SI-027 authority
//! isolation, applied here to payload privacy instead. `render` (the only place text is
//! produced) is a fixed template per variant - it cannot append caller-supplied text either.
//!
//! **AC2 ("notification unavailability does not trigger stronger remediation") holds
//! structurally.** [`notify`](Notifier::notify) returns [`NotificationOutcome`], a two-value
//! enum (`Delivered`/`Fallback`) with no error variant and no field a caller could read as "try
//! something stronger" - this module imports no `AuthorityLevel`/`ActionClass` type and exposes
//! no escalation API, matching `pressure`'s own "cannot express an authority decision" argument.
//! A failed OS-native delivery falls back to the terminal and stops there.

use crate::baseline::AnomalySeverity;
use crate::pressure::PressureState;

/// A notification's content, restricted to a closed, pre-vetted set - see the module docs for
/// why this is the mechanism that makes AC1 hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    /// Disk/budget pressure crossed into a new state (`docs/architecture/GUARDIAN_MODEL.md`
    /// "Pressure states").
    PressureChanged { state: PressureState },
    /// A forecast projects reaching a threshold within the given number of days.
    ForecastWarning { estimated_days_until_threshold: u32 },
    /// A baseline observation was classified at or above [`AnomalySeverity::Elevated`] - the
    /// classification only, never the observed value/metric that produced it.
    AnomalyDetected { severity: AnomalySeverity },
    /// The Guardian service was enabled.
    GuardianEnabled,
    /// The Guardian service was disabled.
    GuardianDisabled,
}

/// The rendered, ready-to-deliver text for one [`NotificationKind`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedNotification {
    title: String,
    body: String,
}

const TITLE: &str = "cancellAI Guardian";

fn render(kind: NotificationKind) -> RenderedNotification {
    let body = match kind {
        NotificationKind::PressureChanged { state } => {
            format!("Disk or budget pressure is now {state:?}.")
        }
        NotificationKind::ForecastWarning {
            estimated_days_until_threshold,
        } => {
            format!(
                "Estimated {estimated_days_until_threshold} day(s) until a tracked threshold is reached."
            )
        }
        NotificationKind::AnomalyDetected { severity } => {
            format!("A {severity:?} anomaly was detected.")
        }
        NotificationKind::GuardianEnabled => "Guardian monitoring is now enabled.".to_string(),
        NotificationKind::GuardianDisabled => "Guardian monitoring is now disabled.".to_string(),
    };
    RenderedNotification {
        title: TITLE.to_string(),
        body,
    }
}

/// Where a rendered notification actually went. Never an error - see the module docs' AC2
/// argument for why this type has no variant a caller could react to by escalating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationOutcome {
    /// The OS-native mechanism accepted the notification.
    Delivered,
    /// The OS-native mechanism was unavailable or refused it; the terminal fallback ran instead.
    Fallback,
}

/// One delivery attempt through a real or fake channel. `deliver` reports success/failure as a
/// plain `bool`, deliberately not a typed error - nothing downstream of a sink is meant to
/// branch on *why* delivery failed, only on *whether* the fallback is needed (AC2).
trait NotificationSink {
    fn deliver(&self, title: &str, body: &str) -> bool;
}

/// Always available, the guaranteed fallback - writing a line to stderr cannot itself fail in a
/// way this module needs to react to.
struct TerminalNotificationSink;

impl NotificationSink for TerminalNotificationSink {
    fn deliver(&self, title: &str, body: &str) -> bool {
        eprintln!("[{title}] {body}");
        true
    }
}

/// The real, OS-native mechanism, selected at compile time - the same `target_os` dispatch
/// `cancellai_guardian::service` uses for its own platform adapters.
struct SystemNotificationSink;

// Kept testable on every host (the same `#[cfg(any(test, target_os = "..."))]` split
// `cancellai_platform::wsl::classify_osrelease` already uses) even though it is only reachable
// in production on macOS.
#[cfg(any(test, target_os = "macos"))]
fn applescript_escape(value: &str) -> String {
    // AppleScript double-quoted string literal escaping: `\` and `"` are the two characters
    // that end or alter the literal early if left unescaped.
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch == '\\' || ch == '"' {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

#[cfg(target_os = "macos")]
fn macos_script(title: &str, body: &str) -> String {
    format!(
        "display notification \"{}\" with title \"{}\"",
        applescript_escape(body),
        applescript_escape(title)
    )
}

#[cfg(target_os = "macos")]
impl NotificationSink for SystemNotificationSink {
    fn deliver(&self, title: &str, body: &str) -> bool {
        std::process::Command::new("osascript")
            .args(["-e", &macos_script(title, body)])
            .status()
            .is_ok_and(|status| status.success())
    }
}

#[cfg(target_os = "linux")]
impl NotificationSink for SystemNotificationSink {
    fn deliver(&self, title: &str, body: &str) -> bool {
        // `notify-send` takes title/body as separate positional arguments (never a shell
        // string), so no escaping is needed here the way AppleScript's embedded literal needs
        // it - a desktop without `notify-send` installed, or without a reachable notification
        // daemon, fails this call cleanly rather than partially.
        std::process::Command::new("notify-send")
            .args([title, body])
            .status()
            .is_ok_and(|status| status.success())
    }
}

#[cfg(target_os = "windows")]
impl NotificationSink for SystemNotificationSink {
    fn deliver(&self, title: &str, body: &str) -> bool {
        // `msg.exe` is present on every Windows edition this workspace targets and needs no
        // extra module/install, unlike a real toast (`BurntToast`/WinRT). Disclosed residual: it
        // shows a blocking modal to the target session rather than a dismissible toast - a
        // deliberate first-cut trade favoring zero new dependencies (ADR-0019 outer ring still
        // applies: a dependency needs a story naming what it replaces) over notification polish.
        let text = format!("{title}\n\n{body}");
        std::process::Command::new("msg")
            .args([&whoami(), "/TIME:0", &text])
            .status()
            .is_ok_and(|status| status.success())
    }
}

#[cfg(target_os = "windows")]
fn whoami() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| "console".to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
impl NotificationSink for SystemNotificationSink {
    fn deliver(&self, _title: &str, _body: &str) -> bool {
        false
    }
}

/// One Guardian notifier: attempts the real OS-native mechanism, falling back to the terminal on
/// any failure - never an escalation, never a panic, never a blocking retry loop.
pub struct Notifier {
    system: SystemNotificationSink,
    terminal: TerminalNotificationSink,
}

impl Default for Notifier {
    fn default() -> Self {
        Self {
            system: SystemNotificationSink,
            terminal: TerminalNotificationSink,
        }
    }
}

impl Notifier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notify(&self, kind: NotificationKind) -> NotificationOutcome {
        let rendered = render(kind);
        if self.system.deliver(&rendered.title, &rendered.body) {
            return NotificationOutcome::Delivered;
        }
        self.terminal.deliver(&rendered.title, &rendered.body);
        NotificationOutcome::Fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baseline::AnomalySeverity;
    use crate::pressure::PressureState;

    /// A conservative, redundant defense-in-depth check on top of the structural guarantee the
    /// module docs describe: even a future template edit that accidentally interpolated a path
    /// would trip this, since none of these markers can appear in any current rendering.
    fn contains_no_path_like_content(text: &str) -> bool {
        !text.contains('/') && !text.contains('\\') && !text.contains('~')
    }

    fn all_kinds() -> Vec<NotificationKind> {
        vec![
            NotificationKind::PressureChanged {
                state: PressureState::Red,
            },
            NotificationKind::PressureChanged {
                state: PressureState::Green,
            },
            NotificationKind::ForecastWarning {
                estimated_days_until_threshold: 3,
            },
            NotificationKind::AnomalyDetected {
                severity: AnomalySeverity::Anomalous,
            },
            NotificationKind::GuardianEnabled,
            NotificationKind::GuardianDisabled,
        ]
    }

    #[test]
    fn every_kind_renders_privacy_safe_text() {
        for kind in all_kinds() {
            let rendered = render(kind);
            assert!(
                contains_no_path_like_content(&rendered.title),
                "title leaked path-like content for {kind:?}"
            );
            assert!(
                contains_no_path_like_content(&rendered.body),
                "body leaked path-like content for {kind:?}"
            );
        }
    }

    #[test]
    fn rendered_body_never_exceeds_a_short_bounded_length() {
        // A closed template set has an inherently bounded length; a body that grew unexpectedly
        // large would itself be a sign a variant started interpolating unbounded caller content.
        for kind in all_kinds() {
            let rendered = render(kind);
            assert!(
                rendered.body.len() < 200,
                "unexpectedly long body for {kind:?}"
            );
        }
    }

    #[test]
    fn applescript_escaping_neutralizes_quotes_and_backslashes() {
        let escaped = applescript_escape("a \"quoted\" \\value");
        assert_eq!(escaped, "a \\\"quoted\\\" \\\\value");
    }

    struct FailingSink;
    impl NotificationSink for FailingSink {
        fn deliver(&self, _title: &str, _body: &str) -> bool {
            false
        }
    }

    struct RecordingSink {
        calls: std::cell::RefCell<Vec<(String, String)>>,
    }
    impl NotificationSink for RecordingSink {
        fn deliver(&self, title: &str, body: &str) -> bool {
            self.calls
                .borrow_mut()
                .push((title.to_string(), body.to_string()));
            true
        }
    }

    #[test]
    fn a_failed_system_delivery_falls_back_to_the_terminal_not_an_error() {
        let system = FailingSink;
        let terminal = RecordingSink {
            calls: std::cell::RefCell::new(Vec::new()),
        };
        let rendered = render(NotificationKind::GuardianEnabled);
        let outcome = if system.deliver(&rendered.title, &rendered.body) {
            NotificationOutcome::Delivered
        } else {
            terminal.deliver(&rendered.title, &rendered.body);
            NotificationOutcome::Fallback
        };
        assert_eq!(outcome, NotificationOutcome::Fallback);
        assert_eq!(terminal.calls.borrow().len(), 1);
    }

    #[test]
    fn a_successful_system_delivery_never_touches_the_fallback() {
        let system = RecordingSink {
            calls: std::cell::RefCell::new(Vec::new()),
        };
        let terminal = RecordingSink {
            calls: std::cell::RefCell::new(Vec::new()),
        };
        let rendered = render(NotificationKind::GuardianDisabled);
        let outcome = if system.deliver(&rendered.title, &rendered.body) {
            NotificationOutcome::Delivered
        } else {
            terminal.deliver(&rendered.title, &rendered.body);
            NotificationOutcome::Fallback
        };
        assert_eq!(outcome, NotificationOutcome::Delivered);
        assert_eq!(system.calls.borrow().len(), 1);
        assert_eq!(terminal.calls.borrow().len(), 0);
    }

    #[test]
    fn terminal_sink_always_reports_delivered() {
        assert!(TerminalNotificationSink.deliver("t", "b"));
    }

    #[test]
    fn notifier_end_to_end_never_panics_regardless_of_real_os_availability() {
        // Best-effort real smoke coverage: whether the real OS mechanism is actually reachable
        // in this environment (a headless CI runner, a sandboxed session, ...) is not something
        // this test can control, so it asserts the one thing that must hold everywhere - no
        // panic, and a defined outcome - rather than which outcome.
        let notifier = Notifier::new();
        let outcome = notifier.notify(NotificationKind::GuardianEnabled);
        assert!(matches!(
            outcome,
            NotificationOutcome::Delivered | NotificationOutcome::Fallback
        ));
    }
}
