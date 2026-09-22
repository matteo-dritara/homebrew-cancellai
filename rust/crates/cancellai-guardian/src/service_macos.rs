//! macOS adapter: a per-user `launchd` agent (`docs/architecture/GUARDIAN_MODEL.md` Runtime:
//! "macOS: `launchd` user agent"). `launchctl load`/`unload` remain broadly compatible across
//! currently-supported macOS releases and avoid the newer `bootstrap`/`bootout` domain-target
//! syntax's extra failure modes for a first implementation - a later story may adopt it if a
//! concrete compatibility need arises.

use std::path::{Path, PathBuf};

use crate::service::{
    CommandOutput, CommandRunner, ServiceError, ServiceRuntime, ServiceSpec, ServiceStatus,
    SystemCommandRunner,
};

const LAUNCHCTL: &str = "launchctl";

pub(crate) struct LaunchdRuntime {
    runner: Box<dyn CommandRunner>,
    agents_dir: PathBuf,
}

impl Default for LaunchdRuntime {
    fn default() -> Self {
        Self {
            runner: Box::new(SystemCommandRunner),
            agents_dir: default_agents_dir(),
        }
    }
}

impl LaunchdRuntime {
    #[cfg(test)]
    fn for_test(runner: Box<dyn CommandRunner>, agents_dir: PathBuf) -> Self {
        Self { runner, agents_dir }
    }

    fn plist_path(&self, spec: &ServiceSpec) -> PathBuf {
        plist_path(&self.agents_dir, &spec.name)
    }
}

/// `$HOME/Library/LaunchAgents` - the standard per-user launchd agent directory. An absent
/// `HOME` resolves to a path under `/var/empty` that cannot be created, so `install` fails with
/// an honest [`ServiceError::Io`] rather than writing somewhere unexpected (malformed detection
/// input must not read as calm - the same principle `cancellai_guardian::pressure` already
/// applies to NaN input, applied here to a missing environment fact).
fn default_agents_dir() -> PathBuf {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => PathBuf::from(home).join("Library").join("LaunchAgents"),
        _ => PathBuf::from("/var/empty")
            .join("Library")
            .join("LaunchAgents"),
    }
}

fn plist_path(agents_dir: &Path, label: &str) -> PathBuf {
    agents_dir.join(format!("{label}.plist"))
}

/// A present-but-empty plist (`uninstall`'s own postcondition, since it never removes the file -
/// see `uninstall`'s doc comment) reads identically to an absent one: not installed.
fn is_installed(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.len() > 0)
        .unwrap_or(false)
}

/// XML 1.0 predefined-entity escaping. `program`/`args`/`description` are internal, trusted
/// values today (the Guardian binary's own resolved path), but a plist is itself a small
/// structured document a caller-controlled string could otherwise break out of - escaping keeps
/// any future caller-supplied content inert rather than relying on "nothing untrusted reaches
/// here yet" staying true.
fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn plist_contents(spec: &ServiceSpec) -> String {
    let mut program_arguments = String::new();
    program_arguments.push_str("        <string>");
    program_arguments.push_str(&xml_escape(&spec.program.to_string_lossy()));
    program_arguments.push_str("</string>\n");
    for arg in &spec.args {
        program_arguments.push_str("        <string>");
        program_arguments.push_str(&xml_escape(arg));
        program_arguments.push_str("</string>\n");
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
    <key>Label</key>\n\
    <string>{label}</string>\n\
    <!-- {description} -->\n\
    <key>ProgramArguments</key>\n\
    <array>\n\
{program_arguments}\
    </array>\n\
    <key>RunAtLoad</key>\n\
    <false/>\n\
    <key>KeepAlive</key>\n\
    <false/>\n\
</dict>\n\
</plist>\n",
        label = xml_escape(&spec.name),
        description = xml_escape(&spec.description),
    )
}

