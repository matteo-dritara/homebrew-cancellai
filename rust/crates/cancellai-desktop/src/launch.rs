//! How the dashboard's tokenised URL reaches the user without reaching anyone else.
//!
//! The path token is the dashboard's only credential, so it is never written to standard output
//! or standard error, and never passed as a process argument, where other local users can read it
//! in a process listing (E19 round 1). It leaves this process only as a file inside a
//! [`PrivateDir`]: a directory this process creates fresh and makes private while it is still
//! empty, before any file exists in it. That ordering is the point. Restricting a file after
//! creating it leaves a window in which another user can open it and keep the handle (E19 round 2
//! and its self-review, on Windows), and a file's own mode does not override access entries it
//! inherited from a shared directory (reproduced on macOS). A file that is created inside an
//! already-private directory has neither problem: nobody else can look it up, and there is nothing
//! for it to inherit but the owner's own access.
//!
//! Two files are written there:
//!
//! - a **launcher page**: an HTML redirect handed, by path, to the system browser opener, and
//!   overwritten with a page that no longer carries the URL as soon as the dashboard is first
//!   loaded (or on exit). Emptied rather than deleted: deletion belongs to the safety executor's
//!   one mutation seam (SI-019, `check_mutation_boundary.py`);
//! - a **URL file**, with `--no-open`, whose path - never its content - is printed.
//!
//! A write or sync that fails part way leaves the file empty, never holding part of the URL.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The startup line printed on standard output. It names the port and, with `--no-open`, the
/// path of the URL file - never the token.
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

/// A random suffix for names nobody else can predict and pre-create.
fn nonce() -> io::Result<String> {
    let token = cancellai_desktop_api::SessionToken::generate().map_err(io::Error::other)?;
    Ok(token.as_str().get(..16).unwrap_or_default().to_string())
}

/// Where this process's private directories go: the per-user runtime directory when the
/// platform has one (`$XDG_RUNTIME_DIR`, owner-only by specification), otherwise the temporary
/// directory. The directory created there is made private either way; the base only decides
/// where it lives.
pub fn default_base() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

/// A directory only the current user can open, created fresh and made private while empty.
#[derive(Debug)]
pub struct PrivateDir {
    path: PathBuf,
}

impl PrivateDir {
    /// Creates `cancellai-desktop-<random>` under `base` and restricts it to the current user
    /// before returning. Refuses - leaving at most an empty directory behind - if the restriction
    /// cannot be applied and confirmed.
    pub fn create(base: &Path) -> io::Result<Self> {
        let path = base.join(format!("cancellai-desktop-{}", nonce()?));
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700);
            builder
        };
        #[cfg(not(unix))]
        let builder = std::fs::DirBuilder::new();
        builder.create(&path)?;
        restrict_directory(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Unix: mode `0700`, confirmed by reading it back (a umask or an odd filesystem must not
/// quietly widen it). macOS additionally drops any access-control entries the directory
/// inherited from its parent - they are evaluated before the mode bits, so `0700` alone does not
/// make it private - and confirms none remain.
#[cfg(unix)]
fn restrict_directory(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    #[cfg(target_os = "macos")]
    macos_acl::strip_and_confirm(path)?;
    let mode = std::fs::metadata(path)?.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "could not make the dashboard's directory private",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn restrict_directory(path: &Path) -> io::Result<()> {
    windows_acl::restrict_to_current_user(path)
}

#[cfg(not(any(unix, windows)))]
fn restrict_directory(_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "no way to make a private directory on this platform",
    ))
}

#[cfg(target_os = "macos")]
mod macos_acl {
    use std::io;
    use std::path::Path;
    use std::process::Command;

    /// `chmod -N` removes every ACL entry; `ls -led` then lists any that remain as numbered
    /// lines (` 0: ...`). The directory is still empty, so nothing inside it was ever exposed.
    pub(super) fn strip_and_confirm(path: &Path) -> io::Result<()> {
        let stripped = Command::new("/bin/chmod").arg("-N").arg(path).status()?;
        let listing = Command::new("/bin/ls").arg("-led").arg(path).output()?;
        let entries_remain = String::from_utf8_lossy(&listing.stdout)
            .lines()
            .skip(1)
            .any(|line| !line.trim().is_empty());
        if stripped.success() && listing.status.success() && !entries_remain {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "could not remove inherited access entries from the dashboard's directory",
            ))
        }
    }
}

