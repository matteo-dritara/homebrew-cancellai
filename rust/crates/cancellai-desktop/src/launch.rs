//! How the dashboard's tokenised URL reaches the user without reaching a log (E19 round 1).
//!
//! The path token is the dashboard's only credential, so it is never written to standard output
//! or standard error, and never passed as a process argument, where other local users can read it
//! in a process listing. It leaves this process in exactly two ways, both files readable only by
//! their owner:
//!
//! - a **launcher page** in the temporary directory: an HTML redirect handed, by path, to the
//!   system browser opener, and overwritten with a page that no longer carries the URL as soon as
//!   the dashboard is first loaded (or on exit). It is emptied rather than deleted: deletion
//!   belongs to the safety executor's one mutation seam (SI-019, `check_mutation_boundary.py`),
//!   and a desktop client has no business holding a second one;
//! - a **URL file** the user names with `--url-file`, for headless use.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The startup line printed on standard output. It names the port, never the token.
pub fn startup_message(port: u16, opened: bool, url_file: Option<&Path>) -> String {
    let mut message = format!("cancellAI dashboard listening on 127.0.0.1:{port} (read-only).");
    if opened {
        message.push_str(" Opened in your browser.");
    }
    if let Some(path) = url_file {
        message.push_str(&format!(" URL written to {}.", path.display()));
    }
    message
}

/// Creates `path` for writing, failing if it already exists, readable only by its owner - and
/// empty. Nothing secret is written until the file is private.
fn create_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    #[cfg(windows)]
    windows_acl::restrict_to_current_user(path)?;
    Ok(file)
}

/// Windows has no mode bits, and a new file inherits whatever its directory grants - a shared or
/// reconfigured directory would hand the token to other local users (E19 round 2). Without an
/// FFI binding (`unsafe` is forbidden outside `cancellai-sealedfs`), the ACL is set and then read
/// back through PowerShell's `Set-Acl`/`Get-Acl`, which ship with every supported Windows: the
/// file's DACL is replaced by one protected (non-inheriting) rule granting the current user's SID
/// full control, and the result is checked to name exactly that SID. Any failure refuses.
#[cfg(windows)]
mod windows_acl {
    use std::io;
    use std::path::Path;
    use std::process::Command;

    const SCRIPT: &str = "$ErrorActionPreference = 'Stop'; \
        $p = $env:CANCELLAI_PRIVATE_PATH; \
        $sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User; \
        $acl = New-Object System.Security.AccessControl.FileSecurity; \
        $acl.SetAccessRuleProtection($true, $false); \
        $acl.AddAccessRule((New-Object System.Security.AccessControl.FileSystemAccessRule($sid, 'FullControl', 'Allow'))); \
        Set-Acl -LiteralPath $p -AclObject $acl; \
        $rules = @((Get-Acl -LiteralPath $p).Access); \
        $ids = @($rules | ForEach-Object { $_.IdentityReference.Translate([System.Security.Principal.SecurityIdentifier]).Value }); \
        if ($ids.Count -ne 1 -or $ids[0] -ne $sid.Value -or $rules[0].AccessControlType -ne 'Allow') { exit 3 }; \
        exit 0";

    pub(super) fn restrict_to_current_user(path: &Path) -> io::Result<()> {
        // The path travels in the environment, not in the command text, so no quoting of a
        // user-chosen path can change what the script does.
        let status = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
            .env("CANCELLAI_PRIVATE_PATH", path)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "could not restrict the file to the current user; refusing to write the dashboard URL",
            ))
        }
    }
}

/// Where secret bytes are written: a file, or (in tests) a writer that fails on purpose.
trait Sink: Write {
    fn sync(&mut self) -> io::Result<()>;
}

impl Sink for File {
    fn sync(&mut self) -> io::Result<()> {
        self.sync_all()
    }
}

/// Empties a freshly created private file unless disarmed - so a write or sync that fails part
/// way, or a panic, cannot leave a token-bearing file behind (E19 round 2). Emptied, not deleted:
/// deletion is the SI-019 seam's alone.
struct ScrubOnFailure<'a> {
    path: &'a Path,
    armed: bool,
}

impl Drop for ScrubOnFailure<'_> {
    fn drop(&mut self) {
        if self.armed {
            scrub(self.path, b"");
        }
    }
}

