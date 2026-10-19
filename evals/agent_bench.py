"""Agentic cost benchmark: does Laconic's agent-work guidance cut turns, tokens and cost?

Builds 5 small synthetic Python repositories, each with one failing unittest, then asks
Claude Code (print mode, isolated settings, tools limited to Read, Edit, Glob, Grep and
`python -m unittest`) to fix each one under two rule sets. Records whether the tests pass
afterwards, plus turns, tokens and cost. Everything runs in temporary folders.

usage: python -I agent_bench.py <rules-a.md> <rules-b.md> <trials> <out.jsonl>
"""

import json
import pathlib
import shutil
import subprocess
import sys
import tempfile

FILLER = "\n".join(
    f"def helper_{i}(value):\n    \"\"\"Return value scaled by {i}.\"\"\"\n    return value * {i}\n" for i in range(1, 160)
)

TASKS = {
    "pagination": {
        "app/pages.py": "def page(items, number, size):\n    \"\"\"Return the given 1-based page of items.\"\"\"\n    start = number * size\n    return items[start:start + size]\n",
        "tests/test_pages.py": "import unittest\nfrom app.pages import page\n\nclass PageTest(unittest.TestCase):\n    def test_first_page(self):\n        self.assertEqual(page(list(range(10)), 1, 3), [0, 1, 2])\n\n    def test_last_page(self):\n        self.assertEqual(page(list(range(10)), 4, 3), [9])\n",
    },
    "gst-rounding": {
        "app/gst.py": "from decimal import Decimal\n\ndef gst(amount):\n    \"\"\"GST at 10%, rounded half up to the cent.\"\"\"\n    return round(Decimal(amount) * Decimal('0.1'), 2)\n",
        "tests/test_gst.py": "import unittest\nfrom decimal import Decimal\nfrom app.gst import gst\n\nclass GstTest(unittest.TestCase):\n    def test_half_cent_rounds_up(self):\n        self.assertEqual(gst('0.25'), Decimal('0.03'))\n\n    def test_plain(self):\n        self.assertEqual(gst('100.00'), Decimal('10.00'))\n",
    },
    "au-dates": {
        "app/dates.py": "from datetime import datetime\n\ndef parse(text):\n    \"\"\"Parse an Australian date such as 03/09/2026 (3 September 2026).\"\"\"\n    return datetime.strptime(text, '%m/%d/%Y').date()\n",
        "tests/test_dates.py": "import unittest\nfrom datetime import date\nfrom app.dates import parse\n\nclass DateTest(unittest.TestCase):\n    def test_day_first(self):\n        self.assertEqual(parse('03/09/2026'), date(2026, 9, 3))\n\n    def test_end_of_month(self):\n        self.assertEqual(parse('31/10/2026'), date(2026, 10, 31))\n",
    },
    "none-guard": {
        "app/names.py": "def initials(name):\n    \"\"\"Initials of a name; an empty string for a missing name.\"\"\"\n    return ''.join(part[0].upper() for part in name.split())\n",
        "tests/test_names.py": "import unittest\nfrom app.names import initials\n\nclass InitialsTest(unittest.TestCase):\n    def test_name(self):\n        self.assertEqual(initials('ada lovelace'), 'AL')\n\n    def test_missing(self):\n        self.assertEqual(initials(None), '')\n",
    },
    "needle-in-large-file": {
        "app/helpers.py": FILLER + "\n\ndef net_amount(gross, fee):\n    \"\"\"Gross less the fee, never below zero.\"\"\"\n    return gross - fee\n\n" + FILLER.replace("helper_", "extra_"),
        "tests/test_helpers.py": "import unittest\nfrom app.helpers import net_amount\n\nclass NetTest(unittest.TestCase):\n    def test_normal(self):\n        self.assertEqual(net_amount(100, 20), 80)\n\n    def test_never_negative(self):\n        self.assertEqual(net_amount(10, 20), 0)\n",
    },
}
PROMPT = "One test in this repository fails. Find the cause, fix the code (not the test) and run the tests to confirm they pass."


def build(task: str, root: pathlib.Path) -> None:
    for rel, text in TASKS[task].items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    (root / "app" / "__init__.py").write_text("", encoding="utf-8")
    (root / "tests" / "__init__.py").write_text("", encoding="utf-8")


def tests_pass(root: pathlib.Path) -> bool:
    r = subprocess.run([sys.executable, "-m", "unittest", "discover", "-s", "tests", "-t", "."], cwd=root, capture_output=True, text=True, timeout=120)
    return r.returncode == 0


def run(rules: str, root: pathlib.Path) -> dict:
    cmd = [shutil.which("claude"), "--print", "--output-format", "json", "--no-session-persistence", "--strict-mcp-config",
           "--setting-sources", "", "--model", "claude-opus-5-5", "--permission-mode", "default",
           "--tools", "Read,Edit,Glob,Grep,Bash", "--allowedTools", "Read", "Edit", "Glob", "Grep", "Bash(python -m unittest:*)",
           "--append-system-prompt-file", rules, "--max-budget-usd", "1.5", PROMPT]
    try:
        r = subprocess.run(cmd, cwd=root, capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=600)
        out = json.loads(r.stdout)
    except (subprocess.TimeoutExpired, ValueError) as e:
        return {"error": type(e).__name__}
    usage = out.get("usage") or {}
    return {
        "turns": out.get("num_turns"),
        "cost": out.get("total_cost_usd"),
        "input_tokens": sum(usage.get(k, 0) or 0 for k in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")),
        "output_tokens": usage.get("output_tokens"),
        "denials": len(out.get("permission_denials") or []),
    }


def main() -> None:
    rules = {"A": sys.argv[1], "B": sys.argv[2]}
    trials = int(sys.argv[3])
    out = pathlib.Path(sys.argv[4])
    done = {(r["task"], r["setup"], r["trial"]) for r in map(json.loads, out.read_text(encoding="utf-8").splitlines())} if out.exists() else set()
    for trial in range(1, trials + 1):
        for task in TASKS:
            for setup, path in rules.items():
                if (task, setup, trial) in done:
                    continue
                with tempfile.TemporaryDirectory() as d:
                    root = pathlib.Path(d)
                    build(task, root)
                    assert not tests_pass(root), f"{task} should start failing"
                    result = run(path, root)
                    result.update(task=task, setup=setup, trial=trial, passed=tests_pass(root))
                with out.open("a", encoding="utf-8") as f:
                    f.write(json.dumps(result) + "\n")
                print(json.dumps(result), flush=True)


if __name__ == "__main__":
    main()
