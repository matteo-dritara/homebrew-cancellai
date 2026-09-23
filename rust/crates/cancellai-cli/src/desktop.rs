//! `desktop-api`: the engine side of the desktop API boundary (E19-S01).
//!
//! Serves exactly the documents `status --json`, `inspect` and `plan --json` print, built by the
//! same functions, so a desktop client cannot see a different inventory or a different plan
//! than a CLI user would. Nothing here can reach `clean`, `configure` or the mutation executor:
//! the protocol has no request for them, and this module calls only the read-only builders.

use std::io::Write as _;

use cancellai_desktop_api::{
    DocumentEnvelope, DocumentKind, DocumentSource, ProviderSummary, Query, Server,
};
use cancellai_model::ErrorCategory;
use cancellai_platform::{Clock, SystemClock};
use cancellai_policy::ToolScope;

use crate::{
    CommonFlags, VERSION, any_incomplete, inventory_doc, plan_actions, plan_doc,
    provider_summaries, resolve_all,
};

/// The engine's document source for the desktop API.
pub(crate) struct EngineDocuments;

fn flags_for(query: Query) -> CommonFlags {
    CommonFlags {
        days: query.days,
        keep_latest: query.keep_latest,
        tool: match query.tool {
            cancellai_desktop_api::ToolScope::All => ToolScope::All,
            cancellai_desktop_api::ToolScope::Claude => ToolScope::Claude,
            cancellai_desktop_api::ToolScope::Codex => ToolScope::Codex,
        },
        json: true,
        allow_running: query.allow_running,
        dry_run: false,
        yes: false,
        keep_claude_history: false,
        verbose: false,
    }
}

impl DocumentSource for EngineDocuments {
    fn engine_version(&self) -> String {
        VERSION.to_string()
    }

    fn document(&self, kind: DocumentKind, query: Query) -> Result<DocumentEnvelope, String> {
        let resolved = resolve_all(&flags_for(query))?;
        let now = SystemClock.now();
        let scan_incomplete = any_incomplete(&resolved);
        let (document, withheld) = match kind {
            DocumentKind::Status | DocumentKind::Inspect => {
                (inventory_doc(&resolved, now), Vec::new())
            }
            DocumentKind::Plan => {
                let (actions, withheld) = plan_actions(&resolved);
                (
                    plan_doc(&resolved, now, actions),
                    withheld.into_iter().map(str::to_string).collect(),
                )
            }
        };
        Ok(DocumentEnvelope {
            kind,
            query,
            document,
            scan_incomplete,
            withheld_by_root_authority: withheld,
            provider_summaries: provider_summaries(&resolved)
                .into_iter()
                .map(
                    |(provider_id, artifacts, bytes, scan_complete)| ProviderSummary {
                        provider_id: provider_id.to_string(),
                        artifacts,
                        bytes,
                        scan_complete,
                    },
                )
                .collect(),
        })
    }
}

/// Binds the loopback server, prints its descriptor as one JSON line on standard output, then
/// serves until terminated (or until `connections` connections have been served).
pub(crate) fn cmd_desktop_api(connections: Option<usize>) -> i32 {
    let failed = |message: String| {
        eprintln!("[{}] {message}", ErrorCategory::InternalFault.code());
        ErrorCategory::InternalFault.exit_code()
    };
    let server = match Server::bind() {
        Ok(server) => server,
        Err(error) => return failed(format!("could not start the desktop API: {error}")),
    };
    let descriptor = match server.descriptor() {
        Ok(descriptor) => descriptor,
        Err(error) => return failed(format!("could not describe the desktop API: {error}")),
    };
    let line = serde_json::to_string(&descriptor)
        .expect("a descriptor is three plain fields and always serializes");
    let mut stdout = std::io::stdout();
    if writeln!(stdout, "{line}")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return failed("could not hand the desktop API descriptor to the parent".to_string());
    }
    match server.serve(&EngineDocuments, connections) {
        Ok(()) => 0,
        Err(error) => failed(format!("the desktop API stopped: {error}")),
    }
}
