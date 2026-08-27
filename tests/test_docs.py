from __future__ import annotations

import unittest

from scripts import check_docs


class DocumentationIntegrityTests(unittest.TestCase):
    def test_repository_local_markdown_links_resolve(self) -> None:
        broken: list[str] = []
        for path in check_docs.markdown_files():
            for link in check_docs.local_links(path):
                if not link.target.exists():
                    broken.append(f"{path}:{link.line}:{link.raw_target}")
        self.assertEqual(broken, [])

    def test_safety_invariant_ids_are_stable_and_contiguous(self) -> None:
        self.assertEqual(check_docs.safety_invariant_ids(), {f"SI-{n:03d}" for n in range(1, 32)})


if __name__ == "__main__":
    unittest.main()
