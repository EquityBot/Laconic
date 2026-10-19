//! Seeded-fault evaluation of `lint`, after snifftest's design
//! (https://github.com/DanRWilloughby/snifftest): a corpus of clean Australian prose,
//! including legitimate forms that look like faults, must produce no findings, and
//! planting one known fault in it must produce exactly that rule, once.

use crate::lint::lint;

/// Clean prose. Each sentence is correct house style; several are near misses.
const CLEAN: &[&str] = &[
    "The team organised 3 workshops and analysed the results in October.",
    "Bring the logs, metrics and traces to the review.",
    "The industries most affected are retail trade, wholesale trade, and accommodation and food services.",
    "If the build fails, rerun it, and report the error.",
    "We funded forty-five places after the go-ahead.",
    "Doctors practise medicine; the practice opens at 9:30 am on 19 October 2026.",
    "Read the meter before you change the program in the dialog box.",
    "The Labor senator spoke for one minute about the 2026-27 budget.",
    "Prices rose 12% whilst demand fell amongst small businesses, and stayed low for a while.",
    "She said 'the licence is current' and left for the centre.",
    "Two reviewers checked the catalogue, for example the colour plates.",
    "It may 5 times fail before it passes.",
    "The patterns credit [humanizer](https://github.com/blader/humanizer) and [the house guide](https://example.com/guide).",
];

/// One planted fault: the rule it must trigger, a clean fragment and its faulty form.
const SEEDS: &[(&str, &str, &str)] = &[
    ("us-spelling", "organised", "organized"),
    ("us-spelling", "colour plates", "color plates"),
    ("us-spelling", "the centre", "the center"),
    ("us-spelling", "catalogue", "catalog"),
    ("us-spelling", "analysed", "analyzed"),
    ("us-spelling", "the review", "the skeptical review"),
    ("us-spelling", "Prices rose 12%", "Prices rose 12 percent"),
    (
        "us-spelling",
        "[the house guide]",
        "[the house color guide]",
    ),
    (
        "em-dash",
        "after the go-ahead.",
        "after the go-ahead \u{2014} finally.",
    ),
    (
        "double-quotes",
        "'the licence is current'",
        "\"the licence is current\"",
    ),
    ("serial-comma", "metrics and traces", "metrics, and traces"),
    (
        "number-word",
        "organised 3 workshops",
        "organised three workshops",
    ),
    (
        "closer",
        "to the review.",
        "to the review. Hope this helps!",
    ),
    (
        "stock-phrase",
        "to the review.",
        "to the review. Bottom line: it works.",
    ),
    ("word-choice", "whilst demand", "while demand"),
    ("word-choice", "amongst small", "among small"),
    ("date", "on 19 October 2026", "on 19th October 2026"),
    ("date", "on 19 October 2026", "on 19 October, 2026"),
    ("date", "on 19 October 2026", "on October 19"),
    ("time", "at 9:30 am", "at 9.30am"),
    ("time", "at 9:30 am", "at 9:30 a.m."),
    ("latin", "for example the colour", "e.g. the colour"),
    (
        "residue",
        "after the go-ahead.",
        "after the go-ahead.\u{200b}",
    ),
    (
        "residue",
        "https://example.com/guide",
        "https://example.com/guide?utm_source=chatgpt.com",
    ),
];

fn corpus() -> String {
    CLEAN.join("\n\n")
}

fn rules(text: &str) -> Vec<&'static str> {
    lint(text).into_iter().map(|f| f.rule).collect()
}

#[test]
fn clean_corpus_has_no_findings() {
    for sentence in CLEAN {
        assert!(
            rules(sentence).is_empty(),
            "false finding in {sentence:?}: {:?}",
            lint(sentence)
        );
    }
}

#[test]
fn every_seeded_fault_is_caught_exactly_once() {
    let clean = corpus();
    let mut failures = Vec::new();
    for &(rule, from, to) in SEEDS {
        assert_eq!(
            clean.matches(from).count(),
            1,
            "seed fragment {from:?} must occur once"
        );
        let found = rules(&clean.replacen(from, to, 1));
        if found != [rule] {
            failures.push(format!("{rule}: {to:?} gave {found:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} seeds missed or over-flagged:\n{}",
        failures.len(),
        SEEDS.len(),
        failures.join("\n")
    );
}

/// Findings on the scraped Style Manual, about 208,700 words of human Australian
/// Government prose. Many are the manual's own examples of forms to avoid or its
/// 'while' and 'among', which the house style overrides. The ceiling only moves down:
/// a rule change that raises it adds false findings on human text.
const STYLE_MANUAL_CEILING: usize = 83;

#[test]
fn human_corpus_findings_stay_under_the_ceiling() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../skills/laconic-prose/references/australian-government-style-manual.md"
    );
    let text = std::fs::read_to_string(path).expect("Style Manual copy");
    let found = lint(&text).len();
    assert!(
        found <= STYLE_MANUAL_CEILING,
        "{found} findings on the Style Manual, ceiling {STYLE_MANUAL_CEILING}"
    );
}

#[test]
fn every_rule_has_a_seed() {
    for rule in [
        "us-spelling",
        "em-dash",
        "double-quotes",
        "serial-comma",
        "number-word",
        "closer",
        "stock-phrase",
        "word-choice",
        "date",
        "time",
        "latin",
        "residue",
    ] {
        assert!(
            SEEDS.iter().any(|(r, _, _)| *r == rule),
            "no seed for {rule}"
        );
    }
}
