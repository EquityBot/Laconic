//! Protected items an edit must keep: numbers, quantities, dated months, inline code,
//! URLs and the words that carry negation, modality, certainty, condition, order and
//! scope. Reports what the original had and the edit lost, and what the edit added. A
//! clean result does not prove the meaning survived; it only rules out these losses. An
//! addition is a reason to check the edit invented nothing, not proof that it did.

use std::collections::BTreeMap;

const MEANING_WORDS: &[&str] = &[
    "not",
    "never",
    "no",
    "none",
    "nothing",
    "nor",
    "neither",
    "without",
    "cannot",
    "can't",
    "don't",
    "doesn't",
    "isn't",
    "won't",
    "must",
    "may",
    "only",
    "except",
    "unless",
    "some",
    "all",
    "any",
    "every",
    "most",
    "should",
    "might",
    "can",
    "could",
    "always",
    "will",
    "shall",
    "if",
    "until",
    "because",
    "before",
    "after",
    "likely",
    "probably",
    "possibly",
    "perhaps",
    "usually",
    "generally",
    "often",
    "sometimes",
    "approximately",
    "roughly",
];

/// Bounds and comparisons; `N` matches a number, so 'over the weekend' is not a bound.
const MEANING_PHRASES: &[&str] = &[
    "up to",
    "at least",
    "at most",
    "more than",
    "less than",
    "fewer than",
    "over N",
    "under N",
    "about N",
    "nearly N",
    "almost N",
    "subject to",
];

/// Attached or following words that give a number its size or unit, with the form a
/// quantity reports. Upper-case `M` is million and lower-case `m` is metres, and `Mb` is
/// bits where `MB` is bytes, so these match case-sensitively; every other spelling
/// matches in any case.
const QUANTITY_WORDS: &[(&[&str], &str)] = &[
    (&["k", "thousand"], "thousand"),
    (&["mn", "million", "millions"], "million"),
    (&["b", "bn", "billion", "billions"], "billion"),
    (&["trillion", "trillions"], "trillion"),
    (
        &[
            "km",
            "kms",
            "kilometre",
            "kilometres",
            "kilometer",
            "kilometers",
        ],
        "km",
    ),
    (&["metre", "metres", "meter", "meters"], "m"),
    (&["cm", "centimetre", "centimetres"], "cm"),
    (&["mm", "millimetre", "millimetres"], "mm"),
    (&["mile", "miles"], "mile"),
    (&["kg", "kgs", "kilogram", "kilograms"], "kg"),
    (&["tonne", "tonnes"], "tonne"),
    (&["lb", "lbs", "pound", "pounds"], "lb"),
    (&["hour", "hours", "hr", "hrs"], "hour"),
    (&["minute", "minutes", "min", "mins"], "minute"),
    (&["second", "seconds", "sec", "secs"], "second"),
    (&["day", "days"], "day"),
    (&["week", "weeks"], "week"),
    (&["month", "months"], "month"),
    (&["year", "years"], "year"),
    (&["kb", "kilobyte", "kilobytes"], "KB"),
    (&["mb", "megabyte", "megabytes"], "MB"),
    (&["gb", "gigabyte", "gigabytes"], "GB"),
    (&["tb", "terabyte", "terabytes"], "TB"),
    (&["kilobit", "kilobits"], "Kb"),
    (&["megabit", "megabits"], "Mb"),
    (&["gigabit", "gigabits"], "Gb"),
    (&["terabit", "terabits"], "Tb"),
];

const MONTHS: &[(&[&str], &str)] = &[
    (&["January", "Jan"], "January"),
    (&["February", "Feb"], "February"),
    (&["March", "Mar"], "March"),
    (&["April", "Apr"], "April"),
    (&["May"], "May"),
    (&["June", "Jun"], "June"),
    (&["July", "Jul"], "July"),
    (&["August", "Aug"], "August"),
    (&["September", "Sept", "Sep"], "September"),
    (&["October", "Oct"], "October"),
    (&["November", "Nov"], "November"),
    (&["December", "Dec"], "December"),
];

