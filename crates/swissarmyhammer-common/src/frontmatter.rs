//! Shared frontmatter parsing functionality
//!
//! [`split_frontmatter_body`] splits text on line-anchored `---` delimiters.
//! Five readers of the frontmatter + markdown body format call it: the entity
//! `io.rs` and `store.rs` readers, `parse_ralph_file` in the ralph MCP tool,
//! the prompt health check, and [`parse_frontmatter`] (through
//! [`parse_frontmatter_with_expansion`]). All five therefore agree on the
//! delimiter rule.
//!
//! It is not the only frontmatter split in the workspace, though.
//! `swissarmyhammer-templating`, `swissarmyhammer-merge`, and `mirdan` each
//! carry a further copy of their own.
//!
//! # YAML Include Expansion
//!
//! Frontmatter supports `@path/to/file` references that expand to the contents
//! of YAML files loaded from the standard directory hierarchy. Use
//! `parse_frontmatter_with_expansion` to enable this feature.
//!
//! ## Example
//!
//! Given `file_groups/source_code.yaml`:
//! ```yaml
//! - "*.js"
//! - "*.ts"
//! ```
//!
//! You can reference it in frontmatter:
//! ```yaml
//! ---
//! match:
//!   files:
//!     - "@file_groups/source_code"
//!     - "*.custom"
//! ---
//! ```

use crate::{Result, SwissArmyHammerError};
use swissarmyhammer_directory::{DirectoryConfig, YamlExpander};

/// Strip a line's terminator, returning the line's content.
///
/// Handles `\n` and `\r\n`. A line with no terminator -- the last line of text
/// that does not end in a newline -- comes back unchanged.
fn line_content(raw: &str) -> &str {
    let without_lf = raw.strip_suffix('\n').unwrap_or(raw);
    without_lf.strip_suffix('\r').unwrap_or(without_lf)
}

/// Whether `raw` is a frontmatter delimiter line: exactly three hyphens, with
/// nothing on the line but its terminator.
fn is_delimiter_line(raw: &str) -> bool {
    line_content(raw) == "---"
}

/// Split frontmatter + body text on line-anchored `---` delimiters.
///
/// Five readers of the frontmatter + markdown body format call it -- the
/// entity `io.rs` and `store.rs` readers, `parse_ralph_file`, the prompt
/// health check, and [`parse_frontmatter`] -- so the delimiter rule holds the
/// same for those five. It is not the only frontmatter split in the
/// workspace: other crates carry copies of their own. Call this one from a
/// new reader rather than writing another.
///
/// The opening delimiter is the first line of `content` and must be exactly
/// three hyphens. The frontmatter runs to the next delimiter line. Returns
/// `(frontmatter, body)`, where `frontmatter` is the bytes between the two
/// delimiter lines and `body` is every byte after the closing delimiter line's
/// terminator. The body is a borrowed slice of `content`, so it keeps its
/// bytes exactly: no newline is added or dropped, and CRLF stays CRLF.
///
/// Only a line that is exactly three hyphens delimits. Three hyphens indented,
/// or with any other text on the line, is ordinary content and stays in the
/// frontmatter. That is what keeps a three-hyphen run inside a YAML scalar --
/// a comment's prose, a title -- from ending the frontmatter early: an emitter
/// writes such a run either indented inside a block scalar or escaped inside a
/// quoted one, never alone at column 0.
///
/// Returns `None` when the first line is not a delimiter line, or when no
/// closing delimiter line follows it. Callers turn that into their own
/// "invalid frontmatter" error, or into "this text has no frontmatter".
pub fn split_frontmatter_body(content: &str) -> Option<(&str, &str)> {
    let opening = content.split_inclusive('\n').next()?;
    if !is_delimiter_line(opening) {
        return None;
    }
    let after_opening = &content[opening.len()..];

    let mut offset = 0;
    for raw in after_opening.split_inclusive('\n') {
        if is_delimiter_line(raw) {
            let frontmatter = &after_opening[..offset];
            let body = &after_opening[offset + raw.len()..];
            return Some((frontmatter, body));
        }
        offset += raw.len();
    }
    None
}

/// Represents parsed frontmatter with metadata and content
#[derive(Debug, Clone)]
pub struct Frontmatter {
    /// Parsed YAML metadata as a serde_json::Value, or None if no frontmatter
    pub metadata: Option<serde_json::Value>,
    /// The content after the frontmatter (or entire content if no frontmatter)
    pub content: String,
}

