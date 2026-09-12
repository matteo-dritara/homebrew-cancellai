#!/usr/bin/env python3
"""Every gate is shown to catch a planted violation, or reported as unable to (E25-S06).

Nothing in this repository had ever deliberately introduced a Safety Invariant violation to find
out whether the gate set catches it, so every claim about what the gates prevent was untested.
The 2026-09-12 methodology review named this M-05, and also named the awkward part: **the
technique was already in-tree and had never been generalised.**
`scripts/rust_python_parity.py self-test` injects a catalogue of known divergence classes and
asserts the comparator catches each; `scripts/diff_harness.py check` does the same for the
differential harness. That is Mills' error seeding, correctly implemented, on two comparators -
and never on the gates that guard the product.

This generalises it. Each mutant below is a small, surgical edit to a copy of the repository that
violates one named invariant. The gate set runs against the copy, and the result is recorded:
**which gate caught it, or that none did.** An invariant with no killing gate is asserted, not
verified, and the report says so in those words.

Three design rules, each of which a weaker harness gets wrong:

**It never touches the committed tree.** Every run works on a `git archive` export in a temporary
directory. A harness that mutates the repository it is auditing will eventually leave a mutant
behind, and that mutant will be a real defect that looks like a test artifact.

**A mutant that fails to apply is an error, not a pass.** If the text a mutant expects is no
longer there, the code has moved and the mutant is testing nothing. Reporting that as "no gate
caught it" would be wrong in the dangerous direction; reporting it as a pass would be worse.

**Only fast, hermetic gates run.** `cargo` is not invoked: a harness nobody runs because it takes
twenty minutes measures nothing. The Rust invariants covered here are the ones a static checker
can decide from source text, which is exactly the set `check_mutation_boundary.py` was built for.

Stdlib-only, like every other governance checker here.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "project" / "generated" / "GATE_SENSITIVITY.md"
INVARIANTS = ROOT / "docs" / "security" / "SAFETY_INVARIANTS.md"


@dataclass(frozen=True)
class Mutant:
    """One planted violation: what it breaks, where, and what it should cost."""

    identifier: str
    invariant: str
    claim: str
    path: str
    find: str
    replace: str
    gates: tuple[str, ...]


# The gate commands a mutant may be checked against. Fast and hermetic: no cargo, no network.
GATES: dict[str, tuple[str, ...]] = {
    "pytest": ("python3", "-m", "pytest", "tests", "-x", "-q"),
    "characterization": ("python3", "scripts/characterize.py", "check"),
    "docs": ("python3", "scripts/check_docs.py", "check"),
    "project-os": ("python3", "scripts/project_os.py", "check"),
    "rust-workspace": ("python3", "scripts/check_rust_workspace.py", "check"),
    "agent-skills": ("python3", "scripts/check_agent_skills.py", "check"),
    "evidence": ("python3", "scripts/check_evidence.py", "check"),
    "risk-classification": ("python3", "scripts/check_risk_classification.py", "check"),
    "agent-toolchain": ("python3", "scripts/check_agent_toolchain.py", "check"),
    "mutation-boundary-rust": ("python3", "scripts/check_mutation_boundary.py", "check"),
}

MUTANTS: tuple[Mutant, ...] = (
    Mutant(
        identifier="protected-name-removed",
        invariant="SI-001",
        claim="Protected/unknown state is non-destructive",
        path="cancellai.py",
        find='"settings.json"',
        replace='"settings-DISARMED.json"',
        gates=("pytest", "characterization"),
    ),
    Mutant(
        identifier="protected-name-case-sensitive",
        invariant="SI-006",
        claim="Protected-name barriers are defense in depth, and do not depend on spelling",
        path="cancellai.py",
        # Anchored on `protected_component`'s own folding, not on the first `.lower()` in the
        # file - an earlier version hit `extract_uuid`'s session-id lowercasing, a mutant that
        # touched no protected-name code and survived the suite while the report counted SI-006
        # as verified. Found by an independent review.
        find="folded = {canonical_name(name): name for name in protected_names}",
        replace="folded = {name: name for name in protected_names}",
        gates=("pytest",),
    ),
    Mutant(
        identifier="unreadable-reads-as-empty",
        invariant="SI-008",
        claim="Partial scan is non-destructive",
        path="cancellai.py",
        find="except OSError",
        replace="except RuntimeError",
        gates=("pytest",),
    ),
    Mutant(
        identifier="second-deletion-path",
        invariant="SI-019",
        claim="One mutation boundary: only the safety executor's platform seam deletes anything",
        path="rust/crates/cancellai-cli/src/roots.rs",
        find="use std::path::{Path, PathBuf};",
        replace="use std::fs::remove_dir_all;\nuse std::path::{Path, PathBuf};",
        gates=("mutation-boundary-rust",),
    ),
    Mutant(
        identifier="dependency-cycle",
        invariant="SI-019",
        claim="The kernel does not depend on the crates it governs",
        path="rust/crates/cancellai-model/Cargo.toml",
        find="[dependencies]",
        replace='[dependencies]\ncancellai-cli = { path = "../cancellai-cli" }',
        gates=("rust-workspace",),
    ),
    Mutant(
        identifier="invariant-deleted",
        invariant="SI-001",
        claim="The invariant set itself is stable and referenced",
        path="docs/security/SAFETY_INVARIANTS.md",
        find="### SI-019",
        replace="### SI-919",
        gates=("docs",),
    ),
    Mutant(
        identifier="story-risk-lowered",
        invariant="process",
        claim="A story may not declare a risk level below the floor its paths set",
        path="project/risk_floors.json",
        find='"level": "CR4",\n      "reason": "The safety kernel decides',
        replace='"level": "CR0",\n      "reason": "The safety kernel decides',
        gates=("risk-classification",),
    ),
    Mutant(
        identifier="evidence-row-removed",
        invariant="process",
        claim="Every acceptance criterion carries an evidence row",
        path="project/evidence/E26-S01/EVIDENCE.md",
        find="| AC1 -",
        replace="| AC99 -",
        gates=("evidence",),
    ),
    Mutant(
        identifier="unmanaged-component",
        invariant="process",
        claim="A component present and absent from the manifest fails the gate",
        path="project/agent_toolchain.json",
        find='"skill:.claude/skills/toolchain"',
        replace='"skill:.claude/skills/REMOVED"',
        gates=("agent-toolchain",),
    ),
    Mutant(
        identifier="skill-points-nowhere",
        invariant="process",
        claim="A skill's pointers resolve",
        path=".claude/skills/orient/SKILL.md",
        find="docs/CONSTITUTION.md",
        replace="docs/CONSTITUTION_GONE.md",
        gates=("agent-skills",),
    ),
    Mutant(
        identifier="story-status-forged",
        invariant="process",
        claim="A story cannot sit past ready_for_review without committed evidence",
        path="project/epics/E11.json",
        # Anchored on a story that is planned and has no evidence packet, so forging its status
        # is exactly the violation this gate exists to catch. E11 is a future phase and its
        # stories are stable; an earlier version anchored on a story this session was actively
        # moving, and the anchor went stale within the hour - which the harness reported as an
        # error rather than as a pass, which is the whole point of that rule.
        find='"id": "E11-S01",\n      "title": "Policy schema and scopes",\n      "status": "planned"',
        replace='"id": "E11-S01",\n      "title": "Policy schema and scopes",\n      "status": "done"',
        gates=("project-os",),
    ),
)


class SensitivityError(RuntimeError):
    pass


def export_tree(destination: Path) -> None:
    """A clean copy of HEAD plus the working tree, without `.git`.

    Copied rather than `git archive`d so that uncommitted work is covered too: a mutant run
    against HEAD while the change under review sits unstaged would test the wrong code.
    """
    ignore = shutil.ignore_patterns(".git", "target", "__pycache__", ".venv", ".mypy_cache", ".pytest_cache", ".ruff_cache", "*.pyc")
    shutil.copytree(ROOT, destination, ignore=ignore, dirs_exist_ok=True)


def apply_mutant(tree: Path, mutant: Mutant) -> None:
    target = tree / mutant.path
    if not target.is_file():
        raise SensitivityError(f"{mutant.identifier}: {mutant.path} does not exist")
    text = target.read_text(encoding="utf-8")
    if mutant.find not in text:
        raise SensitivityError(
            f"{mutant.identifier}: the text it mutates is no longer in {mutant.path}. "
            "The code moved and this mutant is testing nothing - repair the mutant, do not delete it"
        )
    target.write_text(text.replace(mutant.find, mutant.replace, 1), encoding="utf-8")


def run_gate(tree: Path, name: str) -> bool:
    """True when the gate passed, i.e. failed to catch the mutant."""
    try:
        result = subprocess.run(  # noqa: S603
            GATES[name], cwd=tree, capture_output=True, text=True, timeout=600, check=False
        )
    except (OSError, subprocess.TimeoutExpired):
        return True
    return result.returncode == 0


def control(gates: set[str]) -> dict[str, bool]:
    """Whether each gate passes on an **unmutated** copy of the tree.

    The control experiment, and the one without which this whole harness means nothing: a gate
    that fails on a clean tree - because the export dropped something it needs, `.git` among them,
    since two checkers shell out to git - would appear to "kill" every mutant it is pointed at, and
    every `killed by` in the report would be an artefact of the copy rather than a property of the
    gate.
    """
    with tempfile.TemporaryDirectory() as tmp:
        tree = Path(tmp) / "repo"
        export_tree(tree)
        return {name: run_gate(tree, name) for name in sorted(gates)}


def evaluate(mutant: Mutant) -> tuple[str | None, list[str]]:
    """The first gate that caught the mutant, and every gate that was run."""
    with tempfile.TemporaryDirectory() as tmp:
        tree = Path(tmp) / "repo"
        export_tree(tree)
        apply_mutant(tree, mutant)
        ran: list[str] = []
        killers: list[str] = []
        for name in mutant.gates:
            ran.append(name)
            if not run_gate(tree, name):
                killers.append(name)
    # Every declared gate runs. Short-circuiting on the first kill meant a mutant's second gate
    # was never exercised, so the report could name one gate while saying nothing about another.
    return (", ".join(killers) if killers else None), ran


RELEASE_GATES = ROOT / "docs" / "development" / "RELEASE_GATES.md"


def unclassified_gates() -> list[str]:
    """Gates this harness can run that `RELEASE_GATES.md` does not classify.

    A gate nobody has called structural or behavioural is a gate whose green means whatever the
    reader assumes, which is the failure mode the classification exists to prevent.
    """
    try:
        text = RELEASE_GATES.read_text(encoding="utf-8")
    except OSError:
        return sorted(GATES)
    missing = []
    for name, command in sorted(GATES.items()):
        script = next((part for part in command if part.startswith("scripts/")), None)
        needle = f"`{Path(script).name} check`" if script else "`pytest`"
        if needle not in text:
            missing.append(name)
    return missing


def declared_invariants() -> set[str]:
    import re

    return set(re.findall(r"^### (SI-\d{3})", INVARIANTS.read_text(encoding="utf-8"), re.MULTILINE))


def render(results: list[tuple[Mutant, str | None, list[str]]], baseline: dict[str, bool]) -> str:
    # A kill by a gate that fails on a clean tree is not a kill. Coverage counts only kills by
    # gates the control pass cleared, which is the difference between measuring the gate and
    # measuring the export.
    sound_gates = {name for name, passed in baseline.items() if passed}

    def credible(killer: str | None) -> bool:
        return bool(killer) and all(name.strip() in sound_gates for name in str(killer).split(","))

    covered = {m.invariant for m, killer, _ in results if credible(killer) and m.invariant.startswith("SI-")}
    unkilled = [m for m, killer, _ in results if not credible(killer)]
    all_invariants = declared_invariants()
    lines = [
        "# Gate Sensitivity",
        "",
        "<!-- Generated by scripts/gate_sensitivity.py. Do not edit by hand. -->",
        "",
        "Each row is a planted violation of a named claim, applied to a throwaway copy of this",
        "repository, with the gate set run against it. **An invariant with no killing gate is",
        "asserted, not verified.** This is Mills' error seeding, generalised from the two comparators",
        "that already used it (`rust_python_parity.py self-test`, `diff_harness.py check`) to the",
        "gates that guard the product.",
        "",
        "| Mutant | Claim | Killed by | Gates run |",
        "| --- | --- | --- | --- |",
    ]
    for mutant, killer, ran in results:
        if not killer:
            verdict = "**NONE - the claim is asserted, not verified**"
        elif not credible(killer):
            verdict = f"`{killer}` - **but that gate fails on a clean tree, so this is not evidence**"
        else:
            verdict = f"`{killer}`"
        lines.append(f"| `{mutant.identifier}` ({mutant.invariant}) | {mutant.claim} | {verdict} | {', '.join(ran)} |")

    unsound = sorted(name for name, passed in baseline.items() if not passed)
    never_fired = sorted(set(baseline) - {n.strip() for _, k, _ in results if k for n in str(k).split(",")})
    unclassified = unclassified_gates()
    lines += [
        "",
        "## Control: every gate on an unmutated copy",
        "",
        "Without this the table above means nothing. A gate that fails on a clean export - because",
        "the copy dropped something it needs - appears to kill every mutant it is pointed at.",
        "",
        f"- Gates exercised: **{len(baseline)}**. Passing on a clean tree: **{sum(baseline.values())}**.",
    ]
    if unsound:
        lines += [
            f"- **{len(unsound)} gate(s) fail on a clean tree and their kills are not evidence:** "
            + ", ".join(f"`{name}`" for name in unsound)
            + ".",
        ]
    else:
        lines.append("- Every gate passes on a clean tree, so each kill above is attributable to its mutant.")
    lines += [
        f"- Gates exercised here that no mutant has ever made fail: **{len(never_fired)}**"
        + (f" - {', '.join(f'`{n}`' for n in never_fired)}. A gate never observed failing is of unknown strength." if never_fired else "."),
        f"- Gates this harness can run that `RELEASE_GATES.md` does not classify: **{len(unclassified)}**"
        + (f" - {', '.join(unclassified)}." if unclassified else "."),
        "",
        "## Coverage",
        "",
        f"- Safety Invariants declared: **{len(all_invariants)}**.",
        f"- Invariants with at least one mutant a gate kills: **{len(covered)}** ({', '.join(sorted(covered)) or 'none'}).",
        f"- Mutants no gate caught: **{len(unkilled)}**" + (f" - {', '.join(m.identifier for m in unkilled)}." if unkilled else "."),
        "",
        "The first number is the one to read honestly. Most invariants have **no mutant here at all**,",
        "which is not the same as having no killing gate - it means nobody has yet written the",
        "violation that would test them. Coverage grows by adding mutants, and a mutant that no gate",
        "kills is a finding rather than a failure of this harness.",
        "",
        "## What this cannot tell you",
        "",
        "- A mutant is a violation somebody thought of. Seeded defects are systematically easier than",
        "  real ones, because people plant what they already check for.",
        "- Only fast, hermetic gates run. `cargo test`, `cargo clippy` and the differential parity",
        "  gate are not exercised here, so a Rust invariant that only a compiled test can decide is",
        "  outside this harness by construction.",
        "- A gate that kills a mutant has been shown able to fail. It has not been shown to catch",
        "  the class the mutant stands for.",
    ]
    return "\n".join(lines).rstrip("\n") + "\n"


def collect() -> tuple[list[tuple[Mutant, str | None, list[str]]], dict[str, bool]]:
    baseline = control({gate for mutant in MUTANTS for gate in mutant.gates})
    return [(mutant, *evaluate(mutant)) for mutant in MUTANTS], baseline


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Plant a violation of each claim and record which gate catches it.")
    parser.add_argument("command", nargs="?", default="run", choices=["run", "generate", "check", "list"])
    return parser


def main(argv: list[str] | None = None) -> int:
    command = build_parser().parse_args(argv).command
    if command == "list":
        print(json.dumps([{"id": m.identifier, "invariant": m.invariant, "gates": list(m.gates)} for m in MUTANTS], indent=2))
        return 0
    try:
        results, baseline = collect()
    except (SensitivityError, OSError) as exc:
        print(f"GATE SENSITIVITY ERROR: {exc}", file=sys.stderr)
        return 2
    report = render(results, baseline)
    if command == "generate":
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text(report, encoding="utf-8")
        print(f"wrote {OUTPUT.relative_to(ROOT)}")
        return 0
    if command == "check":
        broken = sorted(name for name, passed in baseline.items() if not passed)
        if broken:
            # Reporting this in the document and still exiting zero was itself a finding: a run
            # whose control failed has measured the export, not the gates.
            print(
                f"GATE SENSITIVITY ERROR: {broken} fail on an unmutated tree, so their kills are not evidence",
                file=sys.stderr,
            )
            return 2
        current = OUTPUT.read_text(encoding="utf-8") if OUTPUT.is_file() else ""
        if current != report:
            print("GATE SENSITIVITY ERROR: the committed report is stale; run `generate`", file=sys.stderr)
            return 2
        credible = sum(1 for _, k, _ in results if k)
        print(f"gate sensitivity OK: {len(results)} mutants, {credible} killed, every gate clean on an unmutated tree")
        return 0
    print(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
