#!/usr/bin/env python3
"""Validate the community provider verification workflow (E16-S06, SI-021).

Automates two things `.github/CONTRIBUTING.md`'s "Provider contributions" section previously
stated only in prose:

- **manifest lint**: every committed manifest under `rust/crates/cancellai-provider-api/
  manifests/*.json` carries only fields `cancellai-provider-api::manifest`'s Rust schema
  actually defines - a fast, Rust-toolchain-independent re-check of the same
  `deny_unknown_fields` property that crate's own parser enforces, so a smuggled `trust`/
  `capability`/`authority` field in a manifest is caught here too, not only at `cargo build`
  time (defense in depth, not a replacement for the Rust parser being the actual runtime gate);
- **trust-level labeling / maintainer promotion criteria**: `project/provider_trust.json`
  records, for every shipped manifest, which `cancellai_model::ProviderTrust` tier it is
  currently assigned. A tier above `untrusted` requires non-empty `verified_by` and at least
  one `fixture_references` entry - the same evidence shape
  `cancellai-safety::trust_promotion::TrustPromotionEvidence` requires before
  `TrustedTier::promote` accepts a raise, reimplemented here as a registry-level check because
  this file, not a Rust type, is the thing a pull request actually edits.

AC1 ("a community PR cannot mark itself Built-in Verified"): a PR can freely propose any
`tier` value in its own diff to `project/provider_trust.json`, but this check fails closed if
that tier is not `untrusted` and the evidence fields are empty - "cannot mark itself" means
the registry entry alone is never sufficient, evidence must accompany it, and `.github/
CODEOWNERS`'s existing `/project/provider_trust.json` entry means only the project owner can
merge a change to this file at all. Neither control alone is the guarantee; both together are
(the same "structural shape plus reviewed gate" pattern `cancellai-safety::TrustedTier` uses
for the identical property at the Rust type level, `docs/adrs` cross-references below).

AC2 ("promotion requires maintainer-owned evidence and compatibility tests"): enforced by the
same non-empty `verified_by`/`fixture_references` requirement above.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
REGISTRY_PATH = ROOT / "project" / "provider_trust.json"
MANIFEST_DIR = ROOT / "rust" / "crates" / "cancellai-provider-api" / "manifests"

VALID_TIERS = {"untrusted", "local_custom", "community_verified", "builtin_verified"}

# Mirrors rust/crates/cancellai-provider-api/src/manifest.rs's own struct field sets exactly -
# any key outside these is something that crate's `#[serde(deny_unknown_fields)]` would also
# reject, checked here independently so CI does not need the Rust toolchain to catch it.
MANIFEST_TOP_LEVEL_KEYS = {"schema_version", "provider_id", "display_name", "vendor_notes", "roots", "artifacts"}
ROOT_KEYS = {"name", "env_var", "env_var_is_full_path", "subdir", "default_relative_to_home", "markers"}
MARKER_KEYS = {"relative_path", "probe", "identifying"}
ARTIFACT_KEYS = {"root", "category", "relative_glob"}


class ProviderTrustError(RuntimeError):
    pass


def _err(errors: list[str], where: str, message: str) -> None:
    errors.append(f"{where}: {message}")


def _check_no_unknown_keys(obj: Any, allowed: set[str], where: str, errors: list[str]) -> None:
    if not isinstance(obj, dict):
        _err(errors, where, "expected an object")
        return
    unknown = set(obj.keys()) - allowed
    if unknown:
        _err(errors, where, f"unrecognized field(s) {sorted(unknown)} - not part of the manifest schema")


def lint_manifest(doc: Any, where: str, errors: list[str]) -> str | None:
    """Structural manifest lint. Returns the manifest's provider_id if present and well-formed."""
    _check_no_unknown_keys(doc, MANIFEST_TOP_LEVEL_KEYS, where, errors)
    if not isinstance(doc, dict):
        return None
    provider_id = doc.get("provider_id")
    if not isinstance(provider_id, str) or not provider_id:
        _err(errors, where, "provider_id must be a non-empty string")
        provider_id = None
    for i, root in enumerate(doc.get("roots") or []):
        _check_no_unknown_keys(root, ROOT_KEYS, f"{where}:roots[{i}]", errors)
        if isinstance(root, dict):
            for j, marker in enumerate(root.get("markers") or []):
                _check_no_unknown_keys(marker, MARKER_KEYS, f"{where}:roots[{i}].markers[{j}]", errors)
    for i, artifact in enumerate(doc.get("artifacts") or []):
        _check_no_unknown_keys(artifact, ARTIFACT_KEYS, f"{where}:artifacts[{i}]", errors)
    return provider_id


