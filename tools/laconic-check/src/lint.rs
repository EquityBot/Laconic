//! Candidate findings in the original prose of a Markdown file.
//!
//! Code (fenced and inline), URLs, link targets, HTML lines and quoted spans are
//! masked before the rules run, so protected text never produces a finding, except
//! that a double-quoted span in prose is itself a finding.

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
        check_line(&masked, line_no, &mut findings);
    }
    findings
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
    // Words, for spelling.
    let mut start = None;
    for k in 0..=lower_chars.len() {
        let is_word = k < lower_chars.len() && lower_chars[k].is_alphabetic();
        match (start, is_word) {
            (None, true) => start = Some(k),
            (Some(s), false) => {
                let word: String = lower_chars[s..k].iter().collect();
                if let Some(au) = au_form(&word) {
                    findings.push(Finding {
                        line,
                        column: s + 1,
                        rule: "us-spelling",
                        message: format!("'{word}': Australian form is '{au}'"),
                    });
                }
                if let Some(numeral) = number_word(&word)
                    && !starts_sentence(&lower_chars, s)
                    && !lower_chars.get(k).is_some_and(|c| *c == '-')
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
        if earlier.contains(',') && !compound_final && !compound_earlier {
            findings.push(Finding { line, column: k + 1, rule: "serial-comma", message: "possible comma before a final 'and' or 'or'; keep it only if it prevents a misreading".into() });
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

/// True when the word at `s` begins a sentence (or the line).
fn starts_sentence(chars: &[char], s: usize) -> bool {
    // Skip spaces and Markdown markers (list bullets, emphasis, headings, table pipes).
    let previous = chars[..s]
        .iter()
        .rev()
        .find(|c| !(c.is_whitespace() || matches!(c, '*' | '_' | '#' | '-' | '|' | '(')));
    previous.is_none_or(|c| matches!(c, '.' | '!' | '?' | ':'))
}

fn au_form(word: &str) -> Option<String> {
    if let Some((_, au)) = US_WORDS.iter().find(|(us, _)| *us == word) {
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
    fn reports_line_and_column() {
        let f = &lint("ok\nThe color.")[0];
        assert_eq!((f.line, f.column, f.rule), (2, 5, "us-spelling"));
    }
}
