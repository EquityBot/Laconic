//! Candidate findings in the original prose of a Markdown file.
//!
//! Code (fenced and inline), URLs, link targets, HTML lines and quoted spans are
//! masked before the rules run, so protected text never produces a finding, except
//! that a double-quoted span in prose is itself a finding.

use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, PartialEq)]
pub struct Finding {
    pub line: usize,
    pub column: usize,
    pub rule: &'static str,
    pub message: String,
}

const CLOSERS: &[&str] = &[
    "hope this helps",
    "let me know if",
    "feel free to",
    "happy to help",
    "rest assured",
    "great question",
    "don't hesitate",
];

/// Stock phrases from the pattern catalogue that rarely carry meaning: staged
/// openers, emphasis crutches, summary labels and business jargon. Matched as whole
/// words, so 'circle backwards' is not a finding.
const STOCK_PHRASES: &[&str] = &[
    "here's the thing",
    "let me be clear",
    "make no mistake",
    "let that sink in",
    "the uncomfortable truth",
    "here's what nobody tells you",
    "the part everyone misses",
    "it's worth noting",
    "it is worth noting",
    "worth flagging",
    "it's important to note",
    "it is important to note",
    "bottom line:",
    "in short:",
    "in today's world",
    "in a world where",
    "earns its keep",
    "deep dive",
    "circle back",
    "double down",
    "game changer",
    "game-changer",
    "paradigm shift",
];
/// US forms that are not caught by the -ize/-yze rule, with the Australian form.
const US_WORDS: &[(&str, &str)] = &[
    ("color", "colour"),
    ("colors", "colours"),
    ("colored", "coloured"),
    ("center", "centre"),
    ("centers", "centres"),
    ("centered", "centred"),
    ("behavior", "behaviour"),
    ("behaviors", "behaviours"),
    ("behavioral", "behavioural"),
    ("favor", "favour"),
    ("favorite", "favourite"),
    ("favorable", "favourable"),
    ("honor", "honour"),
    ("labor", "labour"),
    ("neighbor", "neighbour"),
    ("labeled", "labelled"),
    ("labeling", "labelling"),
    ("canceled", "cancelled"),
    ("canceling", "cancelling"),
    ("traveled", "travelled"),
    ("traveling", "travelling"),
    ("traveler", "traveller"),
    ("modeled", "modelled"),
    ("modeling", "modelling"),
    ("catalog", "catalogue"),
    ("defense", "defence"),
    ("offense", "offence"),
    ("artifact", "artefact"),
    ("artifacts", "artefacts"),
    ("toward", "towards"),
    ("percent", "per cent"),
    ("acknowledgment", "acknowledgement"),
];

/// House overrides of the Style Manual, which prefers while and among.
const WORD_CHOICES: &[(&str, &str)] = &[("while", "whilst"), ("among", "amongst")];

/// Words before 'while' that make it a noun: a while, a short while, worth your while.
const WHILE_NOUN_BEFORE: &[&str] = &[
    "a", "the", "this", "that", "short", "long", "little", "good", "great", "whole", "my", "your",
    "his", "her", "its", "our", "their",
];

/// Words that open a subordinate clause; a comma closing one is not a list comma.
const SUBORDINATORS: &[&str] = &[
    "if", "when", "because", "although", "though", "while", "after", "before", "since", "unless",
    "once", "whenever",
];

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

/// Words that end in -ize but are not US spellings of -ise words.
const IZE_EXCEPTIONS: &[&str] = &[
    "size", "sizes", "sized", "sizing", "seize", "seized", "seizes", "prize", "prizes", "capsize",
    "maize", "baize", "assize",
];

pub fn lint(text: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut fence: Option<&str> = None;
    // A leading YAML frontmatter block is metadata, not prose.
    let mut in_frontmatter = text.starts_with("---\n") || text.starts_with("---\r\n");
    for (index, raw) in text.lines().enumerate() {
        let line_no = index + 1;
        let trimmed = raw.trim_start();
        if in_frontmatter {
            if index > 0 && raw.trim_end() == "---" {
                in_frontmatter = false;
            }
            continue;
        }
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(if trimmed.starts_with("```") {
                "```"
            } else {
                "~~~"
            });
            continue;
        }
        if trimmed.starts_with('<') || trimmed.starts_with('>') {
            continue; // HTML and block quotations are not original prose.
        }
        let chars: Vec<char> = raw.chars().collect();
        let (masked, doubles) = mask(&chars);
        for column in doubles {
            findings.push(Finding {
                line: line_no,
                column,
                rule: "double-quotes",
                message: "double quotation marks in prose; use single marks".into(),
            });
        }
        check_residue(&chars, line_no, &mut findings);
        check_line(&masked, line_no, &mut findings);
    }
    findings
}