/// Overwrites `path` with `replacement`. Best effort: it runs on failure and drop paths.
fn scrub(path: &Path, replacement: &[u8]) {
    if let Ok(mut file) = OpenOptions::new().write(true).truncate(true).open(path) {
        let _ = file.write_all(replacement);
        let _ = file.sync_all();
    }
}

/// Creates a private file at `path` and writes `content` to it, or leaves it empty on failure.
fn write_private_with<S: Sink>(
    path: &Path,
    content: &[u8],
    wrap: impl FnOnce(File) -> S,
) -> io::Result<()> {
    let file = create_private(path)?;
    let mut guard = ScrubOnFailure { path, armed: true };
    let mut sink = wrap(file);
    sink.write_all(content)?;
    sink.sync()?;
    drop(sink);
    guard.armed = false;
    Ok(())
}

fn write_private(path: &Path, content: &[u8]) -> io::Result<()> {
    write_private_with(path, content, |file| file)
}

/// Writes `url` to a new owner-only file at `path`. Refuses to overwrite: an existing file could
/// be a link planted to redirect the token somewhere else. On a failed write the file is left
/// empty, never holding part of the URL.
pub fn write_url_file(path: &Path, url: &str) -> io::Result<()> {
    write_private(path, format!("{url}\n").as_bytes())
}

/// A launcher page on disk, emptied of its URL when dropped.
#[derive(Debug)]
pub struct Launcher {
    path: PathBuf,
}

impl Launcher {
    /// Writes an owner-only HTML page in `directory` that redirects to `url`. The file name is
    /// random, so it cannot be predicted and pre-created.
    pub fn create(directory: &Path, url: &str) -> io::Result<Self> {
        Self::create_with(directory, url, |file| file)
    }

    fn create_with<S: Sink>(
        directory: &Path,
        url: &str,
        wrap: impl FnOnce(File) -> S,
    ) -> io::Result<Self> {
        let nonce = cancellai_desktop_api::SessionToken::generate().map_err(io::Error::other)?;
        let nonce = nonce.as_str().get(..16).unwrap_or_default().to_string();
        let path = directory.join(format!("cancellai-desktop-{nonce}.html"));
        let escaped = crate::render::escape(url);
        let page = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\">\
             <meta name=\"referrer\" content=\"no-referrer\">\
             <meta http-equiv=\"refresh\" content=\"0;url={escaped}\"><title>cancellAI</title>\
             </head><body><a href=\"{escaped}\">Open the cancellAI dashboard</a></body></html>"
        );
        write_private_with(&path, page.as_bytes(), wrap)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Overwrites the page with one that no longer carries the URL. Idempotent.
    pub fn expire(&self) {
        scrub(
            &self.path,
            b"<!doctype html><html><head><meta charset=\"utf-8\"><title>cancellAI</title>\
              </head><body>This launcher has expired. Start cancellai-desktop again.</body></html>",
        );
    }
}

