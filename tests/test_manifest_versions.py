"""Checks that every plugin manifest declares one version and CHANGELOG.md names it."""

import json
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
MANIFESTS = (
    ".claude-plugin/plugin.json",
    ".claude-plugin/marketplace.json",
    ".codex-plugin/plugin.json",
    ".agents/plugins/marketplace.json",
    "gemini-extension.json",
    "qwen-extension.json",
    "kimi.plugin.json",
    "plugin.json",
    "package.json",
)


def declared_versions() -> dict[str, str]:
    versions = {}
    for name in MANIFESTS:
        manifest = json.loads((ROOT / name).read_text(encoding="utf-8"))
        if "version" in manifest:
            versions[name] = manifest["version"]
        for plugin in manifest.get("plugins", []):
            if "version" in plugin:
                versions[f"{name} ({plugin['name']})"] = plugin["version"]
    return versions


class ManifestVersionTest(unittest.TestCase):
    def test_declared_versions_agree(self):
        versions = declared_versions()
        # A renamed key or moved file must not leave nothing to compare.
        self.assertGreaterEqual(len(versions), 2, versions)
        self.assertEqual(1, len(set(versions.values())), versions)

    def test_changelog_names_the_current_versions(self):
        changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        plugin = next(iter(declared_versions().values()))
        self.assertIn(f"## [plugin {plugin}] - ", changelog)
        cargo = (ROOT / "tools/laconic-check/Cargo.toml").read_text(encoding="utf-8")
        checker = re.search(r'^version = "([^"]+)"', cargo, re.M).group(1)
        self.assertIn(f"## [laconic-check {checker}] - ", changelog)


if __name__ == "__main__":
    unittest.main()
