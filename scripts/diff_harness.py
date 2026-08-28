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

KeyFn = Callable[[dict[str, Any], dict[str, str]], Any]


def _artifact_key(record: dict[str, Any], index: dict[str, str]) -> Any:
    # identity_token is the content-derived key (DOMAIN_MODEL.md's AgentArtifact.IdentityToken);
    # fall back to the record's own artifact_id only if it is somehow missing one (a shape
    # error check_schemas.py already flags separately).
    return record.get("identity_token", record.get("artifact_id"))


def _root_key(record: dict[str, Any], index: dict[str, str]) -> Any:
    return record.get("provider_id")


def _scan_key(record: dict[str, Any], index: dict[str, str]) -> Any:
    return record.get("scope")


def _action_key(record: dict[str, Any], index: dict[str, str]) -> Any:
    targets = record.get("target_artifact_ids") or []
    tokens = tuple(sorted(index.get(t, t) for t in targets))
    return (tokens, record.get("action_class"))


def _action_id_key(record: dict[str, Any], index: dict[str, str]) -> Any:
    # Explanation/result records are keyed by the plan's own action_id today. This is a
    # documented residual limitation (see docs/architecture/JSON_CONTRACTS.md and this
    # story's evidence packet): a fully content-derived key needs the same target-artifact
    # resolution _action_key uses, threaded through from the plan that produced these
    # records, which no caller needs yet with no second engine to compare against.
    return record.get("action_id")


# document_type -> [ (list_field_name, key_fn, fields_dropped_before_comparing_each_record) ]
LIST_FIELDS: dict[str, list[tuple[str, KeyFn, set[str]]]] = {
    "inventory": [
        ("provider_roots", _root_key, {"id"}),
        ("scan_completeness", _scan_key, set()),
        ("artifacts", _artifact_key, {"artifact_id"}),
    ],
    "plan": [
        # target_artifact_ids is dropped too: it holds opaque artifact_id values, which the
        # key function already resolved through the identity_token index to build the match
        # key. Comparing the raw ids afterward would reintroduce exactly the opacity that
        # resolution exists to remove.
        ("actions", _action_key, {"action_id", "target_artifact_ids"}),
    ],
    "explanation": [
        ("explanations", _action_id_key, set()),
    ],
    "result": [
        ("action_results", _action_id_key, set()),
    ],
}


def _drop_fields(record: dict[str, Any], fields: set[str]) -> dict[str, Any]:
    return {k: v for k, v in record.items() if k not in fields}


def _compare_list(
    list_a: list[dict[str, Any]],
    list_b: list[dict[str, Any]],
    key_fn: KeyFn,
    dropped: set[str],
    index_a: dict[str, str],
    index_b: dict[str, str],
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
) -> list[str]:
    """Compare two JSON_CONTRACTS.md documents. Empty result means no divergence.

    `artifact_index_{a,b}` maps that side's own artifact_id -> identity_token, built from
    that side's inventory document (`{a["artifact_id"]: a["identity_token"] for a in
    inventory["artifacts"]}`). Omit it only when comparing two documents produced by the
    *same* engine run (ids are then already self-consistent) - a real cross-engine
    comparison of a plan/explanation/result document requires it, or action matching
    silently degrades to comparing opaque, engine-assigned ids against each other.
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
    list_field_names = {name for name, _fn, _dropped in list_fields}
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

    index_a = artifact_index_a or {}
    index_b = artifact_index_b or {}
    for field_name, key_fn, dropped in list_fields:
        _compare_list(doc_a.get(field_name) or [], doc_b.get(field_name) or [], key_fn, dropped, index_a, index_b, field_name, out)

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
