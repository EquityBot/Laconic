"""Scores humanising responses with deterministic checkers, without a judge.

For each response to a case in humanise-cases.jsonl, it measures the rewrite
against the source text in the prompt:

- length: rewrite words as a share of source words;
- laconic-check lint findings and `preserve` drops (numbers, code, URLs and
  meaning words such as 'not' that the rewrite lost);
- optionally, the avoid-ai-writing detector's 0 to 100 score and finding count
  (--avoid <path to bin/avoid-ai-writing.js>), and dslop's finding count
  (--dslop-config <dslop.toml>, run through uvx).

The same checks run on the source texts, so each condition can be read against
where it started. Every checker has a house view; a finding is a candidate.

    python3 evals/humanise_metrics.py evals/results/responses.jsonl --avoid avoid/bin/avoid-ai-writing.js
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
import tomllib
from collections import defaultdict
from pathlib import Path
from statistics import mean
from typing import Any, Callable

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import run_evals  # noqa: E402

FINDING =re.compile(r"^\S.*:\d+:\d+: ", re.MULTILINE)
# Pattern findings carry a position ('<stdin>:1:17 em-dash'); whole-text rhythm
# metrics do not ('<stdin> sentence-length-cv=0.19 (threshold 0.30)').
DSLOP_FINDING = re.compile(r"<stdin>(?::\d+:\d+)? [a-z]")
DROPPED = re.compile(r"^dropped ", re.MULTILINE)


def source_text(prompt: str) -> str:
    """The text to humanise: everything after the instruction paragraph."""
    return prompt.split("\n\n", 1)[1]


def words(text: str) -> int:
    return len(text.split())


def _run(cmd: list[str], text: str | None = None, findings: re.Pattern[str] | None = None) -> str:
    """A checker's stdout. Exit 0 is success. With `findings`, exit 1 is success only
    when stdout holds a finding, because 1 also reports an operational failure (uvx
    exits 1 when it cannot launch). Any other status raises rather than counting as
    zero findings."""
    done = subprocess.run(cmd, input=text, capture_output=True, text=True, encoding="utf-8", timeout=120)
    found = findings is not None and done.returncode == 1 and findings.search(done.stdout)
    if done.returncode != 0 and not found:
        raise RuntimeError(f"{' '.join(cmd)} exited {done.returncode}: {done.stderr.strip()}")
    return done.stdout


def lint_count(text: str) -> int:
    return len(FINDING.findall(_run(["laconic-check", "lint", "--warn", "-"], text)))


def preserve_drops(source: str, rewrite: str) -> int:
    """Protected items the rewrite dropped; preserve exits 1 when it finds any."""
    with tempfile.TemporaryDirectory() as tmp:
        original, edited = Path(tmp, "original.md"), Path(tmp, "edited.md")
        original.write_text(source, encoding="utf-8")
        edited.write_text(rewrite, encoding="utf-8")
        out = _run(["laconic-check", "preserve", str(original), str(edited)], findings=DROPPED)
    return len(DROPPED.findall(out))


def checker_metrics(avoid: Path | None, dslop_config: Path | None) -> Callable[[str], dict[str, float]]:
    # dslop silently falls back to its defaults when its config file is missing or
    # is not valid TOML, so check both before measuring anything.
    if dslop_config:
        if not dslop_config.is_file():
            raise FileNotFoundError(f"dslop config not found: {dslop_config}")
        tomllib.loads(dslop_config.read_text(encoding="utf-8"))

    def measure(text: str) -> dict[str, float]:
        result: dict[str, float] = {"lint": lint_count(text)}
        if avoid:
            report = json.loads(_run(["node", str(avoid), "--context", "general"], text))
            result["avoid_score"] = report["score"]
            result["avoid_findings"] = len(report["issues"])
        if dslop_config:
            out = _run(["uvx", "dslop", "--config", str(dslop_config), "-"], text, findings=DSLOP_FINDING)
            result["dslop"] = len(DSLOP_FINDING.findall(out))
        return result

    return measure


def summarise(
    rows: list[dict[str, Any]],
    cases: dict[str, dict[str, Any]],
    measure: Callable[[str], dict[str, float]],
    drops: Callable[[str, str], int] = preserve_drops,
) -> dict[str, dict[str, float]]:
    """Mean metrics per condition, plus a 'source' row for the original texts."""
    per: dict[str, list[dict[str, float]]] = defaultdict(list)
    for case_id in sorted({row["case_id"] for row in rows}):
        source = source_text(cases[case_id]["prompt"])
        per["source"].append({**measure(source), "length": 1.0, "drops": 0})
    for row in rows:
        source = source_text(cases[row["case_id"]]["prompt"])
        rewrite = row["response"]
        per[row["condition"]].append({
            **measure(rewrite),
            "length": words(rewrite) / words(source),
            "drops": drops(source, rewrite),
        })
    return {
        condition: {"rows": len(items), **{key: round(mean(item[key] for item in items), 3) for key in items[0]}}
        for condition, items in sorted(per.items())
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("responses", type=Path)
    parser.add_argument("--cases", type=Path, default=ROOT / "evals" / "humanise-cases.jsonl")
    parser.add_argument("--avoid", type=Path, help="Path to avoid-ai-writing's bin/avoid-ai-writing.js")
    parser.add_argument("--dslop-config", type=Path, help="dslop.toml; runs dslop through uvx")
    args = parser.parse_args()
    cases = {case["id"]: case for case in run_evals.load_cases(args.cases)}
    rows = [row for row in run_evals.read_jsonl(args.responses) if row["case_id"] in cases]
    print(json.dumps(summarise(rows, cases, checker_metrics(args.avoid, args.dslop_config)), indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