/// Parses YAML frontmatter from markdown content
///
/// Reads content with YAML frontmatter delimited by `---` markers.
/// If no frontmatter is found, returns the entire content unchanged.
///
/// Delegates the split to [`split_frontmatter_body`], so only a line that is
/// exactly three hyphens delimits: a three-hyphen run indented inside a YAML
/// block scalar, or embedded in a longer line, stays in the frontmatter
/// instead of closing it early.
///
/// # Arguments
/// * `content` - The raw content potentially containing YAML frontmatter
///
/// # Returns
/// * `Ok(Frontmatter)` - Successfully parsed frontmatter and content
/// * `Err(_)` - YAML parsing error if frontmatter is malformed
///
/// # Examples
/// ```
/// use swissarmyhammer_common::frontmatter::parse_frontmatter;
///
/// let content = r#"---
/// title: Example
/// description: A test document
/// ---
///
/// Main Content
/// This is the body.
/// "#;
///
/// let result = parse_frontmatter(content).unwrap();
/// assert!(result.metadata.is_some());
/// assert!(result.content.contains("Main Content"));
/// ```
pub fn parse_frontmatter(content: &str) -> Result<Frontmatter> {
    parse_frontmatter_internal(
        content,
        None::<&YamlExpander<swissarmyhammer_directory::SwissarmyhammerConfig>>,
    )
}

/// Parses YAML frontmatter with `@` include expansion.
///
/// This is like `parse_frontmatter` but expands `@path/to/file` references
/// in the YAML using the provided expander.
///
/// # Arguments
/// * `content` - The raw content potentially containing YAML frontmatter
/// * `expander` - The YAML expander with loaded includes
///
/// # Examples
/// ```ignore
/// use swissarmyhammer_common::frontmatter::parse_frontmatter_with_expansion;
/// use swissarmyhammer_directory::{YamlExpander, SwissarmyhammerConfig};
///
/// let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
/// expander.load_all().unwrap();
///
/// let content = r#"---
/// files:
///   - "@file_groups/source_code"
/// ---
/// Content here
/// "#;
///
/// let result = parse_frontmatter_with_expansion(content, &expander).unwrap();
/// ```
pub fn parse_frontmatter_with_expansion<C: DirectoryConfig>(
    content: &str,
    expander: &YamlExpander<C>,
) -> Result<Frontmatter> {
    parse_frontmatter_internal(content, Some(expander))
}