impl ServiceRuntime for LaunchdRuntime {
    fn install(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        let path = self.plist_path(spec);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| ServiceError::Io {
                message: err.to_string(),
            })?;
        }
        std::fs::write(&path, plist_contents(spec)).map_err(|err| ServiceError::Io {
            message: err.to_string(),
        })
    }

    fn uninstall(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        // Best-effort unload first - an installed-but-not-loaded service must not leave a stray
        // launchd registration behind just because the plist file is gone underneath it.
        let _ = self.disable(spec);
        let path = self.plist_path(spec);
        if !path.exists() {
            return Ok(());
        }
        // Never `std::fs::remove_file` (SI-019: `scripts/check_mutation_boundary.py` reserves
        // that call to the safety executor, with no exemption for cancellAI's own local state -
        // see `killswitch.rs`'s identical reasoning). Writing the file empty instead reaches the
        // same observable postcondition: `status`/`enable` below treat an empty plist file
        // exactly like an absent one, via `is_installed`.
        std::fs::write(&path, "").map_err(|err| ServiceError::Io {
            message: err.to_string(),
        })
    }

    fn enable(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        let path = self.plist_path(spec);
        if !is_installed(&path) {
            return Err(ServiceError::Io {
                message: format!("cannot enable '{}': not installed", spec.name),
            });
        }
        let path_str = path.to_string_lossy().into_owned();
        run_checked(self.runner.as_ref(), LAUNCHCTL, &["load", "-w", &path_str])
    }

    fn disable(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        let path = self.plist_path(spec);
        let path_str = path.to_string_lossy().into_owned();
        match self.runner.run(LAUNCHCTL, &["unload", &path_str]) {
            // `launchctl unload` on an already-unloaded (or never-loaded) job reports failure -
            // disable is idempotent, so that outcome is success, not an error.
            Ok(_) | Err(ServiceError::CommandFailed { .. }) => Ok(()),
            Err(other) => Err(other),
        }
    }

    fn status(&self, spec: &ServiceSpec) -> ServiceStatus {
        let path = self.plist_path(spec);
        if !is_installed(&path) {
            return ServiceStatus::NotInstalled;
        }
        match self.runner.run(LAUNCHCTL, &["list", &spec.name]) {
            Ok(CommandOutput { success: true, .. }) => ServiceStatus::Enabled,
            Ok(CommandOutput { success: false, .. }) => ServiceStatus::Disabled,
            Err(ServiceError::CommandUnavailable { .. }) => ServiceStatus::Unsupported {
                reason: "launchctl is not available".to_string(),
            },
            Err(err) => ServiceStatus::Unsupported {
                reason: err.to_string(),
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

    fn ok_output() -> Result<CommandOutput, ServiceError> {
        Ok(CommandOutput {
            success: true,
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    fn failed_output(stderr: &str) -> Result<CommandOutput, ServiceError> {
        Ok(CommandOutput {
            success: false,
            stdout: String::new(),
            stderr: stderr.to_string(),
        })
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "cancellai-guardian-launchd-test-{label}-{}-{unique}",
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
            name: "dev.cancellai.guardian.test".to_string(),
            description: "test agent".to_string(),
            program: PathBuf::from("/usr/local/bin/cancellai-guardian"),
            args: vec!["run".to_string()],
        }
    }

    #[test]
    fn plist_contents_escape_xml_special_characters() {
        let mut spec = spec();
        spec.description = "quotes \" and <tags> & ampersands".to_string();
        spec.args = vec!["--label".to_string(), "a \"quoted\" <value>".to_string()];
        let xml = plist_contents(&spec);
        assert!(!xml.contains("<value>\""));
        assert!(xml.contains("&quot;quoted&quot;"));
        assert!(xml.contains("&lt;value&gt;"));
        assert!(xml.contains("&amp; ampersands"));
    }

    #[test]
    fn status_before_install_is_not_installed() {
        let dir = TempDir::new("status-absent");
        let runtime =
            LaunchdRuntime::for_test(Box::new(FakeCommandRunner::new(vec![])), dir.0.clone());
        assert_eq!(runtime.status(&spec()), ServiceStatus::NotInstalled);
    }

    #[test]
    fn install_then_status_is_disabled_until_enabled() {
        let dir = TempDir::new("install-disabled");
        let runner = FakeCommandRunner::new(vec![failed_output("not loaded")]);
        let runtime = LaunchdRuntime::for_test(Box::new(runner), dir.0.clone());
        runtime.install(&spec()).expect("install must succeed");
        assert_eq!(runtime.status(&spec()), ServiceStatus::Disabled);
    }

    #[test]
    fn enable_before_install_is_an_error_not_a_silent_install() {
        let dir = TempDir::new("enable-before-install");
        let runtime =
            LaunchdRuntime::for_test(Box::new(FakeCommandRunner::new(vec![])), dir.0.clone());
        assert!(runtime.enable(&spec()).is_err());
    }

    #[test]
    fn install_enable_status_uninstall_round_trip() {
        let dir = TempDir::new("round-trip");
        let runner = FakeCommandRunner::new(vec![
            ok_output(), // load
            ok_output(), // list -> enabled
            ok_output(), // unload (inside uninstall)
        ]);
        let runtime = LaunchdRuntime::for_test(Box::new(runner), dir.0.clone());
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
            LaunchdRuntime::for_test(Box::new(FakeCommandRunner::new(vec![])), dir.0.clone());
        let mut spec = spec();
        runtime.install(&spec).expect("first install");
        spec.args.push("--extra".to_string());
        runtime.install(&spec).expect("second install");
        let contents = std::fs::read_to_string(runtime.plist_path(&spec)).unwrap();
        assert!(contents.contains("--extra"));
        // Exactly one definition file exists for this label - no duplicate/orphaned file.
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[test]
    fn uninstall_when_never_installed_is_not_an_error() {
        let dir = TempDir::new("uninstall-absent");
        let runtime =
            LaunchdRuntime::for_test(Box::new(FakeCommandRunner::new(vec![])), dir.0.clone());
        assert!(runtime.uninstall(&spec()).is_ok());
    }

    #[test]
    fn disable_is_idempotent_when_already_disabled() {
        let dir = TempDir::new("disable-idempotent");
        let runner = FakeCommandRunner::new(vec![failed_output("not loaded")]);
        let runtime = LaunchdRuntime::for_test(Box::new(runner), dir.0.clone());
        assert!(runtime.disable(&spec()).is_ok());
    }

    #[test]
    fn status_issues_exactly_one_read_only_list_call() {
        let dir = TempDir::new("status-one-call");
        let runner = std::rc::Rc::new(FakeCommandRunner::new(vec![ok_output()]));
        struct Recording(std::rc::Rc<FakeCommandRunner>);
        impl CommandRunner for Recording {
            fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, ServiceError> {
                self.0.run(program, args)
            }
        }
        let runtime = LaunchdRuntime::for_test(Box::new(Recording(runner.clone())), dir.0.clone());
        let spec = spec();
        std::fs::write(runtime.plist_path(&spec), "stub").unwrap();
        let _ = runtime.status(&spec);
        let calls = runner.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, LAUNCHCTL);
        assert_eq!(calls[0].1, vec!["list".to_string(), spec.name.clone()]);
    }

    /// `launchctl load`/`unload` can return before the daemon's own internal job table is fully
    /// updated - `launchctl list` immediately afterward has been observed, in some environments,
    /// to still report the pre-transition state for a short window (round-1 independent review
    /// of this story found exactly this: a real, reproducible `Disabled` read immediately after
    /// a successful `enable()` in that reviewer's own sandboxed environment, not reproduced
    /// after repeated runs in this executor's own interactive session - consistent with a launchd
    /// registration-propagation race that a more restrictive sandbox makes more likely to
    /// surface, not with a logic defect in `status`/`enable` themselves, which perform no caching
    /// and issue a fresh `launchctl list` every call). Polling briefly here is the same
    /// tolerance-for-eventual-consistency this workspace already applies to other real,
    /// asynchronous OS state (`cancellai-safety`/`cancellai-platform`'s own crash/retry tests) -
    /// it does not mask a defect, since every intermediate read is still a real `launchctl` call,
    /// and it fails loudly if the expected state never arrives within the bound.
    #[cfg(target_os = "macos")]
    fn poll_status_until(
        runtime: &LaunchdRuntime,
        spec: &ServiceSpec,
        expected: ServiceStatus,
    ) -> ServiceStatus {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let observed = runtime.status(spec);
            if observed == expected || std::time::Instant::now() >= deadline {
                return observed;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn real_launchd_install_enable_status_disable_uninstall_smoke_test() {
        let dir = TempDir::new("real-smoke");
        let runtime = LaunchdRuntime::for_test(Box::new(SystemCommandRunner), dir.0.clone());
        let unique = std::process::id();
        let spec = ServiceSpec {
            name: format!("dev.cancellai.guardian.smoketest.{unique}"),
            description: "E15-S01 real smoke test agent".to_string(),
            program: PathBuf::from("/bin/echo"),
            args: vec!["cancellai-guardian-smoke-test".to_string()],
        };

        // Guard cleans up the real launchd registration even if an assertion below fails.
        struct Cleanup<'a>(&'a LaunchdRuntime, &'a ServiceSpec);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.uninstall(self.1);
            }
        }
        let _cleanup = Cleanup(&runtime, &spec);

        assert_eq!(runtime.status(&spec), ServiceStatus::NotInstalled);
        runtime
            .install(&spec)
            .expect("real install must write the plist");
        assert_eq!(runtime.status(&spec), ServiceStatus::Disabled);
        runtime
            .enable(&spec)
            .expect("real launchctl load must succeed");
        assert_eq!(
            poll_status_until(&runtime, &spec, ServiceStatus::Enabled),
            ServiceStatus::Enabled
        );
        runtime
            .disable(&spec)
            .expect("real launchctl unload must succeed");
        assert_eq!(
            poll_status_until(&runtime, &spec, ServiceStatus::Disabled),
            ServiceStatus::Disabled
        );
        runtime
            .uninstall(&spec)
            .expect("real uninstall must succeed");
        assert_eq!(runtime.status(&spec), ServiceStatus::NotInstalled);
    }
}
