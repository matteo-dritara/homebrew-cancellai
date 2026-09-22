//! Cross-platform user-service runtime (E15-S01, `docs/architecture/GUARDIAN_MODEL.md`'s
//! "Runtime" section): one Guardian engine, one consistent [`ServiceRuntime`] contract, with
//! platform-specific adapters selected at compile time - `service_macos`/`service_linux`/
//! `service_windows` never all exist in the same binary, so a Linux build carries no path that
//! could reach the Windows scheduler (`docs/PLATFORMS.md`'s WSL note: "do not silently install
//! a Windows host service from the Linux guest" holds by construction, not by a runtime check).
//!
//! Each adapter splits pure content/argument construction (fully unit-testable on any host, the
//! same `#[cfg(any(test, target_os = "..."))]` split `cancellai_platform::wsl` already uses)
//! from OS invocation behind [`CommandRunner`], so the orchestration logic (install-then-enable
//! sequencing, status parsing, idempotent re-install) is exercised everywhere via
//! [`FakeCommandRunner`], while a real smoke test additionally runs the genuine OS mechanism -
//! but only on the one CI platform it is real on (`#[cfg(target_os = "...")]` gating each
//! platform's own smoke test, matching this workspace's existing Windows/Unix test-gating
//! convention).

use std::fmt;
use std::path::PathBuf;

/// What to install: the label/name a platform scheduler knows the Guardian engine by, and the
/// program/arguments the scheduler should invoke. Never carries detection/authority logic
/// itself - this module only registers *that* a program runs, never *what* it may do once
/// running (`docs/CONSTITUTION.md`: "route mutation through one safety boundary"; Guardian's own
/// decision/authority layers are E15-S03/S04, not this one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSpec {
    /// Stable identifier reused across install/enable/disable/status/uninstall. Must be usable
    /// as a filename component and, on macOS, as a `launchd` reverse-DNS label - callers should
    /// keep it restricted to `[A-Za-z0-9._-]`.
    pub name: String,
    /// Human-readable description carried where the platform mechanism supports one (currently
    /// documentation-only on all three adapters; no mechanism parses it).
    pub description: String,
    /// The executable the platform scheduler should launch.
    pub program: PathBuf,
    /// Arguments passed to `program` verbatim - never interpreted through a shell on any
    /// platform (`std::process::Command::args`, not a shell string), so this module carries no
    /// shell-injection surface even for adversarial content.
    pub args: Vec<String>,
}

/// The Guardian service's installation/activation state, read consistently regardless of which
/// platform adapter produced it (AC1: "enabled/disabled and status inspected consistently").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceStatus {
    /// No service definition exists for this [`ServiceSpec::name`].
    NotInstalled,
    /// A service definition exists but is not currently loaded/active.
    Disabled,
    /// A service definition exists and is loaded/active.
    Enabled,
    /// The platform mechanism this adapter targets is not usable in the current environment
    /// (binary missing, no user session bus, ...). Never collapsed into [`NotInstalled`] or
    /// [`Enabled`] - an unknown mechanism state must read as unknown, not as calm
    /// (`cancellai_guardian::pressure`'s own NaN-never-reads-as-safe precedent, applied here to
    /// service state instead of a pressure signal).
    ///
    /// [`NotInstalled`]: ServiceStatus::NotInstalled
    /// [`Enabled`]: ServiceStatus::Enabled
    Unsupported { reason: String },
}

/// A failure from one lifecycle operation. Always a value, never a panic - AC2 ("service failure
/// does not block manual CLI operation") requires that a Guardian lifecycle failure be an
/// ordinary `Result` a caller can report and move past, not a process abort. `cancellai-cli`
/// does not depend on this crate at all (verified by this crate's own dependency graph position,
/// ADR-0019 outer ring), so no failure here can propagate into a manual CLI invocation by
/// construction; this type exists for the Guardian binary's own callers (its `status`/`install`/
/// ... subcommands) to report without panicking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceError {
    /// The platform command this adapter needs (`launchctl`, `systemctl`, `schtasks`) is not on
    /// `PATH` at all.
    CommandUnavailable { command: &'static str },
    /// The platform command ran but reported failure.
    CommandFailed { command: String, stderr: String },
    /// Writing or removing the service definition file failed (permissions, missing parent
    /// directory, no resolvable home directory, ...).
    Io { message: String },
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceError::CommandUnavailable { command } => {
                write!(f, "required command '{command}' is not available")
            }
            ServiceError::CommandFailed { command, stderr } => {
                write!(f, "command '{command}' failed: {stderr}")
            }
            ServiceError::Io { message } => write!(f, "service definition I/O failed: {message}"),
        }
    }
}

