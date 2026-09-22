//! Later user-service runtime (`docs/architecture/GUARDIAN_MODEL.md`): predictive
//! pressure/anomaly signals and bounded, safety-floor-respecting remediation
//! (`SI-027`, `SI-028` - detection severity and Guardian never self-escalate authority).
//!
//! E15-S01 adds the first real command surface: `install`/`uninstall`/`enable`/`disable`/
//! `status` manage this binary's own OS-native user-service registration
//! (`cancellai_guardian::service`). `run` remains the E02-S01 skeleton - the detection/decision/
//! authority loop a real installed service would invoke is later Guardian scope (E15-S03/S04),
//! not this story's.

use cancellai_guardian::service::{
    GuardianService, ServiceError, ServiceRuntime, ServiceSpec, ServiceStatus,
};
use cancellai_model as _;
use cancellai_policy as _;
use cancellai_safety as _;
use cancellai_store as _;

/// One stable identifier reused as the macOS launchd label, the Linux systemd unit name, and the
/// Windows scheduled task name - see `cancellai_guardian::service`'s module docs for why a
/// single string works unmodified across all three.
const SERVICE_NAME: &str = "dev.cancellai.guardian";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(&args));
}

fn run(args: &[String]) -> i32 {
    let Some(subcommand) = args.first().map(String::as_str) else {
        print_usage();
        return 2;
    };
    match subcommand {
        "install" => dispatch(|service, spec| service.install(spec)),
        "uninstall" => dispatch(|service, spec| service.uninstall(spec)),
        "enable" => dispatch(|service, spec| service.enable(spec)),
        "disable" => dispatch(|service, spec| service.disable(spec)),
        "status" => status(),
        "run" => {
            println!("cancellai-guardian: workspace skeleton (E02-S01), not yet implemented");
            0
        }
        _ => {
            print_usage();
            2
        }
    }
}

fn print_usage() {
    eprintln!(
        "usage: cancellai-guardian <install|uninstall|enable|disable|status|run>\n\n\
install/uninstall/enable/disable/status manage this binary's own OS-native user-service\n\
registration ({SERVICE_NAME}); run is the detection/remediation entry point (not yet\n\
implemented)."
    );
}

fn dispatch(op: impl FnOnce(&GuardianService, &ServiceSpec) -> Result<(), ServiceError>) -> i32 {
    let spec = match service_spec() {
        Ok(spec) => spec,
        Err(message) => {
            eprintln!("cancellai-guardian: {message}");
            return 3;
        }
    };
    match op(&GuardianService::new(), &spec) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("cancellai-guardian: {err}");
            3
        }
    }
}

fn status() -> i32 {
    let spec = match service_spec() {
        Ok(spec) => spec,
        Err(message) => {
            eprintln!("cancellai-guardian: {message}");
            return 3;
        }
    };
    match GuardianService::new().status(&spec) {
        ServiceStatus::NotInstalled => {
            println!("not installed");
            0
        }
        ServiceStatus::Disabled => {
            println!("installed, disabled");
            0
        }
        ServiceStatus::Enabled => {
            println!("installed, enabled");
            0
        }
        ServiceStatus::Unsupported { reason } => {
            println!("unsupported: {reason}");
            0
        }
    }
}

fn service_spec() -> Result<ServiceSpec, String> {
    let program = std::env::current_exe()
        .map_err(|err| format!("cannot resolve this binary's own path: {err}"))?;
    Ok(ServiceSpec {
        name: SERVICE_NAME.to_string(),
        description: "cancellAI Guardian - local pressure/anomaly monitoring".to_string(),
        program,
        args: vec!["run".to_string()],
    })
}
