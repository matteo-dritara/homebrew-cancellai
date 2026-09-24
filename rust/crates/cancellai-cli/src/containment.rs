//! Signed incident containment on the live mutation path (E06-S07, ADR-0039).
//!
//! Three things live here, and nothing else in the CLI touches containment:
//!
//! - [`LedgerState::load`]: the persisted history (`cancellai-store::containment_state`),
//!   replayed and re-verified by the kernel against the compiled project key plus the owner's
//!   trusted publishers. A missing history is an empty ledger; one that exists and cannot be read,
//!   parsed or re-verified - and an unreadable trust file - is [`LedgerState::Unknown`], never
//!   empty (C-01).
//! - [`apply_authority`]: every `Delete` action's authority is computed with
//!   `effective_authority_under_containment` from the artifact's own facts, the build's compiled
//!   release channel and the ledger, instead of being asserted. An action below
//!   `minimum_authority_for(Delete)` becomes `Observe` with the constraint and incidents that
//!   capped it. `plan` and `clean` both call it, so a preview never differs from a run.
//! - The `containment install|list|lift` commands.

use std::collections::HashMap;

use cancellai_model::{Action, ActionClass, ArtifactId, AuthorityLevel, Reversibility};
use cancellai_policy::{ClassifiedArtifact, builtin_provider_trust};
use cancellai_safety::authority::minimum_authority_for;
use cancellai_safety::{
    AuthorityInputs, BuildChannel, ContainmentEvent, ContainmentLedger, ContainmentTarget,
    IncidentPlatform, MAX_NOTICE_BYTES, containment_trust_policy,
    effective_authority_under_containment,
};
use cancellai_store::LocalStateRoot;
use cancellai_store::containment_state::{self, Load, StoredEvent};

/// The cancellAI incident-response public key (publisher `cancellai-incident`, ADR-0039). The
/// private half was generated on 2026-09-23 without touching disk in plaintext and is held
/// encrypted to the owner's key, outside the repository and CI.
const PROJECT_INCIDENT_KEY_HEX: &str =
    "d9a118c0a0d0fcb56dd2fb3f13ae3ca7d67dcfa604335dd1c3d5fae0e0f1f993";

