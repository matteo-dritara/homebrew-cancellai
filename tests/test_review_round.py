"""Tests for the review harness (E34-S01).

What matters is that the harness decides what a reviewer's record cannot be trusted to say about
itself: which model reviewed, whether it was the executor's family, which paths it changed, and
that no existing record can be overwritten.
"""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

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
        _text, checksum = rr.committed_brief("E06-S04")
        prompt = rr.render_prompt("E06", ["E06-S04"], "pre", "E06-PRE-REVIEW-1.md", "OpenCode/x", 1)
        self.assertIn(checksum, prompt)
        self.assertIn("ADVISORY PRE-REVIEW", prompt)
        self.assertIn("Pre-Review: advisory", prompt)


if __name__ == "__main__":
    unittest.main()