/// Internal implementation that optionally expands includes.
fn parse_frontmatter_internal<C: DirectoryConfig>(
    content: &str,
    expander: Option<&YamlExpander<C>>,
) -> Result<Frontmatter> {
    // Check for partial marker first - these don't have frontmatter
    if content.trim_start().starts_with("{% partial %}") {
        return Ok(Frontmatter {
            metadata: None,
            content: content.to_string(),
        });
    }

    // Check for YAML frontmatter delimiter
    if let Some((yaml_content, body_content)) = split_frontmatter_body(content) {
        // Parse YAML frontmatter
        let mut yaml_value: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(yaml_content).map_err(|e| SwissArmyHammerError::Other {
                message: format!("Invalid YAML frontmatter: {e}"),
            })?;

        // Expand includes if an expander is provided
        if let Some(exp) = expander {
            yaml_value = exp
                .expand(yaml_value)
                .map_err(|e| SwissArmyHammerError::Other {
                    message: format!("Failed to expand YAML includes: {e}"),
                })?;
        }

        // Convert to JSON for consistent handling
        let json_value =
            serde_json::to_value(yaml_value).map_err(|e| SwissArmyHammerError::Other {
                message: format!("Failed to convert YAML to JSON: {e}"),
            })?;

        return Ok(Frontmatter {
            metadata: Some(json_value),
            content: body_content.to_string(),
        });
    }

    // No frontmatter found, return entire content
    Ok(Frontmatter {
        metadata: None,
        content: content.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_with_yaml() {
        let content = r#"---
title: Test Document
description: A test document
parameters:
  - name: test_param
    required: true
---

# Main Content
This is the body content.
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_some());

        let metadata = result.metadata.as_ref().unwrap();
        assert_eq!(
            metadata.get("title").and_then(|v| v.as_str()),
            Some("Test Document")
        );
        assert_eq!(
            metadata.get("description").and_then(|v| v.as_str()),
            Some("A test document")
        );
        assert!(metadata.get("parameters").is_some());

        assert!(result.content.contains("# Main Content"));
        assert!(result.content.contains("This is the body content."));
        assert!(!result.content.starts_with("---"));
    }

    #[test]
    fn test_parse_frontmatter_without_yaml() {
        let content = r#"# Just Regular Content
This is just regular markdown without frontmatter.
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_parse_frontmatter_with_partial_marker() {
        let content = r#"{% partial %}
<div class="header">
  <h1>{{title}}</h1>
</div>"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_parse_frontmatter_malformed_yaml() {
        let content = r#"---
title: Test
invalid_yaml: [unclosed
---

Content here
"#;

        let result = parse_frontmatter(content);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid YAML frontmatter"));
    }

    #[test]
    fn test_parse_frontmatter_incomplete_delimiter() {
        let content = r#"---
title: Test
description: Missing closing delimiter

Content here
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_parse_frontmatter_empty_yaml() {
        let content = r#"---
---

Content after empty frontmatter
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_some());
        let metadata = result.metadata.as_ref().unwrap();
        assert!(metadata.is_null());
        assert!(result.content.contains("Content after empty frontmatter"));
    }

    #[test]
    fn test_parse_frontmatter_content_preservation() {
        let content = r#"---
title: Test
---

    # Content with Leading Whitespace
This should have leading whitespace preserved.
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_some());

        // Content should preserve ALL whitespace after frontmatter, including newlines and indentation
        assert!(result
            .content
            .starts_with("\n    # Content with Leading Whitespace"));
        assert!(result
            .content
            .contains("This should have leading whitespace"));
    }

    #[test]
    fn test_parse_frontmatter_with_expansion_no_includes() {
        // Exercise parse_frontmatter_with_expansion with an empty expander
        // (no @-references to expand). Covers the expansion function entry
        // and the `if let Some(exp) = expander` branch with a pass-through.
        let expander = YamlExpander::<swissarmyhammer_directory::SwissarmyhammerConfig>::new();

        let content = r#"---
title: Expanded Doc
tags:
  - rust
  - test
---

Body content here.
"#;

        let result = parse_frontmatter_with_expansion(content, &expander).unwrap();
        assert!(result.metadata.is_some());

        let metadata = result.metadata.as_ref().unwrap();
        assert_eq!(
            metadata.get("title").and_then(|v| v.as_str()),
            Some("Expanded Doc")
        );
        let tags = metadata.get("tags").and_then(|v| v.as_array()).unwrap();
        assert_eq!(tags.len(), 2);
        assert!(result.content.contains("Body content here."));
    }

    #[test]
    fn test_parse_frontmatter_with_expansion_no_frontmatter() {
        // Exercise parse_frontmatter_with_expansion on content without frontmatter
        let expander = YamlExpander::<swissarmyhammer_directory::SwissarmyhammerConfig>::new();

        let content = "Just plain content, no frontmatter.\n";

        let result = parse_frontmatter_with_expansion(content, &expander).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_parse_frontmatter_with_expansion_malformed_yaml() {
        // Exercise parse_frontmatter_with_expansion with invalid YAML
        let expander = YamlExpander::<swissarmyhammer_directory::SwissarmyhammerConfig>::new();

        let content = r#"---
title: Test
broken: [unclosed
---

Content
"#;

        let result = parse_frontmatter_with_expansion(content, &expander);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid YAML frontmatter"));
    }

    #[test]
    fn test_parse_frontmatter_with_expansion_empty_yaml() {
        // Exercise parse_frontmatter_with_expansion with empty frontmatter block
        let expander = YamlExpander::<swissarmyhammer_directory::SwissarmyhammerConfig>::new();

        let content = r#"---
---

Content after empty frontmatter
"#;

        let result = parse_frontmatter_with_expansion(content, &expander).unwrap();
        assert!(result.metadata.is_some());
        let metadata = result.metadata.as_ref().unwrap();
        assert!(metadata.is_null());
        assert!(result.content.contains("Content after empty frontmatter"));
    }

    #[test]
    fn test_parse_frontmatter_opening_delimiter_no_closing() {
        // Content starts with --- but never has a closing --- delimiter
        // line, so split_frontmatter_body finds no match and we fall
        // through to the "no frontmatter" branch.
        let content = "---\nkey: value\nno closing delimiter here\n";

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content, content);
    }

    #[test]
    fn closing_delimiter_without_trailing_newline_parses_with_an_empty_body() {
        // A closing delimiter at end of file with no terminator used to
        // fall through to "no frontmatter" under the old substring split,
        // because splitn(3, "---\n") never found a second "---\n"
        // occurrence. The line-anchored splitter recognizes the
        // unterminated "---" as a valid closing delimiter line and reads
        // an empty body.
        let content = "---\nkey: value\n---";

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_some());
        let metadata = result.metadata.as_ref().unwrap();
        assert_eq!(metadata.get("key").and_then(|v| v.as_str()), Some("value"));
        assert_eq!(result.content, "");
    }

    #[test]
    fn crlf_frontmatter_parses() {
        // A CRLF file used to fall through to "no frontmatter" because the
        // old substring gate checked `starts_with("---\n")`, which a
        // "---\r\n" opening line never matches. The line-anchored
        // splitter strips the trailing \r before comparing delimiter
        // lines, so it recognizes the CRLF delimiters and parses the
        // frontmatter.
        let content = "---\r\nkey: value\r\n---\r\nbody\r\n";

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_some());
        let metadata = result.metadata.as_ref().unwrap();
        assert_eq!(metadata.get("key").and_then(|v| v.as_str()), Some("value"));
        assert!(result.content.contains("body"));
    }

    #[test]
    fn test_parse_frontmatter_with_expansion_partial_marker() {
        // Partial marker should short-circuit even with an expander provided
        let expander = YamlExpander::<swissarmyhammer_directory::SwissarmyhammerConfig>::new();

        let content = "{% partial %}\n<div>Hello</div>";

        let result = parse_frontmatter_with_expansion(content, &expander).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content, content);
    }

    // --- split_frontmatter_body ---

    #[test]
    fn splits_on_the_first_delimiter_line() {
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n---\nbody\n"),
            Some(("title: x\n", "body\n"))
        );
    }

    #[test]
    fn keeps_the_body_bytes_exactly() {
        // No trailing newline.
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n---\nbody"),
            Some(("title: x\n", "body"))
        );
        // Empty body, closing delimiter terminated.
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n---\n"),
            Some(("title: x\n", ""))
        );
        // Empty body, closing delimiter unterminated.
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n---"),
            Some(("title: x\n", ""))
        );
        // A body that is only newlines keeps every one of them.
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n---\n\n\n"),
            Some(("title: x\n", "\n\n"))
        );
    }

    #[test]
    fn crlf_delimiter_lines_leave_a_crlf_body_intact() {
        assert_eq!(
            split_frontmatter_body("---\r\ntitle: x\r\n---\r\nbody\r\n"),
            Some(("title: x\r\n", "body\r\n"))
        );
    }

    #[test]
    fn three_hyphens_inside_the_frontmatter_do_not_delimit() {
        // Indented, as a YAML block scalar writes them.
        assert_eq!(
            split_frontmatter_body("---\ntext: |-\n  before\n  ---\n  after\n---\nbody"),
            Some(("text: |-\n  before\n  ---\n  after\n", "body"))
        );
        // Embedded in a longer line, as a quoted scalar writes them.
        assert_eq!(
            split_frontmatter_body("---\ntitle: a --- b\n---\nbody"),
            Some(("title: a --- b\n", "body"))
        );
        // A longer run of hyphens is not the delimiter either.
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n----\n---\nbody"),
            Some(("title: x\n----\n", "body"))
        );
    }

    #[test]
    fn three_hyphens_in_the_body_stay_in_the_body() {
        assert_eq!(
            split_frontmatter_body("---\ntitle: x\n---\nbefore\n---\nafter"),
            Some(("title: x\n", "before\n---\nafter"))
        );
    }

    #[test]
    fn empty_frontmatter_splits_to_an_empty_slice() {
        assert_eq!(split_frontmatter_body("---\n---\nbody"), Some(("", "body")));
    }

    #[test]
    fn rejects_text_that_does_not_open_with_a_delimiter_line() {
        assert_eq!(split_frontmatter_body(""), None);
        assert_eq!(split_frontmatter_body("just prose"), None);
        // Content before the opening delimiter is not frontmatter.
        assert_eq!(split_frontmatter_body("prose\n---\ntitle: x\n---\n"), None);
        // The opening line must be only the delimiter.
        assert_eq!(split_frontmatter_body("--- \ntitle: x\n---\n"), None);
        assert_eq!(split_frontmatter_body("---x\ntitle: x\n---\n"), None);
    }

    #[test]
    fn rejects_text_with_no_closing_delimiter_line() {
        assert_eq!(split_frontmatter_body("---\ntitle: x\n"), None);
        assert_eq!(split_frontmatter_body("---\n"), None);
        assert_eq!(split_frontmatter_body("---"), None);
    }
}
