//! Parse `#tag` patterns from markdown text.
//!
//! Tags are `#word` tokens where `word` is one or more alphanumeric characters
//! or hyphens (`[A-Za-z0-9-]`). The character immediately after `#` must be
//! ASCII alphanumeric, so `#[`, `#(`, `#!`, and a leading hyphen `#-x` are not
//! tags. Trailing punctuation is trimmed: `#bug,` and `#bug.` both yield `bug`.
//! The parser skips code blocks and inline code.

use std::collections::BTreeSet;

/// Fenced-code-block delimiters. A line whose trimmed-start begins with either
/// marker toggles fenced-block state. Hoisted to one place so `parse_tags`,
/// `remove_tag`, and `rename_tag` agree on what opens/closes a fence.
const FENCE_MARKERS: [&str; 2] = ["```", "~~~"];

/// True when `trimmed` (a line with leading whitespace already stripped) opens
/// or closes a fenced code block.
fn is_fence_marker(trimmed: &str) -> bool {
    FENCE_MARKERS.iter().any(|m| trimmed.starts_with(m))
}

/// Compute the byte range `[start, end)` of an inline-code span that begins at
/// the opening backtick at `bytes[hash_backtick]`.
///
/// `end` is one byte past the closing backtick, or `bytes.len()` when the span
/// is unterminated (a lone backtick runs to end of line). The returned range
/// always includes the opening backtick and, when present, the closing one —
/// matching the long-standing inline-code skip the three scanners share.
fn inline_code_span(bytes: &[u8], open: usize) -> (usize, usize) {
    let len = bytes.len();
    let mut end = open + 1;
    while end < len && bytes[end] != b'`' {
        end += 1;
    }
    if end < len {
        end += 1; // include the closing backtick
    }
    (open, end)
}

/// Reaction a [`LineVisitor`] gives the scanner at a `#tag` boundary.
enum TagAction {
    /// The visitor consumed a tag spanning `[hash, end)`; resume scanning at `end`.
    Consumed(usize),
    /// Not a tag for this visitor; treat the `#` as a literal byte.
    Skipped,
}

/// Per-line scanning callbacks shared by `parse_tags`, `remove_tag`, and
/// `rename_tag`.
///
/// The scanner ([`scan_line`]) owns the byte walk — inline-code skipping and
/// `#`-boundary detection — and hands each interesting event to the visitor.
/// The visitor decides what a tag means (extract a slug, delete it, rename it)
/// and how non-tag bytes are handled (dropped, or copied to an output buffer).
trait LineVisitor {
    /// Invoked at a `#` whose preceding byte is a valid tag boundary
    /// (start-of-line or a non-alphanumeric, non-`_` byte). `hash` is the byte
    /// index of the `#`. The visitor inspects `line[hash..]` and returns
    /// whether it consumed a tag and how far.
    fn on_tag(&mut self, line: &str, hash: usize) -> TagAction;

    /// Invoked for a single literal byte at `idx` that is neither inside an
    /// inline-code span nor part of a consumed tag. Implementors that rebuild
    /// the line copy the corresponding UTF-8 character; extractors ignore it.
    fn on_literal(&mut self, _line: &str, _idx: usize) {}

    /// Invoked for an inline-code span `[start, end)` (including the backticks).
    /// Implementors that rebuild the line copy it verbatim; extractors ignore it.
    fn on_code_span(&mut self, _line: &str, _start: usize, _end: usize) {}
}

/// Walk one prose line, dispatching inline-code spans and `#tag` boundaries to
/// `visitor`. This is the single byte-scan all three public functions share.
///
/// Inline-code spans are detected first (so `#tag` inside backticks is never a
/// tag), then `#` boundaries: a `#` counts as a tag start only when it is at
/// the line start or preceded by a byte that is not ASCII-alphanumeric and not
/// `_`. Everything else is reported via [`LineVisitor::on_literal`], advancing
/// by whole UTF-8 characters so multibyte text is never split.
fn scan_line<V: LineVisitor>(line: &str, visitor: &mut V) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'`' {
            let (start, end) = inline_code_span(bytes, i);
            visitor.on_code_span(line, start, end);
            i = end;
            continue;
        }

        if bytes[i] == b'#' {
            // A tag start must be at line start or preceded by a byte that is
            // neither alphanumeric nor `_` (so `a#b` and `a_#b` are not tags).
            let preceded_ok =
                i == 0 || (!bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_');
            if preceded_ok {
                if let TagAction::Consumed(end) = visitor.on_tag(line, i) {
                    i = end;
                    continue;
                }
            }
            visitor.on_literal(line, i);
            i += 1;
            continue;
        }

        visitor.on_literal(line, i);
        i += line[i..].chars().next().unwrap().len_utf8();
    }
}

