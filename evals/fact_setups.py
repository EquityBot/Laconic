#!/usr/bin/env python3
"""Fetch rival skills at pinned commits and write the fact-survival runner config.

Run: python -I evals/fact_setups.py evals/results/<new folder>
Then: python -I evals/fact_setups.py --preflight evals/results/<that folder>
"""

import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RIVALS = {
    "humanizer": ("blader/humanizer", "0ef9fe5bc3572734fd3ae6c448fe00a30cd5dcf0"),
    "avoid": ("conorbronsdon/avoid-ai-writing", "9cd07215982eca9736e14caaa63241c0264856f3"),
    "stop-slop": ("hardikpandya/stop-slop", "8da1f030185bdfe8471220585162991eaeb970e9"),
    "caveman": ("JuliusBrussee/caveman", "9b68f7ed12b05e7efaff18e65c5a3fc2849fd0a4"),
}
SKILLS = {
    "laconic-prose": "laconic-prose", "laconic-session": "laconic", "humanizer": "humanizer",
    "avoid": "avoid-ai-writing", "stop-slop": "stop-slop", "caveman": "caveman",
}
BASE = [
    "claude", "--print", "--output-format", "json", "--no-session-persistence",
    "--strict-mcp-config", "--setting-sources", "", "--model", "claude-opus-5-5",
]


def fetch(src: Path, name: str, repo: str, commit: str) -> Path:
    target = src / name
    if not target.exists():
        subprocess.run(["git", "clone", "-q", "--filter=blob:none", "--no-checkout", f"https://github.com/{repo}", str(target)], check=True)
        subprocess.run(["git", "-C", str(target), "-c", "core.longpaths=true", "checkout", "-q", commit], check=True)
    return target


def plugin(setups: Path, name: str, skill: str, path: Path) -> Path:
    """A one-skill plugin, so a setup can reach nothing but its own skill."""
    folder = setups / f"{name}-plugin"
    (folder / ".claude-plugin").mkdir(parents=True)
    (folder / ".claude-plugin" / "plugin.json").write_text(json.dumps({"name": name, "version": "0.0.0"}), encoding="utf-8")
    shutil.copytree(path, folder / "skills" / skill, ignore=shutil.ignore_patterns(".git"))
    return folder


def runners(setups: Path, skills: dict[str, tuple[Path, str]]) -> dict[str, dict]:
    config = {}
    for name, (folder, skill) in skills.items():
        prompt = setups / f"{name}-system.md"
        prompt.write_text(f"For any request to rewrite, humanise or edit text, use the {skill} skill.\n", encoding="utf-8")
        config[name] = BASE + [
            "--plugin-dir", str(folder), "--add-dir", str(folder),
            "--tools", "Skill,Read", "--allowedTools", "Skill",
            "--append-system-prompt-file", str(prompt),
        ]
    config["concise-control"] = BASE + ["--tools", "", "--append-system-prompt-file", str(ROOT / "evals" / "controls" / "terse.md")]
    config["no-skill"] = BASE + ["--tools", ""]
    return {name: {"command": command, "response_format": "claude-json", "budget_flag": "--max-budget-usd"} for name, command in config.items()}


def skills_used(stream: str) -> set[str]:
    """Skill names called in a Claude stream-json transcript."""
    used = set()
    for line in stream.splitlines():
        event = json.loads(line) if line.startswith("{") else {}
        for block in (event.get("message") or {}).get("content") or []:
            if isinstance(block, dict) and block.get("type") == "tool_use" and block.get("name") == "Skill":
                used.add(str(block["input"].get("skill", "")).split(":")[-1])
    return used


def preflight(here: Path) -> int:
    """One call per skill setup, proving it loads its own skill. Prints each reply to read."""
    config = json.loads((here / "runners.json").read_text(encoding="utf-8"))
    prompt = json.loads((ROOT / "evals" / "fact-cases.jsonl").read_text(encoding="utf-8").splitlines()[0])["prompt"]
    failures, cost = 0, 0.0
    for name, runner in config.items():
        if "--plugin-dir" not in runner["command"]:
            continue
        command = [part for arg in runner["command"] for part in (["stream-json", "--verbose"] if arg == "json" else [arg])]
        done = subprocess.run(command, input=prompt, capture_output=True, text=True, encoding="utf-8", timeout=600)
        used = skills_used(done.stdout)
        failures += SKILLS[name] not in used
        results = [json.loads(line) for line in done.stdout.splitlines() if line.startswith('{"type":"result"')]
        cost += sum(float(result.get("total_cost_usd") or 0) for result in results)
        print(f"{name}: wanted {SKILLS[name]}, called {sorted(used)}\n{(results or [{}])[0].get('result', '')[:400]}\n")
    with (here / "overhead.jsonl").open("a", encoding="utf-8") as sink:
        sink.write(json.dumps({"stage": "preflight", "cost_usd": cost}) + "\n")
    return 1 if failures else 0


def main() -> int:
    if len(sys.argv) == 3 and sys.argv[1] == "--preflight":
        return preflight(Path(sys.argv[2]).resolve())
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    here = Path(sys.argv[1]).resolve()
    src, setups = here / "src", here / "setups"
    shutil.rmtree(setups, ignore_errors=True)
    setups.mkdir(parents=True)
    src.mkdir(exist_ok=True)
    rival = {name: fetch(src, name, *pin) for name, pin in RIVALS.items()}
    skills = {
        "laconic-prose": (plugin(setups, "laconic-prose", "laconic-prose", ROOT / "skills" / "laconic-prose"), "laconic-prose"),
        "laconic-session": (plugin(setups, "laconic-session", "laconic", ROOT / "skills" / "laconic"), "laconic"),
        "humanizer": (plugin(setups, "humanizer", "humanizer", rival["humanizer"]), "humanizer"),
        "avoid": (rival["avoid"] / "plugins" / "avoid-ai-writing", "avoid-ai-writing"),
        "stop-slop": (plugin(setups, "stop-slop", "stop-slop", rival["stop-slop"]), "stop-slop"),
        "caveman": (plugin(setups, "caveman", "caveman", rival["caveman"] / "plugins" / "caveman" / "skills" / "caveman"), "caveman"),
    }
    config = runners(setups, skills)
    (here / "runners.json").write_text(json.dumps(config, indent=2), encoding="utf-8")
    print("\n".join(config))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