/// Capitalised words that start a sentence more often than they start a name.
const NOT_NAME_START: &[&str] = &[
    "A", "An", "And", "As", "At", "But", "By", "For", "From", "He", "Her", "His", "I", "If", "In",
    "It", "Its", "My", "No", "Not", "Of", "On", "Or", "Our", "She", "So", "The", "Their", "These",
    "They", "This", "Those", "To", "We", "When", "With", "You", "Your",
];

/// Number words count as their numerals, so 'three' to '3' is not a change.
/// 'one' is left out because it is also a pronoun. shortcut: compounds such as
/// 'twenty-five' read as 20 and 5; spell out a parser if edits hit them.
const NUMBER_WORDS: &[(&str, &str)] = &[
    ("two", "2"),
    ("three", "3"),
    ("four", "4"),
    ("five", "5"),
    ("six", "6"),
    ("seven", "7"),
    ("eight", "8"),
    ("nine", "9"),
    ("ten", "10"),
    ("eleven", "11"),
    ("twelve", "12"),
    ("thirteen", "13"),
    ("fourteen", "14"),
    ("fifteen", "15"),
    ("sixteen", "16"),
    ("seventeen", "17"),
    ("eighteen", "18"),
    ("nineteen", "19"),
    ("twenty", "20"),
    ("thirty", "30"),
    ("forty", "40"),
    ("fifty", "50"),
    ("sixty", "60"),
    ("seventy", "70"),
    ("eighty", "80"),
    ("ninety", "90"),
];

/// Capitalised names in the original that the edit no longer contains. Advisory only:
/// capitals also mark titles, so a hit is a reason to look. Headings and bold labels are
/// skipped, and a run of more than 3 words reads as a title. shortcut: a lone 'Priya' is
/// not seen; catching it needs a sentence splitter.
pub fn names(original: &str, edited: &str) -> Vec<String> {
    let edited = prose(edited).replace('*', "");
    dedup(name_runs(original))
        .into_iter()
        .filter(|name| !edited.contains(name))
        .collect()
}

/// Runs of 2 or 3 capitalised words outside headings and bold labels.
fn name_runs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let is_label = |line: &&str| {
        let line = line.trim_start();
        line.starts_with('#') || line.starts_with("**") || line.starts_with("- **")
    };
    for line in text.lines().filter(|line| !is_label(line)) {
        let line = prose(line);
        let mut run: Vec<&str> = Vec::new();
        let mut tokens = line.split_whitespace().peekable();
        while let Some(token) = tokens.next() {
            let word = token.trim_matches(|c: char| !c.is_alphanumeric());
            let capital = word.starts_with(char::is_uppercase);
            if capital && !(run.is_empty() && NOT_NAME_START.contains(&word)) {
                run.push(word);
            }
            let closes = token.ends_with(|c: char| !c.is_alphanumeric() && c != '*');
            if !capital || closes || tokens.peek().is_none() {
                if (2..=3).contains(&run.len()) {
                    out.push(run.join(" "));
                }
                run.clear();
            }
        }
    }
    out
}

/// Items the original had and the edit lost.
pub fn missing(original: &str, edited: &str) -> BTreeMap<&'static str, Vec<String>> {
    only_in(original, edited, |had, has| (had, has))
}

/// Items the edit has and the original did not, such as an invented figure or `must`.
/// Numbers, quantities, months, code and URLs count as added only when the value is
/// new, not when an existing one repeats; a meaning word counts when it appears more often.
pub fn added(original: &str, edited: &str) -> BTreeMap<&'static str, Vec<String>> {
    only_in(edited, original, |had, has| (has, had))
}

