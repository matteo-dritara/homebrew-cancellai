from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts import check_rust_workspace as rw


def write_crate(base: Path, name: str, deps: list[str]) -> None:
    dep_lines = "\n".join(f'{dep} = {{ path = "../{dep}" }}' for dep in deps)
    (base / name).mkdir(parents=True)
    (base / name / "Cargo.toml").write_text(
        f"""[package]
name = "{name}"
version = "0.1.0"

[dependencies]
{dep_lines}
""",
        encoding="utf-8",
    )


def write_target_doc(path: Path, crate_names: list[str]) -> None:
    entries = "\n".join(f"  {name}/            # test crate" for name in crate_names)
    path.write_text(
        f"""# Architecture: Target

## Target Rust workspace

Names may be refined through ADRs, but dependency direction is normative.

```text
crates/
{entries}
```
""",
        encoding="utf-8",
    )


class RustWorkspaceTests(unittest.TestCase):
    def test_real_workspace_is_valid(self):
        self.assertEqual([], rw.validate())

    def test_documented_crates_matches_real_target_doc(self):
        names = rw.documented_crates()
        self.assertIn("cancellai-model", names)
        self.assertIn("cancellai-safety", names)
        self.assertEqual(len(names), len(set(names)))

    def test_checker_detects_a_dependency_cycle(self):
        with tempfile.TemporaryDirectory() as td:
            crates_dir = Path(td) / "crates"
            write_crate(crates_dir, "cancellai-a", ["cancellai-b"])
            write_crate(crates_dir, "cancellai-b", ["cancellai-a"])
            target_doc = Path(td) / "TARGET.md"
            write_target_doc(target_doc, ["cancellai-a", "cancellai-b"])

            with mock.patch.object(rw, "RUST_CRATES_DIR", crates_dir), mock.patch.object(rw, "TARGET_DOC", target_doc):
                errors = rw.validate()
        self.assertTrue(any("dependency cycle" in e for e in errors), errors)

    def test_checker_detects_model_depending_on_a_provider_crate(self):
        with tempfile.TemporaryDirectory() as td:
            crates_dir = Path(td) / "crates"
            write_crate(crates_dir, "cancellai-model", ["cancellai-provider-claude"])
            write_crate(crates_dir, "cancellai-provider-claude", [])
            target_doc = Path(td) / "TARGET.md"
            write_target_doc(target_doc, ["cancellai-model", "cancellai-provider-claude"])

            with mock.patch.object(rw, "RUST_CRATES_DIR", crates_dir), mock.patch.object(rw, "TARGET_DOC", target_doc):
                errors = rw.validate()
        self.assertTrue(any("cancellai-model" in e and "forbidden dependency" in e for e in errors), errors)

    def test_checker_allows_safety_depending_on_model(self):
        with tempfile.TemporaryDirectory() as td:
            crates_dir = Path(td) / "crates"
            write_crate(crates_dir, "cancellai-model", [])
            write_crate(crates_dir, "cancellai-safety", ["cancellai-model"])
            target_doc = Path(td) / "TARGET.md"
            write_target_doc(target_doc, ["cancellai-model", "cancellai-safety"])

            with mock.patch.object(rw, "RUST_CRATES_DIR", crates_dir), mock.patch.object(rw, "TARGET_DOC", target_doc):
                errors = rw.validate()
        self.assertEqual([], errors)

    def test_checker_detects_an_undocumented_crate_on_disk(self):
        with tempfile.TemporaryDirectory() as td:
            crates_dir = Path(td) / "crates"
            write_crate(crates_dir, "cancellai-model", [])
            write_crate(crates_dir, "cancellai-mystery", [])
            target_doc = Path(td) / "TARGET.md"
            write_target_doc(target_doc, ["cancellai-model"])

            with mock.patch.object(rw, "RUST_CRATES_DIR", crates_dir), mock.patch.object(rw, "TARGET_DOC", target_doc):
                errors = rw.validate()
        self.assertTrue(any("cancellai-mystery" in e and "not documented" in e for e in errors), errors)

    def test_checker_detects_a_documented_crate_missing_on_disk(self):
        with tempfile.TemporaryDirectory() as td:
            crates_dir = Path(td) / "crates"
            write_crate(crates_dir, "cancellai-model", [])
            target_doc = Path(td) / "TARGET.md"
            write_target_doc(target_doc, ["cancellai-model", "cancellai-ghost"])

            with mock.patch.object(rw, "RUST_CRATES_DIR", crates_dir), mock.patch.object(rw, "TARGET_DOC", target_doc):
                errors = rw.validate()
        self.assertTrue(any("cancellai-ghost" in e and "missing under" in e for e in errors), errors)

    def test_checker_detects_a_package_name_directory_mismatch(self):
        with tempfile.TemporaryDirectory() as td:
            crates_dir = Path(td) / "crates"
            (crates_dir / "cancellai-model").mkdir(parents=True)
            (crates_dir / "cancellai-model" / "Cargo.toml").write_text(
                '[package]\nname = "cancellai-wrong-name"\n\n[dependencies]\n', encoding="utf-8"
            )
            target_doc = Path(td) / "TARGET.md"
            write_target_doc(target_doc, ["cancellai-model"])

            with mock.patch.object(rw, "RUST_CRATES_DIR", crates_dir), mock.patch.object(rw, "TARGET_DOC", target_doc):
                errors = rw.validate()
        self.assertTrue(any("does not match its directory" in e for e in errors), errors)


