"""Checks for evals/agent_bench.py with the Claude CLI stubbed: no network or paid calls."""

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "evals"))

import agent_bench  # noqa: E402

# A minimal correct fix per task, proving each benchmark task can be solved.
FIXES = {
    "pagination": ("app/pages.py", "start = number * size", "start = (number - 1) * size"),
    "gst-rounding": (
        "app/gst.py",
        "return round(Decimal(amount) * Decimal('0.1'), 2)",
        "return (Decimal(amount) * Decimal('0.1')).quantize(Decimal('0.01'), rounding='ROUND_HALF_UP')",
    ),
    "au-dates": ("app/dates.py", "'%m/%d/%Y'", "'%d/%m/%Y'"),
    "none-guard": ("app/names.py", "return ''.join(", "return '' if not name else ''.join("),
    "needle-in-large-file": ("app/helpers.py", "return gross - fee", "return max(gross - fee, 0)"),
}


class AgentBenchTest(unittest.TestCase):
    # Same-size fixture edits within a second must not reuse stale bytecode.
    @mock.patch.dict("os.environ", {"PYTHONDONTWRITEBYTECODE": "1"})
    def test_every_task_starts_failing_and_its_fix_passes(self):
        self.assertEqual(set(agent_bench.TASKS), set(FIXES))
        for task, (rel, old, new) in FIXES.items():
            with self.subTest(task=task), tempfile.TemporaryDirectory() as d:
                root = pathlib.Path(d)
                agent_bench.build(task, root)
                self.assertFalse(agent_bench.tests_pass(root))
                path = root / rel
                text = path.read_text(encoding="utf-8")
                self.assertEqual(1, text.count(old))
                path.write_text(text.replace(old, new), encoding="utf-8")
                self.assertTrue(agent_bench.tests_pass(root))

    def test_main_stops_without_the_claude_cli(self):
        argv = ["agent_bench.py", "a.md", "b.md", "1", "out.jsonl"]
        with mock.patch.object(sys, "argv", argv), mock.patch.object(agent_bench.shutil, "which", return_value=None):
            with self.assertRaises(SystemExit) as stop:
                agent_bench.main()
        self.assertIn("not on PATH", str(stop.exception))

    def test_run_reads_turns_tokens_cost_and_denials(self):
        payload = {
            "num_turns": 4,
            "total_cost_usd": 0.09,
            "usage": {"input_tokens": 10, "cache_creation_input_tokens": 100, "cache_read_input_tokens": 1000, "output_tokens": 50},
            "permission_denials": [{"tool": "Bash"}],
        }
        done = subprocess.CompletedProcess([], 0, stdout=json.dumps(payload), stderr="")
        with mock.patch.object(agent_bench.shutil, "which", return_value="claude"), mock.patch.object(
            agent_bench.subprocess, "run", return_value=done
        ) as run:
            result = agent_bench.run("rules.md", pathlib.Path("."))
        self.assertEqual({"turns": 4, "cost": 0.09, "input_tokens": 1110, "output_tokens": 50, "denials": 1}, result)
        cmd = run.call_args.args[0]
        self.assertIn("--max-budget-usd", cmd)
        self.assertEqual(subprocess.DEVNULL, run.call_args.kwargs["stdin"])

    def test_run_reports_unparseable_output_and_timeouts(self):
        bad = subprocess.CompletedProcess([], 1, stdout="not json", stderr="")
        with mock.patch.object(agent_bench.shutil, "which", return_value="claude"), mock.patch.object(
            agent_bench.subprocess, "run", return_value=bad
        ):
            self.assertEqual({"error": "JSONDecodeError"}, agent_bench.run("rules.md", pathlib.Path(".")))
        with mock.patch.object(agent_bench.shutil, "which", return_value="claude"), mock.patch.object(
            agent_bench.subprocess, "run", side_effect=subprocess.TimeoutExpired("claude", 600)
        ):
            self.assertEqual({"error": "TimeoutExpired"}, agent_bench.run("rules.md", pathlib.Path(".")))

    def test_main_resumes_and_skips_finished_runs(self):
        with tempfile.TemporaryDirectory() as d:
            out = pathlib.Path(d) / "out.jsonl"
            out.write_text(json.dumps({"task": "pagination", "setup": "A", "trial": 1}) + "\n", encoding="utf-8")
            argv = ["agent_bench.py", "a.md", "b.md", "1", str(out)]
            with mock.patch.object(sys, "argv", argv), mock.patch.object(
                agent_bench.shutil, "which", return_value="claude"
            ), mock.patch.object(agent_bench, "run", return_value={"turns": 1}) as run, mock.patch(
                "builtins.print"
            ):
                agent_bench.main()
            rows = [json.loads(line) for line in out.read_text(encoding="utf-8").splitlines()]
        self.assertEqual(2 * len(agent_bench.TASKS) - 1, run.call_count)
        self.assertEqual(2 * len(agent_bench.TASKS), len(rows))
        self.assertTrue(all("passed" in row for row in rows[1:]))


if __name__ == "__main__":
    unittest.main()
