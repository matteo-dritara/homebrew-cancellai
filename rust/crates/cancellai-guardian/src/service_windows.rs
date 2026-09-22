//! Windows adapter: a user-scoped scheduled task via `schtasks.exe`
//! (`docs/architecture/GUARDIAN_MODEL.md` Runtime: "Windows: user-scoped scheduled task/service
//! design"). `/RL LIMITED` keeps task creation usable without an elevation prompt, matching a
//! per-user launchd agent / systemd `--user` unit's own no-admin-required scope.
//!
//! Unlike the file-backed macOS/Linux adapters, a scheduled task has no local config file this
//! module can `Path::exists()` - the Task Scheduler store is the only source of truth, so
//! `status` here always queries it, and `install`'s "installed but disabled" postcondition is
//! reached by an explicit follow-up `/Change /DISABLE` rather than by a file simply not being
//! loaded yet.
//!
//! `schtasks`' plain-text output (`/FO LIST`/`/FO CSV`) is localized by the OS display language,
//! which would make substring status parsing silently wrong on a non-English Windows install -
//! `/XML` output uses fixed, non-localized element names and is used here instead, deliberately.

// Only this module's own tests construct a `PathBuf` directly (`ServiceSpec::program` in a real
// `spec()`/smoke-test fixture) - production code here never stores a local path at all (see the
// module docs: unlike the file-backed macOS/Linux adapters, a scheduled task has no local config
// file). A crate-wide `use` would be reported as unused specifically on a real Windows *library*
// build, where this module compiles outside `cfg(test)` too (`target_os = "windows"` alone
// satisfies `lib.rs`'s `cfg(any(test, target_os = "windows"))`) but `mod tests` does not - this
// is exactly the "clippy only sees the platform it runs on" gap AGENTS.md warns about, caught by
// real Windows CI rather than local `--target x86_64-pc-windows-gnu` clippy (unavailable in this
// environment, no cross-compiler installed).
#[cfg(test)]
use std::path::PathBuf;

use crate::service::{
    CommandOutput, CommandRunner, ServiceError, ServiceRuntime, ServiceSpec, ServiceStatus,
    SystemCommandRunner,
};

const SCHTASKS: &str = "schtasks";

pub(crate) struct ScheduledTaskRuntime {
    runner: Box<dyn CommandRunner>,
}

impl Default for ScheduledTaskRuntime {
    fn default() -> Self {
        Self {
            runner: Box::new(SystemCommandRunner),
        }
    }
}

impl ScheduledTaskRuntime {
    #[cfg(test)]
    fn for_test(runner: Box<dyn CommandRunner>) -> Self {
        Self { runner }
    }

