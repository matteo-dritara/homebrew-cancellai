"""Tests for the review harness (E34-S01).

What matters is that the harness decides what a reviewer's record cannot be trusted to say about
itself: which model reviewed, whether it was the executor's family, which paths it changed, and
that no existing record can be overwritten.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts import review_round as rr

LOG = (
    "timestamp=1 level=INFO message=stream providerID=openrouter modelID=nvidia/model-a:free agent=title\n"
    "timestamp=2 level=INFO message=stream providerID=openrouter modelID=nvidia/model-a:free agent=verifier\n"
)


class StreamAttributionTests(unittest.TestCase):
    def test_every_streamed_model_is_collected_once(self) -> None:
        self.assertEqual(rr.streamed_models(LOG), ["openrouter/nvidia/model-a:free"])

    def test_a_session_on_the_declared_model_passes(self) -> None:
        self.assertEqual(rr.stream_problems(rr.streamed_models(LOG), "openrouter/nvidia/model-a:free"), [])

    def test_a_second_model_in_the_session_is_reported(self) -> None:
        log = LOG + "message=stream providerID=openrouter modelID=google/other agent=title\n"
        problems = rr.stream_problems(rr.streamed_models(log), "openrouter/nvidia/model-a:free")
        self.assertTrue(any("google/other" in p for p in problems), problems)

    def test_an_anthropic_model_is_a_self_review(self) -> None:
        for log in (
            "message=stream providerID=anthropic modelID=claude-opus-5-5\n",
            "message=stream providerID=openrouter modelID=anthropic/claude-fable-5-1\n",
        ):
            problems = rr.stream_problems(rr.streamed_models(log), "openrouter/anything")
            self.assertTrue(any("self-review" in p for p in problems), (log, problems))

    def test_a_session_naming_no_model_cannot_be_attributed(self) -> None:
        self.assertTrue(rr.stream_problems([], "openrouter/x"))

    def test_an_executor_model_cannot_even_be_declared(self) -> None:
        self.assertTrue(rr.is_executor_model("openrouter", "anthropic/claude-sonnet-5"))
        self.assertTrue(rr.is_executor_model("anthropic", "anything"))
        self.assertFalse(rr.is_executor_model("openrouter", "nvidia/nemotron-3-ultra-550b-a55b:free"))


class RecordAndPathTests(unittest.TestCase):
    def test_record_names_by_tier(self) -> None:
        self.assertEqual(rr.record_name("E06", "formal", 9), "E06-VERIFIER-REVIEW-ROUND9.md")
        self.assertEqual(rr.record_name("E06", "pre", 1), "E06-PRE-REVIEW-1.md")

    def test_the_next_number_follows_every_existing_record(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            evidence = Path(tmp)
            for name in ("E06-VERIFIER-REVIEW.md", "E06-VERIFIER-REVIEW-ROUND8.md", "E06-S06-VERIFIER-REVIEW.md", "E06-PRE-REVIEW-2.md"):
                (evidence / name).write_text("x", encoding="utf-8")
            self.assertEqual(rr.next_number("E06", "formal", evidence), 9)
            self.assertEqual(rr.next_number("E06", "pre", evidence), 3)
            self.assertEqual(rr.next_number("E07", "pre", evidence), 1)

    def test_a_pre_review_may_change_only_its_own_record(self) -> None:
        allowed = rr.allowed_paths("pre", "E06-PRE-REVIEW-1.md")
        self.assertEqual(rr.path_problems(["project/evidence/E06-PRE-REVIEW-1.md"], allowed, set()), [])
        for path in ("project/epics/E06.json", "project/evidence/E06-S04/SAFETY_VERDICT.md", "scripts/release.py"):
            self.assertTrue(rr.path_problems([path], allowed, set()), path)

    def test_a_formal_round_may_not_touch_production_code_or_executor_evidence(self) -> None:
        allowed = rr.allowed_paths("formal", "E06-VERIFIER-REVIEW-ROUND9.md")
        ok = [
            "project/evidence/E06-VERIFIER-REVIEW-ROUND9.md",
            "project/evidence/E06-S04/SAFETY_VERDICT.md",
            "project/epics/E06.json",
            "project/generated/PROJECT_STATUS.md",
            "docs/BACKLOG.md",
            "tests/test_adversarial.py",
            "rust/crates/cancellai-cli/tests/adversarial.rs",
        ]
        self.assertEqual(rr.path_problems(ok, allowed, set()), [])
        for path in (
            "scripts/release.py",
            "rust/crates/cancellai-cli/src/main.rs",
            "project/evidence/E06-S04/EVIDENCE.md",
            "project/evidence/E06-S04/VERIFIER_BRIEF.md",
            "Formula/cancellai.rb",
        ):
            self.assertTrue(rr.path_problems([path], allowed, set()), path)

    def test_an_existing_review_record_can_never_be_changed(self) -> None:
        allowed = rr.allowed_paths("formal", "E06-VERIFIER-REVIEW.md")
        problems = rr.path_problems(["project/evidence/E06-VERIFIER-REVIEW.md"], allowed, {"project/evidence/E06-VERIFIER-REVIEW.md"})
        self.assertTrue(problems)

    def test_labels(self) -> None:
        self.assertEqual(rr.reviewer_label("codex", None), "Codex")
        self.assertEqual(rr.reviewer_label("opencode", "openrouter/nvidia/x:free"), "OpenCode/nvidia/x:free")
        with self.assertRaises(rr.ReviewError):
            rr.reviewer_label("opencode", None)


class AppendOnlyTests(unittest.TestCase):
    def _repo(self, tmp: str, text: str) -> tuple[Path, str]:
        repo = Path(tmp)
        subprocess.run(["git", "init", "-q"], cwd=repo, check=True)  # noqa: S607
        path = repo / "project" / "evidence" / "E06-S04"
        path.mkdir(parents=True)
        (path / "SAFETY_VERDICT.md").write_text(text, encoding="utf-8")
        subprocess.run(["git", "add", "-A"], cwd=repo, check=True)  # noqa: S607
        subprocess.run(
            ["git", "-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false", "commit", "-qm", "base"],  # noqa: S607
            cwd=repo,
            check=True,
        )
        return repo, rr.git("rev-parse", "HEAD", cwd=repo).strip()

    def test_appending_passes_and_rewriting_is_refused(self) -> None:
        relative = "project/evidence/E06-S04/SAFETY_VERDICT.md"
        with tempfile.TemporaryDirectory() as tmp:
            repo, base = self._repo(tmp, "Round 1\nFAIL\n")
            (repo / relative).write_text("Round 1\nFAIL\n\nRound 2\nPASS\n", encoding="utf-8")
            self.assertEqual(rr.append_only_problems(repo, [relative], base), [])
            (repo / relative).write_text("Round 1\nPASS\n", encoding="utf-8")
            self.assertTrue(rr.append_only_problems(repo, [relative], base))


GIT_ID = ["-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false"]


def _commit_all(repo: Path, message: str) -> str:
    subprocess.run(["git", "add", "-A"], cwd=repo, check=True)  # noqa: S607
    subprocess.run(["git", *GIT_ID, "commit", "-qm", message], cwd=repo, check=True)  # noqa: S603, S607
    return rr.git("rev-parse", "HEAD", cwd=repo).strip()


class RenameTests(unittest.TestCase):
    # Round 1 (R1, E34-S01): a staged rename out of production code reported only its
    # destination, so the deleted source never reached the tier check.
    def test_a_rename_reports_its_source_and_the_source_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)  # noqa: S607
            (repo / "scripts").mkdir()
            (repo / "scripts" / "protected.py").write_text("x = 1\n", encoding="utf-8")
            _commit_all(repo, "base")
            (repo / "tests").mkdir()
            subprocess.run(["git", "mv", "scripts/protected.py", "tests/test_new.py"], cwd=repo, check=True)  # noqa: S607
            changed = rr.changed_paths(repo)
            self.assertEqual(changed, ["scripts/protected.py", "tests/test_new.py"])
            problems = rr.path_problems(changed, rr.allowed_paths("formal", "E34-VERIFIER-REVIEW-ROUND2.md"), set())
            self.assertEqual(problems, ["scripts/protected.py is outside what this tier may change"])

    def test_a_path_with_spaces_or_quotes_is_reported_verbatim(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)  # noqa: S607
            (repo / "a").write_text("", encoding="utf-8")
            _commit_all(repo, "base")
            (repo / 'we"ird name.py').write_text("", encoding="utf-8")
            self.assertEqual(rr.changed_paths(repo), ['we"ird name.py'])


class ImportTests(unittest.TestCase):
    RECORD = "E34-VERIFIER-REVIEW-ROUND2.md"

    def _setup(self, tmp: str) -> tuple[Path, Path, dict]:
        root = Path(tmp) / "main"
        root.mkdir()
        subprocess.run(["git", "init", "-q"], cwd=root, check=True)  # noqa: S607
        (root / "project" / "epics").mkdir(parents=True)
        (root / "project" / "evidence").mkdir(parents=True)
        (root / "project" / "epics" / "E34.json").write_text('{"status": "ready_for_review"}\n', encoding="utf-8")
        (root / "project" / "evidence" / "E34-VERIFIER-REVIEW-ROUND1.md").write_text("Verifier: Codex\n", encoding="utf-8")
        base = _commit_all(root, "base")
        worktree = Path(tmp) / "review"
        rr.git("worktree", "add", "-q", "-b", "review", str(worktree), "HEAD", cwd=root)
        (worktree / "project" / "evidence" / self.RECORD).write_text("Verifier: Codex\n", encoding="utf-8")
        (worktree / "project" / "epics" / "E34.json").write_text('{"status": "done"}\n', encoding="utf-8")
        changed = rr.changed_paths(worktree)
        data = {
            "problems": [],
            "tier": "formal",
            "reviewer": "codex",
            "model": None,
            "label": "Codex",
            "record": self.RECORD,
            "base": base,
            "changed": changed,
            "digests": rr.output_digests(worktree, changed),
        }
        return root, worktree, data

    @staticmethod
    def rewrite_run_file(worktree: Path, data: dict) -> None:
        """What a reviewer able to write the run file could do: re-describe the tree as checked."""
        data["changed"] = rr.changed_paths(worktree)
        data["digests"] = rr.output_digests(worktree, data["changed"])

    def test_a_clean_run_imports(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root, worktree, data = self._setup(tmp)
            self.assertEqual(rr.import_problems(worktree, data, root), [])

    # Round 1 (R1, E34-S01): a record created in the import target after the run was checked
    # used to be overwritten by the worktree's copy.
    def test_a_record_that_appeared_since_the_base_is_never_overwritten(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root, worktree, data = self._setup(tmp)
            (root / "project" / "evidence" / self.RECORD).write_text("someone else's record\n", encoding="utf-8")
            problems = rr.import_problems(worktree, data, root)
            self.assertTrue(any("existing review record" in p for p in problems), problems)
            self.assertTrue(any("appeared in the working tree" in p for p in problems), problems)

    def test_a_target_changed_since_the_base_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root, worktree, data = self._setup(tmp)
            (root / "project" / "epics" / "E34.json").write_text('{"status": "in_progress"}\n', encoding="utf-8")
            problems = rr.import_problems(worktree, data, root)
            self.assertEqual(problems, ["project/epics/E34.json changed in the working tree after the run's base; refusing to overwrite it"])

    def test_a_tampered_run_file_is_rechecked(self) -> None:
        # The run file lives where the reviewer can write, so its verdict is not trusted.
        with tempfile.TemporaryDirectory() as tmp:
            root, worktree, data = self._setup(tmp)
            (worktree / "scripts").mkdir()
            (worktree / "scripts" / "x.py").write_text("", encoding="utf-8")
            self.rewrite_run_file(worktree, data)
            problems = rr.import_problems(worktree, data, root)
            self.assertIn("scripts/x.py is outside what this tier may change", problems)

    # Round 2 (E34-S01): a record edited after the run was checked, with its path list unchanged,
    # was imported as it stood.
    def test_a_record_edited_after_the_check_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root, worktree, data = self._setup(tmp)
            (worktree / "project" / "evidence" / self.RECORD).write_text("wrong header\nVerifier: forged\n", encoding="utf-8")
            self.assertEqual(rr.import_problems(worktree, data, root), ["the worktree changed after the run was checked; re-run the check"])

    def test_a_forged_record_fails_even_when_the_run_file_is_rewritten_to_match(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root, worktree, data = self._setup(tmp)
            (worktree / "project" / "evidence" / self.RECORD).write_text("Verifier: forged\n", encoding="utf-8")
            self.rewrite_run_file(worktree, data)
            self.assertIn(f"{self.RECORD} must name `Verifier: Codex`", rr.import_problems(worktree, data, root))

    def test_an_opencode_run_is_reattributed_from_its_log_at_import(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root, worktree, data = self._setup(tmp)
            model = "openrouter/nvidia/model-a:free"
            data.update(reviewer="opencode", model=model, label=rr.reviewer_label("opencode", model))
            (worktree / "project" / "evidence" / self.RECORD).write_text(f"Verifier: {data['label']}\n", encoding="utf-8")
            (worktree / rr.LOG_DIR).mkdir()
            (worktree / rr.STREAM_LOG).write_text(LOG, encoding="utf-8")
            self.rewrite_run_file(worktree, data)
            self.assertEqual(rr.import_problems(worktree, data, root), [])
            forged = LOG + "timestamp=3 level=INFO message=stream providerID=anthropic modelID=claude-x agent=verifier\n"
            (worktree / rr.STREAM_LOG).write_text(forged, encoding="utf-8")
            self.assertEqual(rr.import_problems(worktree, data, root), ["the worktree changed after the run was checked; re-run the check"])
            self.rewrite_run_file(worktree, data)
            self.assertTrue(rr.import_problems(worktree, data, root))

    def test_a_failed_run_imports_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root, worktree, data = self._setup(tmp)
            data["problems"] = ["the reviewer exited 1"]
            self.assertTrue(rr.import_problems(worktree, data, root))


class ReviewerReachTests(unittest.TestCase):
    # E34 self-review of round 2: the run record sat inside the reviewer-writable worktree, and a
    # background job the reviewer started could outlive it and change the tree after the check.
    def test_the_run_record_is_outside_the_worktree(self) -> None:
        worktree = Path("/reviews/e34-formal-3")
        self.assertEqual(rr.run_file(worktree), Path("/reviews/e34-formal-3.review-run.json"))
        self.assertNotIn(worktree, rr.run_file(worktree).parents)

    def test_nothing_the_reviewer_started_outlives_it(self) -> None:
        import os
        import time

        with tempfile.TemporaryDirectory() as tmp:
            pidfile = Path(tmp) / "pid"
            code = rr.run_in_own_group(["sh", "-c", f"sleep 30 & echo $! > {pidfile}"])
            self.assertEqual(code, 0)
            pid = int(pidfile.read_text(encoding="utf-8"))
            for _ in range(50):
                try:
                    os.kill(pid, 0)
                except ProcessLookupError:
                    break
                time.sleep(0.05)
            else:
                self.fail(f"the reviewer's background job {pid} survived it")


def _sandbox_applies() -> bool:
    """macOS with `sandbox-exec`, and not already inside a sandbox (Codex's own), which refuses a
    nested one."""
    if sys.platform != "darwin" or not shutil.which("sandbox-exec"):
        return False
    probe = subprocess.run(["sandbox-exec", "-p", "(version 1)(allow default)", "true"], capture_output=True, check=False)  # noqa: S607
    return probe.returncode == 0


@unittest.skipUnless(_sandbox_applies(), "needs macOS sandbox-exec outside another sandbox")
class ReviewerSandboxTests(unittest.TestCase):
    # E34-S06: the self-review of round 2 wrote into the main tree and another repository's `.git`
    # from an unsandboxed OpenCode reviewer. Under the harness's sandbox the same writes fail.
    def test_the_sandbox_confines_writes_to_the_worktree(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            main = root / "main"
            main.mkdir()
            subprocess.run(["git", "init", "-q"], cwd=main, check=True)  # noqa: S607
            (main / "README.md").write_text("main\n", encoding="utf-8")
            _commit_all(main, "base")
            worktree = root / "review"
            rr.git("worktree", "add", "-q", "-b", "review", str(worktree), "HEAD", cwd=main)
            script = (
                f"echo inside > {worktree}/inside.txt && echo INSIDE; "
                f"echo outside > {main}/README.md && echo MAIN; "
                f"echo outside > {root}/review{rr.RUN_SUFFIX} && echo RUNFILE; "
                f"git -C {worktree} status --porcelain >/dev/null && echo GIT"
            )
            result = subprocess.run(rr.sandboxed(["sh", "-c", script], worktree), capture_output=True, text=True, check=False)  # noqa: S603
            self.assertIn("INSIDE", result.stdout)
            self.assertIn("GIT", result.stdout)
            self.assertNotIn("MAIN", result.stdout)
            self.assertNotIn("RUNFILE", result.stdout)
            self.assertEqual((main / "README.md").read_text(encoding="utf-8"), "main\n")
            self.assertFalse((root / f"review{rr.RUN_SUFFIX}").exists())

    def test_opencode_configuration_is_not_writable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            worktree = Path(tmp).resolve()
            subprocess.run(["git", "init", "-q"], cwd=worktree, check=True)  # noqa: S607
            target = Path.home() / ".config" / "opencode" / "sandbox-probe.json"
            result = subprocess.run(rr.sandboxed(["sh", "-c", f"echo x > {target}"], worktree), capture_output=True, check=False)  # noqa: S603
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(target.exists())


class UnsandboxedPlatformTests(unittest.TestCase):
    def test_no_sandbox_means_no_opencode_review(self) -> None:
        with mock.patch.object(rr.sys, "platform", "linux"), self.assertRaisesRegex(rr.ReviewError, "sandbox"):
            rr.sandboxed(["opencode"], Path("."))


class ReviewerEnvironmentTests(unittest.TestCase):
    def test_a_reviewer_cannot_push_to_origin(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            subprocess.run(["git", "init", "-q", "--bare", str(root / "remote.git")], check=True)  # noqa: S603, S607
            repo = root / "repo"
            subprocess.run(["git", "init", "-q", str(repo)], check=True)  # noqa: S603, S607
            (repo / "f").write_text("x", encoding="utf-8")
            config = ["-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false"]
            subprocess.run(["git", *config, "add", "f"], cwd=repo, check=True)  # noqa: S603, S607
            subprocess.run(["git", *config, "commit", "-qm", "x"], cwd=repo, check=True)  # noqa: S603, S607
            subprocess.run(["git", "remote", "add", "origin", str(root / "remote.git")], cwd=repo, check=True)  # noqa: S603, S607
            env = rr.reviewer_env(root / "logs")
            pushed = subprocess.run(["git", "push", "-q", "origin", "HEAD:main"], cwd=repo, env=env, capture_output=True, check=False)  # noqa: S607
            self.assertNotEqual(pushed.returncode, 0)
            self.assertEqual(env["OPENCODE_DISABLE_CLAUDE_CODE"], "1")
            self.assertNotIn("SSH_AUTH_SOCK", env)
            self.assertTrue(env["GH_CONFIG_DIR"].startswith(str(root / "logs")))

    def test_a_failed_run_says_why(self) -> None:
        log = 'timestamp=1 level=ERROR run=x message="stream error" error.error.code=503 '
        log += 'error.error.message="Service temporarily overloaded" x=y\n'
        self.assertEqual(rr.last_error(log), "Service temporarily overloaded")
        self.assertEqual(rr.last_error("level=INFO fine\n"), "")


class BriefTests(unittest.TestCase):
    def test_a_story_without_a_committed_brief_is_refused(self) -> None:
        with self.assertRaises(rr.ReviewError):
            rr.committed_brief("E99-S99")

    def test_the_prompt_quotes_each_committed_brief_by_checksum(self) -> None:
        # Hermetic: gate_sensitivity runs this suite on a copy with no `.git`, so the committed
        # brief is supplied here rather than read from the real repository's HEAD.
        checksum = "ab" * 32
        brief = f"# Verifier brief - E06-S04\n\nBrief-Checksum: {checksum}\n"
        with mock.patch.object(rr, "git", return_value=brief):
            self.assertEqual(rr.committed_brief("E06-S04")[1], checksum)
            prompt = rr.render_prompt("E06", ["E06-S04"], "pre", "E06-PRE-REVIEW-1.md", "OpenCode/x", 1)
        self.assertIn(checksum, prompt)
        self.assertIn("ADVISORY PRE-REVIEW", prompt)
        self.assertIn("Pre-Review: advisory", prompt)


if __name__ == "__main__":
    unittest.main()
