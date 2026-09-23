//! How the dashboard's tokenised URL reaches the user without reaching anyone else.
//!
//! The path token is the dashboard's only credential, so it is never written to standard output
//! or standard error, and never passed as a process argument, where other local users can read it
//! in a process listing (E19 round 1). It leaves this process only as a file inside a
//! [`PrivateDir`]: a directory created fresh inside a base that is already private (or sticky),
//! so it is born private and is only ever checked, never re-permissioned. Restricting a file after
//! creating it left a window in which another user could open it (E19 round 2 and its
//! self-review); a `0600` file kept inherited ACL entries on macOS; and restricting the directory
//! by path let a swapped link redirect the permission change onto someone else's directory
//! (self-review 2). Refusing an unsafe base instead of repairing it avoids all three.
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
    /// Creates `cancellai-desktop-<random>` under `base` and confirms it is private before
    /// returning; refuses otherwise, leaving at most an empty directory behind.
    ///
    /// Nothing here changes a permission. An earlier design created the directory and then
    /// restricted it by path; a same-user swap redirected that restriction onto another
    /// directory, rewriting its ACL (E19 self-review 2). Instead the base must already be one no
    /// other account can rename entries in, and the new directory is born private there, so it
    /// is only ever checked - never modified - after creation.
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
        verify_private(base, &path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn refuse(why: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("refusing to write the dashboard URL: {why}"),
    )
}

/// Whether other accounts are unable to rename or replace entries in a base with this owner and
/// mode, for a process running as `me`: it is owned by `me` and nobody else may write it, or it is
/// sticky (like `/tmp`) and owned by `me` or root - the owner of a sticky directory can still
/// rename anything in it, so a sticky directory owned by another account is refused (E19
/// self-review 3).
#[cfg(unix)]
fn base_is_safe(me: u32, base_uid: u32, mode: u32) -> bool {
    let owned_and_closed = base_uid == me && mode & 0o022 == 0;
    let sticky_and_trusted = mode & 0o1000 != 0 && (base_uid == me || base_uid == 0);
    owned_and_closed || sticky_and_trusted
}

