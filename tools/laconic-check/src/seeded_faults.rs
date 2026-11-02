//! Seeded-fault evaluation of `lint`, after snifftest's design
//! (https://github.com/DanRWilloughby/snifftest): a corpus of clean Australian prose,
//! including legitimate forms that look like faults, must produce no findings, and
//! planting one known fault in it must produce exactly that rule, once.

use crate::lint::{ADVISORY, lint};

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
    "Andy wrote to the state senate every week for 6 years.",
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
    (
        "staged-contrast",
        "Bring the logs, metrics and traces to the review.",
        "It's not a log review, it's a trace review.",
    ),
    (
        "staged-contrast",
        "to the review.",
        "to the review, not just a quick look but a full audit.",
    ),
    (
        "reveal-question",
        "Two reviewers checked",
        "The result? Two reviewers checked",
    ),
    (
        "clause-dashes",
        "and stayed low for a while.",
        "and stayed low - for a while. Supply rose - slowly.",
    ),
    ("hyphen", "the program", "the open-source program"),
    ("hyphen", "left for the centre", "e-mailed the centre"),
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

/// Human Australian Government prose in tests/corpus: 3 Productivity Commission reports
/// released in 2018 and 2021, about 198,500 words under CC BY 3.0 AU (sources in its NOTICE).
const HUMAN_CORPUS: &[&str] = &[
    "rising-inequality-2018.md",
    "right-to-repair-2021.md",
    "vulnerable-supply-chains-2021.md",
];
/// Findings on the human corpus. Most follow the Commission's house style, which Laconic's
/// overrides: spaced em dashes, words for numbers below 10, 'while' and 'among', and serial
/// commas. The ceilings only move down; they guard against a net increase on this corpus,
/// not against every new false finding.
const HUMAN_CORPUS_CEILING: usize = 2249;
/// Advisory findings have their own ceiling: 41 paragraphs put clause dashes in 2 or more
/// sentences when the corpus was calibrated on 2 November 2026.
const HUMAN_CORPUS_ADVISORY_CEILING: usize = 41;
/// AI-tell findings at calibration, all human uses: 'it is important to note' 3 times,
/// 'innovative', 'deep dive', 'not just X, but Y' and 'time-frame'.
const HUMAN_CORPUS_TELL_CEILING: usize = 7;

#[test]
fn human_corpus_findings_stay_under_the_ceiling() {
    let (advisory, findings): (Vec<_>, Vec<_>) = HUMAN_CORPUS
        .iter()
        .flat_map(|name| {
            let path = format!("{}/tests/corpus/{name}", env!("CARGO_MANIFEST_DIR"));
            lint(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")))
        })
        .partition(|f| ADVISORY.contains(&f.rule));
    assert!(
        findings.len() <= HUMAN_CORPUS_CEILING,
        "{} findings on the human corpus, ceiling {HUMAN_CORPUS_CEILING}",
        findings.len()
    );
    assert!(
        advisory.len() <= HUMAN_CORPUS_ADVISORY_CEILING,
        "{} advisory findings on the human corpus, ceiling {HUMAN_CORPUS_ADVISORY_CEILING}",
        advisory.len()
    );
    let tells: Vec<_> = findings
        .iter()
        .filter(|f| {
            matches!(
                f.rule,
                "stock-phrase" | "staged-contrast" | "reveal-question" | "hyphen"
            )
        })
        .collect();
    assert!(
        tells.len() <= HUMAN_CORPUS_TELL_CEILING,
        "{} AI tells in human prose, ceiling {HUMAN_CORPUS_TELL_CEILING}: {tells:?}",
        tells.len()
    );
}

#[test]
fn every_rule_has_a_seed() {
    for rule in [
        "us-spelling",
        "em-dash",
        "clause-dashes",
        "double-quotes",
        "serial-comma",
        "number-word",
        "closer",
        "stock-phrase",
        "staged-contrast",
        "reveal-question",
        "hyphen",
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
