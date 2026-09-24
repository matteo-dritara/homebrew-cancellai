#!/usr/bin/env python3
"""Run an independent review round - or an advisory pre-review - through a harness (E34).

Rounds used to be run by hand: the executor paraphrased the brief into a prompt, created a
worktree ad hoc, and imported the reviewer's files by copying them - which is how one reviewer
overwrote a 2026-09-01 record. This script owns every step the executor must not improvise:

- the prompt is rendered from `project/templates/VERIFIER_PROMPT.md` plus the committed verifier
  brief of every story under review, quoted by checksum; a story without a committed brief is
  refused, because a verdict must answer the brief the gate rendered (E28-S02);
- the record's filename is chosen here and a run is refused if it already exists;
- the reviewer runs in its own git worktree - Codex under its workspace sandbox, OpenCode as the
  `verifier` or `pre-reviewer` agent with tool-enforced permissions (`.opencode/agents/`);
- afterwards the harness checks what an honest record cannot be trusted to say about itself:
  every model an OpenCode session streamed from must be the declared one and none may be an
  Anthropic model (the executor is Claude; a review by Claude is a self-review), and every changed
  path must be one the tier allows. OpenCode's permissions are a first barrier, not a sandbox -
  an allowed shell command can still write a file - so the diff check is the enforcement;
- `import` copies exactly the allowed changed paths of a passing run into the working tree.

Tiers: `formal` writes `<EPIC>-VERIFIER-REVIEW-ROUND<n>.md` and may write Safety Verdict sections
and story statuses; it counts as a round. `pre` writes `<EPIC>-PRE-REVIEW-<n>.md` and nothing else;
it never counts as a round or a verdict (PD-028).

Stdlib-only, like every other script here.
"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / "project" / "evidence"
PROMPT_TEMPLATE = ROOT / "project" / "templates" / "VERIFIER_PROMPT.md"
RUN_FILE = ".review-run.json"
LOG_DIR = ".opencode-run"
STREAM_LOG = f"{LOG_DIR}/opencode.log"

# The executor's model family. A reviewer from it is a self-reviewer (AGENT_PROTOCOL.md).
EXECUTOR_PROVIDERS = frozenset({"anthropic"})
EXECUTOR_MODEL_MARKERS = ("claude", "anthropic/")
STREAM_RE = re.compile(r"message=stream providerID=(\S+) modelID=(\S+)")
CHECKSUM_RE = re.compile(r"^Brief-Checksum:\s*([0-9a-f]{64})\s*$", re.MULTILINE)
VERIFIER_LINE_RE = re.compile(r"^Verifier:\s*(.+?)\s*$", re.MULTILINE)
STORY_RE = re.compile(r"^E\d{2}-S\d{2}$")
EPIC_RE = re.compile(r"^E\d{2}$")

# What a formal round may change besides its own record: Safety Verdicts (append-only, checked
# below), story statuses and the files `project_os.py generate`/`process_metrics.py generate`
# rewrite, and adversarial tests. Never an executor evidence packet, never production code.
FORMAL_ALLOWED = (
    "project/evidence/*/SAFETY_VERDICT.md",
    "project/epics/*.json",
    "project/generated/*",
    "docs/BACKLOG.md",
    "docs/ROADMAP.md",
    "docs/DECISION_REGISTER.md",
    "tests/test_*.py",
    "rust/crates/*/tests/*.rs",
)


class ReviewError(RuntimeError):
    pass


@dataclass
class RunRecord:
    epic: str
    stories: list[str]
    tier: str
    reviewer: str
    model: str | None
    label: str
    record: str
    worktree: str
    base: str
    streamed: list[str] = field(default_factory=list)
    changed: list[str] = field(default_factory=list)
    # sha256 of every changed path and of the stream log as checked; import refuses any other bytes.
    digests: dict[str, str] = field(default_factory=dict)
    problems: list[str] = field(default_factory=list)
    exit_code: int | None = None
    seconds: float = 0.0

    @property
    def passed(self) -> bool:
        return not self.problems


def git(*args: str, cwd: Path = ROOT) -> str:
    result = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=False)  # noqa: S603, S607
    if result.returncode != 0:
        raise ReviewError(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def committed_brief(story: str) -> tuple[str, str]:
    """The brief's committed text and checksum. A brief only in the working tree is refused."""
    relative = f"project/evidence/{story}/VERIFIER_BRIEF.md"
    try:
        text = git("show", f"HEAD:{relative}")
    except ReviewError as exc:
        raise ReviewError(f"{story} has no committed verifier brief ({relative}); render and commit it first") from exc
    match = CHECKSUM_RE.search(text)
    if not match:
        raise ReviewError(f"{relative} carries no Brief-Checksum line")
    return text, match.group(1)


