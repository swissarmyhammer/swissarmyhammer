//! Parse `#tag` patterns from markdown text.
//!
//! Tags are `#word` tokens where `word` is one or more alphanumeric characters
//! or hyphens (`[A-Za-z0-9-]`). The character immediately after `#` must be
//! ASCII alphanumeric, so `#[`, `#(`, `#!`, and a leading hyphen `#-x` are not
//! tags. Trailing punctuation is trimmed: `#bug,` and `#bug.` both yield `bug`.
//! The parser skips code blocks and inline code.

use std::collections::BTreeSet;

/// Extract unique tag slugs (names) from markdown text.
///
/// Returns a deduplicated, sorted list of tag name strings (without the `#` prefix).
/// Skips tags inside fenced code blocks and inline code spans.
pub fn parse_tags(text: &str) -> Vec<String> {
    let mut tags = BTreeSet::new();
    let mut in_fenced_block = false;

    for line in text.lines() {
        let trimmed = line.trim_start();

        // Toggle fenced code blocks (``` or ~~~)
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fenced_block = !in_fenced_block;
            continue;
        }
        if in_fenced_block {
            continue;
        }

        // Skip headings (lines starting with #)
        if trimmed.starts_with('#') && trimmed.chars().nth(1).is_none_or(|c| c == '#' || c == ' ') {
            continue;
        }

        // Parse inline, skipping backtick spans
        let bytes = line.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            // Skip inline code
            if bytes[i] == b'`' {
                i += 1;
                while i < len && bytes[i] != b'`' {
                    i += 1;
                }
                if i < len {
                    i += 1; // skip closing backtick
                }
                continue;
            }

            // Match #tag — a slug of [A-Za-z0-9-], requiring an alphanumeric first char
            if bytes[i] == b'#' {
                // Must be start of line or preceded by whitespace/punctuation (not alphanumeric/underscore)
                let preceded_ok =
                    i == 0 || (!bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_');
                // The char right after # must be alphanumeric, else it is not a tag
                // (rejects "#[", "#(", "#!", and leading-hyphen "#-x").
                let first_ok = i + 1 < len && bytes[i + 1].is_ascii_alphanumeric();
                if preceded_ok && first_ok {
                    let start = i + 1;
                    let mut end = start;
                    // Slug runs over [A-Za-z0-9-]; stop at the first char outside it,
                    // which naturally trims trailing punctuation ("#bug," -> "bug").
                    while end < len && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
                        end += 1;
                    }
                    if end > start {
                        let slug = &line[start..end];
                        tags.insert(slug.to_string());
                        i = end;
                        continue;
                    }
                }
            }

            i += 1;
        }
    }

    tags.into_iter().collect()
}

/// Whether a `#slug` match that ends at byte `after` really ends there.
///
/// [`parse_tags`] runs a slug over `[A-Za-z0-9-]` and stops at the first
/// character outside that set, so `#bug,` is the tag `bug`. Every writer
/// ([`remove_tag`], [`rename_tag`]) must end its match by the same rule, or a
/// marker sitting next to punctuation reads as a tag but cannot be edited.
fn slug_ends_at(bytes: &[u8], after: usize) -> bool {
    after >= bytes.len() || !(bytes[after].is_ascii_alphanumeric() || bytes[after] == b'-')
}

/// Whether a `#slug` match starting at byte `i` is preceded by a slug
/// character, which would make it part of a longer word rather than a marker.
fn slug_starts_at(bytes: &[u8], i: usize) -> bool {
    i == 0 || (!bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_')
}

/// Whether a line is a markdown heading, which [`parse_tags`] skips whole.
///
/// A `#word` inside a heading is title text, never a tag, so the writers must
/// leave heading lines untouched.
fn is_heading_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('#') && trimmed.chars().nth(1).is_none_or(|c| c == '#' || c == ' ')
}

/// Append `#tag` to the end of description text.
///
/// If the text already contains the tag, this is a no-op.
///
/// The marker goes inline at the end of the text, separated by a space, which
/// keeps short descriptions reading naturally. When the last line would swallow
/// it — a fence line, a heading, or a line inside a fenced block, all of which
/// [`parse_tags`] skips — the marker goes on its own line instead, so what is
/// written is always read back as a tag.
///
/// A body with an unbalanced fence can swallow the marker either way. The
/// caller is responsible for checking the round trip (see
/// `task::tags::rewrite_body`) rather than reporting a success that did nothing.
pub fn append_tag(text: &str, slug: &str) -> String {
    if parse_tags(text).iter().any(|t| t.as_str() == slug) {
        return text.to_string();
    }

    let mut inline = text.to_string();
    if !inline.is_empty() && !inline.ends_with(char::is_whitespace) {
        inline.push(' ');
    }
    inline.push('#');
    inline.push_str(slug);
    if parse_tags(&inline).iter().any(|t| t.as_str() == slug) {
        return inline;
    }

    let mut own_line = text.to_string();
    if !own_line.is_empty() && !own_line.ends_with('\n') {
        own_line.push('\n');
    }
    own_line.push('#');
    own_line.push_str(slug);
    own_line
}

