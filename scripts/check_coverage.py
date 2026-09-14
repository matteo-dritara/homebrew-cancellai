#!/usr/bin/env python3
"""Coverage may not fall in the crates where falling matters (E27-S02).

Coverage had never been measured in this repository until E27-S01 ran `cargo llvm-cov` for the
first time. The headline was 94.54% of regions, which is a good number and the wrong one to act
on: an average hides its worst member, and the worst member here was `cancellai-sealedfs` - the
crate holding every `unsafe` block and the mutation boundary.

So this is a **ratchet, not a target**. It records what each crate covers today and fails when a
crate falls below what it already had. Nobody has to argue for a number nobody can justify, and
nothing can quietly get worse. A crate that improves and re-records tightens its own floor.

Two deliberate asymmetries:

- Only the **kernel ring** is ratcheted (`cancellai-model`, `cancellai-safety`,
  `cancellai-platform`, `cancellai-sealedfs`) plus `cancellai-policy` and `cancellai-inventory`,
  which decide eligibility and completeness. The TUI and the CLI are recorded for visibility and
  not gated: a rendering regression is a bug, not an authority failure, and gating it would spend
  review attention where the risk is not.
- A crate may exceed its floor without re-recording, but may not fall. `record` is explicit and
  its diff is reviewable, which is the point - a ratchet that lowers itself automatically is a
  number that drifts down one accepted commit at a time.

Needs `cargo llvm-cov`, which is not on a stock CI runner, so this runs where that is installed and
says so rather than passing when it cannot measure.
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
RUST = ROOT / "rust"
BASELINE = ROOT / "project" / "coverage_baseline.json"

# The measurement is taken with a named toolchain, not with whatever `rustup` happens to default
# to. This is not tidiness: the same unchanged workspace measures 95.83% for `cancellai-platform`
# on stable and 63.85% on nightly, because region counting depends on the compiler that produced
# the instrumentation. A baseline without provenance is a number compared against a different
# number, and reporting that as a coverage regression is a gate describing the machine it ran on
# (E27-S07, found by the E27-S06 independent review getting a third value again).
MEASUREMENT_TOOLCHAIN = "stable"

# The crates whose coverage is a safety property rather than a quality one.
RATCHETED = (
    "cancellai-model",
    "cancellai-safety",
    "cancellai-platform",
    "cancellai-sealedfs",
    "cancellai-policy",
    "cancellai-inventory",
)
# Falling by less than this is measurement noise, not a regression: region counts shift when a
# function is split or a match arm is reordered, with no test lost.
TOLERANCE = 0.5


class CoverageError(RuntimeError):
    pass


def measure() -> dict[str, float]:
    """Region coverage per crate, from a real `cargo llvm-cov` run."""
    cargo = shutil.which("cargo")
    if cargo is None:
        raise CoverageError("`cargo` is not on PATH; this gate measures rather than assumes")
    try:
        result = subprocess.run(  # noqa: S603 - fixed argument list, resolved executable
            [cargo, f"+{MEASUREMENT_TOOLCHAIN}", "llvm-cov", "--workspace", "--json", "--summary-only"],
            cwd=RUST,
            capture_output=True,
            text=True,
            timeout=1800,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise CoverageError(f"could not run `cargo llvm-cov`: {exc}") from exc
    if result.returncode != 0:
        raise CoverageError(f"`cargo llvm-cov` failed:\n{result.stderr.strip()[-2000:]}")

    try:
        report = json.loads(result.stdout)
        files = report["data"][0]["files"]
    except (json.JSONDecodeError, KeyError, IndexError) as exc:
        raise CoverageError(f"unexpected `cargo llvm-cov` output: {exc}") from exc

    totals: dict[str, list[int]] = {}
    for entry in files:
        crate = crate_of(entry["filename"])
        if crate is None:
            continue
        regions = entry["summary"]["regions"]
        counts = totals.setdefault(crate, [0, 0])
        counts[0] += int(regions["count"])
        counts[1] += int(regions["covered"])
    return {crate: round(100.0 * covered / count, 2) if count else 100.0 for crate, (count, covered) in sorted(totals.items())}


def provenance() -> dict[str, str]:
    """What produced a measurement: the toolchain, its compiler, and the coverage tool.

    Recorded beside the numbers so a later comparison can tell a coverage regression from a
    different compiler. Everything here is a version string; nothing about the machine.
    """
    cargo = shutil.which("cargo")
    if cargo is None:
        return {"toolchain": MEASUREMENT_TOOLCHAIN, "rustc": "unknown", "cargo_llvm_cov": "unknown"}

    def version(binary: str, *args: str) -> str:
        resolved = shutil.which(binary)
        if resolved is None:
            return "unknown"
        try:
            result = subprocess.run(  # noqa: S603 - fixed argument list, resolved executable
                [resolved, f"+{MEASUREMENT_TOOLCHAIN}", *args], capture_output=True, text=True, timeout=60, check=False
            )
        except (OSError, subprocess.TimeoutExpired):
            return "unknown"
        return result.stdout.strip().splitlines()[0] if result.returncode == 0 and result.stdout.strip() else "unknown"

    return {
        "toolchain": MEASUREMENT_TOOLCHAIN,
        "rustc": version("rustc", "--version"),
        "cargo_llvm_cov": version("cargo", "llvm-cov", "--version"),
    }


def provenance_errors(now: dict[str, str], recorded: dict[str, str]) -> list[str]:
    """A measurement taken with different tools is not comparable, and must not read as a fall."""
    if not recorded:
        return ["the baseline records no measurement provenance; run `record`"]
    differences = [
        f"{key}: measured with {now.get(key, 'unknown')!r}, baseline recorded {recorded.get(key, 'unknown')!r}"
        for key in sorted(recorded)
        if now.get(key) != recorded.get(key)
    ]
    if not differences:
        return []
    return [
        "the measurement was taken with different tooling than the baseline, so the numbers are not "
        "comparable and no conclusion about coverage is drawn:\n    " + "\n    ".join(differences)
    ]


def crate_of(filename: str) -> str | None:
    parts = Path(filename).parts
    if "crates" not in parts:
        return None
    index = parts.index("crates")
    return parts[index + 1] if index + 1 < len(parts) else None


def load_baseline() -> dict[str, Any]:
    if not BASELINE.is_file():
        return {"schema_version": 1, "crates": {}}
    data: dict[str, Any] = json.loads(BASELINE.read_text(encoding="utf-8"))
    return data


def evaluate(measured: dict[str, float], baseline: dict[str, Any]) -> tuple[list[str], list[str]]:
    errors: list[str] = []
    notes: list[str] = []
    recorded: dict[str, float] = baseline.get("crates", {})
    for crate in RATCHETED:
        if crate not in measured:
            errors.append(f"{crate}: ratcheted but absent from the coverage report - did it stop building?")
            continue
        if crate not in recorded:
            errors.append(f"{crate}: ratcheted with no recorded floor; run `record`")
            continue
        now, floor = measured[crate], float(recorded[crate])
        if now + TOLERANCE < floor:
            errors.append(f"{crate}: region coverage fell to {now:.2f}% from a recorded {floor:.2f}%")
        elif now > floor + TOLERANCE:
            notes.append(f"{crate}: {now:.2f}% is above its recorded {floor:.2f}% - `record` to tighten the ratchet")
    for crate, value in measured.items():
        if crate not in RATCHETED:
            notes.append(f"{crate}: {value:.2f}% (recorded for visibility, not ratcheted)")
    return errors, notes


def record() -> int:
    measured = measure()
    BASELINE.write_text(
        json.dumps(
            {
                "schema_version": 2,
                "comment": "Region coverage per crate, from `cargo llvm-cov`. A ratchet, not a target: see scripts/check_coverage.py.",
                "provenance": provenance(),
                "crates": measured,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"recorded {len(measured)} crates in {BASELINE.relative_to(ROOT)}")
    for crate, value in measured.items():
        marker = "*" if crate in RATCHETED else " "
        print(f"  {marker} {crate:26} {value:6.2f}%")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Ratchet region coverage for the crates where falling matters.")
    parser.add_argument("command", nargs="?", default="check", choices=["check", "record", "report"])
    return parser


def main(argv: list[str] | None = None) -> int:
    command = build_parser().parse_args(argv).command
    try:
        if command == "record":
            return record()
        baseline = load_baseline()
        mismatch = provenance_errors(provenance(), baseline.get("provenance", {}))
        if mismatch:
            for line in mismatch:
                print(f"COVERAGE ERROR: {line}", file=sys.stderr)
            return 2
        measured = measure()
        errors, notes = evaluate(measured, baseline)
    except CoverageError as exc:
        print(f"COVERAGE ERROR: {exc}", file=sys.stderr)
        return 2

    if command == "report":
        for crate, value in measured.items():
            print(f"  {'*' if crate in RATCHETED else ' '} {crate:26} {value:6.2f}%")
        return 0
    for note in notes:
        print(f"note: {note}")
    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    if errors:
        return 2
    print(f"coverage OK: {len(RATCHETED)} ratcheted crates at or above their recorded floor")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
