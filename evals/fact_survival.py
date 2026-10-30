#!/usr/bin/env python3
"""Measure whether rewrites keep each fact and qualifier in a source text.

Ledgers (`fact-ledgers.jsonl`) are written from the sources alone, before any
rewrite exists. A blind judge sees one rewrite and the ledger items, never the
setup's name, and must quote the rewrite for every item it calls kept. A quote
that is not in the rewrite counts as dropped, so a judge cannot invent support.
"""

import argparse
import json
import random
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from statistics import mean
from typing import Any, Callable, Iterable

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "evals"))
import humanise_metrics  # noqa: E402

DEFAULT_SOURCES = ROOT / "evals" / "humanise-cases.jsonl"
DEFAULT_LEDGERS = ROOT / "evals" / "fact-ledgers.jsonl"
INSTRUCTION = "Rewrite this text for a busy Australian reader. Reply with the rewritten text only."
JUDGE_COMMAND = [
    "claude", "--print", "--output-format", "json", "--no-session-persistence",
    "--strict-mcp-config", "--setting-sources", "", "--tools", "", "--model", "claude-sonnet-5-5",
]
VERDICTS = {"kept", "hardened", "dropped"}
SETUPS = {"laconic-prose", "laconic-session", "humanizer", "avoid", "stop-slop", "caveman", "concise-control", "no-skill"}
ADDED = re.compile(r"^added ", re.MULTILINE)


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def build_cases(sources: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """The same source texts under one instruction no setup is named in."""
    return [
        {
            "id": case["id"].replace("hum-", "fact-", 1),
            "category": "rewrite",
            "prompt": f"{INSTRUCTION}\n\n{humanise_metrics.source_text(case['prompt'])}",
            "risk": case["risk"],
            "criteria": ["Keeps every fact and qualifier in the ledger."],
        }
        for case in sources
    ]


def load_ledgers(path: Path) -> dict[str, list[dict[str, str]]]:
    ledgers = {}
    for row in read_jsonl(path):
        case_id = row["case_id"].replace("hum-", "fact-", 1)
        if case_id in ledgers:
            raise ValueError(f"duplicate ledger: {case_id}")
        for item in row["items"]:
            if item["kind"] not in {"fact", "qualifier"} or not item["text"].strip():
                raise ValueError(f"bad item in {case_id}: {item}")
        ledgers[case_id] = row["items"]
    return ledgers


def judge_prompt(source: str, rewrite: str, items: list[dict[str, str]]) -> str:
    listing = "\n".join(f"{item['id']} ({item['kind']}): {item['text']}" for item in items)
    return (
        "You check whether a rewritten text still carries specific claims. Judge only the "
        "text below; ignore its style and length.\n\n"
        "For each item return one verdict:\n"
        "- kept: the text carries the claim, in any wording, including its condition or hedge. "
        "'three' and '3' are the same.\n"
        "- hardened: the text carries the claim but states it more strongly or more widely "
        "than the item, because a hedge, condition, exception or limit is gone.\n"
        "- dropped: the text does not carry the claim.\n\n"
        "For kept and hardened, give `quote`: the shortest exact passage of the text that "
        "carries the claim, copied character for character. For dropped, give an empty quote.\n\n"
        "Also list under `invented` each exact passage of the text that states a material fact, "
        "figure, promise or experience the source does not support. Ignore rewording, tone and "
        "reasonable inference.\n\n"
        'Reply with JSON only: {"items": [{"id": "f1", "verdict": "kept", "quote": "..."}], "invented": ["..."]}\n\n'
        f"<source>\n{source}\n</source>\n\n<text>\n{rewrite}\n</text>\n\nItems:\n{listing}\n"
    )


def _normalise(text: str) -> str:
    text = text.translate({0x2018: "'", 0x2019: "'", 0x201c: '"', 0x201d: '"'})
    return " ".join(text.split()).casefold()


def parse_reply(reply: str, rewrite: str, items: list[dict[str, str]]) -> tuple[list[dict[str, Any]], list[str]]:
    """Verdicts and verified invented passages. An unverifiable quote or a missing item is dropped."""
    match = re.search(r"\{.*\}", reply, re.DOTALL)
    if not match:
        raise ValueError("judge reply holds no JSON")
    parsed = json.loads(match.group())
    given = {row.get("id"): row for row in parsed["items"]}
    body = _normalise(rewrite)
    verdicts = []
    for item in items:
        row = given.get(item["id"], {})
        verdict, quote = row.get("verdict", "dropped"), str(row.get("quote") or "")
        if verdict not in VERDICTS:
            raise ValueError(f"unknown verdict: {verdict}")
        unverified = verdict != "dropped" and (not quote.strip() or _normalise(quote) not in body)
        verdicts.append({
            "id": item["id"],
            "kind": item["kind"],
            "verdict": "dropped" if unverified else verdict,
            "judge_verdict": verdict,
            "unverified_quote": unverified,
            "quote": quote,
        })
    if not isinstance(parsed.get("invented"), list):
        raise ValueError("judge reply has no invented list")
    invented = list(dict.fromkeys(_normalise(q) for q in parsed["invented"] if isinstance(q, str) and q.strip()))
    return verdicts, [q for q in invented if q in body]


def _call_judge(prompt: str, command: list[str], timeout: float) -> tuple[str, float]:
    done = subprocess.run(command, input=prompt, capture_output=True, text=True, encoding="utf-8", timeout=timeout)
    if done.returncode != 0:
        raise RuntimeError(f"judge exited {done.returncode}: {done.stderr.strip()[:300]}")
    reply = json.loads(done.stdout)
    return reply["result"], float(reply.get("total_cost_usd") or 0)


def judge_rows(
    rows: Iterable[dict[str, Any]],
    ledgers: dict[str, list[dict[str, str]]],
    sources: dict[str, str],
    output: Path,
    call: Callable[[str], tuple[str, float]],
    jobs: int = 4,
    budget_usd: float = 12.0,
) -> float:
    """Judge rows not yet in `output`; resumable. Returns the cost of this run."""
    done = set()
    if output.exists():
        done = {(r["setup"], r["case_id"], r["trial"]) for r in read_jsonl(output)}
    todo = [r for r in rows if (r["runner"], r["case_id"], r["trial"]) not in done]
    missing = sorted({r["case_id"] for r in todo} - set(ledgers))
    if missing:
        raise ValueError(f"no ledger for: {missing}")
    random.Random(0).shuffle(todo)  # Spreads a budget stop across setups.

    def one(row: dict[str, Any]) -> dict[str, Any]:
        items = ledgers[row["case_id"]]
        cost = 0.0
        for attempt in range(3):
            reply, spent = call(judge_prompt(sources[row["case_id"]], row["response"], items))
            cost += spent
            try:
                verdicts, invented = parse_reply(reply, row["response"], items)
                break
            except (ValueError, KeyError, AttributeError, json.JSONDecodeError):
                if attempt == 2:
                    raise
        return {"setup": row["runner"], "case_id": row["case_id"], "trial": row["trial"], "cost": cost, "verdicts": verdicts, "invented": invented}

    spent = 0.0
    with ThreadPoolExecutor(jobs) as pool, output.open("a", encoding="utf-8") as sink:
        pending = iter(todo)
        running = {pool.submit(one, row) for row in _take(pending, jobs)}
        while running:
            finished = next(as_completed(running))
            running.remove(finished)
            result = finished.result()
            spent += result["cost"]
            sink.write(json.dumps(result, ensure_ascii=False) + "\n")
            sink.flush()
            if spent < budget_usd:
                running |= {pool.submit(one, row) for row in _take(pending, 1)}
    return spent


def calibrate(
    fixtures: list[dict[str, Any]],
    ledgers: dict[str, list[dict[str, str]]],
    sources: dict[str, str],
    call: Callable[[str], tuple[str, float]],
) -> dict[str, Any]:
    """The judge against hand-labelled rewrites. '@source' stands for a verbatim copy."""
    errors, compared, cost = [], 0, 0.0
    for fixture in fixtures:
        source = sources[fixture["case_id"]]
        rewrite = source if fixture["rewrite"] == "@source" else fixture["rewrite"]
        items = ledgers[fixture["case_id"]]
        reply, spent = call(judge_prompt(source, rewrite, items))
        cost += spent
        verdicts, invented = parse_reply(reply, rewrite, items)
        for verdict in verdicts:
            expected = fixture["expected"].get(verdict["id"], fixture["default"])
            compared += 1
            if verdict["verdict"] != expected:
                errors.append({"fixture": fixture["name"], "item": verdict["id"], "expected": expected, "got": verdict["verdict"]})
        compared += 1
        if bool(invented) != fixture["invented"]:
            errors.append({"fixture": fixture["name"], "item": "invented", "expected": fixture["invented"], "got": invented})
    return {"compared": compared, "agreement": 1 - len(errors) / compared, "errors": errors, "cost_usd": cost}


def total_spend(folder: Path) -> float:
    """Reported cost of every stage so far: overhead, generation and judging."""
    total = 0.0
    for path in [*sorted(folder.glob("run-*.jsonl")), folder / "overhead.jsonl", folder / "scores.jsonl"]:
        if path.exists():
            total += sum(float(row.get("cost_usd") or row.get("cost") or 0) for row in read_jsonl(path))
    return total


def _take(iterator: Any, count: int) -> list[Any]:
    taken = []
    for row in iterator:
        taken.append(row)
        if len(taken) == count:
            break
    return taken


def _added_count(source: str, rewrite: str) -> int:
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        original, edited = Path(tmp, "original.md"), Path(tmp, "edited.md")
        original.write_text(source, encoding="utf-8")
        edited.write_text(rewrite, encoding="utf-8")
        done = subprocess.run(
            ["laconic-check", "preserve", str(original), str(edited)],
            capture_output=True, text=True, encoding="utf-8", timeout=120,
        )
    if done.returncode not in (0, 1):
        raise RuntimeError(f"preserve exited {done.returncode}: {done.stderr.strip()}")
    return len(ADDED.findall(done.stdout))


def _rate(verdicts: list[dict[str, Any]], kind: str) -> float | None:
    chosen = [v for v in verdicts if v["kind"] == kind]
    return sum(v["verdict"] == "kept" for v in chosen) / len(chosen) if chosen else None


def paired_interval(
    a: dict[str, list[float]], b: dict[str, list[float]], rounds: int = 10000, seed: int = 0
) -> tuple[float, float, float]:
    """Mean a minus b, with a 95% bootstrap interval that resamples whole texts."""
    cases = sorted(set(a) & set(b))
    if not cases:
        raise ValueError("no shared cases")
    gaps = {case: mean(a[case]) - mean(b[case]) for case in cases}
    rng = random.Random(seed)
    draws = sorted(mean(gaps[rng.choice(cases)] for _ in cases) for _ in range(rounds))
    return mean(gaps.values()), draws[int(0.025 * rounds)], draws[int(0.975 * rounds)]


def summarise(
    scores: list[dict[str, Any]],
    responses: list[dict[str, Any]],
    sources: dict[str, str],
    reference: str,
    added: Callable[[str, str], int] = _added_count,
    trials: int = 2,
    setups: set[str] | None = None,
) -> dict[str, Any]:
    by_setup: dict[str, list[dict[str, Any]]] = {}
    for score in scores:
        by_setup.setdefault(score["setup"], []).append(score)
    if reference not in by_setup:
        raise ValueError(f"reference setup has no scores: {reference}")
    if setups is not None and set(by_setup) != setups:
        raise ValueError(f"setups differ from the plan: {sorted(set(by_setup) ^ setups)}")
    expected = sorted((case, trial) for case in sources for trial in range(1, trials + 1))
    for setup, rows in by_setup.items():
        scored = sorted((r["case_id"], r["trial"]) for r in rows)
        answered = sorted((r["case_id"], r["trial"]) for r in responses if r["runner"] == setup)
        if scored != expected or answered != expected:
            raise ValueError(f"incomplete, duplicate or unequal coverage: {setup}")
    summary: dict[str, Any] = {}
    per_case: dict[str, dict[str, list[float]]] = {}
    for setup, rows in sorted(by_setup.items()):
        verdicts = [v for row in rows for v in row["verdicts"]]
        answers = [r for r in responses if r["runner"] == setup]
        cases: dict[str, list[float]] = {}
        for row in rows:
            cases.setdefault(row["case_id"], []).append(
                sum(v["verdict"] == "kept" for v in row["verdicts"]) / len(row["verdicts"])
            )
        per_case[setup] = cases
        summary[setup] = {
            "texts": len(rows),
            "survival_by_text": mean(mean(values) for values in cases.values()),
            "fact_survival": _rate(verdicts, "fact"),
            "qualifier_survival": _rate(verdicts, "qualifier"),
            "hardened": sum(v["verdict"] == "hardened" for v in verdicts),
            "dropped": sum(v["verdict"] == "dropped" for v in verdicts),
            "unverified_quotes": sum(v["unverified_quote"] for v in verdicts),
            "items": len(verdicts),
            "length_share": mean(
                humanise_metrics.words(r["response"]) / humanise_metrics.words(sources[r["case_id"]]) for r in answers
            ),
            "invented_per_text": mean(len(row["invented"]) for row in rows),
            "added_per_text": mean(added(sources[r["case_id"]], r["response"]) for r in answers),
            "generation_cost": sum(float(r.get("cost_usd") or 0) for r in answers),
        }
    for setup, cases in per_case.items():
        if setup != reference:
            gap, low, high = paired_interval(per_case[reference], cases)
            summary[setup]["reference_gap"] = {"mean": gap, "low": low, "high": high}
    return summary


def _sources(path: Path) -> dict[str, str]:
    return {case["id"]: humanise_metrics.source_text(case["prompt"]) for case in read_jsonl(path)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("build-cases")
    build.add_argument("--sources", type=Path, default=DEFAULT_SOURCES)
    build.add_argument("--output", type=Path, required=True)
    judge = commands.add_parser("judge")
    judge.add_argument("responses", type=Path)
    judge.add_argument("--ledgers", type=Path, default=DEFAULT_LEDGERS)
    judge.add_argument("--output", type=Path, required=True)
    judge.add_argument("--jobs", type=int, default=4)
    judge.add_argument("--cases", type=Path, default=ROOT / "evals" / "fact-cases.jsonl")
    judge.add_argument("--budget-usd", type=float, default=12.0)
    judge.add_argument("--call-timeout", type=float, default=300.0)
    calibration = commands.add_parser("calibrate")
    calibration.add_argument("--fixtures", type=Path, default=ROOT / "evals" / "fact-calibration.jsonl")
    calibration.add_argument("--ledgers", type=Path, default=DEFAULT_LEDGERS)
    calibration.add_argument("--cases", type=Path, default=ROOT / "evals" / "fact-cases.jsonl")
    calibration.add_argument("--overhead", type=Path, help="Append this run's cost to this file")
    spend = commands.add_parser("spend", help="Exit 1 once reported cost reaches the cap")
    spend.add_argument("folder", type=Path)
    spend.add_argument("--cap", type=float, required=True)
    report = commands.add_parser("report")
    report.add_argument("scores", type=Path)
    report.add_argument("responses", type=Path)
    report.add_argument("--cases", type=Path, required=True)
    report.add_argument("--reference", required=True)
    report.add_argument("--trials", type=int, default=2)
    args = parser.parse_args()

    if args.command == "build-cases":
        lines = [json.dumps(case, ensure_ascii=False) for case in build_cases(read_jsonl(args.sources))]
        args.output.write_text("\n".join(lines) + "\n", encoding="utf-8")
    elif args.command == "judge":
        spent = judge_rows(
            read_jsonl(args.responses), load_ledgers(args.ledgers), _sources(args.cases), args.output,
            lambda prompt: _call_judge(prompt, JUDGE_COMMAND, args.call_timeout),
            args.jobs, args.budget_usd,
        )
        print(f"judge cost: ${spent:.2f}")
    elif args.command == "calibrate":
        result = calibrate(
            read_jsonl(args.fixtures), load_ledgers(args.ledgers), _sources(args.cases),
            lambda prompt: _call_judge(prompt, JUDGE_COMMAND, 300.0),
        )
        print(json.dumps(result, indent=2))
        if args.overhead:
            with args.overhead.open("a", encoding="utf-8") as sink:
                sink.write(json.dumps({"stage": "calibrate", "cost_usd": result["cost_usd"]}) + "\n")
    elif args.command == "spend":
        total = total_spend(args.folder)
        print(f"reported cost: ${total:.2f} of ${args.cap:.2f}")
        return 1 if total >= args.cap else 0
    else:
        summary = summarise(
            read_jsonl(args.scores), read_jsonl(args.responses), _sources(args.cases), args.reference, trials=args.trials, setups=SETUPS
        )
        print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