def record_name(epic: str, tier: str, number: int) -> str:
    return f"{epic}-VERIFIER-REVIEW-ROUND{number}.md" if tier == "formal" else f"{epic}-PRE-REVIEW-{number}.md"


def next_number(epic: str, tier: str, evidence: Path = EVIDENCE) -> int:
    if tier == "formal":
        pattern = re.compile(rf"^{epic}(?:-S\d{{2}})?-VERIFIER-REVIEW(?:-ROUND(\d+))?\.md$")
    else:
        pattern = re.compile(rf"^{epic}-PRE-REVIEW-(\d+)\.md$")
    numbers = [int(m.group(1) or 1) for path in evidence.glob(f"{epic}*.md") if (m := pattern.match(path.name))]
    return max(numbers, default=0) + 1


def reviewer_label(reviewer: str, model: str | None) -> str:
    if reviewer == "codex":
        return "Codex"
    if not model:
        raise ReviewError("an OpenCode review needs a pinned --model")
    return f"OpenCode/{model.split('/', 1)[1] if model.startswith('openrouter/') else model}"


def is_executor_model(provider: str, model: str) -> bool:
    lowered = f"{provider}/{model}".lower()
    return provider.lower() in EXECUTOR_PROVIDERS or any(marker in lowered for marker in EXECUTOR_MODEL_MARKERS)


def streamed_models(log_text: str) -> list[str]:
    """Every `provider/model` an OpenCode session streamed from, in order of first use."""
    seen: list[str] = []
    for provider, model in STREAM_RE.findall(log_text):
        name = f"{provider}/{model}"
        if name not in seen:
            seen.append(name)
    return seen


def stream_problems(streamed: list[str], declared: str) -> list[str]:
    problems = []
    if not streamed:
        problems.append("the session log names no model at all; the reviewer cannot be attributed")
    for name in streamed:
        provider, _, model = name.partition("/")
        if is_executor_model(provider, model):
            problems.append(f"{name} is the executor's model family: this would be a self-review, not an independent one")
        elif name != declared:
            problems.append(f"the session streamed from {name}, not the declared {declared}")
    return problems


def allowed_paths(tier: str, record: str) -> tuple[str, ...]:
    own = (f"project/evidence/{record}",)
    return own if tier == "pre" else own + FORMAL_ALLOWED


def append_only_problems(worktree: Path, changed: list[str], base: str) -> list[str]:
    """A Safety Verdict that existed at `base` may only grow: the new text must start with the old."""
    problems = []
    for relative in changed:
        if not relative.endswith("/SAFETY_VERDICT.md"):
            continue
        try:
            before = git("show", f"{base}:{relative}", cwd=worktree)
        except ReviewError:
            continue  # a new Safety Verdict
        after = (worktree / relative).read_text(encoding="utf-8", errors="replace")
        if not after.startswith(before):
            problems.append(f"{relative} was rewritten, not appended to")
    return problems


def path_problems(changed: list[str], allowed: tuple[str, ...], existing_records: set[str]) -> list[str]:
    problems = []
    for path in changed:
        if not any(fnmatch.fnmatchcase(path, pattern) for pattern in allowed):
            problems.append(f"{path} is outside what this tier may change")
        elif path in existing_records:
            problems.append(f"{path} is an existing review record; records are append-only by file")
    return problems


def changed_paths(worktree: Path) -> list[str]:
    """Every path the reviewer changed, added or deleted, relative to the worktree root.

    Rename detection is off, so a rename is its deleted source plus its added destination and the
    source is judged like any other deletion; `-z` gives paths unquoted, whatever they contain."""
    out = git("status", "--porcelain=v1", "-z", "--no-renames", "--untracked-files=all", cwd=worktree)
    paths = [entry[3:] for entry in out.split("\0") if entry]
    return sorted(p for p in paths if p != RUN_FILE and not p.startswith(f"{LOG_DIR}/"))


