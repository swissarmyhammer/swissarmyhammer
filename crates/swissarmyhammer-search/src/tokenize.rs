use convert_case::{split, Boundary};
use unicode_segmentation::UnicodeSegmentation;

const TRIGRAM_WIDTH: usize = 3;

/// Tokenize text using code-aware splitting.
///
/// Process:
/// 1. Split into Unicode words (handles punctuation and whitespace).
/// 2. For each word, apply boundary-based splitting on camelCase, PascalCase, acronyms, underscores, hyphens.
/// 3. Lowercase each segment and preserve term frequency (no dedup, no empty strings).
///
/// Boundaries used: `LowerUpper`, `Acronym`, `Underscore`, `Hyphen`, `Space`.
/// EXCLUDED: digit boundaries, so `sha256`, `utf8`, `base64` stay whole. `UpperLower` is excluded to avoid over-splitting.
pub fn tokenize(text: &str) -> Vec<String> {
    const BOUNDARIES: &[Boundary] = &[
        Boundary::LowerUpper,
        Boundary::Acronym,
        Boundary::Underscore,
        Boundary::Hyphen,
        Boundary::Space,
    ];

    let mut tokens = Vec::new();

    for word in text.unicode_words() {
        // Convert word to String for split function (keep it alive for the duration)
        let word_string = word.to_string();
        let parts = split(&word_string, BOUNDARIES);
        for part in parts {
            if !part.is_empty() {
                tokens.push(part.to_lowercase());
            }
        }
    }

    tokens
}

/// Generate sliding character windows of TRIGRAM_WIDTH from a lowercased string.
///
/// Returns an empty Vec for strings shorter than TRIGRAM_WIDTH chars.
/// Operates on Unicode grapheme boundaries to be Unicode-safe.
pub fn char_trigrams(s: &str) -> Vec<[char; 3]> {
    let chars: Vec<char> = s.chars().collect();
    let mut trigrams = Vec::new();

    if chars.len() < TRIGRAM_WIDTH {
        return trigrams;
    }

    for i in 0..chars.len() - (TRIGRAM_WIDTH - 1) {
        trigrams.push([chars[i], chars[i + 1], chars[i + 2]]);
    }

    trigrams
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_camel_case() {
        let result = tokenize("getUserById");
        assert_eq!(result, vec!["get", "user", "by", "id"]);
    }

    #[test]
    fn tokenize_snake_case() {
        let result = tokenize("get_user_by_id");
        assert_eq!(result, vec!["get", "user", "by", "id"]);
    }

    #[test]
    fn tokenize_acronym() {
        let result = tokenize("getHTTPResponse");
        assert_eq!(result, vec!["get", "http", "response"]);
    }

    #[test]
    fn tokenize_digit_boundary_excluded() {
        let result = tokenize("sha256_hash");
        // sha256 should stay whole (digit boundary excluded), then _hash splits
        assert_eq!(result, vec!["sha256", "hash"]);
    }

    #[test]
    fn tokenize_utf8_stays_whole() {
        let result = tokenize("utf8_encoding");
        assert_eq!(result, vec!["utf8", "encoding"]);
    }

    #[test]
    fn tokenize_base64_stays_whole() {
        let result = tokenize("base64_decoder");
        assert_eq!(result, vec!["base64", "decoder"]);
    }

    #[test]
    fn tokenize_punctuation_stripped() {
        let result = tokenize("fn parse_config() -> Result");
        // Punctuation should be stripped; words extracted and lowercased
        assert_eq!(result, vec!["fn", "parse", "config", "result"]);
    }

    #[test]
    fn tokenize_preserves_term_frequency() {
        let result = tokenize("foo bar foo baz foo");
        // Should preserve duplicates (term frequency)
        assert_eq!(result, vec!["foo", "bar", "foo", "baz", "foo"]);
    }

    #[test]
    fn tokenize_complex_identifier() {
        let result = tokenize("parseHTTPRequest_v2");
        // parse, HTTP (acronym), Request, v2
        assert_eq!(result, vec!["parse", "http", "request", "v2"]);
    }

    #[test]
    fn tokenize_empty_string() {
        let result = tokenize("");
        assert!(result.is_empty());
    }

    #[test]
    fn tokenize_single_word() {
        let result = tokenize("hello");
        assert_eq!(result, vec!["hello"]);
    }

    #[test]
    fn char_trigrams_basic() {
        let result = char_trigrams("get_user");
        let expected = vec![
            ['g', 'e', 't'],
            ['e', 't', '_'],
            ['t', '_', 'u'],
            ['_', 'u', 's'],
            ['u', 's', 'e'],
            ['s', 'e', 'r'],
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn char_trigrams_short_string() {
        let result = char_trigrams("ab");
        assert!(result.is_empty());
    }

    #[test]
    fn char_trigrams_exactly_3_chars() {
        let result = char_trigrams("abc");
        assert_eq!(result, vec![['a', 'b', 'c']]);
    }

    #[test]
    fn char_trigrams_empty_string() {
        let result = char_trigrams("");
        assert!(result.is_empty());
    }

    #[test]
    fn char_trigrams_4_chars() {
        let result = char_trigrams("abcd");
        assert_eq!(result, vec![['a', 'b', 'c'], ['b', 'c', 'd']]);
    }

    #[test]
    fn char_trigrams_unicode() {
        let result = char_trigrams("café");
        // Verify it handles multi-byte Unicode correctly
        assert_eq!(result.len(), 2); // "caf" and "afé"
    }
}
