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


def reply(*rows, invented=()):
    return json.dumps({"items": list(rows), "invented": list(invented)})


def _verdicts(text, rewrite, items):
    return fact_survival.parse_reply(text, rewrite, items)[0]


class ParseVerdictsTest(unittest.TestCase):
    def test_keeps_a_verified_quote_across_case_quotes_and_wrapping(self):
        rewrite = "GST payable:\n$4,812. It’s likely by Friday."
        verdicts = _verdicts(
            reply(
                {"id": "f1", "verdict": "kept", "quote": "gst payable: $4,812"},
                {"id": "q1", "verdict": "kept", "quote": "it's likely by friday"},
            ),
            rewrite,
            ITEMS,
        )
        self.assertEqual(["kept", "kept"], [v["verdict"] for v in verdicts])

    def test_a_quote_not_in_the_rewrite_counts_as_dropped(self):
        verdicts = _verdicts(
            reply({"id": "f1", "verdict": "kept", "quote": "$4,812"}, {"id": "q1", "verdict": "hardened", "quote": ""}),
            "No figures here.",
            ITEMS,
        )
        self.assertEqual(["dropped", "dropped"], [v["verdict"] for v in verdicts])
        self.assertTrue(all(v["unverified_quote"] for v in verdicts))

    def test_a_missing_item_counts_as_dropped_and_hardened_is_kept_apart(self):
        verdicts = _verdicts(
            reply({"id": "q1", "verdict": "hardened", "quote": "by Friday"}), "Due by Friday.", ITEMS
        )
        self.assertEqual(["dropped", "hardened"], [v["verdict"] for v in verdicts])

    def test_an_unknown_verdict_is_an_error_not_a_pass(self):
        with self.assertRaises(ValueError):
            _verdicts(reply({"id": "f1", "verdict": "fine", "quote": "x"}), "x", ITEMS)

    def test_the_judge_prompt_names_no_setup(self):
        prompt = fact_survival.judge_prompt("Source.", "Some text.", ITEMS)
        for name in ("laconic", "humanizer", "caveman", "setup"):
            self.assertNotIn(name, prompt.casefold())


class InventedTest(unittest.TestCase):
    def test_only_invented_passages_found_in_the_rewrite_count(self):
        _, invented = fact_survival.parse_reply(reply(invented=["costs $9", "not there", ""]), "It costs $9.", ITEMS)
        self.assertEqual(["costs $9"], invented)

    def test_repeated_invented_passages_count_once(self):
        _, invented = fact_survival.parse_reply(reply(invented=["costs $9", "Costs  $9"]), "It costs $9.", ITEMS)
        self.assertEqual(["costs $9"], invented)

    def test_a_reply_without_an_invented_list_is_an_error_not_a_zero(self):
        with self.assertRaises(ValueError):
            fact_survival.parse_reply(json.dumps({"items": []}), "Text.", ITEMS)