fn decode_key(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut key = [0u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        let pair = hex.get(index * 2..index * 2 + 2)?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(key)
}

fn this_platform() -> IncidentPlatform {
    if cfg!(target_os = "macos") {
        IncidentPlatform::Macos
    } else if cfg!(windows) {
        IncidentPlatform::Windows
    } else {
        IncidentPlatform::Linux
    }
}

/// What the persisted containment state says about this run.
pub enum LedgerState {
    Known(ContainmentLedger),
    /// The history or the trust file exists and could not be used. Every action is capped at
    /// `Recommend` until the owner repairs or removes it.
    Unknown(String),
}

fn trust_policy(root: &LocalStateRoot) -> Result<cancellai_safety::LocalTrustPolicy, String> {
    let project = decode_key(PROJECT_INCIDENT_KEY_HEX)
        .ok_or_else(|| "the compiled incident-response key is malformed".to_string())?;
    let owner = match containment_state::load_trust(root) {
        Load::Missing => Vec::new(),
        Load::Unreadable(reason) => return Err(format!("owner trust file: {reason}")),
        Load::Found(entries) => {
            let mut owner = Vec::new();
            for entry in entries {
                let key = decode_key(&entry.public_key).ok_or_else(|| {
                    format!(
                        "owner trust file: {} does not have a 64-hex-digit public key",
                        entry.publisher_id
                    )
                })?;
                owner.push((entry.publisher_id, key));
            }
            owner
        }
    };
    containment_trust_policy(project, owner).map_err(|e| format!("owner trust file: {e}"))
}

fn events_of(stored: Vec<StoredEvent>) -> Vec<ContainmentEvent> {
    stored
        .into_iter()
        .map(|event| match event {
            StoredEvent::Install(text) => ContainmentEvent::Install(text),
            StoredEvent::Lift(incident) => ContainmentEvent::Lift(incident),
        })
        .collect()
}

/// The ledger replayed from one read of the history, and that history's chain head - the head an
/// append decided on this ledger must name (E06 review round 8).
fn snapshot(root: &LocalStateRoot) -> Result<(ContainmentLedger, String), String> {
    let policy = trust_policy(root)?;
    match containment_state::load_history(root) {
        Load::Missing => Ok((ContainmentLedger::empty(), containment_state::empty_head())),
        Load::Unreadable(reason) => Err(format!("containment history: {reason}")),
        Load::Found(history) => {
            match ContainmentLedger::replay(&events_of(history.events), &policy) {
                Ok(ledger) => Ok((ledger, history.head)),
                Err((index, error)) => Err(format!(
                    "containment history event {} does not re-verify: {error}",
                    index + 1
                )),
            }
        }
    }
}

fn replay_at(root: &LocalStateRoot) -> LedgerState {
    match snapshot(root) {
        Ok((ledger, _head)) => LedgerState::Known(ledger),
        Err(reason) => LedgerState::Unknown(reason),
    }
}

impl LedgerState {
    /// The ledger as persisted right now. Called by `plan`, and by `clean` both when planning and
    /// again immediately before every deletion, so a notice installed while `clean` waits at its
    /// confirmation prompt - or between two deletions - binds the deletions after it.
    pub fn load() -> Self {
        match LocalStateRoot::existing_platform_default() {
            Ok(None) => LedgerState::Known(ContainmentLedger::empty()),
            Ok(Some(root)) => replay_at(&root),
            Err(error) => LedgerState::Unknown(error.to_string()),
        }
    }
}

/// The authority `classified` may be deleted at under `state`, and the constraints that set it.
fn delete_authority(
    classified: &ClassifiedArtifact,
    state: &LedgerState,
) -> (AuthorityLevel, Vec<String>) {
    let artifact = &classified.artifact;
    let inputs = AuthorityInputs {
        user_requested: AuthorityLevel::Govern,
        artifact_ceiling: artifact.authority_ceiling,
        confidence: artifact.knowledge_confidence,
        activity: artifact.activity_state,
        protection: artifact.protection_state,
        integrity: artifact.integrity_state,
        provider_trust: builtin_provider_trust(),
    };
    let target = ContainmentTarget {
        provider_id: &artifact.provider_id,
        provider_version: None,
        action_class: ActionClass::Delete,
        platform: this_platform(),
    };
    let channel = BuildChannel::from_compiled_env();
    match state {
        LedgerState::Known(ledger) => {
            let effective = effective_authority_under_containment(inputs, channel, ledger, &target);
            let mut why: Vec<String> = effective
                .binding_constraints
                .iter()
                .map(|name| (*name).to_string())
                .collect();
            if let Some(binding) = ledger.binding_for(&target) {
                why.push(format!("incidents {}", binding.incidents.join(", ")));
            }
            (effective.level, why)
        }
        LedgerState::Unknown(reason) => {
            let empty = ContainmentLedger::empty();
            let effective = effective_authority_under_containment(inputs, channel, &empty, &target);
            (
                effective.level.min(AuthorityLevel::Recommend),
                vec![format!("containment_ledger_unknown ({reason})")],
            )
        }
    }
}

fn withhold(action: &mut Action, why: &str) {
    action.action_class = ActionClass::Observe;
    action.authority = AuthorityLevel::Observe;
    action.reversibility = Reversibility::Rebuildable;
    action.execution_preconditions.clear();
    action.reason = format!("deletion withheld: {why}");
}

/// Computes every `Delete` action's authority under `state` and withholds each one that falls
/// below what deletion requires. Returns whether anything was withheld. An action whose target
/// this run did not classify is withheld too: no fact, no authority.
pub fn apply_authority(
    actions: &mut [Action],
    by_id: &HashMap<ArtifactId, &ClassifiedArtifact>,
    state: &LedgerState,
) -> bool {
    let mut any = false;
    let floor = minimum_authority_for(ActionClass::Delete);
    for action in actions.iter_mut() {
        if action.action_class != ActionClass::Delete {
            continue;
        }
        let Some(classified) = action
            .target_artifact_ids
            .first()
            .and_then(|id| by_id.get(id))
        else {
            withhold(action, "its target was not classified in this run");
            any = true;
            continue;
        };
        let (level, why) = delete_authority(classified, state);
        if level < floor {
            withhold(
                action,
                &format!(
                    "effective authority {level:?} is below the {floor:?} deletion requires; capped by {}",
                    why.join("; ")
                ),
            );
            any = true;
        } else {
            action.authority = level;
        }
    }
    any
}

/// Re-checks one planned deletion against the ledger as persisted now, immediately before it
/// runs. `Err` carries the reason it may no longer run.
pub fn recheck(classified: &ClassifiedArtifact) -> Result<AuthorityLevel, String> {
    let state = LedgerState::load();
    let (level, why) = delete_authority(classified, &state);
    if level < minimum_authority_for(ActionClass::Delete) {
        Err(format!(
            "CONTAINED: effective authority {level:?} at deletion time; capped by {}",
            why.join("; ")
        ))
    } else {
        Ok(level)
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read_notice(path: &std::path::Path) -> Result<String, String> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_NOTICE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() > MAX_NOTICE_BYTES {
        return Err(format!(
            "{} is larger than {MAX_NOTICE_BYTES} bytes; refused before parsing",
            path.display()
        ));
    }
    String::from_utf8(bytes).map_err(|_| format!("{} is not UTF-8", path.display()))
}

/// Exit codes: 0 success, 2 invalid input, 4 refused for safety.
pub fn cmd_install(path: &std::path::Path) -> Result<Vec<String>, (i32, String)> {
    let text = read_notice(path).map_err(|e| (4, e))?;
    let root = LocalStateRoot::resolve_platform_default().map_err(|e| (4, e.to_string()))?;
    install_text(&root, text, false)
}

/// Verifies `text` against the current history and trust, and appends it on success - the one
/// path both `install` and `refresh` use.
fn install_text(
    root: &LocalStateRoot,
    text: String,
    from_feed: bool,
) -> Result<Vec<String>, (i32, String)> {
    let (mut ledger, head) = snapshot(root).map_err(|reason| {
        (
            4,
            format!("refusing to install into an unusable ledger: {reason}"),
        )
    })?;
    let policy = trust_policy(root).map_err(|e| (4, e))?;
    let evidence = match ledger.install(&text, &policy, now_unix()) {
        Ok(evidence) => evidence,
        // A refresh racing another refresh of the same feed may read the history after the
        // other one installed this very notice: that is current, not a replay (E06 review
        // round 8). An explicit `install` of an installed notice stays a refused replay.
        Err(cancellai_safety::ContainmentError::Replayed)
            if from_feed && installed(root, &text) =>
        {
            return Ok(vec![
                "already current: the same notice is installed".to_string(),
            ]);
        }
        Err(error) => return Err((4, error.to_string())),
    };
    let nonce = containment_state::append_event(root, &StoredEvent::Install(text.clone()), &head)
        .map_err(|e| (4, format!("could not persist the containment: {e}")))?;
    // Read it back: the persisted history, not the in-memory ledger, is what later runs obey.
    match replay_at(root) {
        LedgerState::Known(_) => {}
        LedgerState::Unknown(reason) => {
            return Err((
                4,
                format!("the containment was written but the history no longer replays: {reason}"),
            ));
        }
    }
    // A concurrent change may have been appended first, in which case this line is not part of
    // the history (E06 review round 8). The same notice installed by the winner is current.
    if !appended(root, &nonce) {
        if installed(root, &text) {
            return Ok(vec![
                "already current: the same notice was installed concurrently".to_string(),
            ]);
        }
        return Err((
            4,
            "a concurrent containment change was recorded first; this install was not applied - retry"
                .to_string(),
        ));
    }
    Ok(evidence
        .iter()
        .map(|record| {
            format!(
                "contained {} for {} at {:?}",
                record.incident_id(),
                record.provider_id(),
                record.ceiling()
            )
        })
        .collect())
}

pub fn cmd_list() -> Result<Vec<String>, (i32, String)> {
    match LedgerState::load() {
        LedgerState::Unknown(reason) => Err((4, format!("containment state is unknown: {reason}"))),
        LedgerState::Known(ledger) => Ok(ledger
            .active()
            .map(|record| {
                format!(
                    "{} {:?} provider={} ceiling={:?} versions={:?} actions={:?} platforms={:?} publisher={} sequence={}",
                    record.incident_id(),
                    record.severity(),
                    record.provider_id(),
                    record.ceiling(),
                    record.provider_versions(),
                    record.action_classes(),
                    record.platforms(),
                    record.knowledge().publisher_id(),
                    record.knowledge().sequence(),
                )
            })
            .collect()),
    }
}

pub fn cmd_lift(incident_id: &str) -> Result<usize, (i32, String)> {
    let root = LocalStateRoot::resolve_platform_default().map_err(|e| (4, e.to_string()))?;
    let (mut ledger, head) = snapshot(&root).map_err(|reason| {
        (
            4,
            format!("refusing to lift from an unusable ledger: {reason}"),
        )
    })?;
    let lifted = ledger.lift_locally(incident_id);
    if lifted.is_empty() {
        return Err((
            2,
            format!("no active containment has incident id {incident_id}"),
        ));
    }
    let nonce =
        containment_state::append_event(&root, &StoredEvent::Lift(incident_id.to_string()), &head)
            .map_err(|e| (4, format!("could not persist the lift: {e}")))?;
    if !appended(&root, &nonce) {
        return Err((
            4,
            "a concurrent containment change was recorded first; the lift was not applied - retry"
                .to_string(),
        ));
    }
    Ok(lifted.len())
}

/// Whether the append that returned `nonce` is part of the accepted history.
fn appended(root: &LocalStateRoot, nonce: &str) -> bool {
    matches!(containment_state::load_history(root), Load::Found(history) if history.nonces.iter().any(|n| n == nonce))
}

/// Whether exactly `text` is an accepted install in the history.
fn installed(root: &LocalStateRoot, text: &str) -> bool {
    matches!(containment_state::load_log(root), Load::Found(events) if events.contains(&StoredEvent::Install(text.to_string())))
}

/// Where the cancellAI incident-response feed is published (E33-S01): the latest cumulative
/// notice, signed with the project key, as a file in the canonical repository. The owner may
/// point elsewhere with `<state>/containment_feed_url` (one `https://` URL); nothing read from the
/// feed changes where it is fetched from.
const PROJECT_FEED_URL: &str = "https://raw.githubusercontent.com/matteo-dritara/homebrew-cancellai/main/containment/notice.json";

fn feed_url(root: &LocalStateRoot) -> Result<String, String> {
    match containment_state::load_feed_override(root) {
        Load::Missing => Ok(PROJECT_FEED_URL.to_string()),
        Load::Unreadable(reason) => Err(format!("feed URL override: {reason}")),
        Load::Found(url) => {
            let url = url.trim().to_string();
            if url.starts_with("https://") && !url.chars().any(char::is_whitespace) {
                Ok(url)
            } else {
                Err("feed URL override must be a single https:// URL".to_string())
            }
        }
    }
}

/// The system `curl`, at its fixed OS location - never resolved through `PATH` (E06 review round
/// 6: a `curl` earlier on `PATH` was executed). The bytes it returns are signature-verified, so a
/// hostile curl could only deny service; running an arbitrary program was the defect.
fn trusted_curl() -> Option<std::path::PathBuf> {
    #[cfg(feature = "test-curl")]
    if let Some(path) = std::env::var_os("CANCELLAI_TEST_CURL") {
        // Test builds only (`--features test-curl`): the release build has no such override.
        return Some(std::path::PathBuf::from(path));
    }
    let candidates: &[&str] = if cfg!(windows) {
        &[r"C:\Windows\System32\curl.exe"]
    } else {
        &["/usr/bin/curl", "/bin/curl"]
    };
    candidates
        .iter()
        .map(std::path::PathBuf::from)
        .find(|path| path.is_file())
}

/// What fetching the feed produced.
enum Fetched {
    Notice(String),
    /// The feed answered 404: no notice has been published.
    NothingPublished,
    Unavailable(String),
}

/// Fetches the feed with the system `curl` (owner decision, E33): https only, including on
/// redirect, a time limit, and at most `MAX_NOTICE_BYTES` read - enforced here, not only by
/// `--max-filesize`, which a server that sends no length cannot trigger. The transport does not
/// need to be trusted for authenticity: the notice is signature-verified exactly as `install`
/// verifies it. A missing `curl` or any network failure is unavailability, never a notice.
fn fetch(url: &str) -> Fetched {
    use std::io::Read as _;
    let Some(curl) = trusted_curl() else {
        return Fetched::Unavailable(
            "no curl at the system location this build trusts".to_string(),
        );
    };
    let spawned = std::process::Command::new(curl)
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-time",
            "30",
            "--max-filesize",
            &MAX_NOTICE_BYTES.to_string(),
            "--write-out",
            "%{stderr}%{http_code}",
            "--output",
            "-",
            url,
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => return Fetched::Unavailable(format!("could not run curl: {error}")),
    };
    let mut body = Vec::new();
    let read = match child.stdout.take() {
        Some(stdout) => stdout
            .take(MAX_NOTICE_BYTES as u64 + 1)
            .read_to_end(&mut body)
            .map(|_| ()),
        None => Ok(()),
    };
    if body.len() > MAX_NOTICE_BYTES {
        child.kill().ok();
        child.wait().ok();
        return Fetched::Unavailable(format!(
            "the feed is larger than {MAX_NOTICE_BYTES} bytes; refused before parsing"
        ));
    }
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(error) => return Fetched::Unavailable(format!("curl did not finish: {error}")),
    };
    if let Err(error) = read {
        return Fetched::Unavailable(format!("could not read the feed: {error}"));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status = stderr
        .trim()
        .rsplit(['\n', ' '])
        .next()
        .unwrap_or("")
        .to_string();
    if !output.status.success() {
        return Fetched::Unavailable(format!("curl failed: {}", stderr.trim()));
    }
    match status.as_str() {
        "200" => match String::from_utf8(body) {
            Ok(text) => Fetched::Notice(text),
            Err(_) => Fetched::Unavailable("the feed is not UTF-8".to_string()),
        },
        "404" => Fetched::NothingPublished,
        other => Fetched::Unavailable(format!("the feed answered HTTP {other}")),
    }
}

/// `containment refresh` (E33-S01): fetch the published notice and install it through exactly
/// the path `containment install` uses. Unavailable, oversized, malformed, replayed, rolled-back
/// or untrusted input leaves the history byte-identical; a notice already installed is
/// "already current", not an error.
pub fn cmd_refresh() -> Result<Vec<String>, (i32, String)> {
    let root = LocalStateRoot::resolve_platform_default().map_err(|e| (4, e.to_string()))?;
    let url = feed_url(&root).map_err(|e| (4, e))?;
    let text = match fetch(&url) {
        Fetched::Notice(text) => text,
        Fetched::NothingPublished => {
            return Ok(vec![format!(
                "no containment notice is published at {url}; the ledger is unchanged"
            )]);
        }
        Fetched::Unavailable(reason) => {
            return Err((
                4,
                format!("containment feed unavailable ({reason}); the ledger is unchanged"),
            ));
        }
    };
    // "Already current" is only true of a history that replays: an unverifiable history holding
    // the same text is unknown, not current (E06 review round 6).
    let ledger = match replay_at(&root) {
        LedgerState::Known(ledger) => ledger,
        LedgerState::Unknown(reason) => {
            return Err((
                4,
                format!("refusing to refresh an unusable ledger: {reason}"),
            ));
        }
    };
    // Current means the feed serves the newest notice this ledger verified from its publisher.
    // An older one - even one installed before - is a rollback and is refused (E06 review round
    // 7); a newer one goes through the full install path, which verifies it.
    if let Ok(bundle) = cancellai_safety::parse_bundle(&text)
        && let Some(last) = ledger.last_sequence(&bundle.publisher_id)
    {
        if bundle.sequence < last {
            return Err((
                4,
                format!(
                    "the feed serves sequence {} from {}, older than the verified {last}; a rolled-back notice is refused and the ledger is unchanged",
                    bundle.sequence, bundle.publisher_id
                ),
            ));
        }
        if bundle.sequence == last
            && let Load::Found(events) = containment_state::load_log(&root)
            && events.contains(&StoredEvent::Install(text.clone()))
        {
            return Ok(vec![format!("already current with {url}")]);
        }
    }
    install_text(&root, text, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compiled_project_key_decodes() {
        assert!(decode_key(PROJECT_INCIDENT_KEY_HEX).is_some());
    }

    #[test]
    fn a_malformed_key_does_not_decode() {
        assert_eq!(decode_key("00"), None);
        assert_eq!(decode_key(&"zz".repeat(32)), None);
    }
}
