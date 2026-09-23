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

/// Creates `path` for writing, failing if it already exists, readable only by its owner.
fn create_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // On Windows a new file inherits its directory's ACL; the per-user temporary directory and a
    // path under the user's profile are readable by that user only.
    options.open(path)
}

/// Writes `url` to a new owner-only file at `path`. Refuses to overwrite: an existing file could
/// be a link planted to redirect the token somewhere else.
pub fn write_url_file(path: &Path, url: &str) -> io::Result<()> {
    let mut file = create_private(path)?;
    file.write_all(url.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()
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
        let nonce = cancellai_desktop_api::SessionToken::generate().map_err(io::Error::other)?;
        let nonce = nonce.as_str().get(..16).unwrap_or_default().to_string();
        let path = directory.join(format!("cancellai-desktop-{nonce}.html"));
        let mut file = create_private(&path)?;
        let escaped = crate::render::escape(url);
        write!(
            file,
            "<!doctype html><html><head><meta charset=\"utf-8\">\
             <meta name=\"referrer\" content=\"no-referrer\">\
             <meta http-equiv=\"refresh\" content=\"0;url={escaped}\"><title>cancellAI</title>\
             </head><body><a href=\"{escaped}\">Open the cancellAI dashboard</a></body></html>"
        )?;
        file.sync_all()?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Overwrites the page with one that no longer carries the URL. Idempotent.
    pub fn expire(&self) {
        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)
        {
            let _ = file.write_all(
                b"<!doctype html><html><head><meta charset=\"utf-8\"><title>cancellAI</title>\
                  </head><body>This launcher has expired. Start cancellai-desktop again.</body></html>",
            );
            let _ = file.sync_all();
        }
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
