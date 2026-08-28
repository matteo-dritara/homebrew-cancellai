#!/usr/bin/env python3
"""Differential comparison harness for the JSON_CONTRACTS.md document family (E01-S05).

Compares two documents of the same document_type - conceptually a Python-reference output
and a Rust-candidate output, though no Rust candidate exists yet - and reports every
semantic divergence. Only the fields docs/development/VERIFICATION_STRATEGY.md#differential-comparison-contract
names as nondeterministic are ignored; everything else must match after records on each side
are paired up by their documented natural key, never by an opaque engine-assigned id.

`check` runs this module's own self-test suite (a self-identical document must compare
clean; each documented divergence class must be caught) - the "harness self-test catches
intentionally injected divergence" verification named in the story's contract. Stdlib-only,
like the other governance checkers.
"""

from __future__ import annotations

import argparse
import copy
import json
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent

# Envelope fields that vary between any two otherwise-identical runs and are never compared.
ENVELOPE_IGNORED_FIELDS = {"generated_at", "generator"}

# Top-level opaque, engine-assigned document ids. Two conformant engines are never required
# to assign the same one for an equivalent document.
TOP_LEVEL_IGNORED_FIELDS = {"inventory_id", "plan_id"}

# Top-level fields compared as sets rather than ordered lists: prose/reference lists whose
# order is not semantically meaningful.
TOP_LEVEL_SET_FIELDS = {"notes", "safety_invariant_refs"}

KeyFn = Callable[[dict[str, Any], dict[str, Any]], Any]


def _artifact_key(record: dict[str, Any], index: dict[str, Any]) -> Any:
    # identity_token is the content-derived key (DOMAIN_MODEL.md's AgentArtifact.IdentityToken);
    # fall back to the record's own artifact_id only if it is somehow missing one (a shape
    # error check_schemas.py already flags separately).
    return record.get("identity_token", record.get("artifact_id"))


def _root_key(record: dict[str, Any], index: dict[str, Any]) -> Any:
    return record.get("provider_id")


def _scan_key(record: dict[str, Any], index: dict[str, Any]) -> Any:
    return record.get("scope")


def _action_key(record: dict[str, Any], index: dict[str, Any]) -> Any:
    targets = record.get("target_artifact_ids") or []
    tokens = tuple(sorted(index.get(t, t) for t in targets))
    return (tokens, record.get("action_class"))


def _action_ref_key(record: dict[str, Any], index: dict[str, Any]) -> Any:
    # `index` here is an action_id -> natural-action-key map built from the corresponding
    # plan document (see _build_action_key_index), not the artifact_index _action_key uses.
    # A record whose action_id is absent from that map has no correlating plan action at all
    # - that is itself a real divergence (an orphaned explanation/result), not something to
    # paper over with the opaque id as a silent fallback.
    action_id = record.get("action_id")
    if isinstance(action_id, str) and action_id in index:
        return index[action_id]
    return ("__unresolved__", action_id)


def _build_action_key_index(plan_doc: dict[str, Any], artifact_index: dict[str, Any]) -> dict[str, Any]:
    """action_id -> the same content-derived key _action_key computes for plan.actions."""
    return {action["action_id"]: _action_key(action, artifact_index) for action in plan_doc.get("actions", []) if "action_id" in action}


# document_type -> [ (list_field_name, key_fn, fields_dropped_before_comparing_each_record, index_kind) ]
# index_kind selects which per-side index compare_documents passes to key_fn: "artifact" is
# the artifact_id -> identity_token map; "action" is the action_id -> natural-action-key map
# built from the corresponding plan document (required for explanation/result - see
# compare_documents).
LIST_FIELDS: dict[str, list[tuple[str, KeyFn, set[str], str]]] = {
    "inventory": [
        ("provider_roots", _root_key, {"id"}, "artifact"),
        ("scan_completeness", _scan_key, set(), "artifact"),
        ("artifacts", _artifact_key, {"artifact_id"}, "artifact"),
    ],
    "plan": [
        # target_artifact_ids is dropped too: it holds opaque artifact_id values, which the
        # key function already resolved through the identity_token index to build the match
        # key. Comparing the raw ids afterward would reintroduce exactly the opacity that
        # resolution exists to remove.
        ("actions", _action_key, {"action_id", "target_artifact_ids"}, "artifact"),
    ],
    "explanation": [
        ("explanations", _action_ref_key, {"action_id"}, "action"),
    ],
    "result": [
        ("action_results", _action_ref_key, {"action_id"}, "action"),
    ],
}


