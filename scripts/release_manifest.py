#!/usr/bin/env python3
"""Validate the canonical release artifact manifest contract (E17-S01).

Defines the machine-verifiable, versioned document a cancellAI release publishes to name its
artifacts exactly once, with checksums, target triples, channel, source commit, build
provenance, and provider-knowledge compatibility range - see
project/schemas/release_manifest.schema.json and docs/security/SUPPLY_CHAIN.md's "Canonical
release evidence" section. Hand-validated against that schema (not a jsonschema dependency),
matching every other governance checker in this repository: stdlib-only, so it keeps working
across the Python -> Rust migration.

Generating a manifest from a real multi-platform build is E17-S02/E17-S03 scope, not this
story's - `check` here validates the committed golden fixture(s) under
tests/fixtures/release_manifest/golden/, the same shape check_schemas.py uses for the
inventory/plan/explanation/result contracts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections.abc import Sequence
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
SCHEMA_PATH = ROOT / "project" / "schemas" / "release_manifest.schema.json"
GOLDEN_DIR = ROOT / "tests" / "fixtures" / "release_manifest" / "golden"

SEMVER_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
GIT_SHA_RE = re.compile(r"^[0-9a-f]{40}$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
ARTIFACT_NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
CHANNELS = ("stable", "beta", "nightly")  # docs/security/SUPPLY_CHAIN.md Release channels


class ReleaseManifestError(RuntimeError):
    pass


def _err(errors: list[str], where: str, message: str) -> None:
    errors.append(f"{where}: {message}")


def _require(doc: dict[str, Any], key: str, where: str, errors: list[str]) -> None:
    if key not in doc:
        _err(errors, where, f"missing required key {key!r}")


def _check_build_identity(doc: dict[str, Any], where: str, errors: list[str]) -> None:
    identity = doc.get("build_identity")
    if not isinstance(identity, dict):
        _err(errors, where, "build_identity must be an object")
        return
    for key in ("repository", "workflow", "run_id"):
        value = identity.get(key)
        if not isinstance(value, str) or not value.strip():
            _err(errors, f"{where}.build_identity", f"{key!r} must be a non-empty string")


def _check_knowledge_compatibility(doc: dict[str, Any], where: str, errors: list[str]) -> None:
    compat = doc.get("knowledge_compatibility")
    if not isinstance(compat, dict):
        _err(errors, where, "knowledge_compatibility must be an object")
        return
    lo, hi = compat.get("min_schema_version"), compat.get("max_schema_version")
    if not isinstance(lo, int) or lo < 1:
        _err(errors, f"{where}.knowledge_compatibility", "min_schema_version must be an integer >= 1")
    if not isinstance(hi, int) or hi < 1:
        _err(errors, f"{where}.knowledge_compatibility", "max_schema_version must be an integer >= 1")
    if isinstance(lo, int) and isinstance(hi, int) and lo > hi:
        _err(
            errors,
            f"{where}.knowledge_compatibility",
            f"min_schema_version ({lo}) must not exceed max_schema_version ({hi})",
        )


def _check_artifacts(doc: dict[str, Any], where: str, errors: list[str]) -> None:
    artifacts = doc.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        _err(errors, where, "artifacts must be a non-empty list")
        return

    seen_names: dict[str, int] = {}
    for index, artifact in enumerate(artifacts):
        artifact_where = f"{where}.artifacts[{index}]"
        if not isinstance(artifact, dict):
            _err(errors, artifact_where, "artifact entry must be an object")
            continue
        for key in ("name", "target_triple", "sha256"):
            _require(artifact, key, artifact_where, errors)

        name = artifact.get("name")
        if isinstance(name, str):
            if not ARTIFACT_NAME_RE.match(name):
                _err(errors, artifact_where, f"name {name!r} does not match {ARTIFACT_NAME_RE.pattern!r}")
            seen_names[name] = seen_names.get(name, 0) + 1
        elif "name" in artifact:
            _err(errors, artifact_where, "name must be a string")

        triple = artifact.get("target_triple")
        if "target_triple" in artifact and (not isinstance(triple, str) or not triple.strip()):
            _err(errors, artifact_where, "target_triple must be a non-empty string")

        checksum = artifact.get("sha256")
        if "sha256" in artifact and (not isinstance(checksum, str) or not SHA256_RE.match(checksum)):
            _err(errors, artifact_where, f"sha256 must be a lowercase 64-hex digest, got {checksum!r}")

    for name, count in seen_names.items():
        if count > 1:
            _err(
                errors,
                where,
                f"artifact name {name!r} appears {count} times; every distributed binary must be represented exactly once (AC2)",
            )


def validate_document(doc: Any, where: str) -> list[str]:
    if not isinstance(doc, dict):
        return [f"{where}: document must be a JSON object"]

    errors: list[str] = []
    for key in (
        "schema_version",
        "document_type",
        "version",
        "channel",
        "source_sha",
        "build_identity",
        "knowledge_compatibility",
        "artifacts",
    ):
        _require(doc, key, where, errors)

    if doc.get("schema_version") != 1:
        _err(errors, where, f"schema_version must be 1, got {doc.get('schema_version')!r}")
    if doc.get("document_type") != "release_manifest":
        _err(errors, where, f"document_type must be 'release_manifest', got {doc.get('document_type')!r}")

    version = doc.get("version")
    if "version" in doc and (not isinstance(version, str) or not SEMVER_RE.match(version)):
        _err(errors, where, f"version must be a SemVer string, got {version!r}")

    channel = doc.get("channel")
    if "channel" in doc and channel not in CHANNELS:
        _err(errors, where, f"channel must be one of {CHANNELS}, got {channel!r}")

    source_sha = doc.get("source_sha")
    if "source_sha" in doc and (not isinstance(source_sha, str) or not GIT_SHA_RE.match(source_sha)):
        _err(errors, where, f"source_sha must be a 40-hex git commit SHA, got {source_sha!r}")

    if "build_identity" in doc:
        _check_build_identity(doc, where, errors)
    if "knowledge_compatibility" in doc:
        _check_knowledge_compatibility(doc, where, errors)
    if "artifacts" in doc:
        _check_artifacts(doc, where, errors)

    known_keys = {
        "schema_version",
        "document_type",
        "version",
        "channel",
        "source_sha",
        "build_identity",
        "knowledge_compatibility",
        "artifacts",
    }
    for key in doc:
        if key not in known_keys:
            _err(errors, where, f"unknown key {key!r}")

    return errors


def sha256_of(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def checksum_matches(data: bytes, expected_sha256: str) -> bool:
    return sha256_of(data) == expected_sha256.lower()


def verify_artifact_checksums(doc: dict[str, Any], artifact_bytes: dict[str, bytes]) -> list[str]:
    """Round-trip check: recompute SHA-256 for every artifact whose bytes are supplied.

    `artifact_bytes` need not cover every artifact in the manifest (a manifest may be
    validated without every binary present, e.g. structural checks in CI); it is only ever
    an error for an artifact this function was actually given bytes for.
    """
    errors: list[str] = []
    by_name = {a.get("name"): a for a in doc.get("artifacts", []) if isinstance(a, dict)}
    for name, data in artifact_bytes.items():
        artifact = by_name.get(name)
        if artifact is None:
            errors.append(f"artifact {name!r} has no matching manifest entry")
            continue
        expected = artifact.get("sha256")
        if not isinstance(expected, str) or not checksum_matches(data, expected):
            errors.append(f"artifact {name!r}: checksum mismatch (manifest says {expected!r}, computed {sha256_of(data)!r})")
    return errors


def build_manifest(
    *,
    version: str,
    channel: str,
    source_sha: str,
    repository: str,
    workflow: str,
    run_id: str,
    knowledge_min: int,
    knowledge_max: int,
    artifacts: Sequence[tuple[str, str, str]],
) -> dict[str, Any]:
    """Assemble a manifest document and validate it before returning.

    `artifacts` is `(name, target_triple, sha256)` triples. Raises ReleaseManifestError with
    every violation found rather than returning a document that would fail its own contract.
    """
    doc: dict[str, Any] = {
        "schema_version": 1,
        "document_type": "release_manifest",
        "version": version,
        "channel": channel,
        "source_sha": source_sha,
        "build_identity": {"repository": repository, "workflow": workflow, "run_id": run_id},
        "knowledge_compatibility": {"min_schema_version": knowledge_min, "max_schema_version": knowledge_max},
        "artifacts": [{"name": name, "target_triple": triple, "sha256": checksum} for name, triple, checksum in artifacts],
    }
    errors = validate_document(doc, "generated manifest")
    if errors:
        raise ReleaseManifestError("refusing to build an invalid manifest:\n" + "\n".join(f"- {e}" for e in errors))
    return doc


def build_manifest_from_files(
    *,
    version: str,
    channel: str,
    source_sha: str,
    repository: str,
    workflow: str,
    run_id: str,
    knowledge_min: int,
    knowledge_max: int,
    artifact_files: Sequence[tuple[str, str, Path]],
) -> dict[str, Any]:
    """Like `build_manifest`, but the checksum for each artifact is computed from real bytes
    read off disk (E17-S02: a manifest generated from an actual build, not typed by hand).

    `artifact_files` is `(name, target_triple, path)` triples; every path must exist and be
    readable, or this raises before any checksum is computed - a manifest naming an artifact
    this machine cannot actually read is not evidence of anything.
    """
    artifacts: list[tuple[str, str, str]] = []
    missing: list[str] = []
    for name, triple, path in artifact_files:
        if not path.is_file():
            missing.append(f"{name}: no such file {path}")
            continue
        artifacts.append((name, triple, sha256_of(path.read_bytes())))
    if missing:
        raise ReleaseManifestError("cannot generate a manifest for missing artifact file(s):\n" + "\n".join(f"- {m}" for m in missing))
    return build_manifest(
        version=version,
        channel=channel,
        source_sha=source_sha,
        repository=repository,
        workflow=workflow,
        run_id=run_id,
        knowledge_min=knowledge_min,
        knowledge_max=knowledge_max,
        artifacts=artifacts,
    )


def verify_checksums_against_directory(doc: dict[str, Any], directory: Path) -> list[str]:
    """Round-trip every artifact in `doc` against a real file named `<artifact name>.*` under
    `directory` (E17-S02's publish-time guard: refuse to publish a manifest whose declared
    checksums do not match the bytes actually built).

    An artifact with no matching file under `directory` is an error - a manifest cannot be
    trusted to describe a release whose artifacts are not all present to check.
    """
    errors: list[str] = []
    artifact_bytes: dict[str, bytes] = {}
    for artifact in doc.get("artifacts", []):
        if not isinstance(artifact, dict):
            continue
        name = artifact.get("name")
        if not isinstance(name, str):
            continue
        matches = sorted(directory.glob(f"{name}.*"))
        if not matches:
            errors.append(f"artifact {name!r}: no file matching {name}.* under {directory}")
            continue
        artifact_bytes[name] = matches[0].read_bytes()
    errors.extend(verify_artifact_checksums(doc, artifact_bytes))
    return errors


def check() -> list[str]:
    errors: list[str] = []
    if not SCHEMA_PATH.exists():
        raise ReleaseManifestError(f"{SCHEMA_PATH} does not exist")
    try:
        json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ReleaseManifestError(f"{SCHEMA_PATH}: invalid JSON: {exc}") from exc

    if not GOLDEN_DIR.is_dir():
        raise ReleaseManifestError(f"{GOLDEN_DIR} does not exist")
    golden_files = sorted(GOLDEN_DIR.glob("*.golden.json"))
    if not golden_files:
        raise ReleaseManifestError(f"no golden release manifests found under {GOLDEN_DIR}")

    for path in golden_files:
        try:
            doc = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            errors.append(f"{path.relative_to(ROOT)}: cannot read/parse: {exc}")
            continue
        errors.extend(validate_document(doc, str(path.relative_to(ROOT))))
    return errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate/generate the cancellAI release artifact manifest.")
    sub = parser.add_subparsers(dest="command")
    sub.add_parser("check", help="Validate the committed golden manifest corpus (default).")

    generate = sub.add_parser("generate", help="Build a manifest from real, already-built artifact files (E17-S02).")
    generate.add_argument("--version", required=True)
    generate.add_argument("--channel", required=True, choices=CHANNELS)
    generate.add_argument("--source-sha", required=True)
    generate.add_argument("--repository", required=True)
    generate.add_argument("--workflow", required=True)
    generate.add_argument("--run-id", required=True)
    generate.add_argument("--knowledge-min", required=True, type=int)
    generate.add_argument("--knowledge-max", required=True, type=int)
    generate.add_argument(
        "--artifact",
        action="append",
        nargs=3,
        metavar=("NAME", "TARGET_TRIPLE", "PATH"),
        default=[],
        help="repeatable: one built artifact's canonical name, target triple, and file path",
    )
    generate.add_argument("--out", required=True, type=Path, help="where to write the generated manifest JSON")

    verify = sub.add_parser(
        "verify-checksums",
        help="Round-trip a manifest's declared checksums against real files (E17-S02 publish-time guard).",
    )
    verify.add_argument("manifest", type=Path, help="path to a release-manifest.json document")
    verify.add_argument("directory", type=Path, help="directory containing '<artifact name>.*' files to check")

    return parser


def _cmd_generate(args: argparse.Namespace) -> int:
    try:
        doc = build_manifest_from_files(
            version=args.version,
            channel=args.channel,
            source_sha=args.source_sha,
            repository=args.repository,
            workflow=args.workflow,
            run_id=args.run_id,
            knowledge_min=args.knowledge_min,
            knowledge_max=args.knowledge_max,
            artifact_files=[(name, triple, Path(path)) for name, triple, path in args.artifact],
        )
    except ReleaseManifestError as exc:
        print(f"RELEASE MANIFEST ERROR: {exc}", file=sys.stderr)
        return 2
    args.out.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {args.out} ({len(doc['artifacts'])} artifact(s))")
    return 0


def _cmd_verify_checksums(args: argparse.Namespace) -> int:
    try:
        doc = json.loads(args.manifest.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"RELEASE MANIFEST ERROR: cannot read {args.manifest}: {exc}", file=sys.stderr)
        return 2
    structural_errors = validate_document(doc, str(args.manifest))
    if structural_errors:
        print("RELEASE MANIFEST ERROR:", file=sys.stderr)
        for error in structural_errors:
            print(f"  {error}", file=sys.stderr)
        return 2
    checksum_errors = verify_checksums_against_directory(doc, args.directory)
    if checksum_errors:
        print("RELEASE MANIFEST ERROR:", file=sys.stderr)
        for error in checksum_errors:
            print(f"  {error}", file=sys.stderr)
        return 2
    print(f"release manifest checksums OK: {len(doc['artifacts'])} artifact(s) match {args.directory}")
    return 0


def _cmd_check() -> int:
    try:
        errors = check()
    except ReleaseManifestError as exc:
        print(f"RELEASE MANIFEST ERROR: {exc}", file=sys.stderr)
        return 2
    if errors:
        print("RELEASE MANIFEST ERROR:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 2
    count = len(list(GOLDEN_DIR.glob("*.golden.json")))
    print(f"release manifest OK: {count} golden document(s) match project/schemas/release_manifest.schema.json")
    return 0


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    command = args.command or "check"
    if command == "generate":
        return _cmd_generate(args)
    if command == "verify-checksums":
        return _cmd_verify_checksums(args)
    return _cmd_check()


if __name__ == "__main__":
    raise SystemExit(main())