class LintPolicyTests(unittest.TestCase):
    """A crate may not quietly drop a workspace lint.

    Cargo's `[lints]` table is all-or-nothing: a crate declaring its own *replaces* the workspace
    table. `cancellai-sealedfs` must declare one, because ADR-0017 lifts `unsafe_code` and `forbid`
    is the one level an inner attribute cannot lift. That was invisible while the workspace table
    held one lint, and became real the moment it held a panic-freedom policy - the crate holding
    every `unsafe` block in the repository was the only crate the policy could not reach.
    """

    WORKSPACE = """[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
unwrap_used = "deny"
undocumented_unsafe_blocks = "deny"
"""

    def test_a_local_table_that_repeats_the_policy_is_accepted(self):
        local = '[lints.rust]\nunsafe_code = "allow"\n\n[lints.clippy]\nunwrap_used = "deny"\nundocumented_unsafe_blocks = "deny"\n'
        workspace = rw.lint_tables(self.WORKSPACE)
        tables = rw.lint_tables(local)
        for tool, lints in workspace.items():
            for lint, level in lints.items():
                if lint == "unsafe_code":
                    continue
                self.assertEqual(level, tables.get(tool, {}).get(lint))

    def test_unsafe_code_is_the_one_permitted_difference(self):
        tables = rw.lint_tables('[lints.rust]\nunsafe_code = "allow"\n')
        self.assertEqual("allow", tables["rust"]["unsafe_code"])

    def test_the_parser_reads_both_workspace_and_crate_tables(self):
        self.assertEqual(
            {"rust": {"unsafe_code": "forbid"}, "clippy": {"unwrap_used": "deny", "undocumented_unsafe_blocks": "deny"}},
            rw.lint_tables(self.WORKSPACE),
        )

    def test_the_committed_workspace_passes(self):
        self.assertEqual([], rw.lint_policy_errors())

    def test_every_unsafe_block_in_the_workspace_carries_a_safety_comment(self):
        # The property the lint enforces, asserted independently of clippy so it survives a lint
        # being relaxed: 41 unsafe blocks, 41 SAFETY comments, all in the one crate ADR-0017 allows.
        root = rw.RUST_CRATES_DIR
        for source in sorted(root.glob("*/src/*.rs")):
            text = source.read_text(encoding="utf-8")
            # Comment lines are excluded, and that is not a convenience: a `SAFETY:` comment
            # explaining why an `unsafe { mem::zeroed() }` was *replaced* mentions the construct
            # without being one, and counting it made this test fail on a change that removed
            # three unsafe blocks (E27-S06).
            code = "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("//"))
            blocks = code.count("unsafe {")
            if not blocks:
                continue
            with self.subTest(source=str(source.relative_to(root))):
                self.assertEqual("cancellai-sealedfs", source.parts[-3], "unsafe outside the ADR-0017 crate")
                self.assertGreaterEqual(text.count("SAFETY:"), blocks)


if __name__ == "__main__":
    unittest.main()
