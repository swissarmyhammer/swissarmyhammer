//! Parse `#tag` patterns from markdown text.
//!
//! Tags are `#word` tokens where `word` is one or more alphanumeric characters
//! or hyphens (`[A-Za-z0-9-]`). The character immediately after `#` must be
//! ASCII alphanumeric, so `#[`, `#(`, `#!`, and a leading hyphen `#-x` are not
//! tags. Trailing punctuation is trimmed: `#bug,` and `#bug.` both yield `bug`.
//! The parser skips code blocks and inline code.

use std::collections::BTreeSet;
use std::ops::Range;

/// Whether a line opens or closes a fenced code block.
///
/// `trimmed` is the line with leading whitespace removed. This is the only
/// place the fence markers are written down, so the reader and the writers can
/// never disagree about where a code block starts.
fn is_fence_line(trimmed: &str) -> bool {
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// Whether a line is a markdown heading, which [`parse_tags`] skips whole.
///
/// A `#word` inside a heading is title text, never a tag, so the writers must
/// leave heading lines untouched.
fn is_heading_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('#') && trimmed.chars().nth(1).is_none_or(|c| c == '#' || c == ' ')
}

/// Walk the lines of `text`, flagging the ones that can carry tags.
///
/// Yields every line in order, paired with whether a `#word` on it counts as a
/// tag. Fence lines, lines inside a fenced block, and headings are `false`:
/// [`parse_tags`] ignores them, so the writers copy them verbatim. This is the
/// one markdown state machine behind the reader and both writers.
///
/// The iterator carries the fenced-block state, so **consume it in full**. A
/// caller that stops early (`take`, `find`, `any`) never runs the fence toggle
/// for the lines it skipped, and every later line it does read gets the wrong
/// `tag_bearing` flag.
fn markdown_lines(text: &str) -> impl Iterator<Item = (&str, bool)> {
    let mut in_fenced_block = false;
    text.lines().map(move |line| {
        let trimmed = line.trim_start();
        if is_fence_line(trimmed) {
            in_fenced_block = !in_fenced_block;
            return (line, false);
        }
        (line, !in_fenced_block && !is_heading_line(line))
    })
}

/// Advance past the inline code span that opens at the backtick at byte `i`.
///
/// Returns the byte index just past the closing backtick, or the end of the
/// line when the span never closes. A backtick never appears inside a
/// multi-byte UTF-8 sequence, so the result is always a character boundary and
/// the skipped slice can be copied whole.
fn skip_inline_code(bytes: &[u8], i: usize) -> usize {
    let mut end = i + 1;
    while end < bytes.len() && bytes[end] != b'`' {
        end += 1;
    }
    (end + 1).min(bytes.len())
}

/// The byte range of the slug of a `#tag` marker starting at byte `i`.
///
/// `None` when no marker starts there: the byte is not `#`, the `#` is glued to
/// the end of a word, or the character right after it is not ASCII alphanumeric
/// — which rejects `#[`, `#(`, `#!`, and the leading hyphen in `#-x`.
///
/// The slug runs over `[A-Za-z0-9-]` and ends at the first character outside
/// that set, so `#bug,` is the tag `bug`. This is the module's only boundary
/// rule: the reader and both writers call it, so a marker next to punctuation
/// can never read as a tag that cannot be edited. A consequence for the
/// writers: text the reader does not count as a tag — `#v2.0` reads as `v2` —
/// is never edited under the full literal.
///
/// # Panics
///
/// `i` must be a valid index into `bytes`.
fn tag_slug_at(bytes: &[u8], i: usize) -> Option<Range<usize>> {
    let glued_to_a_word = i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
    if bytes[i] != b'#' || glued_to_a_word {
        return None;
    }
    let start = i + 1;
    if start >= bytes.len() || !bytes[start].is_ascii_alphanumeric() {
        return None;
    }
    let mut end = start;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
        end += 1;
    }
    Some(start..end)
}

/// Extract unique tag slugs (names) from markdown text.
///
/// Returns a deduplicated, sorted list of tag name strings (without the `#` prefix).
/// Skips tags inside fenced code blocks and inline code spans.
pub fn parse_tags(text: &str) -> Vec<String> {
    let mut tags = BTreeSet::new();
    for (line, tag_bearing) in markdown_lines(text) {
        if tag_bearing {
            collect_line_tags(line, &mut tags);
        }
    }
    tags.into_iter().collect()
}

/// Collect the tag slugs of one tag-bearing line, skipping inline code spans.
fn collect_line_tags(line: &str, tags: &mut BTreeSet<String>) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            i = skip_inline_code(bytes, i);
            continue;
        }
        match tag_slug_at(bytes, i) {
            Some(slug) => {
                tags.insert(line[slug.clone()].to_string());
                i = slug.end;
            }
            None => i += 1,
        }
    }
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
    edit_tag_markers(text, slug, None)
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Rename all occurrences of `#old` to `#new` in text.
pub fn rename_tag(text: &str, old_slug: &str, new_slug: &str) -> String {
    edit_tag_markers(text, old_slug, Some(&format!("#{new_slug}")))
}

/// Rewrite every `#slug` marker in `text`.
///
/// `replacement` is what goes in the marker's place: `Some("#new")` renames,
/// `None` removes. One writer, so [`remove_tag`] and [`rename_tag`] share the
/// markdown state machine ([`markdown_lines`]) and the boundary rule
/// ([`tag_slug_at`]) with [`parse_tags`] — whatever the reader counts as a tag,
/// the writers can always edit.
fn edit_tag_markers(text: &str, slug: &str, replacement: Option<&str>) -> String {
    let mut result = String::with_capacity(text.len());
    for (index, (line, tag_bearing)) in markdown_lines(text).enumerate() {
        if index > 0 {
            result.push('\n');
        }
        if tag_bearing {
            edit_line_markers(line, slug, replacement, &mut result);
        } else {
            result.push_str(line);
        }
    }
    result
}

/// Rewrite the `#slug` markers of one tag-bearing line into `out`.
///
/// Inline code spans are copied through untouched, matching what [`parse_tags`]
/// skips. Removal absorbs one adjacent space so the prose keeps no hole: the
/// space after the marker goes when there is one; a marker touching punctuation
/// (`#bug,`) has none, so the space in front of it goes instead.
fn edit_line_markers(line: &str, slug: &str, replacement: Option<&str>, out: &mut String) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let end = skip_inline_code(bytes, i);
            out.push_str(&line[i..end]);
            i = end;
            continue;
        }
        match tag_slug_at(bytes, i).filter(|found| line[found.clone()] == *slug) {
            Some(found) => {
                i = found.end;
                if let Some(text) = replacement {
                    out.push_str(text);
                } else if i < bytes.len() && bytes[i] == b' ' {
                    i += 1;
                } else if out.ends_with(' ') {
                    out.pop();
                }
            }
            None => {
                let ch = line[i..].chars().next().unwrap();
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
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
