"""Offline guards for third-party CLI installation in load checks."""

import json
import re
import shlex
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = ("plugin-load-check.yml", "pi-load-check.yml")


class CiInstallTest(unittest.TestCase):
    def install_commands(self):
        for name in WORKFLOWS:
            text = (ROOT / ".github/workflows" / name).read_text(encoding="utf8")
            commands = re.findall(r"^\s*(?:run:\s*)?(npm (?:ci|install) .+)$", text, re.M)
            self.assertEqual(1, len(commands), name)
            yield name, shlex.split(commands[0])

    def test_installed_cli_versions_are_exact(self):
        for name, command in self.install_commands():
            with self.subTest(workflow=name):
                self.assertEqual("ci", command[1])
                directory = ROOT / command[command.index("--prefix") + 1]
                manifest = json.loads((directory / "package.json").read_text())
                lock = json.loads((directory / "package-lock.json").read_text())
                dependencies = manifest["dependencies"]
                self.assertEqual(1, len(dependencies))
                self.assertEqual(dependencies, lock["packages"][""]["dependencies"])
                for package, version in dependencies.items():
                    self.assertRegex(version, r"^\d+\.\d+\.\d+$")
                    self.assertEqual(version, lock["packages"][f"node_modules/{package}"]["version"])
                for path, package in lock["packages"].items():
                    if path:
                        self.assertTrue(package["integrity"], path)

    def test_dependency_lifecycle_scripts_are_disabled(self):
        for name, command in self.install_commands():
            with self.subTest(workflow=name):
                self.assertIn("--ignore-scripts", command)

    def test_pi_transport_excludes_the_known_vulnerable_range(self):
        # GHSA-4qv8-2p9j-2685, GHSA-f72v-6w45-q472 and GHSA-xjj6-f33q-929c.
        path = ROOT / ".github/ci/pi/package-lock.json"
        lock = json.loads(path.read_text())
        transports = [package for path, package in lock["packages"].items()
                      if path.endswith("node_modules/undici")]
        self.assertTrue(transports)
        for package in transports:
            version = tuple(int(part) for part in package["version"].split("."))
            self.assertFalse((8, 0, 0) <= version < (8, 12, 2), package["version"])

    def test_pi_glob_matcher_excludes_the_known_vulnerable_range(self):
        # GHSA-g3hv-75wg-m249: minimatch before 11.0.1.
        lock = json.loads((ROOT / ".github/ci/pi/package-lock.json").read_text())
        matchers = [package for path, package in lock["packages"].items()
                    if path.endswith("node_modules/minimatch")]
        self.assertTrue(matchers)
        for package in matchers:
            version = tuple(int(part) for part in package["version"].split("."))
            self.assertGreaterEqual(version, (11, 0, 1), package["version"])

    def test_load_checks_do_not_retain_checkout_credentials(self):
        for name in WORKFLOWS:
            text = (ROOT / ".github/workflows" / name).read_text(encoding="utf8")
            with self.subTest(workflow=name):
                self.assertIn("permissions:\n  contents: read\n", text)
                checkouts = text.split("- uses: actions/checkout@")[1:]
                self.assertTrue(checkouts, name)
                for checkout in checkouts:
                    step = checkout.split("\n      - ", 1)[0]
                    self.assertIn("persist-credentials: false", step)

    def test_workflow_actions_are_pinned_to_full_commit_shas(self):
        workflows = (ROOT / ".github/workflows").iterdir()
        for path in sorted(p for p in workflows if p.suffix in (".yml", ".yaml")):
            text = path.read_text(encoding="utf8")
            for use in re.findall(r"^\s*(?:- )?uses:\s*(.+?)\s*$", text, re.M):
                with self.subTest(workflow=path.name, uses=use):
                    self.assertRegex(use, r"^[\w./-]+@[0-9a-f]{40} # v\d")

    def test_dependabot_covers_actions_and_every_ci_lockfile(self):
        # The suite has no YAML parser, so match each line exactly, block by block.
        config = (ROOT / ".github/dependabot.yml").read_text(encoding="utf8")
        self.assertRegex(config, r"(?m)^version: 2$")
        self.assertRegex(config, r"(?m)^updates:$")
        pieces = re.split(r"(?m)^  - package-ecosystem: (\S+)$", config)[1:]
        blocks = dict(zip(pieces[::2], pieces[1::2]))
        self.assertEqual({"cargo", "github-actions", "npm", "pip"}, set(blocks))
        for ecosystem, block in blocks.items():
            with self.subTest(ecosystem=ecosystem):
                self.assertRegex(block, r"(?m)^    schedule:\n      interval: \w+$")
        self.assertRegex(blocks["github-actions"], r"(?m)^    directory: /$")
        lockfiles = sorted((ROOT / ".github/ci").glob("*/package-lock.json"))
        self.assertTrue(lockfiles)
        for lock in lockfiles:
            with self.subTest(directory=lock.parent.name):
                self.assertRegex(blocks["npm"], rf"(?m)^      - /\.github/ci/{lock.parent.name}$")
        # Dependabot's pip updater reads every requirements file in the directory.
        self.assertTrue(sorted((ROOT / ".github/ci").glob("requirements-*.txt")))
        self.assertRegex(blocks["pip"], r"(?m)^    directory: /\.github/ci$")
        # laconic-check has no dependencies, so only the fuzz crate's lockfile lists any.
        self.assertTrue((ROOT / "tools/laconic-check/fuzz/Cargo.lock").is_file())
        self.assertRegex(blocks["cargo"], r"(?m)^    directory: /tools/laconic-check/fuzz$")

    def test_pip_runs_only_the_hashed_coverage_install(self):
        # Like the npm guard, this expects one exact command, so any other line naming pip, however it is spelt,
        # chained, continued or folded, fails until it is reviewed and added here.
        expected = {"tests.yml": ['        run: python -m pip install --only-binary=":all:" --require-hashes'
                                  ' -r .github/ci/requirements-coverage.txt']}
        workflows = (ROOT / ".github/workflows").iterdir()
        for name in expected:
            self.assertTrue((ROOT / ".github/workflows" / name).is_file(), name)
        for path in sorted(p for p in workflows if p.suffix in (".yml", ".yaml")):
            with self.subTest(workflow=path.name):
                self.assertEqual(expected.get(path.name, []), pip_lines(path.read_text(encoding="utf8")))
        for text in ("run: pip3.11 install foo", "run: python -m pip --no-input install foo",
                     "run: >\n  python -m pip\n  install foo", "run: uv pip install foo", "run: pipx install foo"):
            with self.subTest(sees=text):
                self.assertTrue(pip_lines(text))
        # Each entry of the requirements file is an exact version followed by sha256 hashes.
        text = (ROOT / ".github/ci/requirements-coverage.txt").read_text(encoding="utf8").replace("\\\n", " ")
        entries = [e for e in text.splitlines() if e.strip() and not e.lstrip().startswith("#")]
        self.assertTrue(entries)
        for entry in entries:
            self.assertRegex(entry, r"^[\w.-]+==[\w.]+( +--hash=sha256:[0-9a-f]{64})+ *$")


def pip_lines(text):
    """Every line of workflow text that names pip, pip3, pip3.11, pipx and the like."""
    return re.findall(r"^.*\bpip(?:x|[\d.]*)\b.*$", text, re.M)


if __name__ == "__main__":
    unittest.main()