/// Visitor that collects unique tag slugs from prose lines.
struct CollectTags<'a> {
    tags: &'a mut BTreeSet<String>,
}

impl LineVisitor for CollectTags<'_> {
    fn on_tag(&mut self, line: &str, hash: usize) -> TagAction {
        let bytes = line.as_bytes();
        let len = bytes.len();
        let start = hash + 1;
        // The char right after `#` must be alphanumeric, else it is not a tag
        // (rejects "#[", "#(", "#!", and leading-hyphen "#-x").
        if start >= len || !bytes[start].is_ascii_alphanumeric() {
            return TagAction::Skipped;
        }
        let mut end = start;
        // Slug runs over [A-Za-z0-9-]; stop at the first char outside it,
        // which naturally trims trailing punctuation ("#bug," -> "bug").
        while end < len && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
            end += 1;
        }
        self.tags.insert(line[start..end].to_string());
        TagAction::Consumed(end)
    }
}

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
        if is_fence_marker(trimmed) {
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

        scan_line(line, &mut CollectTags { tags: &mut tags });
    }

    tags.into_iter().collect()
}

/// Compute the `tags` field value for an entity body.
///
/// Returns the deduplicated `#tag` slugs found in `body` as a JSON string
/// array — exactly what [`parse_tags`] yields, wrapped for the computed-field
/// layer.
///
/// This is the **single source of truth** for the `tags` computed field. Both
/// the read path (the `ComputeEngine` derivation registered in
/// `crate::defaults::register_parse_body_tags`, run on every entity read) and
/// the write path (`crate::derive_handlers::ParseBodyTags::compute`) delegate
/// here, so the value the inspector displays and the baseline the tag-field
/// editor diffs against can never diverge. The body is the source of truth:
/// every `#tag` in the body appears, with no gating on whether a matching
/// `tag` entity already exists.
pub fn body_tags_value(body: &str) -> serde_json::Value {
    serde_json::Value::Array(
        parse_tags(body)
            .into_iter()
            .map(serde_json::Value::String)
            .collect(),
    )
}

/// Append `#tag` to the end of description text.
///
/// If the text already contains the tag, this is a no-op.
/// Adds a space before the tag if the text doesn't end with whitespace.
pub fn append_tag(text: &str, slug: &str) -> String {
    // Check if tag already present
    let existing = parse_tags(text);
    if existing.iter().any(|t| t.as_str() == slug) {
        return text.to_string();
    }

    let mut result = text.to_string();
    if !result.is_empty() && !result.ends_with(char::is_whitespace) {
        result.push(' ');
    }
    result.push('#');
    result.push_str(slug);
    result
}

/// True when the literal tag `pattern` (`#slug`) starting at `bytes[hash]` ends
/// at a tag boundary — whitespace, another `#`, or end of line. Shared by the
/// remove/rename visitors so their match condition stays identical.
fn literal_tag_match_end(line: &str, hash: usize, pattern: &str) -> Option<usize> {
    if !line[hash..].starts_with(pattern) {
        return None;
    }
    let bytes = line.as_bytes();
    let after = hash + pattern.len();
    let at_boundary =
        after >= bytes.len() || bytes[after] == b'#' || bytes[after].is_ascii_whitespace();
    at_boundary.then_some(after)
}

/// Rebuild a line into `out`, copying everything verbatim except the matched
/// literal tag `#slug`, which is rewritten by `on_match`.
///
/// `on_match` receives the matched range `[hash, after)` and the output buffer,
/// returns the byte index to resume scanning from, and is responsible for any
/// replacement text. This factors the only difference between `remove_tag`
/// (drops the tag plus a trailing space) and `rename_tag` (writes the new tag).
struct RewriteTag<'a, F> {
    out: &'a mut String,
    pattern: &'a str,
    on_match: F,
}

