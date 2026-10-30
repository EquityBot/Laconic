import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "evals"))

import fact_survival  # noqa: E402
import run_evals  # noqa: E402

ITEMS = [
    {"id": "f1", "kind": "fact", "text": "GST payable is $4,812."},
    {"id": "q1", "kind": "qualifier", "text": "Lodgment is likely by Friday."},
]


def reply(*rows):
    return json.dumps({"items": list(rows)})


class ParseVerdictsTest(unittest.TestCase):
    def test_keeps_a_verified_quote_across_case_quotes_and_wrapping(self):
        rewrite = "GST payable:\n$4,812. It’s likely by Friday."
        verdicts = fact_survival.parse_verdicts(
            reply(
                {"id": "f1", "verdict": "kept", "quote": "gst payable: $4,812"},
                {"id": "q1", "verdict": "kept", "quote": "it's likely by friday"},
            ),
            rewrite,
            ITEMS,
        )
        self.assertEqual(["kept", "kept"], [v["verdict"] for v in verdicts])

    def test_a_quote_not_in_the_rewrite_counts_as_dropped(self):
        verdicts = fact_survival.parse_verdicts(
            reply({"id": "f1", "verdict": "kept", "quote": "$4,812"}, {"id": "q1", "verdict": "hardened", "quote": ""}),
            "No figures here.",
            ITEMS,
        )
        self.assertEqual(["dropped", "dropped"], [v["verdict"] for v in verdicts])
        self.assertTrue(all(v["unverified_quote"] for v in verdicts))

    def test_a_missing_item_counts_as_dropped_and_hardened_is_kept_apart(self):
        verdicts = fact_survival.parse_verdicts(
            reply({"id": "q1", "verdict": "hardened", "quote": "by Friday"}), "Due by Friday.", ITEMS
        )
        self.assertEqual(["dropped", "hardened"], [v["verdict"] for v in verdicts])

    def test_an_unknown_verdict_is_an_error_not_a_pass(self):
        with self.assertRaises(ValueError):
            fact_survival.parse_verdicts(reply({"id": "f1", "verdict": "fine", "quote": "x"}), "x", ITEMS)

    def test_the_judge_prompt_names_no_setup(self):
        prompt = fact_survival.judge_prompt("Some text.", ITEMS)
        for name in ("laconic", "humanizer", "caveman", "setup"):
            self.assertNotIn(name, prompt.casefold())


class JudgeRowsTest(unittest.TestCase):
    def rows(self):
        return [
            {"runner": "a", "case_id": "c", "trial": 1, "response": "GST is $4,812 and likely by Friday."},
            {"runner": "b", "case_id": "c", "trial": 1, "response": "Nothing."},
        ]

    def test_resumes_without_judging_a_row_twice(self):
        calls = []

        def call(prompt):
            calls.append(prompt)
            return reply(), 0.1

        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp, "scores.jsonl")
            fact_survival.judge_rows(self.rows(), {"c": ITEMS}, output, call, jobs=2)
            fact_survival.judge_rows(self.rows(), {"c": ITEMS}, output, call, jobs=2)
            self.assertEqual(2, len(calls))
            self.assertEqual(2, len(fact_survival.read_jsonl(output)))

    def test_stops_submitting_at_the_budget(self):
        rows = [dict(self.rows()[0], trial=n) for n in range(10)]
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp, "scores.jsonl")
            fact_survival.judge_rows(rows, {"c": ITEMS}, output, lambda _: (reply(), 1.0), jobs=1, budget_usd=3)
            self.assertEqual(3, len(fact_survival.read_jsonl(output)))

    def test_a_case_without_a_ledger_fails_before_any_call(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(ValueError):
                fact_survival.judge_rows(self.rows(), {}, Path(tmp, "s.jsonl"), lambda _: self.fail("called"))


class SummaryTest(unittest.TestCase):
    def test_rates_count_kept_only_and_gap_is_reference_minus_setup(self):
        def verdicts(*kinds_and_verdicts):
            return [{"kind": k, "verdict": v, "unverified_quote": False} for k, v in kinds_and_verdicts]

        scores = [
            {"setup": "a", "case_id": "c", "trial": 1, "verdicts": verdicts(("fact", "kept"), ("qualifier", "kept"))},
            {"setup": "b", "case_id": "c", "trial": 1, "verdicts": verdicts(("fact", "kept"), ("qualifier", "hardened"))},
        ]
        responses = [
            {"runner": "a", "case_id": "c", "response": "one two"},
            {"runner": "b", "case_id": "c", "response": "one two three four"},
        ]
        summary = fact_survival.summarise(scores, responses, {"c": "one two three four"}, "a", added=lambda s, r: 0)
        self.assertEqual(1.0, summary["a"]["qualifier_survival"])
        self.assertEqual(0.0, summary["b"]["qualifier_survival"])
        self.assertEqual(1, summary["b"]["hardened"])
        self.assertEqual(0.5, summary["a"]["length_share"])
        self.assertEqual(0.5, summary["b"]["reference_gap"]["mean"])


class CommittedDataTest(unittest.TestCase):
    def test_cases_are_valid_and_every_case_has_a_ledger(self):
        cases = run_evals.load_cases(ROOT / "evals" / "fact-cases.jsonl")
        self.assertEqual([], run_evals.validate_cases(cases))
        ledgers = fact_survival.load_ledgers(fact_survival.DEFAULT_LEDGERS)
        self.assertEqual({case["id"] for case in cases}, set(ledgers))

    def test_cases_match_their_sources(self):
        built = fact_survival.build_cases(fact_survival.read_jsonl(fact_survival.DEFAULT_SOURCES))
        self.assertEqual(built, fact_survival.read_jsonl(ROOT / "evals" / "fact-cases.jsonl"))


if __name__ == "__main__":
    unittest.main()
