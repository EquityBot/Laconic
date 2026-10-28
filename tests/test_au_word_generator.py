"""Dictionary annotations must not turn local generation into an unbounded scan."""

import importlib.util
import pathlib
import subprocess
import sys
import unittest


GENERATOR = pathlib.Path(__file__).resolve().parents[1] / "tools/laconic-check/gen_au_words.py"
SPEC = importlib.util.spec_from_file_location("gen_au_words", GENERATOR)
generator = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(generator)


class AustralianWordGeneratorTest(unittest.TestCase):
    def test_annotation_removal_preserves_lemma_and_entries(self):
        cases = {
            "word <n> {tag} (note): forms": ("word", "forms"),
            "-@!word <n>": ("word", ""),
            "-: forms": ("-", "forms"),
            "<n> word: a: b": ("word", "a: b"),
            "Croft Manor": ("Croft Manor", ""),
            "a (b) c": ("a c", ""),
            "a (b) (c)d": ("ad", ""),
            "<outer<inner>>word": (">word", ""),
            "x \t<missing {tag}: forms": ("x \t<missing", "forms"),
            "x\u2003<tag>y": ("xy", ""),
            "word <missing": ("word <missing", ""),
            "word [kept]": ("word [kept]", ""),
        }
        for source, expected in cases.items():
            with self.subTest(source=source):
                self.assertEqual(expected, generator.lemma_and_entries(source))

    def test_long_non_annotations_finish_within_deadline(self):
        # Run separately so a regression cannot hang the test runner itself.
        script = (
            "import importlib.util, sys; "
            "spec = importlib.util.spec_from_file_location('generator', sys.argv[1]); "
            "module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module); "
            "text = 'Croft' + sys.argv[2] * 200000 + 'Manor'; "
            "assert module.lemma_and_entries(text) == (text, '')"
        )
        for repeated in (" ", "<", "({<"):
            with self.subTest(repeated=repeated):
                result = subprocess.run(
                    [sys.executable, "-c", script, str(GENERATOR), repeated],
                    stdin=subprocess.DEVNULL, capture_output=True, text=True,
                    timeout=5, check=False,
                )
                self.assertEqual(0, result.returncode, result.stderr)


if __name__ == "__main__":
    unittest.main()