impl<F> LineVisitor for RewriteTag<'_, F>
where
    F: FnMut(&str, usize, usize, &mut String) -> usize,
{
    fn on_tag(&mut self, line: &str, hash: usize) -> TagAction {
        match literal_tag_match_end(line, hash, self.pattern) {
            Some(after) => {
                let end = (self.on_match)(line, hash, after, self.out);
                TagAction::Consumed(end)
            }
            None => TagAction::Skipped,
        }
    }

    fn on_literal(&mut self, line: &str, idx: usize) {
        self.out.push(line[idx..].chars().next().unwrap());
    }

    fn on_code_span(&mut self, line: &str, start: usize, end: usize) {
        self.out.push_str(&line[start..end]);
    }
}

/// Run a [`RewriteTag`] visitor over every prose line of `text`, passing
/// fenced-code lines through verbatim and re-joining with `\n`.
///
/// `pattern` is the literal `#slug` to look for; `on_match` rewrites each match
/// into the per-line output buffer (see [`RewriteTag`]).
fn rewrite_tags<F>(text: &str, pattern: &str, mut on_match: F) -> String
where
    F: FnMut(&str, usize, usize, &mut String) -> usize,
{
    let mut result = String::with_capacity(text.len());
    let mut in_fenced_block = false;
    let mut first_line = true;

    for line in text.lines() {
        if !first_line {
            result.push('\n');
        }
        first_line = false;

        let trimmed = line.trim_start();
        if is_fence_marker(trimmed) {
            in_fenced_block = !in_fenced_block;
            result.push_str(line);
            continue;
        }
        if in_fenced_block {
            result.push_str(line);
            continue;
        }

        let mut visitor = RewriteTag {
            out: &mut result,
            pattern,
            on_match: &mut on_match,
        };
        scan_line(line, &mut visitor);
    }

    result
}

/// Remove all occurrences of `#tag` from description text.
///
/// Cleans up surrounding whitespace so no double-spaces remain.
pub fn remove_tag(text: &str, slug: &str) -> String {
    let pattern = format!("#{}", slug);
    let result = rewrite_tags(text, &pattern, |line, _hash, after, _out| {
        // Drop the tag and a single trailing space so no double-space remains.
        let bytes = line.as_bytes();
        if after < bytes.len() && bytes[after] == b' ' {
            after + 1
        } else {
            after
        }
    });

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
    rewrite_tags(text, &old_pattern, |_line, _hash, after, out| {
        out.push_str(&new_pattern);
        after
    })
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

    // Characterization tests pinning the shared `scan_line` byte-scan behavior
    // across parse/remove/rename, so the consolidation stays byte-for-byte.

    #[test]
    fn test_parse_unterminated_inline_code_skips_rest_of_line() {
        // An opening backtick with no close runs to end of line; the #tag after
        // it is inside the (unterminated) code span and must not be extracted.
        assert!(parse_tags("text `#nope to the end").is_empty());
    }

    #[test]
    fn test_remove_tag_unterminated_inline_code_preserved() {
        // The whole unterminated code span is copied verbatim, tag and all.
        let input = "keep `#bug runs on";
        assert_eq!(remove_tag(input, "bug"), "keep `#bug runs on");
    }

    #[test]
    fn test_hash_not_a_tag_when_preceded_by_alnum_or_underscore() {
        // `a#b` and `a_#b` are not tag boundaries — preserved by all three.
        assert!(parse_tags("a#bug x_#bug").is_empty());
        assert_eq!(remove_tag("a#bug x_#bug", "bug"), "a#bug x_#bug");
        assert_eq!(rename_tag("a#bug x_#bug", "bug", "fix"), "a#bug x_#bug");
    }

    #[test]
    fn test_remove_adjacent_tag_first_matches_second_is_not_a_boundary() {
        // `#a#b`: removing `#a` works because the byte after it is `#`, a valid
        // trailing boundary. But `#b` is preceded by `a` (alphanumeric), so it
        // is NOT a tag start and removing `b` is a no-op. This pins the
        // long-standing boundary semantics shared by the byte-scan.
        assert_eq!(remove_tag("x #a#b y", "a"), "x #b y");
        assert_eq!(remove_tag("x #a#b y", "b"), "x #a#b y");
    }

    #[test]
    fn test_rename_multiple_occurrences_on_one_line() {
        assert_eq!(
            rename_tag("#bug then #bug again", "bug", "fix"),
            "#fix then #fix again"
        );
    }

    #[test]
    fn test_remove_tag_no_match_when_longer_slug() {
        // `#bugfix` must not match a removal of `#bug` (boundary check fails:
        // the byte after `#bug` is `f`, not whitespace/#/end).
        assert_eq!(remove_tag("see #bugfix here", "bug"), "see #bugfix here");
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
