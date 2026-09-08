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
        errors = self._errors_with(
            release_text=release_text.replace(
                "needs: [verify, verify-rust, build-artifacts, release-manifest-generate]",
                "needs: [verify, build-artifacts, release-manifest-generate]",
            )
        )
        self.assertTrue(errors)
        self.assertIn("publish", "\n".join(errors))


# E23-S01: check_platforms.py's ancestor check needs the verified_commit object present
# locally, not merely a checkout that contains the tagged ref. A shallow checkout regression
# must be caught statically, the same way CR-TE-06 taught release_gate_drift_errors() to catch
# a dropped gate rather than relying on a real tag push to notice.
#
# Round 1 independent verifier review (project/evidence/E23-VERIFIER-REVIEW.md) found a first
# version of this check inspected only the job's first `actions/checkout` step: a workflow
# with a full-history checkout aimed at an isolated `path:`, followed by a second, default
# (shallow) checkout into the actual job workspace, passed while the gate still ran shallow -
# reproducing the v1.10.0 failure under a different layout. `ReleaseHistoryMultiCheckoutTests`
# below is that exact bypass, plus adversarial siblings, as permanent regression coverage.
class ReleaseHistoryGateTests(unittest.TestCase):
    def _errors_for(self, release_text: str) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "release.yml"
            path.write_text(release_text, encoding="utf-8")
            with mock.patch.object(check_workflows, "RELEASE_WORKFLOW", path):
                return check_workflows.release_history_gate_errors()

    def test_release_workflow_currently_fetches_full_history_for_the_provenance_gate(self) -> None:
        self.assertEqual(check_workflows.release_history_gate_errors(), [])

    def test_removing_fetch_depth_from_the_verify_checkout_is_caught(self) -> None:
        # Reproduces the exact v1.10.0 incident: the checkout step has no fetch-depth at all,
        # so GitHub Actions defaults to a shallow (depth-1) checkout.
        errors = self._errors_for(
            "jobs:\n"
            "  verify:\n"
            "    steps:\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef # v7.0.1\n"
            "        with:\n"
            "          persist-credentials: false\n"
            "      - run: python3 scripts/check_platforms.py check\n"
        )
        self.assertTrue(errors)
        self.assertIn("fetch-depth", "\n".join(errors))

    def test_a_nonzero_fetch_depth_on_the_verify_checkout_is_caught(self) -> None:
        # A finite depth (e.g. 50) is still not enough: an arbitrarily old verified_commit is
        # not bounded by any fixed depth, so only fetch-depth: 0 (full history) is accepted.
        errors = self._errors_for(
            "jobs:\n"
            "  verify:\n"
            "    steps:\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef # v7.0.1\n"
            "        with:\n"
            "          persist-credentials: false\n"
            "          fetch-depth: 50\n"
            "      - run: python3 scripts/check_platforms.py check\n"
        )
        self.assertTrue(errors)
        self.assertIn("fetch-depth", "\n".join(errors))

    def test_a_job_without_the_provenance_gate_is_not_required_to_fetch_full_history(self) -> None:
        # A shallow checkout is fine for a job that never runs the ancestor-based check - this
        # check only protects the specific gate it exists for, not every checkout in the file.
        errors = self._errors_for(
            "jobs:\n"
            "  verify:\n"
            "    steps:\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef # v7.0.1\n"
            "        with:\n"
            "          persist-credentials: false\n"
            "      - run: python3 -m pytest tests -v\n"
        )
        self.assertEqual(errors, [])

    def test_fetch_depth_on_a_later_unrelated_step_does_not_mask_a_shallow_checkout(self) -> None:
        errors = self._errors_for(
            "jobs:\n"
            "  verify:\n"
            "    steps:\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef # v7.0.1\n"
            "        with:\n"
            "          persist-credentials: false\n"
            "      - run: python3 scripts/check_platforms.py check\n"
            "      - uses: some/other-action@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "        with:\n"
            "          fetch-depth: 0\n"
        )
        self.assertTrue(errors)