/// Items in `from` that `to` lacks; `order` maps the 2 meaning-word counts to
/// (before, after) for the report.
fn only_in(
    from: &str,
    to: &str,
    order: fn(usize, usize) -> (usize, usize),
) -> BTreeMap<&'static str, Vec<String>> {
    let mut out: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let values_to: Vec<String> = numbers(to).iter().map(|n| number_value(n)).collect();
    for (kind, extract) in [
        ("number", numbers as fn(&str) -> Vec<String>),
        ("quantity", quantities),
        ("month", months),
        ("code", code_spans),
        ("url", urls),
    ] {
        let key = |x: &String| {
            if kind == "number" {
                number_value(x)
            } else {
                x.clone()
            }
        };
        let kept: Vec<String> = extract(to).iter().map(key).collect();
        let lost: Vec<String> = dedup(extract(from))
            .into_iter()
            .filter(|x| !kept.contains(&key(x)))
            // A lost number already reports itself; a quantity adds only a changed size or unit.
            .filter(|x| {
                kind != "quantity"
                    || x.split(' ')
                        .next()
                        .is_some_and(|n| values_to.iter().any(|v| v == n))
            })
            .collect();
        if !lost.is_empty() {
            out.insert(kind, lost);
        }
    }
    let (in_from, in_to) = (meaning_tokens(from), meaning_tokens(to));
    let lost: Vec<String> = MEANING_WORDS
        .iter()
        .chain(MEANING_PHRASES)
        .filter_map(|w| {
            let (mut f, mut t) = (count(&in_from, w), count(&in_to, w));
            if matches!(*w, "may" | "will") {
                f = f.saturating_sub(not_modal(from, w));
                t = t.saturating_sub(not_modal(to, w));
            }
            let (b, a) = order(f, t);
            (t < f).then(|| format!("'{w}' ({b} before, {a} after)"))
        })
        .collect();
    if !lost.is_empty() {
        out.insert("meaning word", lost);
    }
    out
}

/// Words for meaning counts, with hedges on a number read as one: 'approximately 50' to
/// 'about 50' is not a change.
fn meaning_tokens(text: &str) -> Vec<String> {
    let mut tokens = words(&prose(text));
    for i in 1..tokens.len() {
        if matches!(
            tokens[i - 1].as_str(),
            "approximately" | "roughly" | "around"
        ) && is_number(&tokens[i])
        {
            tokens[i - 1] = "about".to_string();
        }
    }
    tokens
}

/// Uses of 'may' or 'will' that are a month or part of a name, such as 'Will Smith'.
fn not_modal(text: &str, word: &str) -> usize {
    let dated = months(text)
        .iter()
        .filter(|month| month.eq_ignore_ascii_case(word))
        .count();
    let named = name_runs(text)
        .iter()
        .filter(|name| name.split(' ').any(|part| part.eq_ignore_ascii_case(word)))
        .count();
    dated + named
}

fn dedup(mut v: Vec<String>) -> Vec<String> {
    let mut seen = Vec::new();
    v.retain(|x| {
        if seen.contains(x) {
            false
        } else {
            seen.push(x.clone());
            true
        }
    });
    v
}

/// A number's value, so a required edit such as 1000 to 1,000 is not a change. Only
/// commas that group thousands go; '1,2,3' stays a list.
fn number_value(token: &str) -> String {
    let whole = token
        .split('.')
        .next()
        .unwrap_or(token)
        .trim_end_matches('%');
    let mut groups = whole.split(',');
    let first_fits = groups.next().is_some_and(|g| (1..=3).contains(&g.len()));
    if whole.contains(',') && first_fits && groups.all(|g| g.len() == 3) {
        token.replace(',', "")
    } else {
        token.to_string()
    }
}