/// Remove all occurrences of `#tag` from description text.
///
/// Cleans up surrounding whitespace so no double-spaces remain.
pub fn remove_tag(text: &str, slug: &str) -> String {
    let pattern = format!("#{}", slug);
    let mut result = String::with_capacity(text.len());
    let mut in_fenced_block = false;
    let mut first_line = true;

    for line in text.lines() {
        if !first_line {
            result.push('\n');
        }
        first_line = false;

        let trimmed = line.trim_start();

        // Toggle fenced code blocks
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fenced_block = !in_fenced_block;
            result.push_str(line);
            continue;
        }
        if in_fenced_block || is_heading_line(line) {
            result.push_str(line);
            continue;
        }

        // Process line, removing the tag pattern
        let bytes = line.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            // Skip inline code
            if bytes[i] == b'`' {
                result.push('`');
                i += 1;
                while i < len && bytes[i] != b'`' {
                    let ch = line[i..].chars().next().unwrap();
                    result.push(ch);
                    i += ch.len_utf8();
                }
                if i < len {
                    result.push('`');
                    i += 1;
                }
                continue;
            }

            // Check for #tag pattern
            if bytes[i] == b'#' && slug_starts_at(bytes, i) && line[i..].starts_with(&pattern) {
                let after = i + pattern.len();
                if slug_ends_at(bytes, after) {
                    // Absorb one space so the prose does not keep a hole. The
                    // space after the marker goes when there is one; a marker
                    // touching punctuation ("#bug,") has none, so the space in
                    // front of it goes instead.
                    i = after;
                    if i < len && bytes[i] == b' ' {
                        i += 1;
                    } else if result.ends_with(' ') {
                        result.pop();
                    }
                    continue;
                }
            }

            let ch = line[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        }
    }

    // Clean up trailing whitespace on each line
    result
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Rename all occurrences of `#old` to `#new` in text.
pub fn rename_tag(text: &str, old_slug: &str, new_slug: &str) -> String {
    let old_pattern = format!("#{}", old_slug);
    let new_pattern = format!("#{}", new_slug);
    let mut result = String::with_capacity(text.len());
    let mut in_fenced_block = false;
    let mut first_line = true;

    for line in text.lines() {
        if !first_line {
            result.push('\n');
        }
        first_line = false;

        let trimmed = line.trim_start();

        // Toggle fenced code blocks
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fenced_block = !in_fenced_block;
            result.push_str(line);
            continue;
        }
        if in_fenced_block || is_heading_line(line) {
            result.push_str(line);
            continue;
        }

        // Process line, replacing old tag with new
        let bytes = line.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            // Skip inline code
            if bytes[i] == b'`' {
                result.push('`');
                i += 1;
                while i < len && bytes[i] != b'`' {
                    let ch = line[i..].chars().next().unwrap();
                    result.push(ch);
                    i += ch.len_utf8();
                }
                if i < len {
                    result.push('`');
                    i += 1;
                }
                continue;
            }

            // Check for #old pattern
            if bytes[i] == b'#' && slug_starts_at(bytes, i) && line[i..].starts_with(&old_pattern) {
                let after = i + old_pattern.len();
                if slug_ends_at(bytes, after) {
                    result.push_str(&new_pattern);
                    i = after;
                    continue;
                }
            }

            let ch = line[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        }
    }

    result
}

