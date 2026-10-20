import shutil
import sys
import unittest
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

    @unittest.skipUnless(shutil.which("laconic-check"), "laconic-check is not on the PATH")
    def test_lint_count_reads_laconic_check_findings(self):
        self.assertGreaterEqual(humanise_metrics.lint_count("We analyzed the color.\n"), 2)
        self.assertEqual(0, humanise_metrics.lint_count("We analysed the colour.\n"))


if __name__ == "__main__":
    unittest.main()