def _drop_fields(record: dict[str, Any], fields: set[str]) -> dict[str, Any]:
    return {k: v for k, v in record.items() if k not in fields}


def _compare_list(
    list_a: list[dict[str, Any]],
    list_b: list[dict[str, Any]],
    key_fn: KeyFn,
    dropped: set[str],
    index_a: dict[str, Any],
    index_b: dict[str, Any],
    where: str,
    out: list[str],
) -> None:
    keyed_a = {key_fn(r, index_a): r for r in list_a}
    keyed_b = {key_fn(r, index_b): r for r in list_b}
    if len(keyed_a) != len(list_a):
        out.append(f"{where}: duplicate natural keys among side A records - cannot pair unambiguously")
    if len(keyed_b) != len(list_b):
        out.append(f"{where}: duplicate natural keys among side B records - cannot pair unambiguously")

    only_a = sorted(map(repr, set(keyed_a) - set(keyed_b)))
    only_b = sorted(map(repr, set(keyed_b) - set(keyed_a)))
    for key in only_a:
        out.append(f"{where}: record with key {key} present only in side A")
    for key in only_b:
        out.append(f"{where}: record with key {key} present only in side B")

    for key in sorted(set(keyed_a) & set(keyed_b), key=repr):
        record_a = _drop_fields(keyed_a[key], dropped)
        record_b = _drop_fields(keyed_b[key], dropped)
        if record_a != record_b:
            out.append(f"{where}[{key!r}]: fields differ - A={record_a!r} B={record_b!r}")


def compare_documents(
    doc_a: dict[str, Any],
    doc_b: dict[str, Any],
    *,
    artifact_index_a: dict[str, str] | None = None,
    artifact_index_b: dict[str, str] | None = None,
    plan_a: dict[str, Any] | None = None,
    plan_b: dict[str, Any] | None = None,
) -> list[str]:
    """Compare two JSON_CONTRACTS.md documents. Empty result means no divergence.

    `artifact_index_{a,b}` maps that side's own artifact_id -> identity_token, built from
    that side's inventory document (`{a["artifact_id"]: a["identity_token"] for a in
    inventory["artifacts"]}`). Omit it only when comparing two documents produced by the
    *same* engine run (ids are then already self-consistent) - a real cross-engine
    comparison of a plan document requires it, or action matching silently degrades to
    comparing opaque, engine-assigned artifact ids against each other.

    `plan_a`/`plan_b` are the plan documents that produced the `explanation`/`result`
    records being compared. They are **required** when `doc_a`/`doc_b` are of those two
    types: an explanation/result record only carries an opaque `action_id`, which two
    engines are never required to agree on, so pairing those records at all requires
    resolving each `action_id` back to the same content-derived key `plan.actions` uses
    (via the corresponding plan's own `target_artifact_ids`/`action_class`). There is no
    silent fallback to the opaque id - see the E01-S05 round-one review finding this
    parameter exists to close.
    """
    out: list[str] = []
    doc_type_a = doc_a.get("document_type")
    doc_type_b = doc_b.get("document_type")
    if doc_type_a != doc_type_b or doc_type_a not in LIST_FIELDS:
        out.append(f"document_type mismatch or unsupported: A={doc_type_a!r} B={doc_type_b!r}")
        return out

    if doc_a.get("schema_version") != doc_b.get("schema_version"):
        out.append(f"schema_version mismatch: A={doc_a.get('schema_version')!r} B={doc_b.get('schema_version')!r}")

    ignored_top_level = ENVELOPE_IGNORED_FIELDS | TOP_LEVEL_IGNORED_FIELDS | TOP_LEVEL_SET_FIELDS
    list_fields = LIST_FIELDS[doc_type_a]
    list_field_names = {name for name, _fn, _dropped, _kind in list_fields}
    scalar_keys = (set(doc_a) | set(doc_b)) - ignored_top_level - list_field_names - {"schema_version"}
    for key in sorted(scalar_keys):
        if doc_a.get(key) != doc_b.get(key):
            out.append(f"{key}: top-level field differs - A={doc_a.get(key)!r} B={doc_b.get(key)!r}")

    for key in sorted(TOP_LEVEL_SET_FIELDS):
        if key not in doc_a and key not in doc_b:
            continue
        set_a, set_b = set(doc_a.get(key) or []), set(doc_b.get(key) or [])
        if set_a != set_b:
            out.append(f"{key}: differs as a set - only in A: {sorted(set_a - set_b)}, only in B: {sorted(set_b - set_a)}")

    artifact_index_a = artifact_index_a or {}
    artifact_index_b = artifact_index_b or {}

    needs_action_index = any(kind == "action" for _name, _fn, _dropped, kind in list_fields)
    if needs_action_index:
        if plan_a is None or plan_b is None:
            out.append(
                f"{doc_type_a}: comparing this document type requires plan_a and plan_b "
                "(the plan documents that produced these records) to resolve action_id to a "
                "content-derived key; opaque action_id is never compared or matched directly"
            )
            return out
        action_index_a = _build_action_key_index(plan_a, artifact_index_a)
        action_index_b = _build_action_key_index(plan_b, artifact_index_b)
    else:
        action_index_a = action_index_b = {}

    indexes = {"artifact": (artifact_index_a, artifact_index_b), "action": (action_index_a, action_index_b)}
    for field_name, key_fn, dropped, kind in list_fields:
        side_a, side_b = indexes[kind]
        _compare_list(doc_a.get(field_name) or [], doc_b.get(field_name) or [], key_fn, dropped, side_a, side_b, field_name, out)

    return out


