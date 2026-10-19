"""Checks that every always-on snippet in INSTALL.md matches rules/laconic.md."""

import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import sync_rules  # noqa: E402


class RulesSyncTest(unittest.TestCase):
    def test_install_snippets_match_rules_file(self):
        install = sync_rules.INSTALL.read_text(encoding="utf-8")
        snippets = sync_rules.SNIPPET.findall(install)
        # One snippet per agent route; a changed marker must not silently drop a route.
        self.assertEqual(7, len(snippets), "Expected 7 always-on snippets in INSTALL.md")
        body = sync_rules.rules_body()
        for _, snippet, _ in snippets:
            self.assertEqual(body, snippet, "Run python scripts/sync_rules.py")

    def test_rules_file_stays_compact(self):
        words = len(sync_rules.rules_body().split())
        self.assertLessEqual(words, 240, "rules/laconic.md is loaded into every session; keep it short")


if __name__ == "__main__":
    unittest.main()
