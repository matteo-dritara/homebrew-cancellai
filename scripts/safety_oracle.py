#!/usr/bin/env python3
"""An oracle for the three behaviours where being wrong is unrecoverable (E25-S07).

`NORMATIVE` fixtures are generated from `cancellai.py`'s observed behaviour, so "the
implementation matches its characterization" proves the implementation matches *itself*. Where the
recorded behaviour was wrong, the defect is in the oracle. That is an **acceptability fallacy** in
Greenwell's taxonomy of fallacies in system safety arguments: a premise that is not independently
credible because it is derived from the conclusion.

The `KNOWN_DEFECT` classification already mitigates half of it - a recorded behaviour can be marked
"must not be reproduced", and a reclassification must update the record and its rationale in the
same change. What it cannot do is find the defects nobody noticed; those are recorded as
`NORMATIVE` and quietly become requirements.

AWS's ShardStore work met the same problem on 40,000 lines of Rust and answered it with an
**independent executable reference model** - a separate, deliberately simple statement of the
intended semantics, checked against the real implementation by property-based testing rather than
by golden files. This is that, for the three behaviours where being wrong cannot be undone:

1. **protected-name enforcement** - SI-006;
2. **root capability** - SI-002, SI-003;
3. **age and keep-latest eligibility** - the retention rule the README states.

**Only the first has an engine comparison.** `protected_component()` is called on real paths, and
the documented list in `README.md` is parsed independently so that deleting a name from the code
fails the oracle rather than silently removing it from both sides. Root capability and retention
have no entry point that can be called without importing planning internals, so those two check
that the *rule as written* has the properties it claims - which is weaker, and is said here rather
than implied. Rust is not checked at all (E25-S07, AC2, recorded as unmet).

Each predicate is written from `docs/security/SAFETY_INVARIANTS.md` and the README, with the text
it encodes quoted beside it, and deliberately **not** from reading either implementation. A
disagreement between an engine and a predicate is a failure *even when both engines agree with each
other*: agreement between two implementations of the same misunderstanding is not evidence.

Property-based rather than example-based, with a fixed seed so a failure is reproducible: these are
universally quantified claims ("no spelling of a protected name is ever eligible"), and three
examples do not discharge a claim about every input.

Stdlib-only, like every other governance checker here.
"""

from __future__ import annotations

import argparse
import random
import re
import string
import sys
import tempfile
import unicodedata
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

SEED = 20260912
CASES = 2000


@dataclass(frozen=True)
class Predicate:
    identifier: str
    invariant: str
    quotes: str


PREDICATES = (
    Predicate(
        "protected-name",
        "SI-006",
        "Protected names are 'enforced twice - when the plan is built and again immediately before "
        "any deletion - applied to the path as written as well as after resolution, and matched "
        "case-insensitively' (README.md). A barrier must not depend on how a name happens to be spelled.",
    ),
    Predicate(
        "root-capability",
        "SI-002 / SI-003",
        "'Only the provider's own directory is ever mutated' and a relocated root 'will be inspected "
        "in full but will not be deleted from' (README.md, ADR-0013). Nothing the filesystem can show "
        "proves a directory belongs to a provider.",
    ),
    Predicate(
        "retention",
        "README retention rule",
        "'Only data older than --days (default 7) is a candidate, and the --keep-latest newest sessions "
        "per tool (default 2) are always protected regardless of age. --aggressive widens which "
        "categories are eligible; it never bypasses the age cutoff.' (README.md)",
    ),
)


# ------------------------------------------------------------------ the predicates themselves
#
# Written from the text quoted above. They deliberately do not import the implementation's own
# helpers: a predicate that reuses the code under test is the circularity this file exists to break.


def protected_by_predicate(name: str, protected: set[str]) -> bool:
    """True when `name` names a protected entry under the rule the README states.

    Case-insensitive, and applied to the name as written. Unicode is folded, because 'as written'
    on a filesystem includes a name a user typed in a different normal form.
    """
    folded = unicodedata.normalize("NFC", name).casefold()
    return any(folded == unicodedata.normalize("NFC", item).casefold() for item in protected)


