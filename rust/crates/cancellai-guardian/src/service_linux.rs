//! Linux adapter: `systemd --user` where available, explicit fallback otherwise
//! (`docs/architecture/GUARDIAN_MODEL.md` Runtime). A WSL2 guest reaches this same adapter - it
//! is a real Linux kernel - and this module never shells out to anything Windows-specific, so
//! `docs/PLATFORMS.md`'s "do not silently install a Windows host service from the Linux guest"
//! holds structurally rather than by a runtime WSL check.
//!
//! "Explicit fallback" (not a silent wrong answer) matters concretely here: this workspace's own
//! Linux CI runners have no active `systemd --user` session bus, so every real invocation in this
//! module's own test suite exercises the fallback path for real, not only in theory.

use std::path::{Path, PathBuf};

use crate::service::{
    CommandOutput, CommandRunner, ServiceError, ServiceRuntime, ServiceSpec, ServiceStatus,
    SystemCommandRunner,
};

const SYSTEMCTL: &str = "systemctl";

pub(crate) struct SystemdUserRuntime {
    runner: Box<dyn CommandRunner>,
    unit_dir: PathBuf,
}

impl Default for SystemdUserRuntime {
    fn default() -> Self {
        Self {
            runner: Box::new(SystemCommandRunner),
            unit_dir: default_unit_dir(),
        }
    }
}

impl SystemdUserRuntime {
    #[cfg(test)]
    fn for_test(runner: Box<dyn CommandRunner>, unit_dir: PathBuf) -> Self {
        Self { runner, unit_dir }
    }

    fn unit_path(&self, spec: &ServiceSpec) -> PathBuf {
        unit_path(&self.unit_dir, &spec.name)
    }

    fn unit_name(spec: &ServiceSpec) -> String {
        format!("{}.service", spec.name)
    }
}

/// `$XDG_CONFIG_HOME/systemd/user`, falling back to `$HOME/.config/systemd/user` per the
/// systemd/XDG convention. An absent `HOME` resolves under `/var/empty`, matching the macOS
/// adapter's "unresolvable home fails the write honestly" behavior rather than guessing.
fn default_unit_dir() -> PathBuf {
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME")
        && !config_home.is_empty()
    {
        return PathBuf::from(config_home).join("systemd").join("user");
    }
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => PathBuf::from(home)
            .join(".config")
            .join("systemd")
            .join("user"),
        _ => PathBuf::from("/var/empty")
            .join(".config")
            .join("systemd")
            .join("user"),
    }
}

fn unit_path(unit_dir: &Path, name: &str) -> PathBuf {
    unit_dir.join(format!("{name}.service"))
}

/// systemd unit-file quoting (`systemd.syntax(7)`): a bare word needs no quoting; a word
/// containing whitespace or a double quote is wrapped in double quotes with `"`/`\` escaped.
/// Scoped to this module's own need (building one `ExecStart=` line from trusted, internal
/// argument strings), not a general systemd config parser.
fn systemd_quote(word: &str) -> String {
    if word.is_empty()
        || word
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '\\')
    {
        let mut quoted = String::with_capacity(word.len() + 2);
        quoted.push('"');
        for ch in word.chars() {
            if ch == '"' || ch == '\\' {
                quoted.push('\\');
            }
            quoted.push(ch);
        }
        quoted.push('"');
        quoted
    } else {
        word.to_string()
    }
}

fn exec_start(spec: &ServiceSpec) -> String {
    let mut parts = vec![systemd_quote(&spec.program.to_string_lossy())];
    parts.extend(spec.args.iter().map(|arg| systemd_quote(arg)));
    parts.join(" ")
}

fn unit_contents(spec: &ServiceSpec) -> String {
    format!(
        "[Unit]\n\
Description={description}\n\
\n\
[Service]\n\
Type=simple\n\
ExecStart={exec_start}\n\
Restart=no\n\
\n\
[Install]\n\
WantedBy=default.target\n",
        description = spec.description.replace('\n', " "),
        exec_start = exec_start(spec),
    )
}