# --- self-test ---------------------------------------------------------------------------


def _load_golden(name: str) -> dict[str, Any]:
    data: dict[str, Any] = json.loads((ROOT / "tests" / "fixtures" / "schemas" / "golden" / name).read_text(encoding="utf-8"))
    return data


def selftest() -> list[str]:
    failures: list[str] = []

    def expect(condition: bool, message: str) -> None:
        if not condition:
            failures.append(message)

    plan = _load_golden("plan.golden.json")
    inventory = _load_golden("inventory.golden.json")

    # 1. A document compared against an exact copy of itself must report nothing.
    expect(compare_documents(plan, copy.deepcopy(plan)) == [], "identical plan documents must compare clean")
    expect(compare_documents(inventory, copy.deepcopy(inventory)) == [], "identical inventory documents must compare clean")

    # 2. Changing only whitelisted-nondeterministic fields must still compare clean.
    varied = copy.deepcopy(plan)
    varied["generated_at"] = "2099-01-01T00:00:00Z"
    varied["generator"] = {"name": "cancellai-rust", "version": "0.0.1-different"}
    varied["plan_id"] = "a-totally-different-plan-id"
    for action in varied["actions"]:
        action["action_id"] = f"different-{action['action_id']}"
    expect(
        compare_documents(plan, varied) == [],
        "varying only generated_at/generator/plan_id/action_id (all documented as ignored) must still compare clean",
    )

    # 3. A real semantic divergence on a matched action must be caught.
    diverged = copy.deepcopy(plan)
    diverged["actions"][1]["authority"] = "AUTOPILOT"  # was QUARANTINE in the golden document
    errors = compare_documents(plan, diverged)
    expect(any("fields differ" in e for e in errors), f"changed authority on a matched action must be caught, got: {errors}")

    # 4. An action present on only one side must be caught, not silently dropped.
    extra = copy.deepcopy(plan)
    extra_action = copy.deepcopy(extra["actions"][0])
    extra_action["action_id"] = "action-extra"
    extra_action["target_artifact_ids"] = ["artifact-does-not-exist-elsewhere"]
    extra["actions"].append(extra_action)
    errors = compare_documents(plan, extra)
    expect(any("present only in side B" in e for e in errors), f"an unmatched extra action must be caught, got: {errors}")

    # 5. A missing artifact in the inventory must be caught.
    missing = copy.deepcopy(inventory)
    missing["artifacts"] = []
    errors = compare_documents(inventory, missing)
    expect(any("present only in side A" in e for e in errors), f"a dropped artifact must be caught, got: {errors}")

    # 6. Two documents of different document_type must be rejected outright.
    errors = compare_documents(plan, inventory)
    expect(any("document_type mismatch" in e for e in errors), f"mismatched document_type must be rejected, got: {errors}")

    # 7. Artifact matching uses identity_token, not the opaque artifact_id - renaming only
    #    the id (a legitimate difference between two independently-assigned-id engines) must
    #    not itself be reported as a divergence.
    renamed_ids = copy.deepcopy(inventory)
    for artifact in renamed_ids["artifacts"]:
        artifact["artifact_id"] = f"rust-side-{artifact['artifact_id']}"
    expect(
        compare_documents(inventory, renamed_ids) == [],
        "renaming only artifact_id (identity_token unchanged) must not be reported as a divergence",
    )

    # 8-11. Regression cases for the E01-S05 round-one review finding: explanation/result
    # records must be matched by a content-derived key resolved through the plan, not by
    # opaque action_id - the bug was exactly this producing false divergences.
    explanation = _load_golden("explanation.golden.json")
    result = _load_golden("result.golden.json")

    renamed_plan = copy.deepcopy(plan)
    renamed_plan["plan_id"] = "rust-plan-id"
    for action in renamed_plan["actions"]:
        action["action_id"] = f"rust-{action['action_id']}"

    # 8. Renaming only action_id/plan_id (the plan context resolves each to the same
    #    content-derived key) must not be reported as a divergence for explanation...
    renamed_explanation = copy.deepcopy(explanation)
    renamed_explanation["plan_id"] = "rust-plan-id"
    for item in renamed_explanation["explanations"]:
        item["action_id"] = f"rust-{item['action_id']}"
    errors = compare_documents(explanation, renamed_explanation, plan_a=plan, plan_b=renamed_plan)
    expect(errors == [], f"renaming only action_id/plan_id must not diverge for explanation (with plan context), got: {errors}")

    # ...or for result.
    renamed_result = copy.deepcopy(result)
    renamed_result["plan_id"] = "rust-plan-id"
    for item in renamed_result["action_results"]:
        item["action_id"] = f"rust-{item['action_id']}"
    errors = compare_documents(result, renamed_result, plan_a=plan, plan_b=renamed_plan)
    expect(errors == [], f"renaming only action_id/plan_id must not diverge for result (with plan context), got: {errors}")

    # 9. Comparing explanation/result without the corresponding plan context is a hard error,
    #    not a silent fallback to opaque-id matching - that silent fallback was the bug.
    errors = compare_documents(explanation, copy.deepcopy(explanation))
    expect(
        any("requires plan_a and plan_b" in e for e in errors),
        f"comparing explanation without plan_a/plan_b must fail loudly rather than silently matching by action_id, got: {errors}",
    )

    # 10. A real semantic divergence must still be caught even when ids are also renamed -
    #     the fix must not have traded false positives for false negatives.
    diverged_explanation = copy.deepcopy(renamed_explanation)
    diverged_explanation["explanations"][0]["final_authority"] = "AUTOPILOT"  # golden value is QUARANTINE
    errors = compare_documents(explanation, diverged_explanation, plan_a=plan, plan_b=renamed_plan)
    expect(any("fields differ" in e for e in errors), f"a real divergence on a renamed-id explanation must still be caught, got: {errors}")

    return failures


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Differential comparison harness for cancellAI JSON documents.")
    parser.add_argument("command", nargs="?", default="check", choices=["check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    build_parser().parse_args(argv)
    failures = selftest()
    if failures:
        print("DIFF HARNESS SELF-TEST FAILED:", file=sys.stderr)
        for failure in failures:
            print(f"  {failure}", file=sys.stderr)
        return 2
    print("diff harness OK: self-test cases all behave as documented")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