/// Windows has no mode bits, and a new directory inherits whatever its parent grants. Without an
/// FFI binding (`unsafe` is forbidden outside `cancellai-sealedfs`), the ACL is set and read back
/// through PowerShell's `Set-Acl`/`Get-Acl`, which ship with every supported Windows: the empty
/// directory's DACL is replaced by one protected rule granting the current user's SID full
/// control, inherited by everything later created inside it, and the result is checked to name
/// exactly that SID. Any failure refuses.
#[cfg(windows)]
mod windows_acl {
    use std::io;
    use std::path::Path;
    use std::process::Command;

    // .NET APIs rather than the `Set-Acl`/`Get-Acl` cmdlets: the cmdlets live in a module that
    // Windows PowerShell cannot load when it inherits a PowerShell 7 `PSModulePath` (seen on the
    // CI runner), whereas `DirectoryInfo` is always there. Rules are read back as SIDs directly.
    const SCRIPT: &str = "$ErrorActionPreference = 'Stop'; \
        $d = New-Object System.IO.DirectoryInfo($env:CANCELLAI_PRIVATE_PATH); \
        $sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User; \
        $acl = New-Object System.Security.AccessControl.DirectorySecurity; \
        $acl.SetAccessRuleProtection($true, $false); \
        $inherit = [System.Security.AccessControl.InheritanceFlags]'ContainerInherit, ObjectInherit'; \
        $none = [System.Security.AccessControl.PropagationFlags]::None; \
        $acl.AddAccessRule((New-Object System.Security.AccessControl.FileSystemAccessRule($sid, 'FullControl', $inherit, $none, 'Allow'))); \
        $d.SetAccessControl($acl); \
        $rules = @($d.GetAccessControl().GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier])); \
        if ($rules.Count -ne 1 -or $rules[0].IdentityReference.Value -ne $sid.Value -or $rules[0].AccessControlType -ne 'Allow') { exit 3 }; \
        exit 0";

    pub(super) fn restrict_to_current_user(path: &Path) -> io::Result<()> {
        // The path travels in the environment, not in the command text, so no quoting of a path
        // can change what the script does.
        let status = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
            .env("CANCELLAI_PRIVATE_PATH", path)
            .env_remove("PSModulePath")
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "could not restrict the dashboard's directory to the current user",
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

/// Empties a freshly created file unless disarmed - so a write or sync that fails part way, or a
/// panic, cannot leave a token-bearing file behind (E19 round 2).
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

/// Overwrites `path` with `replacement`. Best effort: it runs on failure and drop paths. The path
/// is inside a [`PrivateDir`], so nobody else can have replaced it.
fn scrub(path: &Path, replacement: &[u8]) {
    if let Ok(mut file) = OpenOptions::new().write(true).truncate(true).open(path) {
        let _ = file.write_all(replacement);
        let _ = file.sync_all();
    }
}

/// Creates `name` inside `dir` (refusing if it exists) and writes `content`, or leaves it empty.
fn write_private_with<S: Sink>(
    dir: &PrivateDir,
    name: &str,
    content: &[u8],
    wrap: impl FnOnce(File) -> S,
) -> io::Result<PathBuf> {
    let path = dir.path().join(name);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&path)?;
    let mut guard = ScrubOnFailure {
        path: &path,
        armed: true,
    };
    let mut sink = wrap(file);
    sink.write_all(content)?;
    sink.sync()?;
    drop(sink);
    guard.armed = false;
    drop(guard);
    Ok(path)
}

/// Writes `url` to `url.txt` in `dir` and returns its path, for `--no-open`.
pub fn write_url_file(dir: &PrivateDir, url: &str) -> io::Result<PathBuf> {
    write_private_with(dir, "url.txt", format!("{url}\n").as_bytes(), |file| file)
}

/// A launcher page, emptied of its URL when dropped.
#[derive(Debug)]
pub struct Launcher {
    path: PathBuf,
}

impl Launcher {
    /// Writes `launcher.html` in `dir`, redirecting to `url`.
    pub fn create(dir: &PrivateDir, url: &str) -> io::Result<Self> {
        Self::create_with(dir, url, |file| file)
    }

