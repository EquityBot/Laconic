#!/usr/bin/env python3
"""Copy the compact rules in rules/laconic.md into every always-on snippet in INSTALL.md.

Run after editing rules/laconic.md. tests/test_rules_sync.py fails when they differ.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
RULES = ROOT / "rules" / "laconic.md"
INSTALL = ROOT / "INSTALL.md"
# An always-on snippet is a fenced Markdown block that starts with this heading.
SNIPPET = re.compile(r"(```markdown\n## Output style\n\n)(.*?)(\n```)", re.S)


def rules_body() -> str:
    text = RULES.read_text(encoding="utf-8").replace("\r\n", "\n")
    return text.split("\n", 1)[1].strip()


def synced(install: str, body: str) -> str:
    return SNIPPET.sub(lambda m: m.group(1) + body + m.group(3), install)


def main() -> int:
    install = INSTALL.read_text(encoding="utf-8")
    updated = synced(install, rules_body())
    if updated != install:
        INSTALL.write_text(updated, encoding="utf-8", newline="")
        print("INSTALL.md snippets updated")
    return 0


if __name__ == "__main__":
    sys.exit(main())
