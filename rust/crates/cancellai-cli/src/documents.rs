//! Assembles the `docs/architecture/JSON_CONTRACTS.md`-shaped inventory/plan/result documents
//! this CLI's `--json` output emits. `cancellai_model::{AgentArtifact, Action}` already
//! implement the safety-critical per-record shapes (`Serialize`); this module supplies the
//! common envelope and the `provider_roots`/`scan_completeness`/`summary` framing around them.

use cancellai_model::{Action, AgentArtifact};
use cancellai_platform::{FilesystemContext, FilesystemContextObservation, Timestamp};
use cancellai_provider_api::{RootConfidence, RootOrigin};
use serde::Serialize;

use crate::timestamp::to_iso8601_utc;

pub const GENERATOR_NAME: &str = "cancellai-cli";
pub const GENERATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize)]
struct Generator {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
pub struct ProviderRootDoc {
    pub id: String,
    pub provider_id: String,
    pub origin: &'static str,
    pub confidence: &'static str,
    pub mutation_eligible: bool,
    /// This root's storage relative to a WSL2 guest's own filesystem (E20-S02/E20-S03 round-1
    /// independent verifier review: `FilesystemContext`/`FilesystemContextObserver` existed
    /// but had no production caller anywhere in this workspace, so `docs/architecture/
    /// PLATFORM_MODEL.md`'s "surfaced rather than abstracted away" promise for a `/mnt`-mounted
    /// Windows drive was not actually kept by any shipped command). `"linux"` /
    /// `"windows_mounted"` / `"other:<fstype>"` mirror `FilesystemContext`'s own variants;
    /// `"unsupported:<reason>"` covers a platform/path this classifier cannot resolve
    /// (including every non-Linux host, where this is simply always `"unsupported:..."`).
    pub filesystem_context: String,
}

impl ProviderRootDoc {
    pub fn new(
        id: String,
        provider_id: String,
        origin: RootOrigin,
        confidence: RootConfidence,
        filesystem_context: FilesystemContextObservation,
    ) -> Self {
        Self {
            id,
            provider_id,
            origin: origin_str(origin),
            confidence: confidence_str(confidence),
            // ADR-0013: only the provider's own default directory may be mutated. Structural
            // evidence (`High`/`Low` confidence) is reported for the operator's benefit - it is
            // cheap to fabricate and therefore never proof of ownership (SI-002), so a custom
            // root is never `mutation_eligible` regardless of how convincing its markers look
            // (E06 verifier review round 1: an earlier version of this document also allowed
            // `RootConfidence::High`, contradicting the destructive-authority gate this exact
            // field is supposed to describe).
            mutation_eligible: matches!(origin, RootOrigin::Default),
            filesystem_context: filesystem_context_str(filesystem_context),
        }
    }
}

fn filesystem_context_str(observation: FilesystemContextObservation) -> String {
    match observation {
        FilesystemContextObservation::Classified(FilesystemContext::Linux) => "linux".to_string(),
        FilesystemContextObservation::Classified(FilesystemContext::WindowsMounted) => {
            "windows_mounted".to_string()
        }
        FilesystemContextObservation::Classified(FilesystemContext::Other { fstype }) => {
            format!("other:{fstype}")
        }
        FilesystemContextObservation::Unsupported { reason } => {
            format!("unsupported:{reason}")
        }
    }
}

fn origin_str(origin: RootOrigin) -> &'static str {
    match origin {
        RootOrigin::Default => "default",
        RootOrigin::Custom => "custom",
    }
}

fn confidence_str(confidence: RootConfidence) -> &'static str {
    match confidence {
        RootConfidence::Default => "default",
        RootConfidence::High => "high",
        RootConfidence::Low => "low",
        RootConfidence::Unknown => "unknown",
    }
}

#[derive(Serialize)]
pub struct ScanCompletenessDoc {
    pub scope: &'static str,
    pub complete: bool,
    pub error_count: u32,
}