def render_prompt(epic: str, stories: list[str], tier: str, record: str, label: str, number: int) -> str:
    briefs = []
    for story in stories:
        text, checksum = committed_brief(story)
        briefs.append(f"### {story} - Brief-Checksum {checksum}\n\n{text}")
    template = PROMPT_TEMPLATE.read_text(encoding="utf-8")
    template = template.replace("<EPIC-ID>", epic).replace("<ROUND>", str(number))
    if tier == "pre":
        rules = f"""
## This run is an ADVISORY PRE-REVIEW (E34, PD-028)

Ignore any instruction above about verdicts, Safety Verdicts or story statuses: this record is
advisory, is never a verdict, and never counts as a round. Write exactly one file,
`project/evidence/{record}`, starting with the line `Pre-Review: advisory` and the line
`Verifier: {label}`, and listing each finding with a reproduction and the repair you would require.
Change nothing else - the harness refuses any other changed path.
"""
    else:
        rules = f"""
## Harness rules for this formal round (E34)

Write the round record as the NEW file `project/evidence/{record}`, starting with the lines
`Review-Scope: epic`, `Round: {number}` and `Verifier: {label}`, with one verdict row per story in the
form `| <STORY-ID> | PASS/PASS_WITH_RESIDUALS/FAIL | evidence |` and every story's Brief-Checksum.
For each CR4 story append (never rewrite) a round section to
`project/evidence/<STORY-ID>/SAFETY_VERDICT.md` with `Verifier: {label}` and `Brief-Checksum:`
lines and a final standalone verdict line. You may set the judged stories' statuses and regenerate
project files (`python3 scripts/project_os.py generate`, `python3 scripts/process_metrics.py generate`).
You may add adversarial tests. Never change production code, never commit, and never overwrite an
existing review record - the harness checks every changed path after you exit.
"""
    return template + rules + "\n## Committed verifier briefs\n\n" + "\n\n".join(briefs) + "\n"


def reviewer_env(log_dir: Path) -> dict[str, str]:
    """The environment a reviewer runs in. It cannot publish: every git remote operation that
    would authenticate fails (no credential helper, no ssh, origin's push URL invalid) and `gh`
    sees an empty config, so it holds no token. OpenCode loads no `~/.claude` prompt or skills
    (the repository pack comes back through `opencode.json`'s `skills.paths`), and downloads no
    update or language server. This is defence in depth: a reviewer allowed `python3` and `cargo`
    can run arbitrary code, so the worktree, the path check and `import` remain the boundary."""
    env = {k: v for k, v in os.environ.items() if k not in {"SSH_AUTH_SOCK", "GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN"}}
    gh_dir = log_dir / "gh-empty"
    gh_dir.mkdir(parents=True, exist_ok=True)
    blocked = {"credential.helper": "", "core.sshCommand": "false", "remote.origin.pushurl": "invalid://review-cannot-push"}
    env.update(
        {
            "GIT_TERMINAL_PROMPT": "0",
            "GIT_CONFIG_COUNT": str(len(blocked)),
            "GH_CONFIG_DIR": str(gh_dir),
            "OPENCODE_DISABLE_CLAUDE_CODE": "1",
            "OPENCODE_DISABLE_AUTOUPDATE": "1",
            "OPENCODE_DISABLE_LSP_DOWNLOAD": "1",
            # Every cargo a reviewer may run - directly or inside an allowed check script - builds
            # from what is already fetched; none of them reaches a registry (E34 round 2).
            "CARGO_NET_OFFLINE": "true",
        }
    )
    for index, (key, value) in enumerate(blocked.items()):
        env[f"GIT_CONFIG_KEY_{index}"] = key
        env[f"GIT_CONFIG_VALUE_{index}"] = value
    return env