/// Chat-tool leftovers: citation markers and tracking tags in links.
const RESIDUE_MARKERS: &[&str] = &[
    "oaicite",
    "contentreference",
    "utm_source=chatgpt",
    "utm_source=openai",
];

/// Flags zero-width characters and chat-tool markers. Unlike the other rules this
/// reads URLs and quoted text, where the markers usually sit; inline code, which may
/// name a marker on purpose, is skipped.
fn check_residue(chars: &[char], line: usize, findings: &mut Vec<Finding>) {
    let mut text = chars.to_vec();
    let mut i = 0;
    while i < text.len() {
        if text[i] == '`'
            && let Some(end) = find(chars, i + 1, |x| x == '`')
        {
            text[i..=end].iter_mut().for_each(|x| *x = ' ');
            i = end;
        }
        i += 1;
    }
    for (k, c) in text.iter().enumerate() {
        // A byte order mark that opens the file is an encoding marker, not residue.
        let bom = *c == '\u{feff}' && line == 1 && k == 0;
        if matches!(
            c,
            '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}'
        ) && !bom
        {
            findings.push(Finding {
                line,
                column: k + 1,
                rule: "residue",
                message: format!("zero-width character U+{:04X}", *c as u32),
            });
        }
    }
    let lower: String = text.iter().collect::<String>().to_lowercase();
    for marker in RESIDUE_MARKERS {
        let mut from = 0;
        while let Some(pos) = lower[from..].find(marker) {
            findings.push(Finding {
                line,
                column: lower[..from + pos].chars().count() + 1,
                rule: "residue",
                message: format!("chat-tool residue '{marker}'"),
            });
            from += pos + marker.len();
        }
    }
}

/// Replaces protected spans with spaces, keeping columns. Returns the masked line and
/// the columns of double-quoted spans.
fn mask(chars: &[char]) -> (Vec<char>, Vec<usize>) {
    let mut out = chars.to_vec();
    let mut doubles = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let blank = |out: &mut Vec<char>, from: usize, to: usize| {
            out[from..to].iter_mut().for_each(|x| *x = ' ')
        };
        if c == '`'
            && let Some(end) = find(chars, i + 1, |x| x == '`')
        {
            blank(&mut out, i, end + 1);
            i = end + 1;
            continue;
        }
        if starts_with(chars, i, "http://") || starts_with(chars, i, "https://") {
            let end = (i..chars.len())
                .find(|&j| chars[j].is_whitespace() || chars[j] == ')' || chars[j] == '>')
                .unwrap_or(chars.len());
            blank(&mut out, i, end);
            i = end;
            continue;
        }
        // Link text that names its own target ([humanizer](https://github.com/x/humanizer))
        // is a name, not prose.
        if c == '['
            && let Some(close) = find(chars, i + 1, |x| x == ']')
            && chars.get(close + 1) == Some(&'(')
            && let Some(end) = find(chars, close + 2, |x| x == ')')
        {
            let text: String = chars[i + 1..close]
                .iter()
                .collect::<String>()
                .to_lowercase();
            let url: String = chars[close + 2..end]
                .iter()
                .collect::<String>()
                .to_lowercase();
            if !text.is_empty() && url.contains(&text) {
                blank(&mut out, i, end + 1);
                i = end + 1;
                continue;
            }
        }
        if c == ']'
            && chars.get(i + 1) == Some(&'(')
            && let Some(end) = find(chars, i + 2, |x| x == ')')
        {
            blank(&mut out, i + 1, end + 1);
            i = end + 1;
            continue;
        }
        if c == '"' || c == '\u{201c}' {
            let close = if c == '"' { '"' } else { '\u{201d}' };
            if let Some(end) = find(chars, i + 1, |x| x == close)
                && end > i + 1
                && end - i <= 120
            {
                doubles.push(i + 1);
                blank(&mut out, i, end + 1);
                i = end + 1;
                continue;
            }
        }
        if (c == '\'' || c == '\u{2018}')
            && opens_quote(chars, i)
            && let Some(end) = (i + 1..chars.len())
                .find(|&j| (chars[j] == '\'' || chars[j] == '\u{2019}') && closes_quote(chars, j))
        {
            blank(&mut out, i, end + 1);
            i = end + 1;
            continue;
        }
        i += 1;
    }
    (out, doubles)
}

