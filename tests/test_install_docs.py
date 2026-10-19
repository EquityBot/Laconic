"""Checks installation paths in INSTALL.md and translations."""

import pathlib
import re
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


class ClaudeCodeUpdateTest(unittest.TestCase):
    def test_marketplace_refresh_is_paired_with_plugin_update(self):
        # `claude plugin marketplace update` refreshes only the marketplace
        # listing; the installed copy stays at its old version until
        # `claude plugin update` runs, so the docs must never offer the first alone.
        refresh = "claude plugin marketplace update laconic"
        update = "claude plugin update laconic@laconic"
        # Translations are optional; check any that exist alongside INSTALL.md.
        translations = sorted((ROOT / ".github/install").glob("INSTALL.*.md"))
        for path in [ROOT / "INSTALL.md", *translations]:
            with self.subTest(file=path.name):
                text = path.read_text(encoding="utf8")
                self.assertIn(update, text)
                for block in re.split(r"\n[^\S\n]*\n", text):
                    if refresh in block:
                        self.assertIn(update, block)


class ZedInstallPathTest(unittest.TestCase):
    def test_zed_install_paths(self):
        # Translations are optional; check any that exist alongside INSTALL.md.
        translations = sorted((ROOT / ".github/install").glob("INSTALL.*.md"))
        for path in [ROOT / "INSTALL.md", *translations]:
            with self.subTest(file=path.name):
                text = path.read_text(encoding="utf8")
                marker = "<summary><strong>Zed</strong></summary>"
                self.assertIn(marker, text)
                section = text.split(marker, 1)[1]
                self.assertIn("</details>", section)
                section = section.split("</details>", 1)[0]

                self.assertNotIn("~/.config/zed/skills", section)
                self.assertIn(
                    "mkdir -p ~/.agents/skills\n"
                    "cp -R laconic/skills/laconic ~/.agents/skills/",
                    section,
                )
                self.assertIn("~/.agents/skills/laconic", section)


class GrokInstallPathTest(unittest.TestCase):
    def test_grok_install_commands(self):
        text = (ROOT / "INSTALL.md").read_text(encoding="utf8")
        marker = "<summary><strong>Grok (<code>grok</code>)</strong></summary>"
        self.assertIn(marker, text)
        section = text.split(marker, 1)[1]
        self.assertIn("</details>", section)
        section = section.split("</details>", 1)[0]

        self.assertIn("grok plugin install ryanduguid/laconic --trust", section)
        self.assertIn("grok plugin enable laconic", section)
        self.assertIn("grok plugin update laconic", section)
        self.assertIn("grok plugin uninstall laconic --confirm", section)
        self.assertIn("/laconic", section)
        self.assertIn("~/.grok/AGENTS.md", section)
        self.assertIn("~/.grok/rules/laconic.md", section)
        self.assertNotIn("~/.claude/.laconic-always", section)


if __name__ == "__main__":
    unittest.main()