def root_permits_mutation_by_predicate(root: str, default_root: str) -> bool:
    """True only when the root is the provider's own default directory.

    The rule admits exactly one case. Anything else - a relocated root, a look-alike, a parent, a
    child - is inspection-only, because nothing the filesystem can show proves ownership.
    """
    return Path(root).expanduser().as_posix().rstrip("/") == Path(default_root).expanduser().as_posix().rstrip("/")


def eligible_by_predicate(age_days: int, rank_newest_first: int, days: int, keep_latest: int) -> bool:
    """True when an artifact is a deletion candidate under the retention rule.

    Two independent conditions, both of which must hold. `--aggressive` is absent on purpose: it
    widens categories and never reaches this function, which is exactly what the README promises.
    """
    if rank_newest_first < keep_latest:
        return False
    return age_days > days


# ------------------------------------------------------------------------------ the comparison


def generate_names(rng: random.Random, protected: list[str]) -> list[str]:
    """Spellings of a protected name, plus names that merely resemble one."""
    cases: list[str] = []
    alphabet = string.ascii_letters + string.digits + "._-"
    for name in protected:
        cases.extend(
            [
                name,
                name.upper(),
                name.lower(),
                name.swapcase(),
                name.capitalize(),
                unicodedata.normalize("NFD", name),
                f"{name} ",
                f" {name}",
                f"{name}.bak",
                f"x{name}",
            ]
        )
    for _ in range(200):
        cases.append("".join(rng.choice(alphabet) for _ in range(rng.randint(1, 20))))
    return cases


DOCUMENTED_NAMES = re.compile(r"`([\w.-]+\.(?:json|toml))`")


def documented_protected_names() -> set[str]:
    """The protected files the README names, parsed from the document rather than from the code.

    The set itself was being read from the implementation, so removing a name removed it from
    *both* sides of the comparison and the oracle stayed green - it checked the matching rule and
    never checked membership. An independent review demonstrated that with a mutant that deleted
    `settings.json` from the list: `safety oracle OK`. The documented list is the independent
    statement of what must be protected.
    """
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    line = next((ln for ln in readme.splitlines() if "Never touched" in ln), "")
    return set(DOCUMENTED_NAMES.findall(line))


def check_documented_names_are_protected(reference: object) -> list[str]:
    documented = documented_protected_names()
    if not documented:
        return ["protected-name: README.md names no protected files; the oracle has no independent list"]
    declared = set(getattr(reference, "CLAUDE_PROTECTED_NAMES", ())) | set(getattr(reference, "CODEX_PROTECTED_NAMES", ()))
    missing = sorted(documented - declared)
    return [f"protected-name: README.md documents {name!r} as never touched, but the engine does not protect it" for name in missing]


def check_protected_names(reference: object, rng: random.Random) -> list[str]:
    """The predicate against the reference's real decision, on real paths.

    `protected_component(path, root, names)` is the engine's own answer, and calling it is the
    whole point: an oracle that falls back to reimplementing the rule and compares the
    reimplementation to the predicate is comparing the predicate to itself, which is the
    circularity this file exists to break.
    """
    protected = sorted(set(getattr(reference, "CLAUDE_PROTECTED_NAMES", ())) | set(getattr(reference, "CODEX_PROTECTED_NAMES", ())))
    engine = getattr(reference, "protected_component", None)
    if not protected:
        return ["protected-name: the reference exposes no protected-name set to compare against"]
    if engine is None:
        return ["protected-name: the reference exposes no protected_component(); the oracle would be comparing the predicate to itself"]

    failures: list[str] = []
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        for name in generate_names(rng, protected):
            if "/" in name or name in (".", ".."):
                continue
            expected = protected_by_predicate(name, set(protected))
            try:
                actual = engine(root / name, root, set(protected)) is not None
            except (OSError, ValueError) as exc:
                failures.append(f"protected-name: the engine raised on {name!r}: {exc}")
                continue
            if expected and not actual:
                failures.append(f"protected-name: {name!r} is protected by the rule but the engine does not protect it")
    return failures