fn find(chars: &[char], from: usize, pred: impl Fn(char) -> bool) -> Option<usize> {
    (from..chars.len()).find(|&j| pred(chars[j]))
}

fn starts_with(chars: &[char], at: usize, s: &str) -> bool {
    s.chars()
        .enumerate()
        .all(|(k, c)| chars.get(at + k) == Some(&c))
}

fn opens_quote(chars: &[char], i: usize) -> bool {
    let before = if i == 0 { ' ' } else { chars[i - 1] };
    let after = chars.get(i + 1).copied().unwrap_or(' ');
    !before.is_alphanumeric() && !after.is_whitespace()
}

fn closes_quote(chars: &[char], j: usize) -> bool {
    let before = chars[j - 1];
    let after = chars.get(j + 1).copied().unwrap_or(' ');
    !before.is_whitespace() && !after.is_alphanumeric()
}

fn check_line(masked: &[char], line: usize, findings: &mut Vec<Finding>) {
    for (k, &c) in masked.iter().enumerate() {
        if c == '\u{2014}' {
            findings.push(Finding {
                line,
                column: k + 1,
                rule: "em-dash",
                message: "em dash in original prose".into(),
            });
        }
    }
    let lower: String = masked.iter().collect::<String>().to_lowercase();
    let lower_chars: Vec<char> = lower.chars().collect();
    for phrase in CLOSERS {
        let mut from = 0;
        while let Some(pos) = lower[from..].find(phrase) {
            let col = lower[..from + pos].chars().count() + 1;
            findings.push(Finding {
                line,
                column: col,
                rule: "closer",
                message: format!("chatbot phrase '{phrase}'"),
            });
            from += pos + phrase.len();
        }
    }
    // One output char per input char, so columns stay exact (to_lowercase can expand
    // a character); every listed phrase is ASCII.
    let plain: String = masked
        .iter()
        .map(|&c| {
            if c == '\u{2019}' {
                '\''
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    for phrase in STOCK_PHRASES {
        let mut from = 0;
        while let Some(pos) = plain[from..].find(phrase) {
            let at = from + pos;
            let end = at + phrase.len();
            let bounded = !plain[..at].ends_with(|c: char| c.is_alphanumeric())
                && (phrase.ends_with(':')
                    || !plain[end..].starts_with(|c: char| c.is_alphanumeric()));
            if bounded {
                findings.push(Finding {
                    line,
                    column: plain[..at].chars().count() + 1,
                    rule: "stock-phrase",
                    message: format!("stock phrase '{phrase}'; state the point directly"),
                });
            }
            from = end;
        }
    }
    // Words, for spelling.
    let mut start = None;
    for k in 0..=lower_chars.len() {
        let is_word = k < lower_chars.len() && lower_chars[k].is_alphabetic();
        match (start, is_word) {
            (None, true) => start = Some(k),
            (Some(s), false) => {
                let word: String = lower_chars[s..k].iter().collect();
                // A capital inside a sentence marks a proper noun (the Labor Party, Pearl
                // Harbor), whose spelling is not a house-style choice.
                let proper_noun = masked.get(s).is_some_and(|c| c.is_uppercase())
                    && !starts_sentence(&lower_chars, s);
                if let Some(au) = au_form(&word).filter(|_| !proper_noun) {
                    findings.push(Finding {
                        line,
                        column: s + 1,
                        rule: "us-spelling",
                        message: format!("'{word}': Australian form is '{au}'"),
                    });
                }
                let noun_while = word == "while"
                    && (WHILE_NOUN_BEFORE.contains(&word_before(&lower_chars, s).as_str())
                        || word_after(&lower_chars, k) == "away");
                if let Some((_, house)) = WORD_CHOICES.iter().find(|(w, _)| *w == word)
                    && !noun_while
                {
                    findings.push(Finding {
                        line,
                        column: s + 1,
                        rule: "word-choice",
                        message: format!("'{word}': the house style prefers '{house}'"),
                    });
                }
                // A number word inside a hyphenated compound (forty-five) is a fixed form.
                if let Some(numeral) = number_word(&word)
                    && !starts_sentence(&lower_chars, s)
                    && !lower_chars.get(k).is_some_and(|c| *c == '-')
                    && !(s > 0 && lower_chars[s - 1] == '-')
                {
                    findings.push(Finding {
                        line,
                        column: s + 1,
                        rule: "number-word",
                        message: format!("'{word}': use the numeral {numeral} (words only at the start of a sentence)"),
                    });
                }
                start = None;
            }
            _ => {}
        }
    }
    check_dates_times_latin(&lower, line, findings);
    // Serial comma candidates: ", and" or ", or" with another comma earlier in the same clause.
    for (k, window) in lower_chars.windows(6).enumerate() {
        let text: String = window.iter().collect();
        let is_and = text.starts_with(", and ");
        let is_or = text.starts_with(", or ");
        if !(is_and || is_or) {
            continue;
        }
        let clause_start = (0..k)
            .rev()
            .find(|&j| matches!(lower_chars[j], '.' | ';' | ':' | '!' | '?' | '('))
            .map_or(0, |j| j + 1);
        // A final item that itself contains 'and' or 'or' may need the comma.
        let item_start = k + if is_and { 6 } else { 5 };
        let item_end = (item_start..lower_chars.len())
            .find(|&j| matches!(lower_chars[j], '.' | ';' | ':' | '!' | '?' | ','))
            .unwrap_or(lower_chars.len());
        let final_item: String = lower_chars[item_start..item_end].iter().collect();
        let compound_final = final_item.contains(" and ") || final_item.contains(" or ");
        // So may a list whose earlier items contain 'and' or 'or'.
        let earlier: String = lower_chars[clause_start..k].iter().collect();
        let compound_earlier = earlier.contains(" and ") || earlier.contains(" or ");
        // 'If it fails, rerun it, and report' joins 2 clauses after an introductory clause.
        let first_word = earlier
            .split(|c: char| !c.is_alphabetic())
            .find(|w| !w.is_empty())
            .unwrap_or("");
        let introductory = SUBORDINATORS.contains(&first_word) && earlier.matches(',').count() == 1;
        if earlier.contains(',') && !compound_final && !compound_earlier && !introductory {
            findings.push(Finding { line, column: k + 1, rule: "serial-comma", message: "possible comma before a final 'and' or 'or'; keep it only if it prevents a misreading".into() });
        }
    }
}

/// Dates, times and Latin abbreviations that break the Style Manual forms.
fn check_dates_times_latin(lower: &str, line: usize, findings: &mut Vec<Finding>) {
    let tokens: Vec<(usize, &str)> = lower
        .split_whitespace()
        .map(|w| (w.as_ptr() as usize - lower.as_ptr() as usize, w))
        .collect();
    let core = |w: &str| -> String {
        w.trim_matches(|c: char| matches!(c, ',' | ';' | '(' | ')' | '*' | '_'))
            .to_string()
    };
    let digits = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_digit());
    let ordinal = |w: &str| {
        ["st", "nd", "rd", "th"]
            .iter()
            .any(|s| w.strip_suffix(s).is_some_and(digits))
    };
    let mut push = |at: usize, rule: &'static str, message: &str| {
        findings.push(Finding {
            line,
            column: lower[..at].chars().count() + 1,
            rule,
            message: message.into(),
        })
    };
    for (i, &(at, raw)) in tokens.iter().enumerate() {
        let word = core(raw);
        let bare = word.trim_end_matches(['.', ':']);
        let prev = i.checked_sub(1).map(|j| core(tokens[j].1));
        let next = tokens.get(i + 1).map(|t| core(t.1));
        if word.starts_with("e.g.") || word.starts_with("i.e.") || bare == "eg" || bare == "ie" {
            push(
                at,
                "latin",
                "Latin abbreviation; write 'for example' or 'that is'",
            );
        }
        // 9am, 9:30pm, 9.30am, 9 a.m.: write 9:30 am.
        let time = bare
            .strip_suffix("am")
            .or_else(|| bare.strip_suffix("pm"))
            .filter(|t| {
                !t.is_empty()
                    && t.chars()
                        .all(|c| c.is_ascii_digit() || c == ':' || c == '.')
            })
            .is_some_and(|t| t.starts_with(|c: char| c.is_ascii_digit()));
        let dotted = matches!(word.as_str(), "a.m." | "p.m." | "a.m" | "p.m");
        let period_separator = word.contains('.')
            && word.split('.').count() == 2
            && word.split('.').all(digits)
            && next
                .as_deref()
                .is_some_and(|n| matches!(n.trim_end_matches('.'), "am" | "pm"));
        if time || dotted || period_separator {
            push(at, "time", "write times as 9:30 am or 9 am");
        }
        if MONTHS.contains(&word.trim_end_matches('.')) {
            let month = word.trim_end_matches('.');
            let prev_day = prev.as_deref().filter(|p| digits(p) || ordinal(p));
            if prev_day.is_some_and(ordinal) {
                push(at, "date", "no ordinal in dates: write 19 October 2026");
            }
            if prev_day.is_some()
                && raw.ends_with(',')
                && next.as_deref().is_some_and(|n| {
                    let n = n.trim_end_matches(['.', ',']);
                    digits(n) && n.len() == 4
                })
            {
                push(
                    at,
                    "date",
                    "no comma before the year: write 19 October 2026",
                );
            }
            if month != "may"
                && next.as_deref().is_some_and(|n| {
                    let n = n.trim_end_matches(['.', ',']);
                    (digits(n) && n.len() <= 2) || ordinal(n)
                })
            {
                push(at, "date", "day before month: write 19 October 2026");
            }
        }
    }
}

/// Numbers from 2 to 20 written as words. 'Zero' and 'one' stay as words.
fn number_word(word: &str) -> Option<u8> {
    const WORDS: [&str; 19] = [
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ];
    WORDS.iter().position(|w| *w == word).map(|i| i as u8 + 2)
}

/// The word before position `s`, lower case, or an empty string.
fn word_before(chars: &[char], s: usize) -> String {
    let end = chars[..s]
        .iter()
        .rposition(|c| c.is_alphabetic())
        .map_or(0, |e| e + 1);
    let gap = &chars[end..s];
    if !gap.iter().all(|c| c.is_whitespace()) {
        return String::new();
    }
    let start = chars[..end]
        .iter()
        .rposition(|c| !c.is_alphabetic())
        .map_or(0, |b| b + 1);
    chars[start..end].iter().collect()
}

/// The word starting after position `k` past spaces, lower case, or an empty string.
fn word_after(chars: &[char], k: usize) -> String {
    let start = (k..chars.len())
        .find(|&j| !chars[j].is_whitespace())
        .unwrap_or(chars.len());
    chars[start..]
        .iter()
        .take_while(|c| c.is_alphabetic())
        .collect()
}

/// True when the word at `s` begins a sentence (or the line).
fn starts_sentence(chars: &[char], s: usize) -> bool {
    // Skip spaces and Markdown markers (list bullets, emphasis, headings, table pipes).
    let previous = chars[..s]
        .iter()
        .rev()
        .find(|c| !(c.is_whitespace() || matches!(c, '*' | '_' | '#' | '-' | '|' | '(')));
    previous.is_none_or(|c| matches!(c, '.' | '!' | '?' | ':'))
}

/// US forms with their Australian spelling, generated from the English Speller Database
/// by `gen_au_words.py`, one "us au" pair per line.
const ESDB_US_WORDS: &str = include_str!("us_words.txt");

fn esdb_us_words() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        ESDB_US_WORDS
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split_once(' '))
            .collect()
    })
}

