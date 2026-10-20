import shutil
import subprocess
import sys
import unittest
from unittest import mock
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "evals"))

import humanise_metrics  # noqa: E402
import run_evals  # noqa: E402


class HumaniseMetricsTest(unittest.TestCase):
    def test_humanise_cases_are_valid_and_every_case_has_checks(self):
        cases = run_evals.load_cases(ROOT / "evals" / "humanise-cases.jsonl")
        self.assertEqual([], run_evals.validate_cases(cases))
        self.assertGreaterEqual(len(cases), 20)
        for case in cases:
            with self.subTest(case=case["id"]):
                self.assertIn("checks", case)
                self.assertTrue(humanise_metrics.source_text(case["prompt"]).strip())

    def test_source_text_drops_only_the_instruction_paragraph(self):
        prompt = "Humanise this.\n\nFirst paragraph.\n\nSecond paragraph."
        self.assertEqual("First paragraph.\n\nSecond paragraph.", humanise_metrics.source_text(prompt))

    def test_summary_reports_source_and_condition_means(self):
        cases = {"c1": {"prompt": "Do it.\n\none two three four"}}
        rows = [
            {"case_id": "c1", "condition": "candidate", "response": "one two"},
            {"case_id": "c1", "condition": "candidate", "response": "one two three four"},
            {"case_id": "c1", "condition": "comparator", "response": "one two three four five six seven eight"},
        ]
        summary = humanise_metrics.summarise(
            rows, cases, measure=lambda text: {"lint": text.count("two")}, drops=lambda source, rewrite: 4 - len(rewrite.split()[:4])
        )
        self.assertEqual({"rows": 1, "lint": 1, "length": 1.0, "drops": 0}, summary["source"])
        self.assertEqual({"rows": 2, "lint": 1, "length": 0.75, "drops": 1}, summary["candidate"])
        self.assertEqual(2.0, summary["comparator"]["length"])

    def test_checker_failures_raise_instead_of_counting_as_clean(self):
        failed = subprocess.CompletedProcess(args=["x"], returncode=2, stdout="", stderr="cannot read input")
        with mock.patch.object(humanise_metrics.subprocess, "run", return_value=failed):
            with self.assertRaisesRegex(RuntimeError, "exited 2: cannot read input"):
                humanise_metrics.lint_count("text")
            with self.assertRaisesRegex(RuntimeError, "exited 2"):
                humanise_metrics.preserve_drops("a", "b")
        drops = subprocess.CompletedProcess(args=["x"], returncode=1, stdout="dropped number: 12\n", stderr="")
        with mock.patch.object(humanise_metrics.subprocess, "run", return_value=drops):
            self.assertEqual(1, humanise_metrics.preserve_drops("a 12", "a"))
        with self.assertRaises(FileNotFoundError):
            humanise_metrics.checker_metrics(None, Path("missing-dslop.toml"))

    @unittest.skipUnless(shutil.which("laconic-check"), "laconic-check is not on the PATH")
    def test_lint_count_reads_laconic_check_findings(self):
        self.assertGreaterEqual(humanise_metrics.lint_count("We analyzed the color.\n"), 2)
        self.assertEqual(0, humanise_metrics.lint_count("We analysed the colour.\n"))


if __name__ == "__main__":
    unittest.main()