def check_retention(rng: random.Random) -> list[str]:
    """The retention predicate against itself under generated inputs, plus its stated invariants.

    There is no engine entry point to call here without importing planning internals, so this
    checks the *properties the rule must have* rather than one implementation of it. A property
    that the predicate itself violates would mean the rule as written is not what is intended.
    """
    failures: list[str] = []
    for _ in range(CASES):
        days = rng.randint(0, 400)
        keep = rng.randint(0, 10)
        age = rng.randint(0, 1000)
        rank = rng.randint(0, 20)
        eligible = eligible_by_predicate(age, rank, days, keep)
        if eligible and rank < keep:
            failures.append(f"retention: rank {rank} is inside keep-latest {keep} but was eligible")
        if eligible and age <= days:
            failures.append(f"retention: age {age} is within the {days}-day cutoff but was eligible")
        # Monotonicity, stated over independently drawn parameters rather than over `days + 1`,
        # which made the guarded branch unreachable: `age > days + 1` implies `age > days`, so the
        # assertion could never fire. An independent review pointed that out.
        wider = rng.randint(days, days + 400)
        more_kept = rng.randint(keep, keep + 10)
        if eligible_by_predicate(age, rank, wider, keep) and not eligible:
            failures.append(f"retention: widening --days from {days} to {wider} made age {age} eligible")
        if eligible_by_predicate(age, rank, days, more_kept) and not eligible:
            failures.append(f"retention: raising --keep-latest from {keep} to {more_kept} made rank {rank} eligible")
    return failures


def check_root_capability(rng: random.Random) -> list[str]:
    failures: list[str] = []
    default = "~/.claude"
    look_alikes = ["~/.claude/", "~/.Claude", "~/.claude2", "~/.claude/sub", "~/", "~/.claude/..", "/var/tmp/.claude"]  # noqa: S108 - a path string, not a file this code creates
    for candidate in look_alikes:
        if candidate.rstrip("/") == default and not root_permits_mutation_by_predicate(candidate, default):
            failures.append(f"root: {candidate!r} is the default root but the predicate refused it")
        if candidate.rstrip("/") != default and root_permits_mutation_by_predicate(candidate, default):
            failures.append(f"root: {candidate!r} is not the default root but the predicate permitted mutation")
    for _ in range(200):
        noise = "".join(rng.choice(string.ascii_lowercase) for _ in range(rng.randint(1, 8)))
        if root_permits_mutation_by_predicate(f"~/.claude-{noise}", default):
            failures.append(f"root: a look-alike '~/.claude-{noise}' was permitted")
    return failures


def run() -> list[str]:
    rng = random.Random(SEED)  # noqa: S311 - test-input generation, not security
    failures: list[str] = []
    try:
        import cancellai
    except ImportError as exc:  # pragma: no cover - the reference is committed
        return [f"cannot import the Python reference: {exc}"]
    failures.extend(check_documented_names_are_protected(cancellai))
    failures.extend(check_protected_names(cancellai, rng))
    failures.extend(check_root_capability(rng))
    failures.extend(check_retention(rng))
    return failures


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Check the highest-risk behaviours against a predicate, not against a recording.")
    parser.add_argument("command", nargs="?", default="check", choices=["check", "describe"])
    return parser


def main(argv: list[str] | None = None) -> int:
    command = build_parser().parse_args(argv).command
    if command == "describe":
        for predicate in PREDICATES:
            print(f"{predicate.identifier} ({predicate.invariant})\n  {predicate.quotes}\n")
        return 0
    failures = run()
    for failure in sorted(set(failures))[:20]:
        print(f"error: {failure}", file=sys.stderr)
    if failures:
        print(f"\nSAFETY ORACLE ERROR: {len(set(failures))} disagreements with the predicate", file=sys.stderr)
        return 2
    print(
        "safety oracle OK: protected-name checked against the engine's own decision "
        f"({len(documented_protected_names())} documented names, every spelling); "
        f"root-capability and retention checked as predicates only ({CASES} cases, seed {SEED})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