/// `systemctl --user ...` fails identically ("Failed to connect to bus: ...") whether the
/// binary exists but no session bus is reachable - the common case on a headless CI runner with
/// no logind session - or the user's D-Bus session is otherwise unavailable. Matched
/// case-insensitively against the substring systemd itself prints, not the whole message, since
/// the exact wording has changed across systemd releases while this substring has not.
pub(crate) fn indicates_session_bus_unavailable(stderr: &str) -> bool {
    stderr.to_lowercase().contains("failed to connect to bus")
}

impl ServiceRuntime for SystemdUserRuntime {
    fn install(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        let path = self.unit_path(spec);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| ServiceError::Io {
                message: err.to_string(),
            })?;
        }
        std::fs::write(&path, unit_contents(spec)).map_err(|err| ServiceError::Io {
            message: err.to_string(),
        })
    }

    fn uninstall(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        let _ = self.disable(spec);
        let path = self.unit_path(spec);
        let removed = match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(ServiceError::Io {
                message: err.to_string(),
            }),
        };
        // Best-effort, like `enable`'s own reload - a bus-unavailable environment must not turn
        // a successful file removal into a reported failure.
        let _ = self.runner.run(SYSTEMCTL, &["--user", "daemon-reload"]);
        removed
    }

    fn enable(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        let path = self.unit_path(spec);
        if !path.exists() {
            return Err(ServiceError::Io {
                message: format!("cannot enable '{}': not installed", spec.name),
            });
        }
        // Best-effort: a unit file changed since the last reload needs this before `enable`
        // picks it up, but a bus-unavailable environment reports the same failure on both calls
        // - deferring to `enable --now`'s own result below keeps that one honest error path.
        let _ = self.runner.run(SYSTEMCTL, &["--user", "daemon-reload"]);
        let name = Self::unit_name(spec);
        run_checked(
            self.runner.as_ref(),
            SYSTEMCTL,
            &["--user", "enable", "--now", &name],
        )
    }

    fn disable(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        let name = Self::unit_name(spec);
        match self
            .runner
            .run(SYSTEMCTL, &["--user", "disable", "--now", &name])
        {
            Ok(CommandOutput { success: true, .. }) => Ok(()),
            // A unit that was never enabled, or a session bus that is not reachable, both
            // surface as a failed `disable` - `disable` is idempotent by contract, so neither
            // blocks it. `status`/`enable` remain the places a real "systemd is unusable here"
            // signal is reported.
            Ok(CommandOutput { success: false, .. }) => Ok(()),
            Err(ServiceError::CommandFailed { .. }) => Ok(()),
            Err(other) => Err(other),
        }
    }

    fn status(&self, spec: &ServiceSpec) -> ServiceStatus {
        let path = self.unit_path(spec);
        if !path.exists() {
            return ServiceStatus::NotInstalled;
        }
        let name = Self::unit_name(spec);
        match self.runner.run(SYSTEMCTL, &["--user", "is-active", &name]) {
            Err(ServiceError::CommandUnavailable { .. }) => ServiceStatus::Unsupported {
                reason: "systemctl is not available".to_string(),
            },
            Err(other) => ServiceStatus::Unsupported {
                reason: other.to_string(),
            },
            Ok(output) if indicates_session_bus_unavailable(&output.stderr) => {
                ServiceStatus::Unsupported {
                    reason: "no systemd --user session bus is reachable".to_string(),
                }
            }
            Ok(output) if output.stdout.trim() == "active" => ServiceStatus::Enabled,
            Ok(_) => ServiceStatus::Disabled,
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

    fn inactive_output() -> Result<CommandOutput, ServiceError> {
        Ok(CommandOutput {
            success: false,
            stdout: "inactive\n".to_string(),
            stderr: String::new(),
        })
    }

    fn bus_unavailable_output() -> Result<CommandOutput, ServiceError> {
        Ok(CommandOutput {
            success: false,
            stdout: String::new(),
            stderr: "Failed to connect to bus: No such file or directory\n".to_string(),
        })
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "cancellai-guardian-systemd-test-{label}-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn spec() -> ServiceSpec {
        ServiceSpec {
            name: "cancellai-guardian-test".to_string(),
            description: "test unit".to_string(),
            program: PathBuf::from("/usr/local/bin/cancellai-guardian"),
            args: vec!["run".to_string()],
        }
    }

    #[test]
    fn exec_start_quotes_arguments_with_whitespace() {
        let mut spec = spec();
        spec.args = vec!["--label".to_string(), "a value with spaces".to_string()];
        let line = exec_start(&spec);
        assert!(line.contains("\"a value with spaces\""));
    }

    #[test]
    fn exec_start_escapes_embedded_quotes_and_backslashes() {
        let mut spec = spec();
        spec.args = vec!["a \"quoted\" \\value".to_string()];
        let line = exec_start(&spec);
        assert!(line.contains("\\\"quoted\\\""));
        assert!(line.contains("\\\\value"));
    }

    #[test]
    fn bus_unavailable_detection_matches_the_real_systemd_message() {
        assert!(indicates_session_bus_unavailable(
            "Failed to connect to bus: No such file or directory"
        ));
        assert!(!indicates_session_bus_unavailable("Unit not found."));
    }

    #[test]
    fn status_before_install_is_not_installed() {
        let dir = TempDir::new("status-absent");
        let runtime =
            SystemdUserRuntime::for_test(Box::new(FakeCommandRunner::new(vec![])), dir.0.clone());
        assert_eq!(runtime.status(&spec()), ServiceStatus::NotInstalled);
    }

    #[test]
    fn install_then_status_is_disabled_until_enabled() {
        let dir = TempDir::new("install-disabled");
        let runner = FakeCommandRunner::new(vec![inactive_output()]);
        let runtime = SystemdUserRuntime::for_test(Box::new(runner), dir.0.clone());
        runtime.install(&spec()).expect("install must succeed");
        assert_eq!(runtime.status(&spec()), ServiceStatus::Disabled);
    }

    #[test]
    fn status_reports_unsupported_when_the_session_bus_is_unreachable() {
        let dir = TempDir::new("status-unsupported");
        let runner = FakeCommandRunner::new(vec![bus_unavailable_output()]);
        let runtime = SystemdUserRuntime::for_test(Box::new(runner), dir.0.clone());
        runtime.install(&spec()).unwrap();
        match runtime.status(&spec()) {
            ServiceStatus::Unsupported { .. } => {}
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn enable_before_install_is_an_error_not_a_silent_install() {
        let dir = TempDir::new("enable-before-install");
        let runtime =
            SystemdUserRuntime::for_test(Box::new(FakeCommandRunner::new(vec![])), dir.0.clone());
        assert!(runtime.enable(&spec()).is_err());
    }

    #[test]
    fn enable_reports_the_real_error_when_the_session_bus_is_unreachable() {
        let dir = TempDir::new("enable-unsupported");
        // daemon-reload (best-effort, ignored) then enable --now (its result is what matters).
        let runner =
            FakeCommandRunner::new(vec![bus_unavailable_output(), bus_unavailable_output()]);
        let runtime = SystemdUserRuntime::for_test(Box::new(runner), dir.0.clone());
        runtime.install(&spec()).unwrap();
        let err = runtime
            .enable(&spec())
            .expect_err("must surface the bus failure, not succeed");
        match err {
            ServiceError::CommandFailed { stderr, .. } => {
                assert!(indicates_session_bus_unavailable(&stderr));
            }
            other => panic!("expected CommandFailed, got {other:?}"),
        }
    }

    #[test]
    fn install_enable_status_uninstall_round_trip() {
        let dir = TempDir::new("round-trip");
        let runner = FakeCommandRunner::new(vec![
            ok_output(""),         // daemon-reload (enable)
            ok_output(""),         // enable --now
            ok_output("active\n"), // is-active
            ok_output(""),         // disable --now (inside uninstall)
        ]);
        let runtime = SystemdUserRuntime::for_test(Box::new(runner), dir.0.clone());
        let spec = spec();

        runtime.install(&spec).expect("install");
        runtime.enable(&spec).expect("enable");
        assert_eq!(runtime.status(&spec), ServiceStatus::Enabled);
        runtime.uninstall(&spec).expect("uninstall");
        assert_eq!(runtime.status(&spec), ServiceStatus::NotInstalled);
    }

    #[test]
    fn reinstalling_over_an_existing_definition_replaces_it_cleanly() {
        let dir = TempDir::new("reinstall");
        let runtime =
            SystemdUserRuntime::for_test(Box::new(FakeCommandRunner::new(vec![])), dir.0.clone());
        let mut spec = spec();
        runtime.install(&spec).expect("first install");
        spec.args.push("--extra".to_string());
        runtime.install(&spec).expect("second install");
        let contents = std::fs::read_to_string(runtime.unit_path(&spec)).unwrap();
        assert!(contents.contains("--extra"));
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[test]
    fn uninstall_when_never_installed_is_not_an_error() {
        let dir = TempDir::new("uninstall-absent");
        let runtime =
            SystemdUserRuntime::for_test(Box::new(FakeCommandRunner::new(vec![])), dir.0.clone());
        assert!(runtime.uninstall(&spec()).is_ok());
    }

    #[test]
    fn status_issues_exactly_one_read_only_is_active_call() {
        let dir = TempDir::new("status-one-call");
        let runner = std::rc::Rc::new(FakeCommandRunner::new(vec![ok_output("active\n")]));
        struct Recording(std::rc::Rc<FakeCommandRunner>);
        impl CommandRunner for Recording {
            fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, ServiceError> {
                self.0.run(program, args)
            }
        }
        let runtime =
            SystemdUserRuntime::for_test(Box::new(Recording(runner.clone())), dir.0.clone());
        let spec = spec();
        std::fs::write(runtime.unit_path(&spec), "stub").unwrap();
        let _ = runtime.status(&spec);
        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, SYSTEMCTL);
        assert_eq!(
            calls[0].1,
            vec![
                "--user".to_string(),
                "is-active".to_string(),
                SystemdUserRuntime::unit_name(&spec)
            ]
        );
    }

    /// Real `systemctl --user`, gated to Linux CI. This workspace's own CI runners have no
    /// active user session bus, so the honest, expected outcome here is the fallback branch -
    /// this test asserts *that* branch is what actually happens, not a happy path this
    /// environment cannot reach.
    #[cfg(target_os = "linux")]
    #[test]
    fn real_systemd_user_smoke_test_or_explicit_fallback() {
        let dir = TempDir::new("real-smoke");
        let runtime = SystemdUserRuntime::for_test(Box::new(SystemCommandRunner), dir.0.clone());
        let unique = std::process::id();
        let spec = ServiceSpec {
            name: format!("cancellai-guardian-smoketest-{unique}"),
            description: "E15-S01 real smoke test unit".to_string(),
            program: PathBuf::from("/bin/echo"),
            args: vec!["cancellai-guardian-smoke-test".to_string()],
        };

        struct Cleanup<'a>(&'a SystemdUserRuntime, &'a ServiceSpec);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.uninstall(self.1);
            }
        }
        let _cleanup = Cleanup(&runtime, &spec);

        assert_eq!(runtime.status(&spec), ServiceStatus::NotInstalled);
        runtime
            .install(&spec)
            .expect("real install must write the unit file");

        match runtime.enable(&spec) {
            Ok(()) => {
                assert_eq!(runtime.status(&spec), ServiceStatus::Enabled);
                runtime.disable(&spec).expect("real disable must succeed");
                assert_eq!(runtime.status(&spec), ServiceStatus::Disabled);
            }
            Err(ServiceError::CommandFailed { stderr, .. })
                if indicates_session_bus_unavailable(&stderr) =>
            {
                // The expected outcome on this workspace's own CI runners: no session bus, so
                // the fallback must be explicit, not a silent NotInstalled/Enabled guess.
                match runtime.status(&spec) {
                    ServiceStatus::Unsupported { .. } => {}
                    other => panic!(
                        "a bus-unavailable environment must report Unsupported, got {other:?}"
                    ),
                }
            }
            Err(other) => panic!("unexpected enable failure: {other:?}"),
        }

        runtime
            .uninstall(&spec)
            .expect("real uninstall must succeed regardless of bus availability");
        assert_eq!(runtime.status(&spec), ServiceStatus::NotInstalled);
    }
}
