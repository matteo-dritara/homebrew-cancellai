#!/usr/bin/env python3
"""Reference-provider compatibility matrix generator/checker (E05-S05).

Runs the Rust `compatibility_matrix` example
(`rust/crates/cancellai-cli/examples/compatibility_matrix.rs`) against the two reference
adapters (`cancellai-provider-claude`, `cancellai-provider-codex`) and renders its JSON output
as a Markdown table inside `docs/PROVIDERS.md`, between generated markers. `generate`
regenerates that section in place; `check` fails if the committed section has drifted from
what the adapters currently produce - the "Generated matrix drift check from adapter
metadata" E05-S05's verification plan names, mirroring the generate/check convention every
other governance script in this directory already uses (`scripts/project_os.py`,
`scripts/gen_docs.py`).
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
RUST_DIR = ROOT / "rust"
PROVIDERS_DOC = ROOT / "docs" / "PROVIDERS.md"

BEGIN_MARKER = "<!-- BEGIN GENERATED: provider-compatibility-matrix -->"
END_MARKER = "<!-- END GENERATED: provider-compatibility-matrix -->"

# Layout scenario order and human labels - fixed here (not derived from the JSON) so the
# table's column order is stable across runs regardless of Rust `HashMap`/collection ordering
# upstream.
LAYOUT_LABELS: dict[str, str] = {
    "known_default_root": "Known (default root)",
    "unknown_custom_root": "Unknown (fail-closed)",
}


class CompatibilityMatrixError(RuntimeError):
    pass


def run_matrix_example() -> list[dict[str, Any]]:
    cargo = shutil.which("cargo")
    if not cargo:
        raise CompatibilityMatrixError("cargo is not available on PATH")
    result = subprocess.run(  # noqa: S603
        [cargo, "run", "--quiet", "--example", "compatibility_matrix", "-p", "cancellai-cli"],
        capture_output=True,
        text=True,
        check=False,
        cwd=RUST_DIR,
        timeout=180,
    )
    if result.returncode != 0:
        raise CompatibilityMatrixError(f"compatibility_matrix example failed (exit {result.returncode}): {result.stderr.strip()}")
    try:
        rows = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise CompatibilityMatrixError(f"compatibility_matrix example did not print valid JSON: {exc}") from exc
    if not isinstance(rows, list) or not rows:
        raise CompatibilityMatrixError("compatibility_matrix example printed no rows")
    for row in rows:
        missing = {"provider_id", "layout", "capability", "support", "confidence"} - row.keys()
        if missing:
            raise CompatibilityMatrixError(f"compatibility_matrix row missing fields {sorted(missing)}: {row}")
    return rows


def render_table(rows: list[dict[str, Any]]) -> str:
    providers = sorted({row["provider_id"] for row in rows})
    layouts = [layout for layout in LAYOUT_LABELS if any(row["layout"] == layout for row in rows)]
    unexpected_layouts = sorted({row["layout"] for row in rows} - set(LAYOUT_LABELS))
    if unexpected_layouts:
        raise CompatibilityMatrixError(
            f"compatibility_matrix example produced unrecognized layout(s) {unexpected_layouts}; add a label to LAYOUT_LABELS"
        )

    capabilities: list[str] = []
    seen: set[str] = set()
    for row in rows:
        if row["capability"] not in seen:
            seen.add(row["capability"])
            capabilities.append(row["capability"])

    by_key = {(row["provider_id"], row["layout"], row["capability"]): row for row in rows}

    lines: list[str] = []
    for provider in providers:
        lines.append(f"### `{provider}`")
        lines.append("")
        header = "| Capability | " + " | ".join(LAYOUT_LABELS[layout] for layout in layouts) + " |"
        separator = "| --- | " + " | ".join("---" for _ in layouts) + " |"
        lines.append(header)
        lines.append(separator)
        for capability in capabilities:
            cells = []
            for layout in layouts:
                cell_row = by_key.get((provider, layout, capability))
                cells.append("-" if cell_row is None else f"`{cell_row['support']}` ({cell_row['confidence']})")
            lines.append(f"| `{capability}` | " + " | ".join(cells) + " |")
        lines.append("")
    return "\n".join(lines).rstrip("\n") + "\n"


def build_section(rows: list[dict[str, Any]]) -> str:
    return f"{BEGIN_MARKER}\n\n{render_table(rows)}\n{END_MARKER}\n"


def current_section(original: str) -> str:
    if BEGIN_MARKER not in original or END_MARKER not in original:
        raise CompatibilityMatrixError(
            f"{PROVIDERS_DOC}: missing {BEGIN_MARKER!r}/{END_MARKER!r} markers - add them once, by hand, before running generate"
        )
    start = original.index(BEGIN_MARKER)
    end = original.index(END_MARKER) + len(END_MARKER)
    return original[start:end] + "\n"


def splice_section(original: str, new_section: str) -> str:
    start = original.index(BEGIN_MARKER)
    end = original.index(END_MARKER) + len(END_MARKER)
    trailing = original[end:].lstrip("\n")
    return original[:start] + new_section.rstrip("\n") + "\n" + (f"\n{trailing}" if trailing else "")


def generate() -> None:
    rows = run_matrix_example()
    original = PROVIDERS_DOC.read_text(encoding="utf-8")
    section = build_section(rows)
    updated = splice_section(original, section)
    PROVIDERS_DOC.write_text(updated, encoding="utf-8")
    print(f"wrote {PROVIDERS_DOC.relative_to(ROOT)}")


def check() -> None:
    rows = run_matrix_example()
    original = PROVIDERS_DOC.read_text(encoding="utf-8")
    committed = current_section(original)
    fresh = build_section(rows)
    if committed.strip() != fresh.strip():
        raise CompatibilityMatrixError(
            f"{PROVIDERS_DOC.relative_to(ROOT)}'s generated compatibility matrix has drifted from the adapters' current output - "
            "run `python3 scripts/check_provider_compatibility.py generate` and commit the result"
        )
    print(f"provider compatibility matrix OK: {len(rows)} rows across {len({row['provider_id'] for row in rows})} provider(s)")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Generate/check the reference-provider compatibility matrix in docs/PROVIDERS.md.")
    parser.add_argument("command", choices=["generate", "check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "generate":
            generate()
        else:
            check()
    except CompatibilityMatrixError as exc:
        print(f"PROVIDER COMPATIBILITY ERROR: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