fn au_form(word: &str) -> Option<String> {
    if let Some((_, au)) = US_WORDS.iter().find(|(us, _)| *us == word) {
        return Some((*au).to_string());
    }
    if let Some(au) = esdb_us_words().get(word) {
        return Some((*au).to_string());
    }
    if IZE_EXCEPTIONS.contains(&word) || word.len() < 6 {
        return None;
    }
    for (us, au) in [
        ("ization", "isation"),
        ("izations", "isations"),
        ("izing", "ising"),
        ("ized", "ised"),
        ("izes", "ises"),
        ("ize", "ise"),
        ("yzing", "ysing"),
        ("yzed", "ysed"),
        ("yzes", "yses"),
        ("yze", "yse"),
    ] {
        if let Some(stem) = word.strip_suffix(us) {
            return Some(format!("{stem}{au}"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(text: &str) -> Vec<&'static str> {
        lint(text).into_iter().map(|f| f.rule).collect()
    }

    #[test]
    fn flags_house_style_candidates_in_prose() {
        assert_eq!(
            rules("We will organize the colors \u{2014} soon."),
            vec!["em-dash", "us-spelling", "us-spelling"]
        );
        assert_eq!(rules("He said \"approved\" today."), vec!["double-quotes"]);
        assert_eq!(
            rules("Bring apples, pears, and plums."),
            vec!["serial-comma"]
        );
        assert_eq!(rules("Hope this helps!"), vec!["closer"]);
    }

    #[test]
    fn flags_stock_phrases_as_whole_words_only() {
        assert_eq!(
            rules("Here\u{2019}s the thing: we should circle back."),
            vec!["stock-phrase", "stock-phrase"]
        );
        assert_eq!(rules("Bottom line: it works."), vec!["stock-phrase"]);
        assert!(rules("Draw a circle backwards. The deep diver surfaced.").is_empty());
        assert!(rules("Avoid 'circle back' and `deep dive` in reports.").is_empty());
        assert!(
            rules("Reconcile the till at the end of the day. The load-bearing wall is cracked.")
                .is_empty()
        );
        assert_eq!(
            rules("Recircle back. Bottom line:works."),
            vec!["stock-phrase"]
        );
    }

    #[test]
    fn stock_phrase_columns_count_characters() {
        let f = &lint("We\u{2019}ll circle back.")[0];
        assert_eq!((f.column, f.rule), (7, "stock-phrase"));
        let f = &lint("\u{130} circle back.")[0];
        assert_eq!((f.column, f.rule), (3, "stock-phrase"));
    }

    #[test]
    fn leaves_protected_and_correct_text_alone() {
        let clean = "Set `color: red` and quote 'Choose a color' at https://example.com/color.\n\
                     Bring apples, pears and plums. The size is fine; we organise and analyse.\n\
                     ```\nbehavior = \"organize\"\n```\n> A quoted line with \"double\" marks.";
        assert!(lint(clean).is_empty(), "{:?}", lint(clean));
    }

    #[test]
    fn numbers_from_2_take_numerals_except_at_a_sentence_start() {
        assert_eq!(rules("We made two commits."), vec!["number-word"]);
        assert!(rules("Two commits landed. One more is due. A two-thirds share.").is_empty());
        assert!(rules("- Three items remain.").is_empty());
    }

    #[test]
    fn skips_frontmatter_and_compound_final_items() {
        assert!(lint("---\ntags: \"A, B\"\n---\nFine text.").is_empty());
        assert!(rules("Read the draft, the notes, and the facts or figures.").is_empty());
    }

    #[test]
    fn a_comma_joining_two_clauses_is_not_a_serial_comma() {
        assert!(rules("The build passed, and the deploy started.").is_empty());
    }

    #[test]
    fn flags_style_manual_word_choices_dates_times_and_latin() {
        assert_eq!(
            rules("While among friends."),
            vec!["word-choice", "word-choice"]
        );
        assert!(
            rules("Whilst amongst friends we waited a while. It was worth your while to while away an hour.")
                .is_empty()
        );
        assert_eq!(
            rules("Leave at 9.30am on 5th October, 2026."),
            vec!["time", "date", "date"]
        );
        assert_eq!(
            rules("Due October 5 at 9 a.m. today."),
            vec!["date", "time"]
        );
        assert_eq!(
            rules("Use e.g. a token, i.e. the key."),
            vec!["latin", "latin"]
        );
        assert!(
            rules("Meet on 19 October 2026 at 9:30 am or 5 pm. It may 5 times fail.").is_empty()
        );
    }

    #[test]
    fn hyphenated_numbers_and_introductory_clauses_are_not_findings() {
        assert!(rules("We funded forty-five places.").is_empty());
        assert!(rules("If the tool is available, run it, and fix each finding.").is_empty());
    }

    #[test]
    fn flags_chat_tool_residue_but_not_markers_named_in_code() {
        assert_eq!(
            rules("See [the guide](https://example.com/?utm_source=chatgpt.com) now."),
            vec!["residue"]
        );
        assert_eq!(rules("Done\u{200b} today."), vec!["residue"]);
        assert_eq!(
            rules("Cited :contentReference[oaicite:0] here."),
            vec!["residue", "residue"]
        );
        assert!(rules("\u{feff}Scan for `oaicite` and `utm_source=` before sending.").is_empty());
    }

    #[test]
    fn reports_line_and_column() {
        let f = &lint("ok\nThe color.")[0];
        assert_eq!((f.line, f.column, f.rule), (2, 5, "us-spelling"));
    }
}