/// Unix: the new directory must be a real directory (not a link) with mode `0700`. Its owner is
/// the process's effective user - `mkdir` creates a directory owned by the creating process's
/// effective user id - which is how the process learns its own identity without a `getuid` call
/// (std has none, and this crate has no `libc`). The base must be a real directory that
/// [`base_is_safe`] accepts for that user. On macOS neither may carry an
/// access-control entry, since those are evaluated before the mode bits and can grant others the
/// right to rename or read. Linux default ACLs are masked by the `0700`/`0600` creation modes.
#[cfg(unix)]
fn verify_private(base: &Path, dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let made = std::fs::symlink_metadata(dir)?;
    if !made.file_type().is_dir() || made.permissions().mode() & 0o077 != 0 {
        return Err(refuse("the new directory is not a private directory"));
    }
    let me = made.uid();
    let base_real = std::fs::canonicalize(base)?;
    let base_meta = std::fs::symlink_metadata(&base_real)?;
    let mode = base_meta.permissions().mode();
    if !base_meta.file_type().is_dir() || !base_is_safe(me, base_meta.uid(), mode) {
        return Err(refuse(
            "the base directory lets other users replace entries in it; set TMPDIR to a private directory",
        ));
    }
    #[cfg(target_os = "macos")]
    for checked in [base_real.as_path(), dir] {
        if macos_acl::has_entries(checked)? {
            return Err(refuse(
                "an access-control entry could let other users read or replace the directory",
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn verify_private(base: &Path, dir: &Path) -> io::Result<()> {
    for checked in [base, dir] {
        if !windows_acl::is_private(checked)? {
            return Err(refuse(
                "the directory grants access to other accounts or is a link; set TEMP to a private directory",
            ));
        }
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn verify_private(_base: &Path, _dir: &Path) -> io::Result<()> {
    Err(refuse(
        "no way to confirm a private directory on this platform",
    ))
}

#[cfg(target_os = "macos")]
mod macos_acl {
    use std::io;
    use std::path::Path;
    use std::process::Command;

    /// Whether `path` itself (not what a link points to: `ls -d` does not follow a link named on
    /// the command line) carries any access-control entry. `ls -le` lists them as numbered lines
    /// after the entry line. Read-only.
    pub(super) fn has_entries(path: &Path) -> io::Result<bool> {
        let listing = Command::new("/bin/ls").arg("-led").arg(path).output()?;
        if !listing.status.success() {
            return Err(io::Error::other(
                "could not list the directory's access entries",
            ));
        }
        Ok(String::from_utf8_lossy(&listing.stdout)
            .lines()
            .skip(1)
            .any(|line| !line.trim().is_empty()))
    }
}

/// Windows: read-only. Without FFI (`unsafe` is forbidden outside `cancellai-sealedfs`), the
/// check runs through PowerShell's .NET access to the directory's security descriptor: the
/// directory must not be a reparse point (junction or link), its owner - who can always change its
/// permissions - must be the current user, `SYSTEM` or `Administrators` (E19 self-review 3), and
/// every allow rule, explicit or inherited, must name one of those. Nothing is modified.
/// Conditional access entries are not reported by .NET's rule listing; ADR-0038 discloses that.
#[cfg(windows)]
mod windows_acl {
    use std::io;
    use std::path::Path;
    use std::process::Command;

    // .NET APIs rather than the ACL cmdlets: the cmdlets live in a module Windows PowerShell
    // cannot load when it inherits a PowerShell 7 `PSModulePath` (seen on the CI runner).
    const SCRIPT: &str = "$ErrorActionPreference = 'Stop'; \
        $d = New-Object System.IO.DirectoryInfo($env:CANCELLAI_PRIVATE_PATH); \
        if (-not $d.Exists) { exit 4 }; \
        if (($d.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) { exit 5 }; \
        $me = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value; \
        $allowed = @($me, 'S-1-5-18', 'S-1-5-32-544'); \
        $sec = $d.GetAccessControl(); \
        if ($allowed -notcontains $sec.GetOwner([System.Security.Principal.SecurityIdentifier]).Value) { exit 6 }; \
        $rules = @($sec.GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier])); \
        foreach ($r in $rules) { if ($r.AccessControlType -eq 'Allow' -and $allowed -notcontains $r.IdentityReference.Value) { exit 3 } }; \
        exit 0";

    pub(super) fn is_private(path: &Path) -> io::Result<bool> {
        // The path travels in the environment, not in the command text, so no quoting of a path
        // can change what the script does.
        let status = Command::new(super::windows_powershell())
            .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
            .env("CANCELLAI_PRIVATE_PATH", path)
            .env_remove("PSModulePath")
            .status()?;
        Ok(status.success())
    }
}

/// The system's Windows PowerShell by absolute path, not whatever `powershell.exe` the search path
/// finds first.
#[cfg(windows)]
fn windows_powershell() -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    PathBuf::from(root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe")
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

    #[cfg(unix)]
    #[test]
    fn base_safety_follows_ownership_and_the_sticky_bit() {
        let (me, other, root) = (501, 502, 0);
        assert!(base_is_safe(me, me, 0o700));
        assert!(base_is_safe(me, me, 0o755));
        assert!(!base_is_safe(me, me, 0o775), "group-writable");
        assert!(!base_is_safe(me, me, 0o777), "world-writable");
        assert!(
            !base_is_safe(me, other, 0o700),
            "another account's directory"
        );
        assert!(base_is_safe(me, root, 0o1777), "/tmp");
        assert!(base_is_safe(me, me, 0o1777));
        assert!(
            !base_is_safe(me, other, 0o1777),
            "a sticky directory's owner can still rename entries in it"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_base_other_users_can_write_is_refused_unless_sticky() {
        use std::os::unix::fs::PermissionsExt;
        let base = scratch("shared");
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o777)).expect("chmod");
        let refused = PrivateDir::create(&base).expect_err("a world-writable base");
        assert_eq!(refused.kind(), io::ErrorKind::PermissionDenied);
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o1777)).expect("chmod");
        let dir = PrivateDir::create(&base).expect("a sticky base, like /tmp");
        assert!(dir.path().starts_with(&base));
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        std::fs::remove_dir_all(&base).ok();
    }

    /// The reproduction from the E19 self-reviews: a base that hands an access entry to
    /// everything created in it. The base is refused, and nothing's permissions are changed.
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_a_base_with_access_entries_is_refused_and_left_unchanged() {
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
        let listing = |p: &Path| {
            let out = std::process::Command::new("/bin/ls")
                .arg("-led")
                .arg(p)
                .output()
                .expect("ls");
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .skip(1)
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        let before = listing(&base);
        assert!(!before.is_empty(), "the fixture must carry an entry");
        let refused = PrivateDir::create(&base).expect_err("an ACL'd base");
        assert_eq!(refused.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            listing(&base),
            before,
            "the base's entries must be left exactly as they were"
        );
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
    fn windows_private_directories_hold_only_private_rules_and_broad_bases_are_refused() {
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
        let allowed = [me.as_str(), "S-1-5-18", "S-1-5-32-544"];

        // The per-user temporary directory is private: the directory and its files carry only
        // the user, SYSTEM and Administrators.
        let base = scratch("acl-ok");
        let dir = PrivateDir::create(&base).expect("private dir under the user's temp");
        let url = write_url_file(&dir, "http://127.0.0.1:1/t").expect("url");
        let launcher = Launcher::create(&dir, "http://127.0.0.1:1/t").expect("launcher");
        for path in [dir.path(), url.as_path(), launcher.path()] {
            let sids = acl_sids(path);
            assert!(!sids.is_empty(), "{}", path.display());
            for sid in &sids {
                assert!(
                    allowed.contains(&sid.as_str()),
                    "{} grants {sid}",
                    path.display()
                );
            }
        }
        drop(launcher);
        std::fs::remove_dir_all(&base).ok();

        // A base that grants Everyone read is refused, and left as it was.
        let broad = scratch("acl-broad");
        let broadened = std::process::Command::new("icacls")
            .arg(&broad)
            .args(["/grant", "*S-1-1-0:(OI)(CI)R"])
            .status()
            .expect("icacls");
        assert!(broadened.success());
        let before = acl_sids(&broad);
        let refused = PrivateDir::create(&broad).expect_err("a broad base");
        assert_eq!(refused.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(acl_sids(&broad), before);
        std::fs::remove_dir_all(&broad).ok();
    }
}