/// The text without ordered list markers, which number the layout and not the content.
/// shortcut: a hard-wrapped line starting '12. ' reads as a marker, so markers stop at
/// 2 digits; a longer list reports its later markers as numbers.
fn without_list_markers(text: &str) -> String {
    text.lines()
        .map(|line| {
            let body = line.trim_start();
            let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            let marker = (1..=2).contains(&digits)
                && [". ", ".\t", ") ", ")\t"]
                    .iter()
                    .any(|marker| body[digits..].starts_with(marker));
            if marker { &body[digits + 2..] } else { line }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether a percentage word follows, so '5 per cent' and '5%' are the same value.
fn starts_with_percent(text: &str) -> bool {
    let mut next = text.split_whitespace().map(str::to_lowercase);
    match next.next().as_deref() {
        Some(w) if w.trim_end_matches(['.', ',', ';', ':', ')']) == "percent" => true,
        Some("per") => next
            .next()
            .is_some_and(|w| w.trim_end_matches(['.', ',', ';', ':', ')']) == "cent"),
        _ => false,
    }
}

/// A number with the size or unit it carries, attached or as the next word: '$47.3M',
/// '150 km', 'three days'. Read from prose, so a value in code or a URL is not one.
fn quantities(text: &str) -> Vec<String> {
    let text = prose(text);
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut out = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        let body = token.trim_start_matches(|c: char| !c.is_alphanumeric());
        let end = body
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, ',' | '.')))
            .unwrap_or(body.len());
        let (value, attached) = body.split_at(end);
        let (value, attached) = if value.is_empty() {
            let word = body
                .trim_end_matches(|c: char| !c.is_alphanumeric())
                .split('-')
                .next()
                .unwrap_or_default()
                .to_lowercase();
            match number_word(&word) {
                Some(n) => (n, &body[word.len()..]),
                None => continue,
            }
        } else {
            (value, attached)
        };
        let bare = value.trim_end_matches(['.', ',']);
        if bare.is_empty() {
            continue;
        }
        let unit = attached.trim_start_matches('-');
        let word = if !unit
            .trim_end_matches(|c: char| !c.is_alphanumeric())
            .is_empty()
        {
            unit
        } else if value.len() == bare.len() && attached.is_empty() {
            match tokens.get(i + 1) {
                Some(next) => next,
                None => continue,
            }
        } else {
            continue;
        };
        if let Some(unit) = quantity_word(word.trim_end_matches(|c: char| !c.is_alphanumeric())) {
            out.push(format!("{} {unit}", number_value(bare)));
        }
    }
    out
}

fn quantity_word(word: &str) -> Option<&'static str> {
    match word {
        "M" => return Some("million"),
        "m" => return Some("m"),
        "Kb" => return Some("Kb"),
        "Mb" => return Some("Mb"),
        "Gb" => return Some("Gb"),
        "Tb" => return Some("Tb"),
        _ => {}
    }
    let lower = word.to_lowercase();
    QUANTITY_WORDS
        .iter()
        .find(|(spellings, _)| spellings.contains(&lower.as_str()))
        .map(|(_, unit)| *unit)
}

/// Capitalised month names next to a number, so 'May I' and 'march on' do not count.
fn months(text: &str) -> Vec<String> {
    let text = prose(text);
    let tokens: Vec<&str> = text
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()))
        .collect();
    let numeric = |i: usize| {
        tokens
            .get(i)
            .is_some_and(|t| t.starts_with(|c: char| c.is_ascii_digit()))
    };
    tokens
        .iter()
        .enumerate()
        .filter(|(i, _)| numeric(i + 1) || (*i > 0 && numeric(i - 1)))
        .filter_map(|(_, t)| {
            MONTHS
                .iter()
                .find(|(spellings, _)| spellings.contains(t))
                .map(|(_, name)| name.to_string())
        })
        .collect()
}

fn numbers(text: &str) -> Vec<String> {
    let text = without_list_markers(text);
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let starts = chars[i].is_ascii_digit() && (i == 0 || !chars[i - 1].is_alphanumeric());
        if starts {
            let mut j = i;
            while j < chars.len()
                && (chars[j].is_ascii_digit() || matches!(chars[j], ',' | '.' | '%'))
            {
                j += 1;
            }
            let token: String = chars[i..j]
                .iter()
                .collect::<String>()
                .trim_end_matches(['.', ','])
                .to_string();
            let after: String = chars[j..chars.len().min(j + 12)].iter().collect();
            let percent = !token.ends_with('%') && starts_with_percent(&after);
            out.push(if percent { token + "%" } else { token });
            i = j;
        } else {
            i += 1;
        }
    }
    let spelled = words(&prose(&text));
    out.extend(spelled.iter().enumerate().filter_map(|(i, w)| {
        let percent = match spelled.get(i + 1).map(String::as_str) {
            Some("percent") => true,
            Some("per") => spelled.get(i + 2).is_some_and(|w| w == "cent"),
            _ => false,
        };
        number_word(w).map(|n| {
            if percent {
                format!("{n}%")
            } else {
                n.to_string()
            }
        })
    }));
    out
}