#[derive(Serialize)]
struct Envelope<'a, T: Serialize> {
    schema_version: u32,
    document_type: &'static str,
    generated_at: String,
    generator: Generator,
    #[serde(flatten)]
    body: &'a T,
}

fn envelope<T: Serialize>(
    document_type: &'static str,
    now: Timestamp,
    body: &T,
) -> serde_json::Value {
    let envelope = Envelope {
        schema_version: 1,
        document_type,
        generated_at: to_iso8601_utc(now),
        generator: Generator {
            name: GENERATOR_NAME,
            version: GENERATOR_VERSION,
        },
        body,
    };
    serde_json::to_value(envelope).expect("envelope is always representable as JSON")
}

#[derive(Serialize)]
struct InventoryBody {
    inventory_id: String,
    /// This process's own runtime environment (E20-S02/E20-S03 round-1 independent verifier
    /// review's "no production caller" finding) - `"wsl2"` or `"native"`, mirroring
    /// `cancellai_platform::RuntimeEnvironment`.
    runtime_environment: &'static str,
    provider_roots: Vec<ProviderRootDoc>,
    scan_completeness: Vec<ScanCompletenessDoc>,
    artifacts: Vec<AgentArtifact>,
}

pub fn inventory_document(
    inventory_id: String,
    now: Timestamp,
    runtime_environment: &'static str,
    provider_roots: Vec<ProviderRootDoc>,
    scan_completeness: Vec<ScanCompletenessDoc>,
    artifacts: Vec<AgentArtifact>,
) -> serde_json::Value {
    envelope(
        "inventory",
        now,
        &InventoryBody {
            inventory_id,
            runtime_environment,
            provider_roots,
            scan_completeness,
            artifacts,
        },
    )
}

#[derive(Serialize)]
struct PlanBody {
    plan_id: String,
    inventory_snapshot_id: String,
    runtime_environment: &'static str,
    provider_roots: Vec<ProviderRootDoc>,
    actions: Vec<Action>,
    notes: Vec<String>,
    safety_invariant_refs: Vec<&'static str>,
}

pub fn plan_document(
    plan_id: String,
    inventory_snapshot_id: String,
    now: Timestamp,
    runtime_environment: &'static str,
    provider_roots: Vec<ProviderRootDoc>,
    actions: Vec<Action>,
    notes: Vec<String>,
) -> serde_json::Value {
    envelope(
        "plan",
        now,
        &PlanBody {
            plan_id,
            inventory_snapshot_id,
            runtime_environment,
            provider_roots,
            actions,
            notes,
            safety_invariant_refs: vec!["SI-007", "SI-013"],
        },
    )
}

#[derive(Serialize)]
pub struct ActionResultDoc {
    pub action_id: String,
    pub status: &'static str,
    pub reason_code: String,
    pub reclaimed_bytes: u64,
    pub post_action_state: &'static str,
}

#[derive(Serialize)]
struct ResultSummary {
    attempted: u32,
    succeeded: u32,
    safely_skipped: u32,
    failed: u32,
    reclaimed_bytes: u64,
}

#[derive(Serialize)]
struct ResultBody {
    plan_id: String,
    action_results: Vec<ActionResultDoc>,
    summary: ResultSummary,
}

pub fn result_document(
    plan_id: String,
    now: Timestamp,
    action_results: Vec<ActionResultDoc>,
) -> serde_json::Value {
    let summary = ResultSummary {
        attempted: action_results.len() as u32,
        succeeded: action_results
            .iter()
            .filter(|r| r.status == "succeeded")
            .count() as u32,
        safely_skipped: action_results
            .iter()
            .filter(|r| r.status == "safely_skipped")
            .count() as u32,
        failed: action_results
            .iter()
            .filter(|r| r.status == "failed")
            .count() as u32,
        reclaimed_bytes: action_results.iter().map(|r| r.reclaimed_bytes).sum(),
    };
    envelope(
        "result",
        now,
        &ResultBody {
            plan_id,
            action_results,
            summary,
        },
    )
}