    fn create_with<S: Sink>(
        dir: &PrivateDir,
        url: &str,
        wrap: impl FnOnce(File) -> S,
    ) -> io::Result<Self> {
        let escaped = crate::render::escape(url);
        let page = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\">\
             <meta name=\"referrer\" content=\"no-referrer\">\
             <meta http-equiv=\"refresh\" content=\"0;url={escaped}\"><title>cancellAI</title>\
             </head><body><a href=\"{escaped}\">Open the cancellAI dashboard</a></body></html>"
        );
        let path = write_private_with(dir, "launcher.html", page.as_bytes(), wrap)?;
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
        let dir = std::env::temp_dir().join(format!(
            "cancellai-desktop-launch-{label}-{}",
            nonce().expect("entropy")
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// Writes `budget` bytes through, then fails; or fails only at sync.
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
    fn a_failed_launcher_write_or_sync_leaves_no_token_on_disk() {
        let base = scratch("launcher-fault");
        let url = "http://127.0.0.1:1/launcher-secret-token";
        for (budget, fail_sync) in [(0, false), (40, false), (180, false), (usize::MAX, true)] {
            let dir = PrivateDir::create(&base).expect("private dir");
            let result = Launcher::create_with(&dir, url, faulty(budget, fail_sync));
            assert!(result.is_err(), "budget {budget} sync {fail_sync}");
            let text = std::fs::read_to_string(dir.path().join("launcher.html")).expect("read");
            assert!(
                !text.contains("secret"),
                "a failed launcher kept the token: {text}"
            );
        }
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_failed_url_file_write_or_sync_leaves_no_url_on_disk() {
        let base = scratch("url-fault");
        for (budget, fail_sync) in [(0, false), (10, false), (usize::MAX, true)] {
            let dir = PrivateDir::create(&base).expect("private dir");
            let written = write_private_with(
                &dir,
                "url.txt",
                b"http://127.0.0.1:1/url-secret-token\n",
                faulty(budget, fail_sync),
            );
            assert!(written.is_err());
            assert_eq!(
                std::fs::read_to_string(dir.path().join("url.txt")).expect("read"),
                ""
            );
        }
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn the_url_file_holds_the_url_and_is_never_overwritten() {
        let base = scratch("url");
        let dir = PrivateDir::create(&base).expect("private dir");
        let path = write_url_file(&dir, "http://127.0.0.1:1/secret").expect("written");
        assert_eq!(path, dir.path().join("url.txt"));
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "http://127.0.0.1:1/secret\n"
        );
        assert!(write_url_file(&dir, "http://127.0.0.1:1/other").is_err());
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "http://127.0.0.1:1/secret\n"
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn the_launcher_redirects_and_forgets_the_url() {
        let base = scratch("launcher");
        let dir = PrivateDir::create(&base).expect("private dir");
        let launcher = Launcher::create(&dir, "http://127.0.0.1:1/tok\"en").expect("created");
        let page = std::fs::read_to_string(launcher.path()).expect("read");
        assert!(page.contains("url=http://127.0.0.1:1/tok&quot;en"));
        assert!(page.contains("no-referrer"));
        launcher.expire();
        let expired = std::fs::read_to_string(launcher.path()).expect("read");
        assert!(!expired.contains("tok"), "{expired}");
        assert!(expired.contains("expired"));
        let path = launcher.path().to_path_buf();
        drop(launcher);
        assert!(
            !std::fs::read_to_string(&path)
                .expect("read")
                .contains("tok")
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn private_directories_are_fresh_and_never_reused() {
        let base = scratch("fresh");
        let a = PrivateDir::create(&base).expect("a");
        let b = PrivateDir::create(&base).expect("b");
        assert_ne!(a.path(), b.path());
        assert!(a.path().starts_with(&base));
        std::fs::remove_dir_all(&base).ok();
    }

    #[cfg(unix)]
    #[test]
    fn unix_private_directories_and_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let base = scratch("mode");
        let dir = PrivateDir::create(&base).expect("private dir");
        let file = write_url_file(&dir, "http://127.0.0.1:1/t").expect("written");
        let mode = |p: &Path| std::fs::metadata(p).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode(dir.path()), 0o700);
        assert_eq!(mode(&file), 0o600);
        std::fs::remove_dir_all(&base).ok();
    }

    /// The macOS reproduction from the E19 self-review: a parent directory that hands an
    /// "everyone may read" entry to everything created in it.
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_inherited_access_entries_do_not_reach_the_private_directory_or_its_files() {
        let base = scratch("acl");
        let granted = std::process::Command::new("/bin/chmod")
            .args([
                "+a",
                "everyone allow read,list,search,file_inherit,directory_inherit",
            ])
            .arg(&base)
            .status()
            .expect("chmod");
        assert!(granted.success());
        let entries = |p: &Path| {
            let out = std::process::Command::new("/bin/ls")
                .arg("-led")
                .arg(p)
                .output()
                .expect("ls");
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .skip(1)
                .filter(|l| !l.trim().is_empty())
                .count()
        };
        // Control: without the private directory, a file here inherits the entry.
        let exposed = base.join("control.txt");
        std::fs::write(&exposed, "x").expect("control");
        assert!(
            entries(&exposed) > 0,
            "the fixture must reproduce the exposure"
        );

        let dir = PrivateDir::create(&base).expect("private dir");
        let url = write_url_file(&dir, "http://127.0.0.1:1/t").expect("url");
        let launcher = Launcher::create(&dir, "http://127.0.0.1:1/t").expect("launcher");
        for path in [dir.path(), url.as_path(), launcher.path()] {
            assert_eq!(entries(path), 0, "{} kept an access entry", path.display());
        }
        drop(launcher);
        std::fs::remove_dir_all(&base).ok();
    }

    #[cfg(windows)]
    fn acl_sids(path: &Path) -> Vec<String> {
        let output = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$p = $env:P; \
                 if ([System.IO.Directory]::Exists($p)) { $i = New-Object System.IO.DirectoryInfo($p) } \
                 else { $i = New-Object System.IO.FileInfo($p) }; \
                 @($i.GetAccessControl().GetAccessRules($true, $true, \
                 [System.Security.Principal.SecurityIdentifier])) | ForEach-Object { $_.IdentityReference.Value }",
            ])
            .env("P", path)
            .env_remove("PSModulePath")
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
    fn windows_private_directories_and_files_grant_only_the_current_user_under_a_broad_parent() {
        let base = scratch("acl");
        // Make the parent deliberately broad: Everyone may read, inherited by new children.
        let broadened = std::process::Command::new("icacls")
            .arg(&base)
            .args(["/grant", "*S-1-1-0:(OI)(CI)R"])
            .status()
            .expect("icacls");
        assert!(broadened.success());
        let me = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value",
            ])
            .env_remove("PSModulePath")
            .output()
            .expect("powershell");
        let me = String::from_utf8_lossy(&me.stdout).trim().to_string();
        let dir = PrivateDir::create(&base).expect("private dir");
        let url = write_url_file(&dir, "http://127.0.0.1:1/t").expect("url");
        let launcher = Launcher::create(&dir, "http://127.0.0.1:1/t").expect("launcher");
        for path in [dir.path(), url.as_path(), launcher.path()] {
            assert_eq!(acl_sids(path), vec![me.clone()], "{}", path.display());
        }
        drop(launcher);
        std::fs::remove_dir_all(&base).ok();
    }
}
