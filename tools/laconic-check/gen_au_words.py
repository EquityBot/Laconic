#!/usr/bin/env python3
"""Generate src/us_words.txt: US spellings with their Australian form, from ESDB.

ESDB (the English Speller Database, formerly SCOWL, https://github.com/en-wl/wordlist)
tags each spelling of a word sense: A for American, D for Australian, B for British
(-ise), which stands for D in a group that has no D line. A pair is kept when:

- the US form is the preferred American spelling and the Australian form is the
  preferred Australian spelling of the same sense, and they differ;
- the sense is common (ESDB size 60 or below, the size its author has vetted);
- the US form is not an accepted Australian spelling of any sense (meter the device,
  practice the noun, program, tire the verb);
- the US form is not in KEEP, a reviewed list of ESDB pairing quirks and forms that
  are standard in Australian usage too;
- the Style Manual does not use the US form itself (focused), unless it names the form
  only to reject it (percent).

Run from the repository root with a local copy of ESDB's data/scowl-pre.txt:

    python tools/laconic-check/gen_au_words.py path/to/scowl-pre.txt
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
MANUAL = ROOT / "skills/laconic-prose/references/australian-government-style-manual.md"
OUT = ROOT / "tools/laconic-check/src/us_words.txt"
MAX_SIZE = 60
VARIANT = re.compile(r"^(?:[ABZCD_][.=?v~V\-@x]?|\+)$")
# US forms the manual mentions only to reject them; flag them although they appear in it.
MANUAL_REJECTS = {"percent", "toward", "acknowledgment", "judgment", "judgments"}
# US forms the linter must not flag, reviewed from the generated list on 19 October 2026.
KEEP = {
    # A dialog box is a computing term.
    "dialog", "dialogs",
    # ESDB pairs these with an unrelated word or the wrong sense (asses the donkeys).
    "asses", "cardie", "cardies", "ecus", "novene", "patine", "trabecule",
    # Both forms are standard in Australian usage, so the US form is not an error.
    "abettor", "abettors", "adapter", "adapters", "bused", "busing", "collectible",
    "collectibles", "guerrilla", "guerrillas", "hankie", "leaped", "myna", "mynas",
    "specialties", "specialty", "spilled", "unbeknownst", "veranda", "verandas",
    # Not spelling variants of one word.
    "oversize", "sweepstakes",
    # The suggested form is also a slur.
    "dike", "dikes", "fagot", "fagots",
}
# Australian science writes sulfur, following IUPAC.
KEEP_PREFIXES = ("sulf",)


def words_of(lemma: str, entries: str) -> "list[str | None]":
    """The lemma and its derived forms, in order; None marks an unusable slot."""
    out: "list[str | None]" = [] if lemma == "-" else [lemma]
    depth, item, items = 0, "", []
    for c in entries:
        depth += c == "("
        depth -= c == ")"
        if c == "," and depth == 0:
            items.append(item)
            item = ""
        else:
            item += c
    if item.strip():
        items.append(item)
    for item in items:
        item = item.strip()
        if item.startswith("("):
            item = item.strip("()").split("|")[0].strip()
            if ":" in item:
                out.append(None)
                continue
        out.append(item.rstrip("*-@~!†").lstrip("-@!") or None)
    return out


def inline_choices(entries: str) -> "list[dict[str, str]]":
    """Spelling choices inside one derived form, such as (A: labeled | B: labelled).

    Each choice maps a spelling code to its word; '' marks an unmarked form."""
    choices = []
    for item in re.findall(r"\(([^()]*)\)", entries):
        choice = {}
        for alt in item.split("|"):
            codes, _, word = alt.strip().rpartition(": ")
            word = word.rstrip("*-@~!†").lstrip("-@!")
            for code in codes.split() or [""]:
                choice.setdefault(code, word)
        choices.append(choice)
    return choices


def parse(line: str):
    """(variant tokens, words) for a line, or None for a line without variant info."""
    parts = line.split(": ", 1)
    if len(parts) < 2:
        return None
    sizes = re.match(r"\d+", parts[0])
    size = int(sizes.group()) if sizes else 99
    rest = parts[1]
    head, _, tail = rest.partition(": ")
    tokens = head.split()
    if not tokens or not all(VARIANT.match(t) for t in tokens):
        return [], size, rest
    return tokens, size, tail


def lemma_and_entries(text: str):
    lemma_info, _, entries = text.partition(": ")
    parts = []
    missing_closers = set()
    closers = {"<": ">", "{": "}", "(": ")"}
    index = 0
    while index < len(lemma_info):
        char = lemma_info[index]
        closer = closers.get(char)
        if closer and closer not in missing_closers:
            end = lemma_info.find(closer, index + 1)
            if end != -1:
                while parts and parts[-1].isspace():
                    parts.pop()
                index = end + 1
                continue
            # A failed search covers the remaining suffix; never scan it again.
            missing_closers.add(closer)
        parts.append(char)
        index += 1
    lemma = "".join(parts).strip()
    return lemma.lstrip("-@!") if lemma != "-" else "-", entries


def groups(path: pathlib.Path):
    group = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            if group:
                yield group
            group = []
        elif not line.startswith("#"):
            group.append(line.split(" #", 1)[0])
    if group:
        yield group


def main(source: str) -> None:
    pairs, au_valid = {}, set()
    for group in groups(pathlib.Path(source)):
        rows = [r for r in (parse(l) for l in group) if r]
        has_d = any(t.startswith("D") for tokens, _, _ in rows for t in tokens)
        by_key = {}
        for tokens, size, text in rows:
            if not has_d:
                tokens = ["D" + t[1:] if t.startswith("B") else t for t in tokens]
            lemma, entries = lemma_and_entries(text)
            forms = words_of(lemma, entries)
            if not tokens or any(t in ("D", "D.", "D=") for t in tokens):
                au_valid.update(w for w in forms if w and w.islower())
            for choice in inline_choices(entries):
                au_form = choice.get("D") or (None if "D" in choice else choice.get("B"))
                au_valid.update(w for w in (choice.get(""), au_form) if w and w.islower())
                us_form = choice.get("A")
                if size <= MAX_SIZE and us_form and au_form and us_form != au_form:
                    if us_form.islower() and au_form.islower() and us_form.isalpha():
                        pairs.setdefault(us_form, au_form)
            us = "A" in tokens
            au = "D" in tokens
            if us != au:
                by_key.setdefault("us" if us else "au", []).append((size, forms))
        for (size, us_forms), (_, au_forms) in zip(by_key.get("us", []), by_key.get("au", [])):
            if size > MAX_SIZE or len(us_forms) != len(au_forms):
                continue
            for us, au in zip(us_forms, au_forms):
                if us and au and us != au and us.islower() and au.islower() and us.isalpha():
                    pairs.setdefault(us, au)
    manual_words = set(re.findall(r"[a-z]+", MANUAL.read_text(encoding="utf-8").lower()))
    kept = {
        us: au
        for us, au in pairs.items()
        if us not in au_valid
        and us not in KEEP
        and not us.startswith(KEEP_PREFIXES)
        and (us not in manual_words or us in MANUAL_REJECTS)
        and len(us) >= 4
    }
    header = (
        "# US spelling -> Australian spelling, generated by gen_au_words.py from the English\n"
        "# Speller Database (https://github.com/en-wl/wordlist). Copyright 2000-2026 by Kevin\n"
        "# Atkinson; Australian data copyright 2016 by Benjamin Titze. Used under their\n"
        "# permissive notices, reproduced in us_words.COPYRIGHT. ESDB commit 1e5b7d3a72.\n"
    )
    OUT.write_text(header + "".join(f"{us} {au}\n" for us, au in sorted(kept.items())), encoding="utf-8", newline="\n")
    print(f"{len(pairs)} candidate pairs, {len(kept)} written to {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: gen_au_words.py path/to/scowl-pre.txt")
    main(sys.argv[1])
