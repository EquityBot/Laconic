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

/// Prepositions that open an introductory phrase ('In the original flow, the server...').
const INTRO_PREPOSITIONS: &[&str] = &[
    "in", "on", "at", "for", "with", "without", "from", "by", "during", "under", "after", "before",
    "within", "across", "through", "outside", "inside",
];

/// Words that end an introductory marker whose comma is not a list comma.
const MARKERS_BEFORE_COMMA: &[&str] = &[
    "for example",
    "for instance",
    "that is",
    "however",
    "in short",
];

/// Words that open a parenthetical segment between commas ('a folder, not a repository,').
const PARENTHETICAL_OPENERS: &[&str] = &["which", "not"];

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
        // The clause starts after sentence punctuation or the bracket that encloses this
        // comma; a bracket pair that closes before it belongs to the clause.
        let mut depth = 0;
        let mut clause_start = 0;
        for j in (0..k).rev() {
            match lower_chars[j] {
                ')' => depth += 1,
                '(' if depth == 0 => {
                    clause_start = j + 1;
                    break;
                }
                '(' => depth -= 1,
                '.' if j + 1 < lower_chars.len()
                    && lower_chars[j + 1].is_ascii_digit()
                    && j > 0
                    && lower_chars[j - 1].is_ascii_digit() => {}
                '.' | ';' | ':' | '!' | '?' if depth == 0 => {
                    clause_start = j + 1;
                    break;
                }
                _ => {}
            }
        }
        let list_commas = list_commas(&lower_chars, clause_start, k);
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
        // So does 'In the original flow, the server sends a code, and the client...'.
        let words_before_first_comma = list_commas.first().map_or(0, |&c| {
            lower_chars[clause_start..c]
                .iter()
                .collect::<String>()
                .split_whitespace()
                .count()
        });
        let introductory = list_commas.len() == 1
            && (SUBORDINATORS.contains(&first_word)
                || (INTRO_PREPOSITIONS.contains(&first_word)
                    && words_before_first_comma >= 3
                    && starts_sentence_at(&lower_chars, clause_start)));
        if !list_commas.is_empty() && !compound_final && !compound_earlier && !introductory {
            findings.push(Finding { line, column: k + 1, rule: "serial-comma", message: "possible comma before a final 'and' or 'or'; keep it only if it prevents a misreading".into() });
        }
    }
}

/// Commas in `chars[from..to]` that could separate list items. Skips commas inside numbers
/// (75,000), inside a bracket pair, after markers such as 'for example', and the pair of
/// commas around a 'which' or 'not' segment.
fn list_commas(chars: &[char], from: usize, to: usize) -> Vec<usize> {
    let mut found = Vec::new();
    let mut depth = 0i32;
    let mut closes_parenthetical = false;
    for j in from..to {
        match chars[j] {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                let in_number = j > 0
                    && chars[j - 1].is_ascii_digit()
                    && chars.get(j + 1).is_some_and(|c| c.is_ascii_digit());
                if in_number {
                    continue;
                }
                let before: String = chars[from..j].iter().collect();
                let after_marker = MARKERS_BEFORE_COMMA
                    .iter()
                    .any(|m| before.trim_end().ends_with(m));
                let opens = PARENTHETICAL_OPENERS.contains(&word_after(chars, j + 1).as_str());
                if closes_parenthetical {
                    closes_parenthetical = false;
                } else if opens {
                    closes_parenthetical = true;
                } else if !after_marker {
                    found.push(j);
                }
            }
            _ => {}
        }
    }
    found
}

