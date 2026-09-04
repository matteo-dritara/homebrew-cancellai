from __future__ import annotations

import re
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts import check_workflows


class WorkflowPolicyTests(unittest.TestCase):
    def test_active_workflows_follow_supply_chain_policy(self) -> None:
        check_workflows.validate_workflows()

    def test_declared_contexts_expand_the_matrix_and_exclude_triggers(self) -> None:
        declared = check_workflows.declared_check_names()
        # A matrix job never reports its bare name; requiring it blocks every pull request
        # forever while reporting nothing. This is the bug that was live in the repository.
        self.assertNotIn("test", declared)
        self.assertLessEqual({"test (3.10)", "test (3.14)"}, declared)
        # `on:` keys are not jobs.
        self.assertFalse({"push", "pull_request", "schedule"} & declared)

    def test_every_required_check_matches_a_real_job(self) -> None:
        required = check_workflows.required_check_names()
        self.assertTrue(required, "REPOSITORY_GOVERNANCE.md must list the required checks")
        declared = check_workflows.declared_check_names()
        for name in required:
            self.assertIn(name, declared, name)


# E22-S01 (CR-TE-06): release.yml re-runs every gate at the tagged commit, and
# scripts/check_workflows.py must fail rather than pass silently if it stops doing so.
class ReleaseGateDriftTests(unittest.TestCase):
    def test_precommit_gate_commands_covers_the_full_checker_set(self) -> None:
        commands = check_workflows.precommit_gate_commands()
        # Every check AGENTS.md's "Current Python checks" list names as a repository-owned
        # gate is a local pre-commit hook with a matching `entry:`.
        self.assertEqual(commands["release-consistency"], "python3 scripts/release.py check")
        self.assertEqual(commands["rust-python-parity-gate"], "python3 scripts/rust_python_parity.py check")
        self.assertEqual(commands["mutation-boundary-check"], "python3 scripts/check_mutation_boundary.py check")
        self.assertEqual(
            commands["provider-compatibility-check"],
            "python3 scripts/check_provider_compatibility.py check",
        )
        # Staged only at commit-msg: lints the commit message text, not repository state.
        self.assertNotIn("conventional-commit", commands)

    def test_release_workflow_currently_carries_every_gate(self) -> None:
        self.assertEqual(check_workflows.release_gate_drift_errors(), [])

    def test_a_removed_precommit_gate_in_release_yml_is_caught(self) -> None:
        # Reproduces CR-TE-06 mechanically: release.yml missing a gate main enforces must
        # fail this check, not report success the way v1.8.0 did.
        release_yml = (
            "jobs:\n"
            "  verify:\n"
            "    steps:\n"
            "      - run: python3 -m pytest tests -v\n"
            "      - run: python3 scripts/project_os.py check\n"
            "  verify-rust:\n"
            "    steps:\n"
            "      - run: cargo fmt --check\n"
            "      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings\n"
            "      - run: cargo test --workspace\n"
            "      - run: cargo deny check\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "release.yml"
            path.write_text(release_yml, encoding="utf-8")
            with mock.patch.object(check_workflows, "RELEASE_WORKFLOW", path):
                errors = check_workflows.release_gate_drift_errors()
        joined = "\n".join(errors)
        self.assertIn("release-consistency", joined)
        self.assertIn("python3 scripts/release.py check", joined)

    def test_a_removed_rust_quality_gate_in_release_yml_is_caught(self) -> None:
        # Reproduces the specific v1.8.0 incident: release.yml runs no Rust check at all
        # while rust.yml's `quality` job requires fmt/clippy/test/deny.
        precommit_commands = check_workflows.precommit_gate_commands()
        release_yml_lines = ["jobs:", "  verify:", "    steps:"]
        release_yml_lines += [f"      - run: {command}" for command in precommit_commands.values()]
        release_yml_lines += ["  verify-rust:", "    steps:", "      - run: cargo fmt --check"]
        release_yml = "\n".join(release_yml_lines) + "\n"
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "release.yml"
            path.write_text(release_yml, encoding="utf-8")
            with mock.patch.object(check_workflows, "RELEASE_WORKFLOW", path):
                errors = check_workflows.release_gate_drift_errors()
        joined = "\n".join(errors)
        self.assertIn("cargo clippy", joined)
        self.assertIn("cargo test --workspace", joined)
        self.assertIn("cargo deny check", joined)

    def test_agents_md_lists_the_full_python_gate_set(self) -> None:
        commands = check_workflows.agents_md_python_gate_commands()
        self.assertIn("python3 -m pytest tests -v", commands)
        self.assertIn("python3 -m ruff check .", commands)
        self.assertIn("python3 -m ruff format --check .", commands)
        self.assertTrue(any(c.startswith("python3 -m mypy ") for c in commands))
        self.assertNotIn("python3 -m pip install -r requirements-dev.txt", commands)

    def _release_and_rust_text(self) -> tuple[str, str]:
        return (
            check_workflows.RELEASE_WORKFLOW.read_text(encoding="utf-8"),
            check_workflows.RUST_WORKFLOW.read_text(encoding="utf-8"),
        )

    def _errors_with(self, release_text: str | None = None, rust_text: str | None = None) -> list[str]:
        base_release, base_rust = self._release_and_rust_text()
        with tempfile.TemporaryDirectory() as tmp:
            release_path = Path(tmp) / "release.yml"
            rust_path = Path(tmp) / "rust.yml"
            release_path.write_text(release_text if release_text is not None else base_release, encoding="utf-8")
            rust_path.write_text(rust_text if rust_text is not None else base_rust, encoding="utf-8")
            with (
                mock.patch.object(check_workflows, "RELEASE_WORKFLOW", release_path),
                mock.patch.object(check_workflows, "RUST_WORKFLOW", rust_path),
            ):
                return check_workflows.release_gate_drift_errors()

    # E22 verifier review round 1: each of these six independent regressions against the real
    # release.yml previously returned an empty error list. Every one must now be non-empty.
    def test_removing_pytest_from_release_yml_is_caught(self) -> None:
        release_text, _ = self._release_and_rust_text()
        errors = self._errors_with(release_text=release_text.replace("      - run: python3 -m pytest tests -v\n", ""))
        self.assertTrue(errors)
        self.assertIn("pytest", "\n".join(errors))

    def test_removing_ruff_check_from_release_yml_is_caught(self) -> None:
        release_text, _ = self._release_and_rust_text()
        errors = self._errors_with(release_text=release_text.replace("      - run: python3 -m ruff check .\n", ""))
        self.assertTrue(errors)

    def test_removing_mypy_from_release_yml_is_caught(self) -> None:
        release_text, _ = self._release_and_rust_text()
        stripped = re.sub(r"      - run: python3 -m mypy.*\n", "", release_text)
        errors = self._errors_with(release_text=stripped)
        self.assertTrue(errors)

    def test_removing_windows_from_either_matrix_is_caught(self) -> None:
        release_text, _ = self._release_and_rust_text()
        errors = self._errors_with(
            release_text=release_text.replace(
                "os: [macos-latest, ubuntu-latest, windows-latest]",
                "os: [macos-latest, ubuntu-latest]",
            )
        )
        self.assertTrue(errors)
        self.assertIn("verify-rust", "\n".join(errors))

    def test_disabling_verify_rust_with_an_if_condition_is_caught(self) -> None:
        release_text, _ = self._release_and_rust_text()
        errors = self._errors_with(
            release_text=release_text.replace("  verify-rust:\n    runs-on:", "  verify-rust:\n    if: false\n    runs-on:")
        )
        self.assertTrue(errors)
        self.assertIn("conditional", "\n".join(errors))

    def test_nonblocking_clippy_via_continue_on_error_is_caught(self) -> None:
        release_text, _ = self._release_and_rust_text()
        errors = self._errors_with(
            release_text=release_text.replace(
                "      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings\n        working-directory: rust\n",
                "      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings\n"
                "        working-directory: rust\n        continue-on-error: true\n",
            )
        )
        self.assertTrue(errors)
        self.assertIn("continue-on-error", "\n".join(errors))

    def test_dropping_verify_rust_from_publish_needs_is_caught(self) -> None:
        release_text, _ = self._release_and_rust_text()
        errors = self._errors_with(release_text=release_text.replace("needs: [verify, verify-rust]", "needs: [verify]"))
        self.assertTrue(errors)
        self.assertIn("publish", "\n".join(errors))


if __name__ == "__main__":
    unittest.main()