impl std::error::Error for ServiceError {}

/// The consistent lifecycle contract every platform adapter implements identically (AC1).
/// `status` is always read-only - no implementation may invoke a state-changing command from it
/// (verified per-adapter by `status_never_calls_a_mutating_command` below).
pub trait ServiceRuntime {
    /// Write the service definition. Idempotent: installing over an existing definition for the
    /// same name replaces it cleanly rather than duplicating or corrupting it.
    fn install(&self, spec: &ServiceSpec) -> Result<(), ServiceError>;
    /// Remove the service definition. Uninstalling a name that was never installed is not an
    /// error - the postcondition (`status` reads [`ServiceStatus::NotInstalled`]) already holds.
    fn uninstall(&self, spec: &ServiceSpec) -> Result<(), ServiceError>;
    /// Load/activate an installed definition. Calling this before `install` is a
    /// [`ServiceError`], never a silent no-op or an implicit install.
    fn enable(&self, spec: &ServiceSpec) -> Result<(), ServiceError>;
    /// Unload/deactivate a definition, leaving it installed. Idempotent on an
    /// already-disabled service.
    fn disable(&self, spec: &ServiceSpec) -> Result<(), ServiceError>;
    /// Read the current state. Never mutates.
    fn status(&self, spec: &ServiceSpec) -> ServiceStatus;
}

/// Output of one external command invocation, captured rather than left as a raw
/// `std::process::Output` so adapters and fakes share one small shape.
#[derive(Debug, Clone)]
pub(crate) struct CommandOutput {
    pub(crate) success: bool,
    // Read by the Linux (`is-active` output) and Windows (`/XML` output) adapters' own
    // `status()`; the macOS adapter only reads `success`/`stderr`, so a macOS-only production
    // build (this field's only reader compiled out) would otherwise flag it dead.
    #[allow(dead_code)]
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

/// Seam between an adapter's orchestration logic and the real OS process boundary - mirrors
/// this workspace's other observer traits (`cancellai_platform::EnvironmentObserver`,
/// `IdentityObserver`): production code takes `Box<dyn CommandRunner>` and uses
/// [`SystemCommandRunner`]; tests use `FakeCommandRunner` (defined per adapter module, next to
/// the orchestration it exercises) to exercise sequencing and status parsing without a real
/// `launchctl`/`systemctl`/`schtasks` on every host.
pub(crate) trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, ServiceError>;
}

/// The real, process-spawning runner. `program` is looked up on `PATH` by the OS loader, never
/// through a shell - no adapter constructs a shell string anywhere in this module.
#[derive(Debug, Default)]
pub(crate) struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, ServiceError> {
        match std::process::Command::new(program).args(args).output() {
            Ok(output) => Ok(CommandOutput {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(ServiceError::CommandUnavailable {
                    command: leak_static_for_error(program),
                })
            }
            Err(err) => Err(ServiceError::Io {
                message: err.to_string(),
            }),
        }
    }
}

/// [`ServiceError::CommandUnavailable`] names a `&'static str` because every real call site
/// passes a compile-time-known command name (`"launchctl"`, `"systemctl"`, `"schtasks"`) - this
/// never actually leaks memory at runtime for those. Kept as a tiny named function rather than
/// inlined `Box::leak` so the reason is visible where it is used.
fn leak_static_for_error(program: &str) -> &'static str {
    match program {
        "launchctl" => "launchctl",
        "systemctl" => "systemctl",
        "schtasks" => "schtasks",
        _ => "unknown-command",
    }
}

#[cfg(target_os = "macos")]
type PlatformRuntime = crate::service_macos::LaunchdRuntime;
#[cfg(target_os = "linux")]
type PlatformRuntime = crate::service_linux::SystemdUserRuntime;
#[cfg(target_os = "windows")]
type PlatformRuntime = crate::service_windows::ScheduledTaskRuntime;
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
type PlatformRuntime = UnsupportedRuntime;