# E23-S01 round 2 repair: adversarial coverage for multi-checkout workflow layouts, the exact
# class of bypass round 1 independent review found.
class ReleaseHistoryMultiCheckoutTests(unittest.TestCase):
    def _errors_for(self, release_text: str) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "release.yml"
            path.write_text(release_text, encoding="utf-8")
            with mock.patch.object(check_workflows, "RELEASE_WORKFLOW", path):
                return check_workflows.release_history_gate_errors()

    def test_full_history_checkout_at_an_isolated_path_does_not_satisfy_a_shallow_workspace(self) -> None:
        # The exact round-1 finding: a full-history checkout aimed at `path:
        # full-history-copy`, then a second, default (shallow) checkout populating the actual
        # job workspace the gate runs in.
        errors = self._errors_for(
            "jobs:\n"
            "  verify:\n"
            "    steps:\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "        with:\n"
            "          fetch-depth: 0\n"
            "          path: full-history-copy\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "      - run: python3 scripts/check_platforms.py check\n"
        )
        self.assertTrue(errors, "the round-1 bypass must be rejected")
        self.assertIn("fetch-depth", "\n".join(errors))

    def test_a_later_full_history_checkout_overwriting_the_workspace_root_is_accepted(self) -> None:
        # The mirror image, and a real workflow pattern: an initial default (shallow) checkout,
        # then a second checkout - also targeting the workspace root - with fetch-depth: 0.
        # actions/checkout re-populates whatever directory it targets, so the *later* checkout
        # determines the final state; this must pass.
        errors = self._errors_for(
            "jobs:\n"
            "  verify:\n"
            "    steps:\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "        with:\n"
            "          fetch-depth: 0\n"
            "      - run: python3 scripts/check_platforms.py check\n"
        )
        self.assertEqual(errors, [])

    def test_a_later_shallow_checkout_overwriting_a_prior_full_history_root_checkout_is_caught(self) -> None:
        # The inverse ordering of the round-1 bypass: full history at the root first, then a
        # second, unrelated shallow checkout that also targets the root and clobbers it. Only
        # the state at the moment the gate's own run step executes matters.
        errors = self._errors_for(
            "jobs:\n"
            "  verify:\n"
            "    steps:\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "        with:\n"
            "          fetch-depth: 0\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "      - run: python3 scripts/check_platforms.py check\n"
        )
        self.assertTrue(errors)
        self.assertIn("fetch-depth", "\n".join(errors))

    def test_the_gate_run_in_a_working_directory_checked_out_full_is_accepted(self) -> None:
        # A full-history checkout at a named path, with the gate step's own
        # `working-directory:` pointed at that same path (not the default workspace root),
        # while the workspace root itself stays shallow - the gate genuinely runs somewhere
        # with full history, so this must pass.
        errors = self._errors_for(
            "jobs:\n"
            "  verify:\n"
            "    steps:\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "        with:\n"
            "          fetch-depth: 0\n"
            "          path: full-history-copy\n"
            "      - run: python3 scripts/check_platforms.py check\n"
            "        working-directory: full-history-copy\n"
        )
        self.assertEqual(errors, [])

    def test_the_gate_run_in_a_working_directory_never_checked_out_is_caught(self) -> None:
        # A full-history checkout exists, but the gate's own working-directory points somewhere
        # that checkout never targeted - nothing establishes that directory has any history at
        # all (in practice the command would not even find its target file there).
        errors = self._errors_for(
            "jobs:\n"
            "  verify:\n"
            "    steps:\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "        with:\n"
            "          fetch-depth: 0\n"
            "      - run: python3 scripts/check_platforms.py check\n"
            "        working-directory: some-other-dir\n"
        )
        self.assertTrue(errors)
        self.assertIn("fetch-depth=None", "\n".join(errors))

    def test_a_leading_dot_slash_on_path_and_working_directory_are_recognized_as_the_same_dir(self) -> None:
        # `path: ./full-history-copy` and `working-directory: full-history-copy` name the same
        # directory; a literal-string mismatch here must not cause a false failure.
        errors = self._errors_for(
            "jobs:\n"
            "  verify:\n"
            "    steps:\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "        with:\n"
            "          fetch-depth: 0\n"
            "          path: ./full-history-copy\n"
            "      - run: python3 scripts/check_platforms.py check\n"
            "        working-directory: full-history-copy\n"
        )
        self.assertEqual(errors, [])

    def test_a_path_scoped_full_history_checkout_after_the_gate_step_does_not_help(self) -> None:
        # A full-history checkout that only happens *after* the gate already ran must not be
        # credited retroactively.
        errors = self._errors_for(
            "jobs:\n"
            "  verify:\n"
            "    steps:\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "      - run: python3 scripts/check_platforms.py check\n"
            "      - uses: actions/checkout@deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n"
            "        with:\n"
            "          fetch-depth: 0\n"
        )
        self.assertTrue(errors)


if __name__ == "__main__":
    unittest.main()