/// Normalize a tag name into a slug that round-trips through [`parse_tags`].
///
/// The slug charset is `[A-Za-z0-9-]` (case-preserving), matching the parser
/// contract documented at the top of this module. Each maximal run of
/// characters outside that charset — spaces, punctuation, `#`, null bytes, and
/// non-ASCII characters — collapses into a single `-`, and leading/trailing
/// `-` are trimmed. This guarantees that `#{normalize_slug(name)}` written into
/// a body is read back as the same slug, so tagging and parsing stay in sync.
///
/// # Parameters
///
/// - `raw` — the user-supplied tag name (e.g. `"Bug Fix"`, `"v2.0"`).
///
/// # Returns
///
/// The normalized `[A-Za-z0-9-]` slug. Returns the empty string for input with
/// no slug characters.
pub fn normalize_slug(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut last_was_hyphen = true; // synthetic leading boundary suppresses a leading hyphen
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_hyphen = false;
        } else if !last_was_hyphen {
            out.push('-');
            last_was_hyphen = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_tags() {
        let tags = parse_tags("Fix the #bug in #login");
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0], "bug");
        assert_eq!(tags[1], "login");
    }

    #[test]
    fn test_parse_deduplicates() {
        let tags = parse_tags("#bug and #bug again");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], "bug");
    }

    #[test]
    fn test_parse_skips_code_blocks() {
        let tags = parse_tags("text #real\n```\n#fake\n```\nmore #also-real");
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().any(|t| t == "real"));
        assert!(tags.iter().any(|t| t == "also-real"));
        assert!(!tags.iter().any(|t| t == "fake"));
    }

    #[test]
    fn test_parse_skips_inline_code() {
        let tags = parse_tags("use `#not-a-tag` but #real");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], "real");
    }

    #[test]
    fn test_parse_skips_headings() {
        let tags = parse_tags("# Heading\n## Sub heading\n#real-tag here");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], "real-tag");
    }

    #[test]
    fn test_parse_hyphenated_tags() {
        let tags = parse_tags("this is #high-priority stuff");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], "high-priority");
    }

    #[test]
    fn test_parse_tag_at_start() {
        let tags = parse_tags("#bug at the start");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], "bug");
    }

    #[test]
    fn test_parse_tag_at_end() {
        let tags = parse_tags("at the end #bug");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], "bug");
    }

    #[test]
    fn test_parse_no_tags() {
        let tags = parse_tags("no tags here");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_parse_empty() {
        let tags = parse_tags("");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_parse_slug_charset_only() {
        // A slug is [A-Za-z0-9-]; the char after # must be alphanumeric, and the
        // slug stops at the first char outside the charset (trailing trim).
        let tags = parse_tags("#Bug #sample! #CamelCase #emoji🎉");
        assert_eq!(tags.len(), 4);
        assert!(tags.contains(&"Bug".to_string()));
        assert!(tags.contains(&"sample".to_string())); // '!' trims the slug
        assert!(tags.contains(&"CamelCase".to_string()));
        // "#emoji🎉" trims at the non-ASCII char, yielding "emoji"
        assert!(tags.contains(&"emoji".to_string()));
        assert!(!tags.contains(&"emoji🎉".to_string()));
    }

    #[test]
    fn test_parse_rejects_punctuation_after_hash() {
        // The char immediately after # must be ASCII alphanumeric.
        // Regression: "#[serial(cwd)]" must NOT auto-tag (card 01KSR24VH91GS5SN5J3573J6TG).
        assert!(parse_tags("#[serial(cwd)]").is_empty());
        assert!(parse_tags("#(foo)").is_empty());
        assert!(parse_tags("#!x").is_empty());
    }

    #[test]
    fn test_parse_rejects_leading_hyphen() {
        // A leading hyphen is not alphanumeric, so "#-x" is not a tag.
        assert!(parse_tags("#-x").is_empty());
    }

    #[test]
    fn test_parse_trims_trailing_punctuation() {
        assert_eq!(parse_tags("#bug,"), vec!["bug".to_string()]);
        assert_eq!(parse_tags("#bug."), vec!["bug".to_string()]);
    }

    #[test]
    fn test_parse_happy_path_slugs() {
        assert_eq!(parse_tags("#bug"), vec!["bug".to_string()]);
        assert_eq!(
            parse_tags("#multi-word-tag"),
            vec!["multi-word-tag".to_string()]
        );
    }

    #[test]
    fn test_append_tag() {
        assert_eq!(append_tag("some text", "bug"), "some text #bug");
    }

    #[test]
    fn test_append_tag_already_present() {
        assert_eq!(append_tag("has #bug already", "bug"), "has #bug already");
    }

    #[test]
    fn test_append_tag_empty() {
        assert_eq!(append_tag("", "bug"), "#bug");
    }

    #[test]
    fn test_remove_tag() {
        assert_eq!(remove_tag("fix #bug in code", "bug"), "fix in code");
    }

    #[test]
    fn test_remove_tag_at_end() {
        assert_eq!(remove_tag("fix issue #bug", "bug"), "fix issue");
    }

    #[test]
    fn test_remove_tag_not_present() {
        assert_eq!(remove_tag("no tags here", "bug"), "no tags here");
    }

    #[test]
    fn test_remove_tag_preserves_code_blocks() {
        let input = "text #bug\n```\n#bug inside\n```";
        let result = remove_tag(input, "bug");
        assert_eq!(result, "text\n```\n#bug inside\n```");
    }

    #[test]
    fn test_rename_tag() {
        assert_eq!(
            rename_tag("fix #bug in #bug-related code", "bug", "defect"),
            "fix #defect in #bug-related code"
        );
    }

    #[test]
    fn test_rename_tag_preserves_code() {
        let input = "#old outside `#old` inside";
        assert_eq!(
            rename_tag(input, "old", "new"),
            "#new outside `#old` inside"
        );
    }

    #[test]
    fn test_rename_tag_multibyte_chars() {
        // Em dash and other multi-byte UTF-8 chars must not panic
        let input = "description — with #old em dash";
        assert_eq!(
            rename_tag(input, "old", "new"),
            "description — with #new em dash"
        );
    }

    #[test]
    fn test_remove_tag_multibyte_chars() {
        let input = "text — with #bug em dash";
        assert_eq!(remove_tag(input, "bug"), "text — with em dash");
    }

    /// `parse_tags` ends a slug at the first character outside `[A-Za-z0-9-]`,
    /// so `#bug,` IS the tag `bug`. `remove_tag` must end its match the same
    /// way, or a tag next to punctuation is unremovable — and "replace the tag
    /// set" silently keeps it.
    #[test]
    fn test_remove_tag_next_to_punctuation() {
        // A marker touching punctuation has no trailing space to absorb, so the
        // space in front of it goes instead — otherwise the prose keeps an
        // orphan space ("Fix , then ship").
        for (text, expected) in [
            ("fix #bug, then ship", "fix, then ship"),
            ("done #bug.", "done."),
            ("a #bug! b", "a! b"),
            ("see (#bug) here", "see () here"),
        ] {
            let result = remove_tag(text, "bug");
            assert_eq!(result, expected, "remove_tag({text:?})");
            assert!(
                !parse_tags(&result).contains(&"bug".to_string()),
                "remove_tag left {text:?} still tagged: {result:?}"
            );
        }
    }

    /// `parse_tags` skips heading lines, so a `#word` in a heading was never a
    /// tag. `remove_tag` must leave it alone or it eats title text.
    #[test]
    fn test_remove_tag_skips_heading_lines() {
        let input = "# Fix #login\n\nsee also #login";
        assert_eq!(remove_tag(input, "login"), "# Fix #login\n\nsee also");
    }

    /// Same boundary rule as removal: a marker next to punctuation is a tag, so
    /// renaming must reach it.
    #[test]
    fn test_rename_tag_next_to_punctuation() {
        let result = rename_tag("fix #bug, then ship", "bug", "defect");
        assert_eq!(parse_tags(&result), vec!["defect".to_string()]);
    }

    /// Heading text is not a tag, so renaming must not rewrite it.
    #[test]
    fn test_rename_tag_skips_heading_lines() {
        let input = "# Fix #login\n\nsee also #login";
        assert_eq!(
            rename_tag(input, "login", "auth"),
            "# Fix #login\n\nsee also #auth"
        );
    }

    /// Appending must round-trip: whatever `append_tag` writes, `parse_tags`
    /// must read back. A body ending in a fence line or a heading swallows an
    /// inline marker, so those cases need the marker on its own line.
    #[test]
    fn test_append_tag_round_trips_on_bodies_that_swallow_inline_markers() {
        for text in [
            "Repro:\n```\ncargo test\n```",
            "Intro\n\n## Acceptance",
            "# Just a heading",
            "plain body",
            "",
        ] {
            let result = append_tag(text, "bug");
            assert!(
                parse_tags(&result).contains(&"bug".to_string()),
                "append_tag did not round-trip for {text:?}, got: {result:?}"
            );
        }
    }

    #[test]
    fn test_normalize_slug() {
        // Spaces and out-of-charset runs collapse to a single hyphen, so the
        // result round-trips through parse_tags ([A-Za-z0-9-], case-preserving).
        assert_eq!(normalize_slug("Bug Fix"), "Bug-Fix");
        assert_eq!(normalize_slug("high_priority"), "high-priority");
        assert_eq!(normalize_slug("UPPERCASE"), "UPPERCASE");
        assert_eq!(normalize_slug("--trim--"), "trim");
        assert_eq!(normalize_slug("keep-123"), "keep-123");
        assert_eq!(normalize_slug("#hashtag"), "hashtag");
        assert_eq!(normalize_slug("émojis 🎉"), "mojis");
    }

    #[test]
    fn test_normalize_slug_round_trips_through_parse() {
        for raw in ["Bug Fix", "v2.0", "#hashtag", "keep-123", "UPPERCASE"] {
            let slug = normalize_slug(raw);
            let body = append_tag("", &slug);
            assert!(
                parse_tags(&body).contains(&slug),
                "slug {slug:?} from {raw:?} did not round-trip through parse_tags"
            );
        }
    }
}