def run_reviewer(reviewer: str, model: str | None, tier: str, worktree: Path, prompt: str, log_dir: Path) -> tuple[int, str]:
    log_dir.mkdir(parents=True, exist_ok=True)
    prompt_file = log_dir / "prompt.md"
    prompt_file.write_text(prompt, encoding="utf-8")
    env = reviewer_env(log_dir)
    if reviewer == "codex":
        command = [
            "codex", "exec", "-s", "workspace-write",
            "-c", "sandbox_workspace_write.network_access=true",
            "-C", str(worktree), "-o", str(log_dir / "final.md"), "-",
        ]  # fmt: skip
        with prompt_file.open("rb") as stdin, (log_dir / "events.log").open("wb") as out:
            result = subprocess.run(command, stdin=stdin, stdout=out, stderr=subprocess.STDOUT, env=env, check=False)  # noqa: S603
        return result.returncode, ""
    agent = "verifier" if tier == "formal" else "pre-reviewer"
    command = [
        "opencode", "run", "--agent", agent, "--model", str(model), "--format", "json",
        "--print-logs", "--log-level", "INFO", "--title", f"review {tier}",
        "Follow the attached instructions exactly.", "--file", str(prompt_file),
    ]  # fmt: skip
    with (log_dir / "events.jsonl").open("wb") as out, (log_dir / "opencode.log").open("wb") as err:
        result = subprocess.run(command, cwd=worktree, stdout=out, stderr=err, env=env, check=False)  # noqa: S603
    return result.returncode, (log_dir / "opencode.log").read_text(encoding="utf-8", errors="replace")


ERROR_RE = re.compile(r'level=ERROR .*?(?:error\.error\.message|error)="([^"]*)"')


def last_error(log_text: str) -> str:
    """The last error OpenCode logged, so a failed run says why (e.g. an overloaded provider)."""
    found = ERROR_RE.findall(log_text)
    return found[-1] if found else ""


def check_record(worktree: Path, record: str, label: str, tier: str) -> list[str]:
    path = worktree / "project" / "evidence" / record
    if not path.exists():
        return [f"the reviewer wrote no {record}"]
    text = path.read_text(encoding="utf-8", errors="replace")
    problems = []
    named = VERIFIER_LINE_RE.search(text)
    if not named or named.group(1) != label:
        problems.append(f"{record} must name `Verifier: {label}`")
    if tier == "pre" and not text.startswith("Pre-Review: advisory"):
        problems.append(f"{record} must start with `Pre-Review: advisory`")
    return problems


def cmd_run(args: argparse.Namespace) -> int:
    epic, tier, reviewer = args.epic, args.tier, args.reviewer
    stories = [s.strip() for s in args.stories.split(",") if s.strip()]
    if not EPIC_RE.match(epic) or not stories or not all(STORY_RE.match(s) for s in stories):
        raise ReviewError("--epic must be E## and --stories a comma list of E##-S##")
    model = args.model
    if reviewer == "opencode" and model and is_executor_model(*model.split("/", 1)):
        raise ReviewError(f"{model} is the executor's model family and cannot review independently")
    label = reviewer_label(reviewer, model)
    number = args.number or next_number(epic, tier)
    record = record_name(epic, tier, number)
    if (EVIDENCE / record).exists():
        raise ReviewError(f"project/evidence/{record} already exists; records are never overwritten")
    prompt = render_prompt(epic, stories, tier, record, label, number)

    base = git("rev-parse", "HEAD").strip()
    worktree = Path(args.workdir).resolve() / f"{epic.lower()}-{tier}-{number}"
    if worktree.exists():
        raise ReviewError(f"{worktree} already exists")
    branch = f"review/{epic.lower()}-{tier}-{number}-{int(time.time())}"
    git("worktree", "add", "-q", "-b", branch, str(worktree), "HEAD")
    run = RunRecord(epic, stories, tier, reviewer, model, label, record, str(worktree), base)
    started = time.monotonic()
    run.exit_code, log_text = run_reviewer(reviewer, model, tier, worktree, prompt, worktree / LOG_DIR)
    run.seconds = round(time.monotonic() - started, 1)

    if run.exit_code != 0:
        run.problems.append(f"the reviewer exited {run.exit_code}" + (f": {last_error(log_text)}" if last_error(log_text) else ""))
    if reviewer == "opencode":
        run.streamed = streamed_models(log_text)
        run.problems += stream_problems(run.streamed, str(model))
    run.changed = changed_paths(worktree)
    existing = {f"project/evidence/{p.name}" for p in EVIDENCE.glob("*REVIEW*.md")}
    run.problems += path_problems(run.changed, allowed_paths(tier, record), existing)
    run.problems += append_only_problems(worktree, run.changed, base)
    run.problems += check_record(worktree, record, label, tier)
    run.digests = output_digests(worktree, run.changed)
    (worktree / RUN_FILE).write_text(json.dumps(asdict(run), indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"passed": run.passed, **asdict(run)}, indent=2))
    return 0 if run.passed else 1