/// True when position `at` opens a sentence or the line, not a clause after ':' or ';'.
fn starts_sentence_at(chars: &[char], at: usize) -> bool {
    chars[..at]
        .iter()
        .rev()
        .find(|c| !c.is_whitespace())
        .is_none_or(|c| matches!(c, '.' | '!' | '?'))
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
            // 'Wednesday 22 October 2026' names a weekday the calendar can check.
            let year = next
                .as_deref()
                .map(|n| n.trim_end_matches(['.', ',']))
                .filter(|n| digits(n) && n.len() == 4)
                .and_then(|n| n.parse::<i32>().ok());
            let day = prev_day
                .filter(|p| digits(p))
                .and_then(|p| p.parse::<u32>().ok());
            let named = i
                .checked_sub(2)
                .map(|j| core(tokens[j].1))
                .and_then(|w| WEEKDAYS.iter().position(|d| *d == w));
            if let (Some(year), Some(day), Some(named)) = (year, day, named) {
                let month_index = MONTHS.iter().position(|m| *m == month).unwrap_or(0) as u32 + 1;
                if let Some(actual) = weekday(year, month_index, day)
                    && actual != named
                {
                    push(
                        tokens[i - 2].0,
                        "weekday",
                        &format!(
                            "{day} {} {year} is a {}, not a {}",
                            capitalise(month),
                            capitalise(WEEKDAYS[actual]),
                            capitalise(WEEKDAYS[named])
                        ),
                    );
                }
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

const WEEKDAYS: [&str; 7] = [
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];

/// Day of the week (0 is Sunday) for a Gregorian date, or None if the date does not exist.
fn weekday(year: i32, month: u32, day: u32) -> Option<usize> {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let length = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&month) || day == 0 || day > length[month as usize - 1] {
        return None;
    }
    // Sakamoto's method.
    const OFFSETS: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let d = y + y / 4 - y / 100 + y / 400 + OFFSETS[month as usize - 1] + day as i32;
    Some(d.rem_euclid(7) as usize)
}

fn capitalise(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
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
    }

    /// Labelled from the serial-comma findings on 164 eval responses (20 October 2026):
    /// real serial commas, then sentences the rule used to flag wrongly.
    #[test]
    fn serial_comma_rule_matches_labelled_eval_responses() {
        let real = [
            "Risks include `id > :cursor` pagination, sharding by `id % n`, and integer-typed fields in clients.",
            "It's your gross business income, excluding GST, input-taxed sales such as residential rent, and sales not connected to an enterprise.",
            "The client creates a random secret, sends a hash of it when the login starts, and reveals the secret itself when it redeems the code.",
            "The file may never have been created, it may be gitignored, or the build may be resolving the path wrongly.",
            "Re-key caches with a dual-read: try the UUID key first, then the integer key, and write the UUID key.",
            "It excludes GST-free sales such as basic food, input-taxed sales, and sales not connected with your business.",
            "It's expired, revoked, mistyped, or issued for a different environment.",
            "Use whatever your other tests use, such as a fixture, a test helper that signs a JWT, or a seeded test user's login.",
            "Add the UUID next to the integer, write both, move readers one at a time behind flags, and remove the integer at the end.",
            "So I assumed PostgreSQL, a Kafka-style stream with a schema registry, and a REST API with external clients.",
            "Wait until the gate has passed, v1 is sunset, event retention has expired, and the longest token lifetime has passed.",
            // A list after a colon, and a list of 'wh' clauses, are lists (held-out responses).
            "Store the nonce locally: in session storage for an SPA, secure storage for a mobile app, or the server-side session.",
            "The paragraph never says what the platform does, who it's for, or what result it produces.",
        ];
        for sentence in real {
            assert_eq!(
                rules(sentence),
                vec!["serial-comma"],
                "should flag: {sentence}"
            );
        }
        let not_lists = [
            "In Australia you must register once your GST turnover reaches $75,000, and $68,000 is under that.",
            "This includes taxi and ride-sourcing drivers (for example, Uber), and anyone claiming fuel tax credits.",
            "Find the entry from just before the operation (for example, `feature@{2}`, or the line before it).",
            "The working directory is an empty temporary folder, not a git repository, and nothing names a project.",
            "Without a secret, the authorisation code is the only protection, and the code can leak in several ways.",
            "The authorisation server signs the user in, which may include multi-factor authentication, and asks them to approve.",
            "It is a single file. 10,000 notes is a small load for it, and its FTS5 extension gives you fast search.",
            "For example, a payment entered twice in the ledger, or a receipt left out.",
            "You made 2 commits (C, D), and meanwhile someone added E to `main`.",
            "In the original flow, the AS returns a short-lived code via redirect, and the client exchanges it for tokens.",
            "They do not have the verifier, which was never sent anywhere until step 6, and step 6 goes straight to the AS.",
            "Use base64url, not standard base64, and strip the padding.",
            "Which project should I deploy (for example, `site`), and how do you normally deploy it?",
            "Check for standard base64 (`+`, `/`, `=`) instead of base64url, or hashing a hex string.",
            "The threshold is $75,000 ($150,000 for non-profit organisations), and $68,000 is below it.",
            "For public clients, the code itself was the only credential, and it travels through the browser.",
        ];
        for sentence in not_lists {
            assert!(rules(sentence).is_empty(), "should not flag: {sentence}");
        }
    }

    #[test]
    fn flags_a_weekday_that_does_not_match_its_date() {
        let found = lint("Reply by Wednesday 22 October 2026, please.");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule, "weekday");
        assert_eq!(found[0].column, 10);
        assert_eq!(
            found[0].message,
            "22 October 2026 is a Thursday, not a Wednesday"
        );
        assert_eq!(rules("Lodge by Friday, 31 October 2026."), vec!["weekday"]);
        // Correct weekdays, leap days and dates without a year or weekday pass.
        assert!(rules("Reply by Thursday 22 October 2026.").is_empty());
        assert!(rules("It was Tuesday 29 February 2028.").is_empty());
        assert!(rules("Reply by Wednesday 22 October.").is_empty());
        assert!(rules("Reply by 22 October 2026.").is_empty());
        // A date that does not exist is not checked against a weekday.
        assert!(rules("Monday 30 February 2026 is not a date.").is_empty());
    }

    #[test]
    fn weekday_follows_the_gregorian_calendar() {
        assert_eq!(weekday(2026, 10, 19), Some(1));
        assert_eq!(weekday(2000, 1, 1), Some(6));
        assert_eq!(weekday(1900, 3, 1), Some(4));
        assert_eq!(weekday(2024, 2, 29), Some(4));
        assert_eq!(weekday(2025, 2, 29), None);
        assert_eq!(weekday(2026, 13, 1), None);
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