fn number_word(w: &str) -> Option<&'static str> {
    NUMBER_WORDS
        .iter()
        .find(|(word, _)| *word == w)
        .map(|(_, n)| *n)
}

fn code_spans(text: &str) -> Vec<String> {
    split_code(text).0
}

/// Code spans and the text outside them. As in Markdown, a span opens and closes
/// with the same number of backticks, and an unmatched run is literal text.
fn split_code(text: &str) -> (Vec<String>, String) {
    let (mut spans, mut outside) = (Vec::new(), String::new());
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        outside.push_str(&rest[..start]);
        let run = backtick_run(&rest[start..]);
        let after = &rest[start + run..];
        match closing_run(after, run) {
            Some(end) => {
                if !after[..end].is_empty() {
                    spans.push(after[..end].to_string());
                }
                outside.push(' ');
                rest = &after[end + run..];
            }
            None => {
                outside.push_str(&rest[start..start + run]);
                rest = after;
            }
        }
    }
    outside.push_str(rest);
    (spans, outside)
}

fn backtick_run(s: &str) -> usize {
    s.len() - s.trim_start_matches('`').len()
}

fn closing_run(s: &str, run: usize) -> Option<usize> {
    let mut i = 0;
    while let Some(p) = s[i..].find('`') {
        let len = backtick_run(&s[i + p..]);
        if len == run {
            return Some(i + p);
        }
        i += p + len;
    }
    None
}

fn urls(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|w| w.find("http").map(|p| &w[p..]))
        .filter(|w| w.starts_with("http://") || w.starts_with("https://"))
        .map(|w| {
            w.trim_end_matches(['.', ',', ')', ';', '\'', '"'])
                .to_string()
        })
        .collect()
}

