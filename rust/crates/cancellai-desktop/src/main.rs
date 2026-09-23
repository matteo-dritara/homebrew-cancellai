//! `cancellai-desktop`: start the engine's desktop API, then serve the read-only dashboard.
//!
//! ```text
//! cancellai-desktop [--cli <path>] [--open] [--requests <n>]
//! ```
//!
//! `--cli` names the `cancellai-cli` binary (default: `$CANCELLAI_CLI`, then the one beside this
//! executable, then `cancellai-cli` on `PATH`). `--open` asks the operating system to open the
//! dashboard URL in the default browser. `--requests` exits after that many page requests.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, ExitCode, Stdio};

use cancellai_desktop::{ApiViewSource, Dashboard};
use cancellai_desktop_api::Descriptor;

#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    cli: Option<PathBuf>,
    open: bool,
    requests: Option<usize>,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--cli" => {
                options.cli = Some(PathBuf::from(iter.next().ok_or("--cli needs a path")?));
            }
            "--open" => options.open = true,
            "--requests" => {
                let value = iter.next().ok_or("--requests needs a number")?;
                let n: usize = value
                    .parse()
                    .map_err(|_| format!("--requests: not a number: {value}"))?;
                if n == 0 {
                    return Err("--requests must be at least 1".to_string());
                }
                options.requests = Some(n);
            }
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unrecognized argument: {other}")),
        }
    }
    Ok(options)
}

fn cli_path(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    if let Some(path) = std::env::var_os("CANCELLAI_CLI") {
        return PathBuf::from(path);
    }
    let name = format!("cancellai-cli{}", std::env::consts::EXE_SUFFIX);
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&name)))
        .filter(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from(name))
}

/// Kills the engine's API process when the dashboard exits, however it exits.
struct EngineProcess(Child);

impl Drop for EngineProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_engine(cli: &PathBuf) -> Result<(EngineProcess, Descriptor), String> {
    let mut child = Command::new(cli)
        .arg("desktop-api")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start {}: {error}", cli.display()))?;
    let stdout = child.stdout.take();
    let engine = EngineProcess(child);
    let mut line = String::new();
    BufReader::new(stdout.ok_or("the engine's standard output was not captured")?)
        .read_line(&mut line)
        .map_err(|error| format!("could not read the desktop API descriptor: {error}"))?;
    let descriptor = serde_json::from_str(line.trim())
        .map_err(|error| format!("the engine printed no desktop API descriptor: {error}"))?;
    Ok((engine, descriptor))
}

fn open_in_browser(url: &str) {
    let mut command = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(windows) {
        let mut c = Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    } else {
        Command::new("xdg-open")
    };
    if command.arg(url).spawn().is_err() {
        eprintln!("could not open a browser; visit the URL above");
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse(&args) {
        Ok(options) => options,
        Err(message) => {
            if !message.is_empty() {
                eprintln!("{message}");
            }
            eprintln!("usage: cancellai-desktop [--cli <path>] [--open] [--requests <n>]");
            return ExitCode::from(2);
        }
    };
    let (_engine, descriptor) = match start_engine(&cli_path(options.cli)) {
        Ok(started) => started,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(3);
        }
    };
    let dashboard = match Dashboard::bind() {
        Ok(dashboard) => dashboard,
        Err(error) => {
            eprintln!("could not start the dashboard: {error}");
            return ExitCode::from(3);
        }
    };
    let url = dashboard.url();
    println!("{url}");
    if options.open {
        open_in_browser(&url);
    }
    match dashboard.serve(&ApiViewSource::new(descriptor), options.requests) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("the dashboard stopped: {error}");
            ExitCode::from(3)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn options_parse_and_refuse() {
        assert_eq!(parse(&[]), Ok(Options::default()));
        assert_eq!(
            parse(&args(&[
                "--cli",
                "/x/cancellai-cli",
                "--open",
                "--requests",
                "3"
            ])),
            Ok(Options {
                cli: Some(PathBuf::from("/x/cancellai-cli")),
                open: true,
                requests: Some(3)
            })
        );
        for bad in [
            &["--cli"][..],
            &["--requests"],
            &["--requests", "x"],
            &["--requests", "0"],
            &["clean"],
            &["--help"],
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn an_explicit_cli_path_wins() {
        assert_eq!(
            cli_path(Some(PathBuf::from("/opt/cli"))),
            PathBuf::from("/opt/cli")
        );
    }
}
