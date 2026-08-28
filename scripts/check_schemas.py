#!/usr/bin/env python3
"""Validate the versioned inventory/plan/explanation/result JSON contracts (E01-S03).

Enforces docs/architecture/JSON_CONTRACTS.md against the golden documents under
tests/fixtures/schemas/golden/. Stdlib-only, like the other governance checkers, so it
stays usable before and after the Python -> Rust migration.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
GOLDEN_DIR = ROOT / "tests" / "fixtures" / "schemas" / "golden"

ENVELOPE_ORDER = ("schema_version", "document_type", "generated_at", "generator")
ACTION_CLASSES = {"OBSERVE", "QUARANTINE", "ARCHIVE", "DELETE"}
AUTHORITY_LEVELS = ("OBSERVE", "RECOMMEND", "QUARANTINE", "GOVERN", "AUTOPILOT")
REVERSIBILITY = {"REBUILDABLE", "QUARANTINABLE", "ARCHIVABLE", "VENDOR_CONDITIONAL", "IRREVERSIBLE", "UNKNOWN"}
RESULT_STATUSES = {"attempted", "succeeded", "safely_skipped", "failed"}


class SchemaError(RuntimeError):
    pass


def _err(errors: list[str], where: str, message: str) -> None:
    errors.append(f"{where}: {message}")


def _require(doc: dict[str, Any], key: str, where: str, errors: list[str]) -> None:
    if key not in doc:
        _err(errors, where, f"missing required key {key!r}")


def check_envelope_order(doc: dict[str, Any], where: str, errors: list[str], expected_type: str) -> None:
    keys = list(doc.keys())
    prefix = tuple(keys[: len(ENVELOPE_ORDER)])
    if prefix != ENVELOPE_ORDER:
        _err(errors, where, f"envelope keys must appear first, in this order: {ENVELOPE_ORDER}; got {prefix}")
    if doc.get("schema_version") != 1:
        _err(errors, where, f"schema_version must be 1, got {doc.get('schema_version')!r}")
    if doc.get("document_type") != expected_type:
        _err(errors, where, f"document_type must be {expected_type!r}, got {doc.get('document_type')!r}")
    generator = doc.get("generator")
    if not isinstance(generator, dict) or "name" not in generator or "version" not in generator:
        _err(errors, where, "generator must be an object with 'name' and 'version'")


def check_action(action: dict[str, Any], where: str, errors: list[str]) -> None:
    for key in (
        "action_id",
        "target_artifact_ids",
        "action_class",
        "reason",
        "authority",
        "reversibility",
        "evidence_ids",
        "execution_preconditions",
    ):
        if key not in action:
            _err(
                errors,
                where,
                f"action missing required key {key!r} (every action carries reason, authority, reversibility and preconditions - E01-S03 AC3)",
            )

    action_class = action.get("action_class")
    if action_class not in ACTION_CLASSES:
        _err(errors, where, f"action_class {action_class!r} is not one of {sorted(ACTION_CLASSES)}")
    if action.get("authority") not in AUTHORITY_LEVELS:
        _err(errors, where, f"authority {action.get('authority')!r} is not one of {AUTHORITY_LEVELS}")
    if action.get("reversibility") not in REVERSIBILITY:
        _err(errors, where, f"reversibility {action.get('reversibility')!r} is not one of {sorted(REVERSIBILITY)}")

    reason = action.get("reason")
    if not isinstance(reason, str) or not reason.strip():
        _err(errors, where, "reason must be a non-empty string")
    if not isinstance(action.get("evidence_ids"), list) or not action.get("evidence_ids"):
        _err(errors, where, "evidence_ids must be a non-empty list")
    if not isinstance(action.get("target_artifact_ids"), list) or not action.get("target_artifact_ids"):
        _err(errors, where, "target_artifact_ids must be a non-empty list")

    preconditions = action.get("execution_preconditions")
    if not isinstance(preconditions, list):
        _err(errors, where, "execution_preconditions must be a list")
    elif not preconditions and action_class != "OBSERVE":
        _err(errors, where, f"action_class {action_class!r} requires at least one execution precondition (SI-013/SI-016)")


def check_inventory(doc: dict[str, Any], where: str, errors: list[str]) -> None:
    check_envelope_order(doc, where, errors, "inventory")
    for key in ("inventory_id", "provider_roots", "scan_completeness", "artifacts"):
        _require(doc, key, where, errors)
    for index, artifact in enumerate(doc.get("artifacts") or []):
        artifact_where = f"{where}.artifacts[{index}]"
        for key in (
            "artifact_id",
            "identity_token",
            "provider_id",
            "artifact_type",
            "risk_class",
            "reversibility",
            "knowledge_confidence",
            "activity_state",
            "residency_state",
            "protection_state",
            "integrity_state",
            "authority_ceiling",
            "evidence_ids",
        ):
            if key not in artifact:
                _err(errors, artifact_where, f"missing required key {key!r}")
        if not artifact.get("evidence_ids"):
            _err(errors, artifact_where, "evidence_ids must be a non-empty list")


def check_plan(doc: dict[str, Any], where: str, errors: list[str]) -> None:
    check_envelope_order(doc, where, errors, "plan")
    for key in ("plan_id", "inventory_snapshot_id", "provider_roots", "actions", "notes", "safety_invariant_refs"):
        _require(doc, key, where, errors)
    for index, action in enumerate(doc.get("actions") or []):
        check_action(action, f"{where}.actions[{index}]", errors)


def check_explanation(doc: dict[str, Any], where: str, errors: list[str]) -> None:
    check_envelope_order(doc, where, errors, "explanation")
    for key in ("plan_id", "explanations"):
        _require(doc, key, where, errors)
    for index, item in enumerate(doc.get("explanations") or []):
        item_where = f"{where}.explanations[{index}]"
        steps = item.get("steps")
        if not isinstance(steps, list) or not steps:
            _err(errors, item_where, "steps must be a non-empty ordered list")
            continue
        final_authority = item.get("final_authority")
        last_step_authority = steps[-1].get("resulting_authority")
        if final_authority != last_step_authority:
            _err(
                errors,
                item_where,
                f"final_authority ({final_authority!r}) must equal the last step's resulting_authority ({last_step_authority!r})",
            )


def check_result(doc: dict[str, Any], where: str, errors: list[str]) -> None:
    check_envelope_order(doc, where, errors, "result")
    for key in ("plan_id", "action_results", "summary"):
        _require(doc, key, where, errors)

    succeeded = skipped = failed = 0
    for index, item in enumerate(doc.get("action_results") or []):
        item_where = f"{where}.action_results[{index}]"
        status = item.get("status")
        if status not in RESULT_STATUSES:
            _err(errors, item_where, f"status {status!r} is not one of {sorted(RESULT_STATUSES)}")
        if "reason_code" not in item:
            _err(errors, item_where, "missing required key 'reason_code'")
        if status == "succeeded":
            succeeded += 1
        elif status == "safely_skipped":
            skipped += 1
        elif status == "failed":
            failed += 1

    summary = doc.get("summary") or {}
    if summary.get("succeeded") != succeeded:
        _err(errors, where, f"summary.succeeded ({summary.get('succeeded')!r}) does not match action_results ({succeeded})")
    if summary.get("safely_skipped") != skipped:
        _err(
            errors,
            where,
            f"summary.safely_skipped ({summary.get('safely_skipped')!r}) does not match action_results ({skipped}) - "
            "SI-014: a skip is not a success",
        )
    if summary.get("failed") != failed:
        _err(errors, where, f"summary.failed ({summary.get('failed')!r}) does not match action_results ({failed})")


CHECKERS: dict[str, Callable[[dict[str, Any], str, list[str]], None]] = {
    "inventory": check_inventory,
    "plan": check_plan,
    "explanation": check_explanation,
    "result": check_result,
}


def validate_document(doc: Any, where: str) -> list[str]:
    if not isinstance(doc, dict):
        return [f"{where}: document must be a JSON object"]
    doc_type = doc.get("document_type")
    if not isinstance(doc_type, str) or doc_type not in CHECKERS:
        return [f"{where}: unknown or missing document_type {doc_type!r}, expected one of {sorted(CHECKERS)}"]
    checker = CHECKERS[doc_type]
    errors: list[str] = []
    checker(doc, where, errors)
    return errors


def validate() -> list[str]:
    errors: list[str] = []
    if not GOLDEN_DIR.is_dir():
        raise SchemaError(f"{GOLDEN_DIR} does not exist")
    golden_files = sorted(GOLDEN_DIR.glob("*.golden.json"))
    if not golden_files:
        raise SchemaError(f"no golden documents found under {GOLDEN_DIR}")

    seen_types: set[str] = set()
    for path in golden_files:
        try:
            doc = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            errors.append(f"{path.relative_to(ROOT)}: cannot read/parse: {exc}")
            continue
        where = str(path.relative_to(ROOT))
        errors.extend(validate_document(doc, where))
        if isinstance(doc, dict) and isinstance(doc.get("document_type"), str):
            seen_types.add(doc["document_type"])

    missing_types = set(CHECKERS) - seen_types
    if missing_types:
        errors.append(f"no golden document covers required document_type(s): {sorted(missing_types)}")
    return errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate the cancellAI JSON document contracts.")
    parser.add_argument("command", nargs="?", default="check", choices=["check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    build_parser().parse_args(argv)
    try:
        errors = validate()
    except SchemaError as exc:
        print(f"SCHEMA ERROR: {exc}", file=sys.stderr)
        return 2
    if errors:
        print("SCHEMA ERROR:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 2
    count = len(list(GOLDEN_DIR.glob("*.golden.json")))
    print(f"schemas OK: {count} golden documents match docs/architecture/JSON_CONTRACTS.md")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
