//! Utility for splitting markdown content into a YAML frontmatter section and a body.
//!
//! Markdown files may optionally begin with a YAML frontmatter block delimited by `---`
//! fences:
//!
//! ```text
//! ---
//! title: My Document
//! date: 2026-01-01
//! ---
//!
//! Body content starts here.
//! ```
//!
//! Files without such a fence are treated as pure body content with no frontmatter.

/// The result of splitting a markdown document into its frontmatter and body parts.
#[derive(Debug, Clone, PartialEq)]
pub struct FrontmatterParts {
    /// The raw YAML frontmatter string (without the surrounding `---` fences).
    /// `None` if the document has no frontmatter.
    pub frontmatter: Option<String>,
    /// The body text — everything after the closing `---` fence (or the entire content
    /// if there is no frontmatter). Includes any leading newline after the closing fence.
    pub body: String,
}

/// Split a markdown document into its optional YAML frontmatter and body.
///
/// Frontmatter is recognized when the document starts with a line that is exactly `---`
/// (no trailing whitespace), followed by any number of lines, and then a closing line
/// that is exactly `---`. If those conditions are not met the entire content is returned
/// as the body with no frontmatter.
///
/// # Arguments
/// - `content` — the raw markdown string to split
///
/// # Returns
/// A [`FrontmatterParts`] containing the optional frontmatter and the body.
pub fn split_frontmatter(content: &str) -> FrontmatterParts {
    // A frontmatter block must start on the very first line.
    let lines: Vec<&str> = content.lines().collect();

    if lines.first() == Some(&"---") {
        // Find the closing fence starting from line 1 (not 0, to avoid matching the
        // opening fence itself).
        if let Some(close_idx) = lines[1..].iter().position(|l| *l == "---") {
            // close_idx is relative to lines[1..], so the absolute index is close_idx + 1.
            let close_abs = close_idx + 1;

            // Frontmatter is the content between the two fences.
            let frontmatter = lines[1..close_abs].join("\n");

            // Body is everything after the closing fence line.
            // We preserve a leading newline if there is content after the fence.
            let body_lines = &lines[close_abs + 1..];
            let body = if body_lines.is_empty() {
                String::new()
            } else {
                let mut s = body_lines.join("\n");
                // Preserve a trailing newline if the original content had one.
                if content.ends_with('\n') {
                    s.push('\n');
                }
                s
            };

            return FrontmatterParts {
                frontmatter: Some(frontmatter),
                body,
            };
        }
    }

    // No valid frontmatter — entire content is the body.
    FrontmatterParts {
        frontmatter: None,
        body: content.to_owned(),
    }
}

/// Reassemble a markdown document from its frontmatter and body parts.
///
/// If `frontmatter` is `Some`, the output begins with `---\n{frontmatter}\n---\n`
/// followed by the body. If `frontmatter` is `None`, the body is returned as-is.
///
/// # Arguments
/// - `frontmatter` — optional YAML frontmatter string (without fences)
/// - `body` — the markdown body content
///
/// # Returns
/// The reassembled markdown string.
pub fn join_frontmatter(frontmatter: Option<&str>, body: &str) -> String {
    match frontmatter {
        Some(fm) => format!("---\n{fm}\n---\n{body}"),
        None => body.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_with_frontmatter() {
        let content = "---\ntitle: Hello\ndate: 2026-01-01\n---\n\nBody text here.\n";
        let parts = split_frontmatter(content);
        assert_eq!(
            parts.frontmatter,
            Some("title: Hello\ndate: 2026-01-01".to_owned())
        );
        assert_eq!(parts.body, "\nBody text here.\n");
    }

    #[test]
    fn split_without_frontmatter() {
        let content = "Just body content.\nSecond line.\n";
        let parts = split_frontmatter(content);
        assert_eq!(parts.frontmatter, None);
        assert_eq!(parts.body, content);
    }

    #[test]
    fn split_frontmatter_only_no_body() {
        let content = "---\ntitle: Hello\n---\n";
        let parts = split_frontmatter(content);
        assert_eq!(parts.frontmatter, Some("title: Hello".to_owned()));
        assert_eq!(parts.body, "");
    }

    #[test]
    fn split_empty_frontmatter() {
        let content = "---\n---\nBody.\n";
        let parts = split_frontmatter(content);
        assert_eq!(parts.frontmatter, Some(String::new()));
        assert_eq!(parts.body, "Body.\n");
    }

    #[test]
    fn split_no_closing_fence() {
        // Only opening fence — not valid frontmatter, treat as body.
        let content = "---\ntitle: Hello\nNo closing fence\n";
        let parts = split_frontmatter(content);
        assert_eq!(parts.frontmatter, None);
        assert_eq!(parts.body, content);
    }

    #[test]
    fn roundtrip_with_frontmatter() {
        let content = "---\ntitle: Hello\n---\n\nBody text.\n";
        let parts = split_frontmatter(content);
        let reassembled = join_frontmatter(parts.frontmatter.as_deref(), &parts.body);
        assert_eq!(reassembled, content);
    }

    #[test]
    fn roundtrip_without_frontmatter() {
        let content = "No frontmatter here.\n";
        let parts = split_frontmatter(content);
        let reassembled = join_frontmatter(parts.frontmatter.as_deref(), &parts.body);
        assert_eq!(reassembled, content);
    }

    #[test]
    fn join_with_frontmatter() {
        let result = join_frontmatter(Some("title: Hello"), "\nBody.\n");
        assert_eq!(result, "---\ntitle: Hello\n---\n\nBody.\n");
    }

    #[test]
    fn join_without_frontmatter() {
        let result = join_frontmatter(None, "Body only.\n");
        assert_eq!(result, "Body only.\n");
    }
}
