//! Protected items an edit must keep: numbers, inline code, URLs and the words that
//! carry negation, modality and scope. Reports what the original had and the edit lost,
//! and what the edit added. A clean result does not prove the meaning survived; it only
//! rules out these losses. An addition is a reason to check the edit invented nothing,
//! not proof that it did.

use std::collections::BTreeMap;

const MEANING_WORDS: &[&str] = &[
    "not", "never", "no", "none", "nothing", "nor", "neither", "without", "cannot", "can't",
    "don't", "doesn't", "isn't", "won't", "must", "may", "only", "except", "unless",
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
    let (in_from, in_to) = (words(from), words(to));
    let lost: Vec<String> = MEANING_WORDS
        .iter()
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
    out
}

fn code_spans(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .collect()
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

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .replace('\u{2019}', "'")
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .map(|w| w.trim_matches('\'').to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

fn count(words: &[String], w: &str) -> usize {
    words.iter().filter(|x| x.as_str() == w).count()
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
            vec!["'not' (1 before, 0 after)", "'must' (1 before, 0 after)"]
        );
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