def load_manifests(errors: list[str]) -> dict[str, str]:
    """provider_id -> relative path, for every manifest that lints cleanly enough to identify."""
    provider_ids: dict[str, str] = {}
    for path in sorted(MANIFEST_DIR.glob("*.json")):
        where = str(path.relative_to(ROOT))
        try:
            doc = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            _err(errors, where, f"cannot read/parse: {exc}")
            continue
        provider_id = lint_manifest(doc, where, errors)
        if provider_id is not None:
            if provider_id in provider_ids:
                _err(errors, where, f"duplicate provider_id {provider_id!r}, also used by {provider_ids[provider_id]}")
            else:
                provider_ids[provider_id] = where
    return provider_ids


def validate_registry_entry(entry: Any, where: str, errors: list[str]) -> str | None:
    allowed = {"provider_id", "tier", "verified_by", "fixture_references"}
    _check_no_unknown_keys(entry, allowed, where, errors)
    if not isinstance(entry, dict):
        return None
    provider_id = entry.get("provider_id")
    if not isinstance(provider_id, str) or not provider_id:
        _err(errors, where, "provider_id must be a non-empty string")
        provider_id = None
    tier = entry.get("tier")
    if tier not in VALID_TIERS:
        _err(errors, where, f"tier must be one of {sorted(VALID_TIERS)}, got {tier!r}")
        tier = None
    verified_by = entry.get("verified_by")
    fixture_references = entry.get("fixture_references")
    if not isinstance(fixture_references, list) or not all(isinstance(item, str) for item in fixture_references):
        _err(errors, where, "fixture_references must be a list of strings")
        fixture_references = None
    if tier is not None and tier != "untrusted":
        if not isinstance(verified_by, str) or not verified_by.strip():
            _err(
                errors,
                where,
                f"tier {tier!r} requires a non-empty verified_by - a promotion cannot be self-attested "
                "(mirrors cancellai-safety::trust_promotion's MissingVerifier rule)",
            )
        if not fixture_references:
            _err(
                errors,
                where,
                f"tier {tier!r} requires at least one fixture_references entry - a promotion needs "
                "maintainer-owned compatibility evidence, not a bare claim "
                "(mirrors cancellai-safety::trust_promotion's MissingFixtureEvidence rule)",
            )
    return provider_id


def validate_registry(registry: Any, manifest_provider_ids: dict[str, str], where: str) -> list[str]:
    """Pure cross-check between a parsed registry document and the known manifest provider ids -
    no filesystem access, so tests can exercise it against synthetic documents directly rather
    than mutating the real committed registry file on disk."""
    errors: list[str] = []
    _check_no_unknown_keys(registry, {"schema_version", "publishers"}, where, errors)
    if not isinstance(registry, dict):
        return errors
    if registry.get("schema_version") != 1:
        _err(errors, where, f"schema_version must be 1, got {registry.get('schema_version')!r}")
    publishers = registry.get("publishers")
    if not isinstance(publishers, list):
        _err(errors, where, "publishers must be a list")
        return errors

    registry_provider_ids: dict[str, int] = {}
    for i, entry in enumerate(publishers):
        entry_where = f"{where}:publishers[{i}]"
        provider_id = validate_registry_entry(entry, entry_where, errors)
        if provider_id is None:
            continue
        if provider_id in registry_provider_ids:
            _err(errors, entry_where, f"duplicate provider_id {provider_id!r}, also at publishers[{registry_provider_ids[provider_id]}]")
            continue
        registry_provider_ids[provider_id] = i
        if provider_id not in manifest_provider_ids:
            _err(
                errors,
                entry_where,
                f"registry entry for provider_id {provider_id!r} names no manifest under "
                f"{MANIFEST_DIR.relative_to(ROOT)} - a trust decision must name a real, reviewed manifest",
            )

    missing_registry_entries = set(manifest_provider_ids) - set(registry_provider_ids)
    if missing_registry_entries:
        _err(
            errors,
            where,
            f"manifest(s) with no registry entry: {sorted(missing_registry_entries)} - every shipped "
            "manifest needs an explicit, auditable trust decision, even if that decision is 'untrusted'",
        )

    return errors


def validate() -> list[str]:
    errors: list[str] = []
    manifest_provider_ids = load_manifests(errors)

    try:
        registry = json.loads(REGISTRY_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ProviderTrustError(f"cannot read/parse {REGISTRY_PATH.relative_to(ROOT)}: {exc}") from exc

    errors.extend(validate_registry(registry, manifest_provider_ids, str(REGISTRY_PATH.relative_to(ROOT))))
    return errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate the community provider verification workflow.")
    parser.add_argument("command", nargs="?", default="check", choices=["check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    build_parser().parse_args(argv)
    try:
        errors = validate()
    except ProviderTrustError as exc:
        print(f"PROVIDER TRUST ERROR: {exc}", file=sys.stderr)
        return 2
    if errors:
        print("PROVIDER TRUST ERROR:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 2
    manifest_count = len(list(MANIFEST_DIR.glob("*.json")))
    print(f"provider trust OK: {manifest_count} manifest(s) linted, registry entries complete and evidenced")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