class CalibrateTest(unittest.TestCase):
    FIXTURES = [{"name": "copy", "case_id": "c", "rewrite": "@source", "default": "kept", "expected": {}, "invented": False}]

    def test_a_verbatim_copy_reaches_the_judge_and_disagreements_are_counted(self):
        prompts = []

        def call(prompt):
            prompts.append(prompt)
            return reply({"id": "f1", "verdict": "kept", "quote": "GST"}, {"id": "q1", "verdict": "dropped", "quote": ""}), 0

        result = fact_survival.calibrate(self.FIXTURES, {"c": ITEMS}, {"c": "GST is due."}, call)
        self.assertIn("<text>\nGST is due.\n</text>", prompts[0])
        self.assertEqual(3, result["compared"])
        self.assertEqual([("q1", "kept", "dropped")], [(e["item"], e["expected"], e["got"]) for e in result["errors"]])


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
            fact_survival.judge_rows(self.rows(), {"c": ITEMS}, {"c": "Source."}, output, call, jobs=2)
            fact_survival.judge_rows(self.rows(), {"c": ITEMS}, {"c": "Source."}, output, call, jobs=2)
            self.assertEqual(2, len(calls))
            self.assertEqual(2, len(fact_survival.read_jsonl(output)))

    def test_a_retried_judge_call_still_counts_its_cost(self):
        replies = iter([("not json", 0.5), (reply(), 0.25)])
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp, "scores.jsonl")
            spent = fact_survival.judge_rows(self.rows()[:1], {"c": ITEMS}, {"c": "Source."}, output, lambda _: next(replies))
            self.assertEqual(0.75, spent)
            self.assertEqual(0.75, fact_survival.read_jsonl(output)[0]["cost"])

    def test_stops_submitting_at_the_budget(self):
        rows = [dict(self.rows()[0], trial=n) for n in range(10)]
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp, "scores.jsonl")
            fact_survival.judge_rows(rows, {"c": ITEMS}, {"c": "Source."}, output, lambda _: (reply(), 1.0), jobs=1, budget_usd=3)
            self.assertEqual(3, len(fact_survival.read_jsonl(output)))

    def test_a_case_without_a_ledger_fails_before_any_call(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(ValueError):
                fact_survival.judge_rows(self.rows(), {}, {}, Path(tmp, "s.jsonl"), lambda _: self.fail("called"))


class SummaryTest(unittest.TestCase):
    @staticmethod
    def marks(*kinds_and_verdicts):
        return [{"kind": k, "verdict": v, "unverified_quote": False} for k, v in kinds_and_verdicts]

    def test_unequal_coverage_is_an_error_not_a_silent_intersection(self):
        scores = [
            {"setup": "a", "case_id": "c", "trial": 1, "verdicts": self.marks(("fact", "kept"))},
            {"setup": "a", "case_id": "d", "trial": 1, "verdicts": self.marks(("fact", "kept"))},
            {"setup": "b", "case_id": "c", "trial": 1, "verdicts": self.marks(("fact", "kept"))},
        ]
        responses = [{"runner": s["setup"], "case_id": s["case_id"], "trial": 1, "response": "x"} for s in scores]
        with self.assertRaises(ValueError):
            fact_survival.summarise(scores, responses, {"c": "x", "d": "x"}, "a", added=lambda s, r: 0)

    def test_a_setup_missing_from_the_plan_is_an_error(self):
        one = {"setup": "a", "case_id": "c", "trial": 1, "verdicts": self.marks(("fact", "kept")), "invented": []}
        answer = {"runner": "a", "case_id": "c", "trial": 1, "response": "x"}
        with self.assertRaises(ValueError):
            fact_survival.summarise([one], [answer], {"c": "x"}, "a", added=lambda s, r: 0, trials=1, setups={"a", "b"})

    def test_a_duplicate_row_or_an_unrun_trial_is_an_error(self):
        one = {"setup": "a", "case_id": "c", "trial": 1, "verdicts": self.marks(("fact", "kept")), "invented": []}
        answer = {"runner": "a", "case_id": "c", "trial": 1, "response": "x"}
        for scores, responses, trials in (([one, one], [answer, answer], 1), ([one], [answer], 2)):
            with self.assertRaises(ValueError):
                fact_survival.summarise(scores, responses, {"c": "x"}, "a", added=lambda s, r: 0, trials=trials)

    def test_rates_count_kept_only_and_gap_is_reference_minus_setup(self):
        verdicts = self.marks
        scores = [
            {"setup": "a", "case_id": "c", "trial": 1, "verdicts": verdicts(("fact", "kept"), ("qualifier", "kept")), "invented": ["x"]},
            {"setup": "b", "case_id": "c", "trial": 1, "verdicts": verdicts(("fact", "kept"), ("qualifier", "hardened")), "invented": []},
        ]
        responses = [
            {"runner": "a", "case_id": "c", "trial": 1, "response": "one two"},
            {"runner": "b", "case_id": "c", "trial": 1, "response": "one two three four"},
        ]
        summary = fact_survival.summarise(scores, responses, {"c": "one two three four"}, "a", added=lambda s, r: 0, trials=1)
        self.assertEqual(1.0, summary["a"]["qualifier_survival"])
        self.assertEqual(0.0, summary["b"]["qualifier_survival"])
        self.assertEqual(1, summary["b"]["hardened"])
        self.assertEqual(0.5, summary["a"]["length_share"])
        self.assertEqual(0.5, summary["b"]["reference_gap"]["mean"])
        self.assertEqual(0.5, summary["a"]["survival_by_text"] - summary["b"]["survival_by_text"])
        self.assertEqual(1, summary["a"]["invented_per_text"])


class SpendTest(unittest.TestCase):
    def test_counts_generation_overhead_and_judging(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            (folder / "run-a.jsonl").write_text('{"cost_usd": 1.5}\n{"cost_usd": null}\n', encoding="utf-8")
            (folder / "overhead.jsonl").write_text('{"stage": "preflight", "cost_usd": 0.5}\n', encoding="utf-8")
            (folder / "scores.jsonl").write_text('{"cost": 2.0}\n', encoding="utf-8")
            (folder / "responses.jsonl").write_text('{"cost_usd": 99}\n', encoding="utf-8")
            self.assertEqual(4.0, fact_survival.total_spend(folder))


class CommittedDataTest(unittest.TestCase):
    def test_cases_are_valid_and_every_case_has_a_ledger(self):
        cases = run_evals.load_cases(ROOT / "evals" / "fact-cases.jsonl")
        self.assertEqual([], run_evals.validate_cases(cases))
        ledgers = fact_survival.load_ledgers(fact_survival.DEFAULT_LEDGERS)
        self.assertEqual({case["id"] for case in cases}, set(ledgers))

    def test_calibration_fixtures_name_real_cases_items_and_verdicts(self):
        ledgers = fact_survival.load_ledgers(fact_survival.DEFAULT_LEDGERS)
        fixtures = fact_survival.read_jsonl(ROOT / "evals" / "fact-calibration.jsonl")
        self.assertGreaterEqual(len(fixtures), 6)
        for fixture in fixtures:
            ids = {item["id"] for item in ledgers[fixture["case_id"]]}
            self.assertLessEqual(set(fixture["expected"]), ids)
            self.assertLessEqual({fixture["default"], *fixture["expected"].values()}, fact_survival.VERDICTS)

    def test_cases_match_their_sources(self):
        built = fact_survival.build_cases(fact_survival.read_jsonl(fact_survival.DEFAULT_SOURCES))
        self.assertEqual(built, fact_survival.read_jsonl(ROOT / "evals" / "fact-cases.jsonl"))


if __name__ == "__main__":
    unittest.main()
