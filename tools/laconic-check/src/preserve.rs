//! Protected items an edit must keep: numbers, inline code, URLs and the words that
//! carry negation, modality and scope. Reports what the original had and the edit lost,
//! and what the edit added. A clean result does not prove the meaning survived; it only
//! rules out these losses. An addition is a reason to check the edit invented nothing,
//! not proof that it did.

use std::collections::BTreeMap;

const MEANING_WORDS: &[&str] = &[
    "not", "never", "no", "none", "nothing", "nor", "neither", "without", "cannot", "can't",
    "don't", "doesn't", "isn't", "won't", "must", "may", "only", "except", "unless", "some", "all",
    "any", "every", "most", "should", "might", "can", "could", "always",
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

/// Items the original had and the edit lost.
pub fn missing(original: &str, edited: &str) -> BTreeMap<&'static str, Vec<String>> {
    only_in(original, edited, |had, has| (had, has))
}

/// Items the edit has and the original did not, such as an invented figure or `must`.
/// Numbers, code and URLs count as added only when the value is new, not when an
/// existing one repeats; a meaning word counts when it appears more often.
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
    for (kind, extract) in [
        ("number", numbers as fn(&str) -> Vec<String>),
        ("code", code_spans),
        ("url", urls),
    ] {
        let kept = extract(to);
        let lost: Vec<String> = dedup(extract(from))
            .into_iter()
            .filter(|x| !kept.contains(x))
            .collect();
        if !lost.is_empty() {
            out.insert(kind, lost);
        }
    }
    let (in_from, in_to) = (words(&prose(from)), words(&prose(to)));
    let lost: Vec<String> = MEANING_WORDS
        .iter()
        .chain(MEANING_PHRASES)
        .filter_map(|w| {
            let (f, t) = (count(&in_from, w), count(&in_to, w));
            let (b, a) = order(f, t);
            (t < f).then(|| format!("'{w}' ({b} before, {a} after)"))
        })
        .collect();
    if !lost.is_empty() {
        out.insert("meaning word", lost);
    }
    out
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

fn numbers(text: &str) -> Vec<String> {
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
            out.push(token);
            i = j;
        } else {
            i += 1;
        }
    }
    out.extend(
        words(&prose(text))
            .iter()
            .filter_map(|w| number_word(w))
            .map(str::to_string),
    );
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

/// Occurrences of a word or space-separated phrase, where `N` matches a number.
fn count(words: &[String], phrase: &str) -> usize {
    let parts: Vec<&str> = phrase.split(' ').collect();
    words
        .windows(parts.len())
        .filter(|window| {
            window.iter().zip(&parts).all(|(w, p)| match *p {
                "N" => w.starts_with(|c: char| c.is_ascii_digit()) || number_word(w).is_some(),
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
}