/// One Guardian engine (the story's own wording): a single public type whose implementation is
/// selected at compile time, never a caller-visible choice between three engines. Not `Debug` -
/// the real adapters hold a `Box<dyn CommandRunner>`, which carries no `Debug` bound.
#[derive(Default)]
pub struct GuardianService {
    inner: PlatformRuntime,
}

impl GuardianService {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ServiceRuntime for GuardianService {
    fn install(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        self.inner.install(spec)
    }

    fn uninstall(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        self.inner.uninstall(spec)
    }

    fn enable(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        self.inner.enable(spec)
    }

    fn disable(&self, spec: &ServiceSpec) -> Result<(), ServiceError> {
        self.inner.disable(spec)
    }

    fn status(&self, spec: &ServiceSpec) -> ServiceStatus {
        self.inner.status(spec)
    }
}

/// Any target this workspace does not name a real adapter for (`docs/PLATFORMS.md`'s tier-2
/// list). Every operation reports [`ServiceStatus::Unsupported`]/[`ServiceError::CommandUnavailable`]
/// honestly rather than pretending one of the three real mechanisms applies.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub(crate) struct UnsupportedRuntime;

#[allow(dead_code)]
impl ServiceRuntime for UnsupportedRuntime {
    fn install(&self, _spec: &ServiceSpec) -> Result<(), ServiceError> {
        Err(ServiceError::CommandUnavailable {
            command: "no user-service mechanism is implemented for this platform",
        })
    }

    fn uninstall(&self, _spec: &ServiceSpec) -> Result<(), ServiceError> {
        Ok(())
    }

    fn enable(&self, _spec: &ServiceSpec) -> Result<(), ServiceError> {
        Err(ServiceError::CommandUnavailable {
            command: "no user-service mechanism is implemented for this platform",
        })
    }

    fn disable(&self, _spec: &ServiceSpec) -> Result<(), ServiceError> {
        Ok(())
    }

    fn status(&self, _spec: &ServiceSpec) -> ServiceStatus {
        ServiceStatus::Unsupported {
            reason: "no user-service mechanism is implemented for this platform".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_unavailable_is_reported_for_a_missing_binary() {
        let runner = SystemCommandRunner;
        let result = runner.run("cancellai-guardian-definitely-not-a-real-binary", &[]);
        assert!(matches!(
            result,
            Err(ServiceError::CommandUnavailable { .. })
        ));
    }

    #[test]
    fn a_real_command_succeeds_and_captures_output() {
        let runner = SystemCommandRunner;
        // `echo` exists on every CI platform this workspace runs on (Windows' cmd builtin
        // included via std::process::Command's PATHEXT resolution) - if this one call cannot
        // run, no adapter in this module could run anything either, so failing loud here beats
        // every adapter test failing separately for the same underlying reason.
        #[cfg(windows)]
        let output = runner.run("cmd", &["/C", "echo hello"]);
        #[cfg(not(windows))]
        let output = runner.run("echo", &["hello"]);
        let output = output.expect("echo must be runnable in this test environment");
        assert!(output.success);
        assert!(output.stdout.contains("hello"));
    }

    #[test]
    fn unsupported_runtime_never_reports_installed_or_enabled() {
        let runtime = UnsupportedRuntime;
        let spec = ServiceSpec {
            name: "test".to_string(),
            description: "test".to_string(),
            program: PathBuf::from("/bin/true"),
            args: vec![],
        };
        assert_eq!(
            runtime.status(&spec),
            ServiceStatus::Unsupported {
                reason: "no user-service mechanism is implemented for this platform".to_string()
            }
        );
        assert!(runtime.install(&spec).is_err());
        assert!(runtime.enable(&spec).is_err());
        // uninstall/disable on a mechanism that was never installed must not error - the
        // postcondition already holds.
        assert!(runtime.uninstall(&spec).is_ok());
        assert!(runtime.disable(&spec).is_ok());
    }

    #[test]
    fn guardian_service_is_the_single_public_engine_type() {
        // Compile-time assertion: GuardianService implements the one shared trait regardless of
        // which platform module backs it - if this did not typecheck on some target, the
        // workspace's cross-platform CI matrix (macOS/Linux/Windows check jobs) would fail to
        // build rather than this test failing at runtime.
        fn assert_runtime<T: ServiceRuntime>() {}
        assert_runtime::<GuardianService>();
    }
}
