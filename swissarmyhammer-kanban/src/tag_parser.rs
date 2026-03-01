//! Parse `#tag` patterns from markdown text.
//!
//! Tags are `#word` tokens where `word` is one or more lowercase alphanumeric
//! characters or hyphens. The parser skips code blocks and inline code.

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
        if trimmed.starts_with('#') && trimmed.chars().nth(1).is_none_or(|c| c == '#' || c == ' ')
        {
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

            // Match #tag — a tag is # followed by any non-whitespace, non-# characters
            if bytes[i] == b'#' {
                // Must be start of line or preceded by whitespace/punctuation (not alphanumeric/underscore)
                let preceded_ok =
                    i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
                if preceded_ok {
                    let start = i + 1;
                    let mut end = start;
                    while end < len
                        && bytes[end] != b'#'
                        && !bytes[end].is_ascii_whitespace()
                    {
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
        if in_fenced_block {
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
            if bytes[i] == b'#' {
                let preceded_ok =
                    i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
                if preceded_ok && line[i..].starts_with(&pattern) {
                    let after = i + pattern.len();
                    // Ensure the match ends at a boundary (whitespace, #, or end)
                    let at_boundary = after >= len
                        || bytes[after] == b'#'
                        || bytes[after].is_ascii_whitespace();
                    if at_boundary {
                        // Skip the tag and any trailing space
                        i = after;
                        if i < len && bytes[i] == b' ' {
                            i += 1;
                        }
                        continue;
                    }
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
        if in_fenced_block {
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
            if bytes[i] == b'#' {
                let preceded_ok =
                    i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
                if preceded_ok && line[i..].starts_with(&old_pattern) {
                    let after = i + old_pattern.len();
                    // Boundary: whitespace, #, or end of line
                    let at_boundary = after >= len
                        || bytes[after] == b'#'
                        || bytes[after].is_ascii_whitespace();
                    if at_boundary {
                        result.push_str(&new_pattern);
                        i = after;
                        continue;
                    }
                }
            }

            let ch = line[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        }
    }

    result
}

/// Normalize a tag slug: replace spaces with underscores, strip `#` and null bytes.
pub fn normalize_slug(raw: &str) -> String {
    raw.chars()
        .map(|c| if c == ' ' { '_' } else { c })
        .filter(|&c| c != '#' && c != '\0')
        .collect::<String>()
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
    fn test_parse_allows_any_non_whitespace_non_hash() {
        // Tags can contain any characters except whitespace and #
        let tags = parse_tags("#Bug #sample! #CamelCase #emoji🎉");
        assert_eq!(tags.len(), 4);
        assert!(tags.contains(&"Bug".to_string()));
        assert!(tags.contains(&"sample!".to_string()));
        assert!(tags.contains(&"CamelCase".to_string()));
        assert!(tags.contains(&"emoji🎉".to_string()));
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
        assert_eq!(
            remove_tag("no tags here", "bug"),
            "no tags here"
        );
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
        assert_eq!(rename_tag(input, "old", "new"), "#new outside `#old` inside");
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

    #[test]
    fn test_normalize_slug() {
        assert_eq!(normalize_slug("Bug Fix"), "Bug_Fix");
        assert_eq!(normalize_slug("high_priority"), "high_priority");
        assert_eq!(normalize_slug("UPPERCASE"), "UPPERCASE");
        assert_eq!(normalize_slug("--trim--"), "--trim--");
        assert_eq!(normalize_slug("keep-123"), "keep-123");
        assert_eq!(normalize_slug("#hashtag"), "hashtag");
        assert_eq!(normalize_slug("émojis 🎉"), "émojis_🎉");
    }
}
