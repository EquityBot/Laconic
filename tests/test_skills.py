"""Structural checks for the two Laconic skills and their references."""

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
SKILLS = ROOT / "skills"
LINK = re.compile(r"\]\(([^)\s#]+)(?:#[^)]*)?\)")
SCRAPE = "australian-government-style-manual.md"


def frontmatter(path: pathlib.Path) -> str:
    text = path.read_text(encoding="utf-8").replace("\r\n", "\n")
    return text.split("---\n", 2)[1]


class SkillStructureTest(unittest.TestCase):
    def test_skill_names_match_their_folders(self):
        for name in ("laconic", "laconic-prose"):
            with self.subTest(skill=name):
                self.assertIn(f"name: {name}\n", frontmatter(SKILLS / name / "SKILL.md"))

    def test_session_mode_is_explicit_and_prose_skill_is_model_invocable(self):
        self.assertIn("disable-model-invocation: true", frontmatter(SKILLS / "laconic" / "SKILL.md"))
        self.assertNotIn("disable-model-invocation", frontmatter(SKILLS / "laconic-prose" / "SKILL.md"))

    def test_prose_skill_does_not_import_session_persistence(self):
        text = (SKILLS / "laconic-prose" / "SKILL.md").read_text(encoding="utf-8")
        self.assertNotIn("../laconic/", text)
        self.assertNotIn("stop laconic", text)

    def test_relative_links_resolve(self):
        for path in SKILLS.rglob("*.md"):
            if path.name == SCRAPE:
                continue
            text = re.sub(r"```.*?```", "", path.read_text(encoding="utf-8"), flags=re.S)
            for target in LINK.findall(text):
                if re.match(r"[a-z]+:", target):
                    continue
                with self.subTest(file=str(path.relative_to(ROOT)), link=target):
                    self.assertTrue((path.parent / target).exists())

    def test_no_em_dashes_in_skill_files_or_compact_rules(self):
        for path in [SKILLS / "laconic" / "SKILL.md", SKILLS / "laconic-prose" / "SKILL.md", ROOT / "rules" / "laconic.md"]:
            with self.subTest(file=path.name):
                self.assertNotIn("\u2014", path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