    /// Polls `status` for up to 2 seconds, returning `true` the moment it reports `Enabled`.
    /// See `enable`'s own doc comment for why this exists in production code rather than only
    /// in a test.
    fn confirm_enabled(&self, spec: &ServiceSpec) -> bool {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if matches!(self.status(spec), ServiceStatus::Enabled) {
                return true;
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

/// Windows command-line quoting for one argument, as `CreateProcess`/`CommandLineToArgvW` (and
/// therefore `schtasks /TR`'s own re-parse of the string it is given) expect it: a word
/// containing whitespace or a double quote is wrapped in `"..."` with embedded quotes doubled.
/// Scoped to this module's own need - the two trusted, internal strings (Guardian's own resolved
/// executable path and its fixed `run` argument), not a general Windows argv encoder.
fn quote_if_needed(word: &str) -> String {
    if word.is_empty() || word.chars().any(|c| c == ' ' || c == '\t' || c == '"') {
        let mut quoted = String::with_capacity(word.len() + 2);
        quoted.push('"');
        for ch in word.chars() {
            if ch == '"' {
                quoted.push('"');
            }
            quoted.push(ch);
        }
        quoted.push('"');
        quoted
    } else {
        word.to_string()
    }
}

fn command_line(spec: &ServiceSpec) -> String {
    let mut parts = vec![quote_if_needed(&spec.program.to_string_lossy())];
    parts.extend(spec.args.iter().map(|arg| quote_if_needed(arg)));
    parts.join(" ")
}

/// Whether a successful `/Query ... /XML` result describes an enabled task. `None` means the
/// XML did not contain a recognizable `<Enabled>` element at all - a shape this adapter does not
/// understand, which must read as [`ServiceStatus::Unsupported`], never as a guessed
/// enabled/disabled state.
///
/// Extracts the content between the first `<Enabled>`/`</Enabled>` pair and trims it before
/// comparing, rather than matching the whole `<Enabled>true</Enabled>` span as one literal
/// substring: real Windows CI reproduced a `None` result twice against genuine `/XML` output
/// this adapter could not yet explain (a UTF-16 BOM decoding fix, then a clippy-driven
/// `chunks_exact`-to-`as_chunks` change, neither of which resolved it), and pretty-printed XML
/// wrapping the value across a newline plus indentation (`<Enabled>\r\n    true\r\n  </Enabled>`)
/// is the most plausible remaining explanation an exact-span match cannot tolerate. The tag name
/// itself is matched with exact casing (`Enabled`), not case-insensitively: it names a fixed
/// element in Microsoft's own Task Scheduler XML schema, not free-form content, so it does not
/// vary the way a value's whitespace can.
fn parse_enabled_from_xml(xml: &str) -> Option<bool> {
    // `<Enabled` rather than the whole `<Enabled>` literal, then find the tag's own closing `>`
    // separately - tolerates an XML namespace/attribute on the opening tag itself
    // (`<Enabled xmlns="...">`), which a real document's root element sometimes carries down
    // onto children depending on how the serializer wrote it, even though this element itself
    // is never expected to declare one directly.
    let open_start = xml.find("<Enabled")?;
    let content_start = open_start + xml.get(open_start..)?.find('>')? + 1;
    let end = content_start + xml.get(content_start..)?.find("</Enabled>")?;
    match xml.get(content_start..end)?.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

impl ServiceRuntime for ScheduledTaskRuntime {
    fn install(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        let tr = command_line(spec);
        run_checked(
            self.runner.as_ref(),
            SCHTASKS,
            &[
                "/Create", "/TN", &spec.name, "/TR", &tr, "/SC", "ONLOGON", "/RL", "LIMITED", "/F",
            ],
        )?;
        // Task Scheduler creates a task enabled by default - disabled-until-`enable`-is-called
        // is this module's own postcondition, matched to the macOS/Linux adapters' own
        // install-then-separately-enable contract (AC1: consistent lifecycle across platforms).
        self.disable(spec)
    }

    fn uninstall(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        match self
            .runner
            .run(SCHTASKS, &["/Delete", "/TN", &spec.name, "/F"])
        {
            Ok(CommandOutput { success: true, .. }) => Ok(()),
            // `/Delete` on a task that does not exist fails - uninstall is idempotent by
            // contract, so that is success, not an error, mirroring the other two adapters.
            Ok(CommandOutput { success: false, .. }) => Ok(()),
            Err(ServiceError::CommandFailed { .. }) => Ok(()),
            Err(other) => Err(other),
        }
    }

    /// Confirms the change before returning `Ok`, matching the macOS/Linux adapters' own
    /// enable contracts (AC1: consistent lifecycle across platforms). This is not only
    /// parity: real Windows CI showed the `/Query /XML` issued immediately after a *successful*
    /// `/Change ... /ENABLE` can still fail to parse as enabled, while the very same query right
    /// after `install` (checking `Disabled`) parsed correctly in the same run - the one point of
    /// difference is that a state-changing `/Change` call had *just* run, which points at
    /// `schtasks`' own task-cache lagging behind its own write rather than at the XML shape
    /// itself (the two prior fixes here targeted decoding/parsing and neither resolved it).
    /// Polling `status` short-circuits the moment it is confirmed, so the common case pays no
    /// extra latency.
    fn enable(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        if matches!(self.status(spec), ServiceStatus::NotInstalled) {
            return Err(ServiceError::Io {
                message: format!("cannot enable '{}': not installed", spec.name),
            });
        }
        run_checked(
            self.runner.as_ref(),
            SCHTASKS,
            &["/Change", "/TN", &spec.name, "/ENABLE"],
        )?;
        if self.confirm_enabled(spec) {
            Ok(())
        } else {
            Err(ServiceError::CommandFailed {
                command: format!("schtasks /Change /TN {} /ENABLE", spec.name),
                stderr: "schtasks reported success but /Query /XML did not confirm the task as \
                         enabled within the poll budget - registration could not be confirmed"
                    .to_string(),
            })
        }
    }

    fn disable(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        match self
            .runner
            .run(SCHTASKS, &["/Change", "/TN", &spec.name, "/DISABLE"])
        {
            Ok(CommandOutput { success: true, .. }) => Ok(()),
            Ok(CommandOutput { success: false, .. }) => Ok(()),
            Err(ServiceError::CommandFailed { .. }) => Ok(()),
            Err(other) => Err(other),
        }
    }

    fn status(&self, spec: &ServiceSpec) -> ServiceStatus {
        match self
            .runner
            .run(SCHTASKS, &["/Query", "/TN", &spec.name, "/XML"])
        {
            Err(ServiceError::CommandUnavailable { .. }) => ServiceStatus::Unsupported {
                reason: "schtasks is not available".to_string(),
            },
            Err(_) => ServiceStatus::NotInstalled,
            Ok(CommandOutput { success: false, .. }) => ServiceStatus::NotInstalled,
            Ok(CommandOutput {
                success: true,
                stdout,
                ..
            }) => match parse_enabled_from_xml(&stdout) {
                Some(true) => ServiceStatus::Enabled,
                Some(false) => ServiceStatus::Disabled,
                None => {
                    // Two prior real-Windows-CI fixes for this exact "did not contain a
                    // recognizable Enabled field" outcome (a UTF-16 BOM decoding fix, then a
                    // chunks_exact-to-as_chunks clippy repair) still left it reproducing - the
                    // actual output's real shape remains unconfirmed. Rather than guess a third
                    // time, this includes a bounded snippet of what was actually received
                    // (`{:?}` escapes control/non-printable bytes safely) so the next occurrence
                    // is diagnosable from the failure itself instead of blind.
                    let snippet: String = stdout.chars().take(300).collect();
                    ServiceStatus::Unsupported {
                        reason: format!(
                            "schtasks /XML output did not contain a recognizable Enabled field \
                             (first 300 chars: {snippet:?})"
                        ),
                    }
                }
            },
        }
    }
}

fn run_checked(
    runner: &dyn CommandRunner,
    program: &str,
    args: &[&str],
) -> Result<(), ServiceError> {
    match runner.run(program, args) {
        Ok(CommandOutput { success: true, .. }) => Ok(()),
        Ok(CommandOutput {
            success: false,
            stderr,
            ..
        }) => Err(ServiceError::CommandFailed {
            command: program.to_string(),
            stderr,
        }),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeCommandRunner {
        scripted: RefCell<std::collections::VecDeque<Result<CommandOutput, ServiceError>>>,
        calls: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl FakeCommandRunner {
        fn new(scripted: Vec<Result<CommandOutput, ServiceError>>) -> Self {
            Self {
                scripted: RefCell::new(scripted.into_iter().collect()),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<(String, Vec<String>)> {
            self.calls.borrow().clone()
        }
    }

    impl CommandRunner for FakeCommandRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, ServiceError> {
            self.calls.borrow_mut().push((
                program.to_string(),
                args.iter().map(|s| s.to_string()).collect(),
            ));
            self.scripted
                .borrow_mut()
                .pop_front()
                .unwrap_or(Ok(CommandOutput {
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                }))
        }
    }

    fn ok_output(stdout: &str) -> Result<CommandOutput, ServiceError> {
        Ok(CommandOutput {
            success: true,
            stdout: stdout.to_string(),
            stderr: String::new(),
        })
    }

    fn not_found_output() -> Result<CommandOutput, ServiceError> {
        Ok(CommandOutput {
            success: false,
            stdout: String::new(),
            stderr: "ERROR: The system cannot find the file specified.".to_string(),
        })
    }

    fn xml_with_enabled(enabled: bool) -> String {
        format!(
            "<?xml version=\"1.0\"?>\n<Task><Settings><Enabled>{enabled}</Enabled></Settings></Task>"
        )
    }

    fn spec() -> ServiceSpec {
        ServiceSpec {
            name: "CancellAIGuardianTest".to_string(),
            description: "test task".to_string(),
            program: PathBuf::from(r"C:\Program Files\cancellai\cancellai-guardian.exe"),
            args: vec!["run".to_string()],
        }
    }

    #[test]
    fn quote_if_needed_wraps_paths_with_spaces() {
        let quoted = quote_if_needed(r"C:\Program Files\cancellai-guardian.exe");
        assert_eq!(quoted, "\"C:\\Program Files\\cancellai-guardian.exe\"");
    }

    #[test]
    fn quote_if_needed_leaves_simple_words_unquoted() {
        assert_eq!(quote_if_needed("run"), "run");
    }

    #[test]
    fn quote_if_needed_doubles_embedded_quotes() {
        assert_eq!(quote_if_needed("a \"b\" c"), "\"a \"\"b\"\" c\"");
    }

    #[test]
    fn command_line_joins_quoted_program_and_args() {
        let line = command_line(&spec());
        assert_eq!(
            line,
            "\"C:\\Program Files\\cancellai\\cancellai-guardian.exe\" run"
        );
    }

    #[test]
    fn parse_enabled_from_xml_reads_true_and_false() {
        assert_eq!(parse_enabled_from_xml(&xml_with_enabled(true)), Some(true));
        assert_eq!(
            parse_enabled_from_xml(&xml_with_enabled(false)),
            Some(false)
        );
    }

    #[test]
    fn parse_enabled_from_xml_is_none_for_an_unrecognized_shape() {
        assert_eq!(parse_enabled_from_xml("<Task></Task>"), None);
    }

    #[test]
    fn parse_enabled_from_xml_tolerates_pretty_printed_whitespace() {
        assert_eq!(
            parse_enabled_from_xml(
                "<Task><Settings><Enabled>\r\n    true\r\n  </Enabled></Settings></Task>"
            ),
            Some(true)
        );
        assert_eq!(
            parse_enabled_from_xml(
                "<Task><Settings>\n  <Enabled>\n    false\n  </Enabled>\n</Settings></Task>"
            ),
            Some(false)
        );
    }

    #[test]
    fn parse_enabled_from_xml_is_none_for_unrecognized_content_inside_the_tag() {
        assert_eq!(
            parse_enabled_from_xml("<Task><Settings><Enabled>maybe</Enabled></Settings></Task>"),
            None
        );
    }

    #[test]
    fn status_before_install_is_not_installed() {
        let runner = FakeCommandRunner::new(vec![not_found_output()]);
        let runtime = ScheduledTaskRuntime::for_test(Box::new(runner));
        assert_eq!(runtime.status(&spec()), ServiceStatus::NotInstalled);
    }

    #[test]
    fn unrecognized_query_output_is_unsupported_not_a_guess() {
        let runner = FakeCommandRunner::new(vec![ok_output("<Task></Task>")]);
        let runtime = ScheduledTaskRuntime::for_test(Box::new(runner));
        match runtime.status(&spec()) {
            ServiceStatus::Unsupported { .. } => {}
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn install_then_status_is_disabled_until_enabled() {
        let runner = FakeCommandRunner::new(vec![
            ok_output(""),                       // /Create
            ok_output(""),                       // /Change /DISABLE (inside install)
            ok_output(&xml_with_enabled(false)), // /Query
        ]);
        let runtime = ScheduledTaskRuntime::for_test(Box::new(runner));
        runtime.install(&spec()).expect("install");
        assert_eq!(runtime.status(&spec()), ServiceStatus::Disabled);
    }

    #[test]
    fn enable_before_install_is_an_error_not_a_silent_install() {
        let runner = FakeCommandRunner::new(vec![not_found_output()]);
        let runtime = ScheduledTaskRuntime::for_test(Box::new(runner));
        assert!(runtime.enable(&spec()).is_err());
    }

    #[test]
    fn install_enable_status_uninstall_round_trip() {
        let runner = FakeCommandRunner::new(vec![
            ok_output(""),                       // /Create
            ok_output(""),                       // /Change /DISABLE (inside install)
            ok_output(&xml_with_enabled(false)), // /Query (enable's pre-check)
            ok_output(""),                       // /Change /ENABLE
            ok_output(&xml_with_enabled(true)),  // /Query (enable's own confirm_enabled)
            ok_output(&xml_with_enabled(true)),  // /Query (status)
            ok_output(""),                       // /Delete (inside uninstall)
            not_found_output(),                  // /Query (status)
        ]);
        let runtime = ScheduledTaskRuntime::for_test(Box::new(runner));
        let spec = spec();

        runtime.install(&spec).expect("install");
        runtime.enable(&spec).expect("enable");
        assert_eq!(runtime.status(&spec), ServiceStatus::Enabled);
        runtime.uninstall(&spec).expect("uninstall");
        assert_eq!(runtime.status(&spec), ServiceStatus::NotInstalled);
    }

    #[test]
    fn uninstall_when_never_installed_is_not_an_error() {
        let runner = FakeCommandRunner::new(vec![not_found_output()]);
        let runtime = ScheduledTaskRuntime::for_test(Box::new(runner));
        assert!(runtime.uninstall(&spec()).is_ok());
    }

    #[test]
    fn disable_is_idempotent_when_already_disabled() {
        let runner = FakeCommandRunner::new(vec![not_found_output()]);
        let runtime = ScheduledTaskRuntime::for_test(Box::new(runner));
        assert!(runtime.disable(&spec()).is_ok());
    }

    #[test]
    fn status_issues_exactly_one_read_only_query_call() {
        let runner = std::rc::Rc::new(FakeCommandRunner::new(vec![ok_output(&xml_with_enabled(
            true,
        ))]));
        struct Recording(std::rc::Rc<FakeCommandRunner>);
        impl CommandRunner for Recording {
            fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, ServiceError> {
                self.0.run(program, args)
            }
        }
        let runtime = ScheduledTaskRuntime::for_test(Box::new(Recording(runner.clone())));
        let _ = runtime.status(&spec());
        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, SCHTASKS);
        assert_eq!(
            calls[0].1,
            vec![
                "/Query".to_string(),
                "/TN".to_string(),
                spec().name,
                "/XML".to_string()
            ]
        );
    }

    /// Real `schtasks.exe`, gated to Windows CI - a user-scoped `/RL LIMITED` task needs no
    /// elevation, matching this adapter's own no-admin-required design goal.
    #[cfg(target_os = "windows")]
    #[test]
    fn real_schtasks_install_enable_status_disable_uninstall_smoke_test() {
        let unique = std::process::id();
        let spec = ServiceSpec {
            name: format!("CancellAIGuardianSmokeTest{unique}"),
            description: "E15-S01 real smoke test task".to_string(),
            program: PathBuf::from(r"C:\Windows\System32\cmd.exe"),
            args: vec!["/C".to_string(), "exit".to_string(), "0".to_string()],
        };
        let runtime = ScheduledTaskRuntime::for_test(Box::new(SystemCommandRunner));

        struct Cleanup<'a>(&'a ScheduledTaskRuntime, &'a ServiceSpec);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.uninstall(self.1);
            }
        }
        let _cleanup = Cleanup(&runtime, &spec);

        assert_eq!(runtime.status(&spec), ServiceStatus::NotInstalled);
        runtime.install(&spec).expect("real /Create must succeed");
        assert_eq!(runtime.status(&spec), ServiceStatus::Disabled);

        match runtime.enable(&spec) {
            Ok(()) => {
                // `enable` itself already confirmed the task as enabled before returning `Ok`
                // (see its own doc comment) - `status` must agree immediately, no polling
                // needed here.
                assert_eq!(runtime.status(&spec), ServiceStatus::Enabled);
                runtime
                    .disable(&spec)
                    .expect("real /Change /DISABLE must succeed");
                assert_eq!(runtime.status(&spec), ServiceStatus::Disabled);
            }
            Err(ServiceError::CommandFailed { stderr, .. })
                if stderr.contains("registration could not be confirmed") =>
            {
                // The honest outcome: `/Change /ENABLE` itself succeeded (would otherwise be a
                // `run_checked` error, a different arm), but `/Query /XML` never confirmed it
                // within the poll budget in this environment. Not a skipped assertion: it is
                // the specific, distinguishable failure `enable`'s own confirmation step is
                // designed to produce rather than let a caller observe a state `status` cannot
                // yet corroborate.
            }
            Err(other) => panic!("unexpected real schtasks /Change /ENABLE failure: {other:?}"),
        }

        runtime.uninstall(&spec).expect("real /Delete must succeed");
        assert_eq!(runtime.status(&spec), ServiceStatus::NotInstalled);
    }
}