def base_bytes(relative: str, base: str, root: Path) -> bytes | None:
    """The path's content at `base`, or None when it did not exist there."""
    result = subprocess.run(["git", "cat-file", "blob", f"{base}:{relative}"], cwd=root, capture_output=True, check=False)  # noqa: S603, S607
    return result.stdout if result.returncode == 0 else None


def output_digests(worktree: Path, changed: list[str]) -> dict[str, str]:
    """What the run checked, byte for byte: each changed file and the log attribution was read from."""
    digests = {}
    for relative in [*changed, STREAM_LOG]:
        path = worktree / relative
        if path.is_file():
            digests[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
    return digests


def import_problems(worktree: Path, data: dict[str, Any], root: Path = ROOT) -> list[str]:
    """Everything that must hold, at the moment of import, before a single byte is copied.

    The run's own checks are repeated rather than trusted - the run file lives in a worktree the
    reviewer could write to - and every target must still be what it was at the run's base: a
    record or change that appeared in the working tree since then is refused, never overwritten."""
    if data["problems"]:
        return ["the run did not pass its checks:", *data["problems"]]
    current = changed_paths(worktree)
    if current != data["changed"] or output_digests(worktree, current) != data.get("digests"):
        return ["the worktree changed after the run was checked; re-run the check"]
    # Re-run on the bytes about to be copied, not trusted from the run file: the record's header
    # and author, and for OpenCode the model attribution the stream log supports.
    problems = check_record(worktree, data["record"], data["label"], data["tier"])
    if data["reviewer"] == "opencode":
        log = worktree / STREAM_LOG
        text = log.read_text(encoding="utf-8", errors="replace") if log.is_file() else ""
        problems += stream_problems(streamed_models(text), str(data["model"]))
    evidence = root / "project" / "evidence"
    existing = {f"project/evidence/{p.name}" for p in evidence.glob("*REVIEW*.md")}
    problems += path_problems(current, allowed_paths(data["tier"], data["record"]), existing)
    problems += append_only_problems(worktree, current, data["base"])
    for relative in current:
        if not (worktree / relative).exists():
            problems.append(f"{relative} was deleted by the reviewer; deletions are never imported")
            continue
        target = root / relative
        before = base_bytes(relative, data["base"], root)
        if before is None and target.exists():
            problems.append(f"{relative} appeared in the working tree after the run's base; refusing to overwrite it")
        elif before is not None and (not target.is_file() or target.read_bytes() != before):
            problems.append(f"{relative} changed in the working tree after the run's base; refusing to overwrite it")
    return problems


def cmd_import(args: argparse.Namespace) -> int:
    """Copies a passing run's allowed changes into the main working tree - nothing else, and
    nothing at all unless every path passes `import_problems`."""
    worktree = Path(args.worktree).resolve()
    data = json.loads((worktree / RUN_FILE).read_text(encoding="utf-8"))
    problems = import_problems(worktree, data)
    if problems:
        raise ReviewError("nothing is imported:\n" + "\n".join(problems))
    for relative in data["changed"]:
        target = ROOT / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(worktree / relative, target)
        print(f"imported {relative}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run", help="run a review round or pre-review in its own worktree")
    run.add_argument("--epic", required=True)
    run.add_argument("--stories", required=True, help="comma-separated story ids under review")
    run.add_argument("--reviewer", choices=["codex", "opencode"], required=True)
    run.add_argument("--tier", choices=["formal", "pre"], required=True)
    run.add_argument("--model", help="OpenCode only: the pinned provider/model, e.g. openrouter/nvidia/...")
    run.add_argument("--number", type=int, help="round or pre-review number (default: the next free one)")
    run.add_argument("--workdir", required=True, help="where to create the review worktree")
    imp = sub.add_parser("import", help="copy a passing run's allowed changes into the working tree")
    imp.add_argument("worktree")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return cmd_run(args) if args.command == "run" else cmd_import(args)
    except ReviewError as exc:
        print(f"REVIEW ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