impl Drop for Launcher {
    fn drop(&mut self) {
        self.expire();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(label: &str) -> PathBuf {
        let token = cancellai_desktop_api::SessionToken::generate().expect("entropy");
        let dir = std::env::temp_dir().join(format!(
            "cancellai-desktop-launch-{label}-{}",
            token.as_str().get(..12).unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// Writes `good` bytes through, then fails; or fails only at sync.
    struct Faulty {
        inner: File,
        budget: usize,
        fail_sync: bool,
    }

    impl Write for Faulty {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.budget == 0 {
                return Err(io::Error::other("injected write failure"));
            }
            let n = buf.len().min(self.budget);
            self.budget -= n;
            self.inner.write(&buf[..n])
        }

        fn flush(&mut self) -> io::Result<()> {
            self.inner.flush()
        }
    }

    impl Sink for Faulty {
        fn sync(&mut self) -> io::Result<()> {
            self.inner.sync_all()?;
            if self.fail_sync {
                return Err(io::Error::other("injected sync failure"));
            }
            Ok(())
        }
    }

    fn faulty(budget: usize, fail_sync: bool) -> impl FnOnce(File) -> Faulty {
        move |inner| Faulty {
            inner,
            budget,
            fail_sync,
        }
    }

    #[test]
    fn a_failed_launcher_write_or_sync_leaves_no_token_on_disk() {
        let dir = scratch("launcher-fault");
        let url = "http://127.0.0.1:1/launcher-secret-token";
        for (budget, fail_sync) in [(0, false), (40, false), (180, false), (usize::MAX, true)] {
            let result = Launcher::create_with(&dir, url, faulty(budget, fail_sync));
            assert!(result.is_err(), "budget {budget} sync {fail_sync}");
        }
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("list") {
            let text = std::fs::read_to_string(entry.expect("entry").path()).expect("read");
            assert!(
                !text.contains("secret"),
                "a failed launcher kept the token: {text}"
            );
            seen += 1;
        }
        assert_eq!(seen, 4, "every attempt created (and emptied) its file");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_failed_url_file_write_or_sync_leaves_no_url_on_disk() {
        let dir = scratch("url-fault");
        for (i, (budget, fail_sync)) in [(0, false), (10, false), (usize::MAX, true)]
            .into_iter()
            .enumerate()
        {
            let path = dir.join(format!("url-{i}.txt"));
            let written = write_private_with(
                &path,
                b"http://127.0.0.1:1/url-secret-token\n",
                faulty(budget, fail_sync),
            );
            assert!(written.is_err());
            assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(windows)]
    fn acl_sids(path: &Path) -> Vec<String> {
        let output = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "@((Get-Acl -LiteralPath $env:P).Access) | ForEach-Object { \
                 $_.IdentityReference.Translate([System.Security.Principal.SecurityIdentifier]).Value }",
            ])
            .env("P", path)
            .output()
            .expect("powershell");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    }

    #[cfg(windows)]
    #[test]
    fn windows_private_files_grant_only_the_current_user_even_under_a_broad_directory() {
        let dir = scratch("acl");
        // Make the directory deliberately broad: grant Everyone read, inherited by new files.
        let broadened = std::process::Command::new("icacls")
            .arg(&dir)
            .args(["/grant", "*S-1-1-0:(OI)(CI)R"])
            .status()
            .expect("icacls");
        assert!(broadened.success());
        let url_file = dir.join("url.txt");
        write_url_file(&url_file, "http://127.0.0.1:1/t").expect("url file");
        let launcher = Launcher::create(&dir, "http://127.0.0.1:1/t").expect("launcher");
        let me = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value",
            ])
            .output()
            .expect("powershell");
        let me = String::from_utf8_lossy(&me.stdout).trim().to_string();
        for path in [url_file.as_path(), launcher.path()] {
            assert_eq!(acl_sids(path), vec![me.clone()], "{}", path.display());
        }
        drop(launcher);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_startup_message_never_carries_a_token() {
        let token = "f".repeat(64);
        for (opened, file) in [(false, None), (true, Some(Path::new("/tmp/u")))] {
            let message = startup_message(4242, opened, file);
            assert!(message.contains("127.0.0.1:4242"));
            assert!(!message.contains(&token));
            assert!(!message.contains("http://"));
        }
    }

    #[test]
    fn the_url_file_is_private_and_never_overwritten() {
        let dir = scratch("url");
        let path = dir.join("url.txt");
        write_url_file(&path, "http://127.0.0.1:1/secret").expect("written");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "http://127.0.0.1:1/secret\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert!(write_url_file(&path, "http://127.0.0.1:1/other").is_err());
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "http://127.0.0.1:1/secret\n"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_launcher_redirects_is_private_and_forgets_the_url() {
        let dir = scratch("launcher");
        let url = "http://127.0.0.1:1/tok\"en";
        let path = {
            let launcher = Launcher::create(&dir, url).expect("created");
            let page = std::fs::read_to_string(launcher.path()).expect("read");
            assert!(page.contains("url=http://127.0.0.1:1/tok&quot;en"));
            assert!(page.contains("no-referrer"));
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(launcher.path())
                    .expect("meta")
                    .permissions()
                    .mode();
                assert_eq!(mode & 0o777, 0o600);
            }
            launcher.expire();
            let expired = std::fs::read_to_string(launcher.path()).expect("read");
            assert!(!expired.contains("tok"), "{expired}");
            assert!(expired.contains("expired"));
            launcher.path().to_path_buf()
        };
        // Dropping expires it too (a second, idempotent overwrite).
        assert!(
            !std::fs::read_to_string(&path)
                .expect("read")
                .contains("tok")
        );

        let dropped = Launcher::create(&dir, "http://127.0.0.1:1/secret-token").expect("created");
        let dropped_path = dropped.path().to_path_buf();
        drop(dropped);
        assert!(
            !std::fs::read_to_string(&dropped_path)
                .expect("read")
                .contains("secret-token")
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