/// The text without code spans and URLs, so `--all` or a path is not a meaning word.
/// A link label before its URL, as in `[all](https://...)`, is kept.
fn prose(text: &str) -> String {
    split_code(text)
        .1
        .split_whitespace()
        .map(|w| {
            match [w.find("http://"), w.find("https://")]
                .into_iter()
                .flatten()
                .min()
            {
                Some(p) => &w[..p],
                None => w,
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Marks punctuation between words, so a phrase such as 'up to' cannot span 'up. To'.
const BOUNDARY: &str = "|";

/// Lowercase words, with a boundary after punctuation that ends a phrase. Link brackets
/// do not end one ('up [to](url)'), and nor does a mark before a digit ('over .5', '3,000').
fn words(text: &str) -> Vec<String> {
    let chars: Vec<char> = text
        .to_lowercase()
        .replace('\u{2019}', "'")
        .chars()
        .collect();
    let segmented: String = chars
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let ends_phrase = matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '(' | ')')
                && !chars.get(i + 1).is_some_and(char::is_ascii_digit);
            if ends_phrase { '\n' } else { c }
        })
        .collect();
    let mut out = Vec::new();
    for segment in segmented.split('\n') {
        out.extend(
            segment
                .split(|c: char| !(c.is_alphanumeric() || c == '\''))
                .map(|w| w.trim_matches('\'').to_string())
                .filter(|w| !w.is_empty()),
        );
        out.push(BOUNDARY.to_string());
    }
    out
}

fn is_number(word: &str) -> bool {
    word.starts_with(|c: char| c.is_ascii_digit()) || number_word(word).is_some()
}

/// Occurrences of a word or space-separated phrase, where `N` matches a number.
fn count(words: &[String], phrase: &str) -> usize {
    let parts: Vec<&str> = phrase.split(' ').collect();
    words
        .windows(parts.len())
        .filter(|window| {
            window.iter().zip(&parts).all(|(w, p)| match *p {
                "N" => is_number(w),
                _ => w == p,
            })
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_dropped_numbers_code_urls_and_negations() {
        let original = "Over 3,000 records, up to 4 at once. Run `make test`. See https://example.com/a. Exports must not include deleted accounts.";
        let edited = "Over 3,000 records. Run tests. Exports should include accounts.";
        let m = missing(original, edited);
        assert_eq!(m["number"], vec!["4"]);
        assert_eq!(m["code"], vec!["make test"]);
        assert_eq!(m["url"], vec!["https://example.com/a"]);
        assert_eq!(
            m["meaning word"],
            vec![
                "'not' (1 before, 0 after)",
                "'must' (1 before, 0 after)",
                "'up to' (1 before, 0 after)"
            ]
        );
    }

    #[test]
    fn reports_changed_quantifiers_modals_and_bounds() {
        let original =
            "Some users may see over 3,000 errors, up to 4 at once, and should retry three times.";
        let edited = "All users must see 3,000 errors, 4 at once, and must retry 3 times.";
        let m = missing(original, edited);
        assert!(!m.contains_key("number"), "three and 3 are the same number");
        assert_eq!(
            m["meaning word"],
            vec![
                "'may' (1 before, 0 after)",
                "'some' (1 before, 0 after)",
                "'should' (1 before, 0 after)",
                "'up to' (1 before, 0 after)",
                "'over N' (1 before, 0 after)"
            ]
        );
        let a = added(original, edited);
        assert!(!a.contains_key("number"));
        assert_eq!(
            a["meaning word"],
            vec!["'must' (0 before, 2 after)", "'all' (0 before, 1 after)"]
        );
    }

    #[test]
    fn link_labels_keep_their_meaning_words() {
        let m = missing(
            "See [all](https://example.com/a) docs. It is [not](https://example.com/b) final.",
            "See [some](https://example.com/a) docs. It is [now](https://example.com/b) final.",
        );
        assert_eq!(
            m["meaning word"],
            vec!["'not' (1 before, 0 after)", "'all' (1 before, 0 after)"]
        );
    }

    #[test]
    fn phrases_do_not_cross_punctuation() {
        let original = "The service is up. To confirm, run a check.";
        let edited = "The service is up. Run a check to confirm.";
        assert!(missing(original, edited).is_empty());
        assert!(added(original, edited).is_empty());
    }

    #[test]
    fn links_and_leading_decimal_points_do_not_split_phrases() {
        let m = missing(
            "Allow up [to](https://example.com) five retries over .5 seconds apart.",
            "Allow five retries .5 seconds apart.",
        );
        assert_eq!(
            m["meaning word"],
            vec![
                "'up to' (1 before, 0 after)",
                "'over N' (1 before, 0 after)"
            ]
        );
    }

    #[test]
    fn double_backtick_code_and_urls_hold_no_meaning_or_number_words() {
        let m = missing(
            "Run ``can --all`` from https://example.com/two-step today.",
            "Run ``may`` from https://example.com/one-step today.",
        );
        assert!(!m.contains_key("meaning word"), "{m:?}");
        assert!(!m.contains_key("number"), "{m:?}");
        assert_eq!(m["code"], vec!["can --all"]);
    }

    #[test]
    fn number_words_match_numerals_and_over_needs_a_number() {
        let original = "Run two checks over the weekend; twelve passed.";
        let edited = "Run 2 checks at the weekend; 12 passed.";
        assert!(missing(original, edited).is_empty());
        assert!(added(original, edited).is_empty());
    }

    #[test]
    fn a_faithful_edit_reports_nothing() {
        let original = "Due on 15 April. It may not run on Windows; see `--force`.";
        let edited = "It may not run on Windows (see `--force`). Due on 15 April.";
        assert!(missing(original, edited).is_empty());
        assert!(added(original, edited).is_empty());
    }

    #[test]
    fn reports_invented_figures_links_code_and_obligations() {
        let original = "Refund the fee. See the policy.";
        let edited = "You must refund the $1,200 fee within 14 days. See https://example.com/p and run `refund --all`.";
        let a = added(original, edited);
        assert_eq!(a["number"], vec!["1,200", "14"]);
        assert_eq!(a["url"], vec!["https://example.com/p"]);
        assert_eq!(a["code"], vec!["refund --all"]);
        assert_eq!(a["meaning word"], vec!["'must' (0 before, 1 after)"]);
        assert!(missing(original, edited).is_empty());
    }

    #[test]
    fn reports_changed_magnitudes_and_units_once() {
        let m = missing(
            "Revenue was $47.3 million. The site is 150 km away.",
            "Revenue was $47.3 billion. The site is 150 miles away.",
        );
        assert_eq!(m["quantity"], vec!["47.3 million", "150 km"]);
        assert!(!m.contains_key("number"), "{m:?}");
        let a = added(
            "Revenue was $47.3 million. The site is 150 km away.",
            "Revenue was $47.3 billion. The site is 150 miles away.",
        );
        assert_eq!(a["quantity"], vec!["47.3 billion", "150 mile"]);
    }

    #[test]
    fn quantity_spellings_match_and_a_dropped_number_reports_once() {
        for (original, edited) in [
            ("Revenue was $47.3M.", "Revenue was $47.3 million."),
            ("Allow 3 days.", "Allow a 3-day window, or three days."),
            ("It weighs 5kg.", "It weighs 5 kilograms."),
            ("Fees of 2bn.", "Fees of 2 billion."),
        ] {
            assert!(missing(original, edited).is_empty(), "{original}");
            assert!(added(original, edited).is_empty(), "{original}");
        }
        let lower_case_m = missing("A 5m cable.", "A 5M cable.");
        assert_eq!(lower_case_m["quantity"], vec!["5 m"]);
        let m = missing("Revenue was $47.3 million.", "Revenue rose.");
        assert_eq!(m["number"], vec!["47.3"]);
        assert!(!m.contains_key("quantity"), "{m:?}");
    }

    #[test]
    fn reports_a_changed_month_and_reads_dated_may_as_a_month() {
        let original = "The contract ends on 3 March 2027.";
        let edited = "The contract ends on 3 May 2027.";
        let m = missing(original, edited);
        assert_eq!(m["month"], vec!["March"]);
        assert!(!m.contains_key("meaning word"), "{m:?}");
        let a = added(original, edited);
        assert_eq!(a["month"], vec!["May"]);
        assert!(!a.contains_key("meaning word"), "{a:?}");
        assert_eq!(
            missing("It may run on 3 May.", "It runs on 3 May.")["meaning word"],
            vec!["'may' (1 before, 0 after)"]
        );
        assert!(missing("Due 3 Sept.", "Due 3 September.").is_empty());
        assert!(!missing("March on. May I?", "Go on. Ask.").contains_key("month"));
    }

    #[test]
    fn reports_dropped_hedges_commitments_conditions_and_order() {
        let m = missing(
            "Costs rose about 50%. It will likely pass if tested. Pay before 3 March.",
            "Costs rose 50%. It passes when tested. Pay after 3 March.",
        );
        assert_eq!(
            m["meaning word"],
            vec![
                "'will' (1 before, 0 after)",
                "'if' (1 before, 0 after)",
                "'before' (1 before, 0 after)",
                "'likely' (1 before, 0 after)",
                "'about N' (1 before, 0 after)"
            ]
        );
        assert!(missing("Ask about the fee.", "Ask the fee.").is_empty());
    }

    #[test]
    fn required_style_edits_are_not_drops_or_additions() {
        for (original, edited) in [
            (
                "We processed 1000 invoices.",
                "We processed 1,000 invoices.",
            ),
            ("Costs rose 5 per cent.", "Costs rose 5%."),
            ("Costs rose 5 percent.", "Costs rose 5%."),
            ("Costs rose five per cent.", "Costs rose 5%."),
            (
                "1. Open the file.\n2) Save it.",
                "Open the file, then save it.",
            ),
        ] {
            assert!(missing(original, edited).is_empty(), "{original}");
            assert!(added(original, edited).is_empty(), "{original}");
        }
        assert_eq!(
            missing("Costs rose 5%.", "Costs rose 5.")["number"],
            vec!["5%"]
        );
        assert_eq!(
            missing("Items 1,2,3 failed.", "Items 123 failed.")["number"],
            vec!["1,2,3"]
        );
        assert_eq!(
            missing("In 2026. It ran.", "It ran.")["number"],
            vec!["2026"]
        );
    }

    #[test]
    fn a_hyphenated_number_word_matches_its_numeral() {
        for (original, edited) in [
            ("A five-day window.", "A 5-day window."),
            ("A 5-day window.", "A five-day window."),
        ] {
            assert!(missing(original, edited).is_empty(), "{original}");
            assert!(added(original, edited).is_empty(), "{original}");
        }
        assert_eq!(
            missing("A five-day window.", "A 5-week window.")["quantity"],
            vec!["5 day"]
        );
    }

    #[test]
    fn bytes_and_bits_are_different_units() {
        assert_eq!(
            missing("Send 100 MB.", "Send 100 Mb.")["quantity"],
            vec!["100 MB"]
        );
        assert!(missing("Send 100 MB.", "Send 100 megabytes.").is_empty());
        assert!(missing("Send 100 Mb.", "Send 100 megabits.").is_empty());
    }

    #[test]
    fn a_name_is_not_a_modal() {
        for original in ["Will Smith signed the plan.", "May Parker signed the plan."] {
            let m = missing(original, "The plan was signed.");
            assert!(!m.contains_key("meaning word"), "{m:?}");
        }
        assert_eq!(
            missing("It will pass.", "It passes.")["meaning word"],
            vec!["'will' (1 before, 0 after)"]
        );
        assert_eq!(
            missing("Will Smith will sign.", "Will Smith signs.")["meaning word"],
            vec!["'will' (1 before, 0 after)"]
        );
    }

    #[test]
    fn equivalent_hedges_match_and_a_dropped_hedge_reports() {
        for hedge in ["approximately", "roughly", "around"] {
            let hedged = format!("Costs rose {hedge} 50%.");
            assert!(
                missing(&hedged, "Costs rose about 50%.").is_empty(),
                "{hedge}"
            );
            assert!(
                added(&hedged, "Costs rose about 50%.").is_empty(),
                "{hedge}"
            );
            assert_eq!(
                missing(&hedged, "Costs rose 50%.")["meaning word"],
                vec!["'about N' (1 before, 0 after)"],
                "{hedge}"
            );
        }
        assert!(missing("Walk around 3 sites.", "Walk 3 sites.").contains_key("meaning word"));
        assert_eq!(
            missing("Roughly half left.", "Half left.")["meaning word"],
            vec!["'roughly' (1 before, 0 after)"]
        );
    }

    #[test]
    fn a_tab_after_a_list_marker_is_still_a_marker() {
        assert!(missing("1.\tOpen it.\n2)\tSave it.", "Open it, then save it.").is_empty());
    }

    #[test]
    fn names_are_advisory_and_ignore_a_leading_common_word() {
        assert_eq!(
            names(
                "Priya Nair approved it. The Audit Committee met.",
                "It was approved."
            ),
            vec!["Priya Nair", "Audit Committee"]
        );
        assert!(names("Priya Nair approved it.", "**Priya Nair** signed off.").is_empty());
        assert!(names("The file is here. Run it.", "Here it is.").is_empty());
        assert!(
            names(
                "## Key Changes
**Improved Reliability** added.",
                "Added."
            )
            .is_empty()
        );
        assert!(
            names(
                "Ended
Priya said.",
                "Priya said."
            )
            .is_empty()
        );
        assert!(names("Enhanced Caching Now Implemented Here.", "Cached.").is_empty());
    }
}
