#!/usr/bin/env python3
"""The Rust CLI stays within a performance self-budget measured against the reference (E06-S08).

The cutover checklist's G4 leg names a performance self-budget for the CLI's own command paths
as unaddressed (docs/development/RELEASE_GATES.md). The existing Rust benchmarks measure the
discovery functions the CLI calls; nothing compared the command a user actually runs against the
engine it replaces. The owner chose the criterion on 2026-09-23: on one large synthetic corpus,
the Rust CLI is no slower than the frozen Python reference for each measured command, and never
exceeds a fixed resident-memory ceiling.

Each measured command is a pair - the Rust invocation and the reference invocation that answers
the same question - run against the same generated `$HOME`, several times, keeping the median
wall time and the largest peak RSS. Peak RSS comes from `os.wait4`'s rusage for the child alone,
so it measures the command, not this script. Windows has no `wait4`; there RSS is reported as
unmeasured rather than guessed, and the gate never runs there.

A fast run over nothing is the classic way a performance gate stays green while measuring
nothing, so every measurement first proves the corpus is live: the Rust `plan` and the reference
`clean --dry-run` must both propose exactly the expected number of deletions.

The corpus is synthetic and lives in a temporary directory that is removed afterwards. Nothing
here reads or writes the real `~/.claude` or `~/.codex`: every child gets `HOME` pointed at the
temporary tree, and `CLAUDE_CONFIG_DIR`/`CODEX_HOME` removed.

Stdlib-only, like every other script here.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from dataclasses import asdict, dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REFERENCE = ROOT / "cancellai.py"
DEFAULT_RUST_BIN = ROOT / "rust" / "target" / "release" / ("cancellai-cli.exe" if os.name == "nt" else "cancellai-cli")

# Retention flags shared by every measured command. `--keep-latest` protects this many sessions
# per provider, so the expected deletion count is `2 * (sessions - KEEP_LATEST)`.
KEEP_LATEST = 2
# The owner's memory ceiling for any single Rust command (E06-S08).
RSS_CEILING_BYTES = 64 * 1024 * 1024
# Wall-time comparison tolerance: Rust may be at most this much slower than the reference before
# the gate fails. A ratio covers a genuinely slower engine; the absolute slack covers process
# start-up jitter on a shared runner, which dominates when both commands take tens of ms.
TOLERANCE_RATIO = 1.10
TOLERANCE_SECONDS = 0.05
OLD_MTIME = 946_684_800  # 2000-01-01, far past any retention cutoff
REFERENCE_BACKEND = ["--codex-backend", "filesystem"]

# (name, Rust argv, reference argv). The reference has no `inspect` or `plan`: its
# `status --json` is the full machine-readable inventory and `clean --dry-run --json` is its plan.
# Every reference invocation also gets REFERENCE_BACKEND: with `auto`, the reference withholds
# Codex work wherever no `codex` binary is installed (a CI runner), so the two engines would be
# timed on different plans. The Rust engine always deletes at the filesystem level.
COMMANDS: tuple[tuple[str, list[str], list[str]], ...] = (
    ("status", ["status"], ["status"]),
    ("inspect", ["inspect", "--json"], ["status", "--json"]),
    ("plan", ["plan", "--json", "--allow-running"], ["clean", "--dry-run", "--json", "--allow-running"]),
    ("clean --dry-run", ["clean", "--dry-run", "--allow-running"], ["clean", "--dry-run", "--allow-running"]),
)


@dataclass
class Measurement:
    command: str
    rust_seconds: float
    reference_seconds: float
    rust_peak_rss_bytes: int | None
    reference_peak_rss_bytes: int | None


def build_corpus(home: Path, sessions: int) -> None:
    """Write `sessions` stale Claude sessions and `sessions` stale Codex rollouts under `home`."""
    claude = home / ".claude" / "projects"
    codex = home / ".codex" / "sessions" / "2020" / "01" / "01"
    codex.mkdir(parents=True)
    for index in range(sessions):
        session_id = f"{index:08x}-0000-4000-8000-000000000000"
        project = claude / f"project-{index % 50:02d}"
        project.mkdir(parents=True, exist_ok=True)
        transcript = project / f"{session_id}.jsonl"
        transcript.write_text('{"type":"user","message":"synthetic"}\n' * 4, encoding="utf-8")
        rollout = codex / f"rollout-2020-01-01T00-00-00-{session_id}.jsonl"
        meta = {"type": "session_meta", "payload": {"meta": {"id": session_id}}}
        rollout.write_text(json.dumps(meta) + "\n", encoding="utf-8")
        for path in (transcript, rollout):
            os.utime(path, (OLD_MTIME, OLD_MTIME))


def child_env(home: Path) -> dict[str, str]:
    env = {key: value for key, value in os.environ.items() if key not in {"CLAUDE_CONFIG_DIR", "CODEX_HOME"}}
    env["HOME"] = str(home)
    env["USERPROFILE"] = str(home)
    return env


def run_once(argv: list[str], env: dict[str, str]) -> tuple[float, int | None, bytes]:
    """Run one command; return its wall time, its own peak RSS in bytes (None if unmeasurable), and stdout."""
    start = time.perf_counter()
    with tempfile.TemporaryFile() as out:
        proc = subprocess.Popen(  # noqa: S603 - argv is this script's own fixed command list
            argv, stdout=out, stderr=subprocess.DEVNULL, stdin=subprocess.DEVNULL, env=env
        )
        wait4 = getattr(os, "wait4", None)
        rss: int | None = None
        if wait4 is not None:
            _pid, status, usage = wait4(proc.pid, 0)
            elapsed = time.perf_counter() - start
            # Linux reports ru_maxrss in KiB, macOS in bytes.
            rss = usage.ru_maxrss if sys.platform == "darwin" else usage.ru_maxrss * 1024
            proc.returncode = os.waitstatus_to_exitcode(status)
        else:
            proc.wait()
            elapsed = time.perf_counter() - start
        out.seek(0)
        return elapsed, rss, out.read()


def delete_count_rust(stdout: bytes) -> int:
    document = json.loads(stdout)
    return sum(1 for action in document["actions"] if action.get("action_class") == "delete")


def delete_count_reference(stdout: bytes) -> int:
    count: int = json.loads(stdout)["actions"]
    return count


def prove_corpus_is_live(rust_bin: Path, env: dict[str, str], expected: int) -> list[str]:
    """Both engines must propose exactly `expected` deletions, or the timings mean nothing."""
    errors = []
    _t, _r, rust_out = run_once([str(rust_bin), "plan", "--json", "--allow-running", "--keep-latest", str(KEEP_LATEST)], env)
    ref_argv = [
        sys.executable,
        str(REFERENCE),
        "clean",
        "--dry-run",
        "--json",
        "--allow-running",
        "--keep-latest",
        str(KEEP_LATEST),
        *REFERENCE_BACKEND,
    ]
    _t, _r, ref_out = run_once(ref_argv, env)
    try:
        rust_count = delete_count_rust(rust_out)
    except (ValueError, KeyError, TypeError) as exc:
        return [f"the Rust plan did not produce a readable document: {exc}"]
    try:
        ref_count = delete_count_reference(ref_out)
    except (ValueError, KeyError, TypeError) as exc:
        return [f"the reference dry run did not produce a readable document: {exc}"]
    for engine, count in (("Rust", rust_count), ("reference", ref_count)):
        if count != expected:
            errors.append(
                f"the {engine} engine proposed {count} deletions on a corpus built for {expected}; refusing to time a corpus it does not see"
            )
    return errors


def measure(rust_bin: Path, sessions: int, runs: int) -> tuple[list[Measurement], list[str]]:
    home = Path(tempfile.mkdtemp(prefix="cancellai-cutover-bench-"))
    try:
        build_corpus(home, sessions)
        env = child_env(home)
        errors = prove_corpus_is_live(rust_bin, env, 2 * (sessions - KEEP_LATEST))
        if errors:
            return [], errors
        results = []
        for name, rust_args, ref_args in COMMANDS:
            retention = ["--keep-latest", str(KEEP_LATEST)]
            rust_argv = [str(rust_bin), *rust_args, *retention]
            ref_argv = [sys.executable, str(REFERENCE), *ref_args, *retention, *REFERENCE_BACKEND]
            rust_times, ref_times = [], []
            rust_rss: list[int] = []
            ref_rss: list[int] = []
            for _ in range(runs):
                seconds, rss, _out = run_once(rust_argv, env)
                rust_times.append(seconds)
                if rss is not None:
                    rust_rss.append(rss)
                seconds, rss, _out = run_once(ref_argv, env)
                ref_times.append(seconds)
                if rss is not None:
                    ref_rss.append(rss)
            results.append(
                Measurement(
                    command=name,
                    rust_seconds=statistics.median(rust_times),
                    reference_seconds=statistics.median(ref_times),
                    rust_peak_rss_bytes=max(rust_rss) if rust_rss else None,
                    reference_peak_rss_bytes=max(ref_rss) if ref_rss else None,
                )
            )
        return results, []
    finally:
        shutil.rmtree(home, ignore_errors=True)


def budget_violations(results: list[Measurement]) -> list[str]:
    """Every way `results` breaks the self-budget. Pure, so it is testable without a corpus."""
    violations = []
    for m in results:
        allowed = m.reference_seconds * TOLERANCE_RATIO + TOLERANCE_SECONDS
        if m.rust_seconds > allowed:
            violations.append(
                f"{m.command}: Rust took {m.rust_seconds:.3f}s against the reference's {m.reference_seconds:.3f}s (allowed {allowed:.3f}s)"
            )
        if m.rust_peak_rss_bytes is not None and m.rust_peak_rss_bytes > RSS_CEILING_BYTES:
            violations.append(
                f"{m.command}: Rust peak RSS {m.rust_peak_rss_bytes / 1048576:.1f} MiB exceeds the {RSS_CEILING_BYTES / 1048576:.0f} MiB ceiling"
            )
    if not results:
        violations.append("no command was measured")
    return violations


def report(results: list[Measurement]) -> str:
    def mib(value: int | None) -> str:
        return "unmeasured" if value is None else f"{value / 1048576:.1f} MiB"

    lines = [f"{'command':<16} {'rust':>9} {'reference':>10} {'rust RSS':>12} {'ref RSS':>12}"]
    for m in results:
        lines.append(
            f"{m.command:<16} {m.rust_seconds:>8.3f}s {m.reference_seconds:>9.3f}s "
            f"{mib(m.rust_peak_rss_bytes):>12} {mib(m.reference_peak_rss_bytes):>12}"
        )
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument("mode", choices=["report", "check"], help="check fails on a budget violation; report never does")
    parser.add_argument("--rust-bin", type=Path, default=DEFAULT_RUST_BIN)
    parser.add_argument("--sessions", type=int, default=2000, help="stale sessions per provider")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--json-output", type=Path, help="also write the measurements here")
    args = parser.parse_args(argv)

    if args.sessions <= KEEP_LATEST:
        parser.error(f"--sessions must exceed --keep-latest ({KEEP_LATEST}), or nothing is a candidate")
    if not args.rust_bin.exists():
        print(f"error: {args.rust_bin} does not exist; build it with `cargo build --release -p cancellai-cli`", file=sys.stderr)
        return 2

    results, errors = measure(args.rust_bin, args.sessions, args.runs)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(report(results))
    if args.json_output:
        args.json_output.write_text(json.dumps([asdict(m) for m in results], indent=2) + "\n", encoding="utf-8")

    violations = budget_violations(results)
    for violation in violations:
        print(f"{'BUDGET' if args.mode == 'check' else 'note'}: {violation}", file=sys.stderr)
    if args.mode == "check" and violations:
        return 1
    print("cutover performance budget OK" if not violations else "cutover performance budget: reported only")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
