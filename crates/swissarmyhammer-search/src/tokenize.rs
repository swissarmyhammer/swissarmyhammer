//! Code-aware tokenization primitives.
//!
//! These are pure functions with no DB or embedding access. They feed the BM25
//! and trigram-Dice scoring stages. There is deliberately no Porter stemming:
//! fuzziness is carried by the character-trigram signal, not by stemming.

use convert_case::{split, Boundary};
use unicode_segmentation::UnicodeSegmentation;

/// Boundaries used to split each Unicode word run into identifier sub-words.
///
/// We include case + acronym + delimiter boundaries:
/// - [`Boundary::LowerUpper`] — `getUser` -> `get`, `User`
/// - [`Boundary::Acronym`] — `HTTPResponse` -> `HTTP`, `Response`
/// - [`Boundary::Underscore`] — `snake_case` -> `snake`, `case`
/// - [`Boundary::Hyphen`] — `kebab-case` -> `kebab`, `case`
/// - [`Boundary::Space`] — defensive; Unicode word splitting normally removes
///   spaces before we reach here.
///
/// `LowerUpper` + `Acronym` together cover every `camelCase`/`PascalCase` and
/// acronym-run case we need. We deliberately do NOT include
/// [`Boundary::UpperLower`]: it splits at every uppercase→lowercase pair, which
/// would shatter the leading capital off each word (`User` -> `U`, `ser`).
///
/// We also deliberately EXCLUDE every digit boundary (`LowerDigit`,
/// `UpperDigit`, `DigitLower`, `DigitUpper`) so that identifiers like `sha256`,
/// `utf8`, and `base64` stay whole rather than splitting the digit run off.
const BOUNDARIES: &[Boundary] = &[
    Boundary::LowerUpper,
    Boundary::Acronym,
    Boundary::Underscore,
    Boundary::Hyphen,
    Boundary::Space,
];

/// Tokenize text into lowercased, code-aware terms.
///
/// The text is first passed through Unicode word segmentation (UAX #29) to strip
/// punctuation and whitespace, then each word run is split on [`BOUNDARIES`] to
/// break `camelCase`, `PascalCase`, acronym runs, and `snake`/`kebab` delimiters.
/// Each resulting segment is lowercased.
///
/// Duplicates are preserved (BM25 needs term frequency, so terms are not
/// deduplicated) and empty segments are dropped.
pub fn tokenize(text: &str) -> Vec<String> {
    text.unicode_words()
        .flat_map(|word| {
            split(&word, BOUNDARIES)
                .into_iter()
                .filter(|seg| !seg.is_empty())
                .map(|seg| seg.to_lowercase())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Return the sliding length-3 character windows of `s`, lowercased.
///
/// Windows are taken over `chars()` so the result is Unicode-safe (codepoint,
/// not byte, windows). Strings shorter than 3 characters return an empty `Vec`.
pub fn char_trigrams(s: &str) -> Vec<[char; 3]> {
    let chars: Vec<char> = s.to_lowercase().chars().collect();
    chars.windows(3).map(|w| [w[0], w[1], w[2]]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_case_splits_lowercased() {
        assert_eq!(tokenize("getUserById"), vec!["get", "user", "by", "id"]);
    }

    #[test]
    fn snake_case_splits() {
        assert_eq!(tokenize("get_user_by_id"), vec!["get", "user", "by", "id"]);
    }

    #[test]
    fn acronym_run_splits() {
        assert_eq!(tokenize("getHTTPResponse"), vec!["get", "http", "response"]);
    }

    #[test]
    fn digit_boundary_excluded_keeps_sha256_whole() {
        assert_eq!(tokenize("sha256_hash"), vec!["sha256", "hash"]);
    }

    #[test]
    fn digit_boundary_excluded_keeps_utf8_whole() {
        assert_eq!(tokenize("utf8"), vec!["utf8"]);
    }

    #[test]
    fn punctuation_stripped_no_empty_strings() {
        assert_eq!(
            tokenize("fn parse_config() -> Result"),
            vec!["fn", "parse", "config", "result"]
        );
    }

    #[test]
    fn term_frequency_preserved_duplicates_not_deduped() {
        assert_eq!(tokenize("foo foo bar"), vec!["foo", "foo", "bar"]);
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert_eq!(tokenize(""), Vec::<String>::new());
    }

    #[test]
    fn char_trigrams_sliding_windows_lowercased() {
        // "get_user" has 8 chars -> 6 sliding windows of length 3.
        assert_eq!(
            char_trigrams("get_user"),
            vec![
                ['g', 'e', 't'],
                ['e', 't', '_'],
                ['t', '_', 'u'],
                ['_', 'u', 's'],
                ['u', 's', 'e'],
                ['s', 'e', 'r'],
            ]
        );
    }

    #[test]
    fn char_trigrams_lowercases_input() {
        assert_eq!(
            char_trigrams("ABCD"),
            vec![['a', 'b', 'c'], ['b', 'c', 'd']]
        );
    }

    #[test]
    fn char_trigrams_short_string_is_empty() {
        assert!(char_trigrams("").is_empty());
        assert!(char_trigrams("a").is_empty());
        assert!(char_trigrams("ab").is_empty());
        assert_eq!(char_trigrams("abc"), vec![['a', 'b', 'c']]);
    }
}
