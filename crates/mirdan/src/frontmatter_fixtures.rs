//! Markdown fixtures that state the frontmatter delimiter rule once.
//!
//! Every reader in this crate splits frontmatter with
//! `swissarmyhammer_common::frontmatter::split_frontmatter_body`, so every
//! reader answers these four files the same way. Each site asserts that answer
//! in its own error contract against the fixtures here, rather than carrying
//! its own copy of the markdown.

/// A well-formed file whose `description` folded block scalar holds an
/// indented three-hyphen run.
///
/// A split that searches for the `---` substring cuts the frontmatter at that
/// run, and the truncated text still parses as YAML, so every key after the
/// run disappears without an error. A line-anchored split keeps every key.
///
/// `name` and `metadata` sit after the description so that a truncating split
/// loses them, which is what makes the loss observable through a reader that
/// exposes only one of the two.
pub(crate) const THREE_HYPHEN_RUN_IN_DESCRIPTION: &str = r#"---
description: >-
  Renders a table.

  ---

  Then explains it.
name: test-skill
metadata:
  version: "1.2.3"
---
# Test
"#;

/// A file whose first line carries text after the three hyphens.
///
/// A `strip_prefix("---")` opener accepts the line and reads the remainder as
/// frontmatter, so a file that opens no block still yields keys. A
/// line-anchored split opens nothing.
pub(crate) const OPENING_LINE_WITH_TRAILING_TEXT: &str = r#"---description: leaked
name: test-skill
---
# Test
"#;

/// A file whose first line is four hyphens.
///
/// No opener may read it as a frontmatter delimiter.
pub(crate) const OPENING_LINE_OF_FOUR_HYPHENS: &str = r#"----
name: test-skill
---
# Test
"#;

/// A file that opens a frontmatter block, never closes it, and holds a
/// three-hyphen run inside a value.
///
/// A split that searches for the `---` substring reads that run as the close
/// and returns a partial mapping. A line-anchored split rejects the file.
pub(crate) const NO_CLOSING_DELIMITER: &str = r#"---
name: test-skill
description: Uses --- as a separator
"#;

/// Write `content` into a `SKILL.md` inside `dir` and return its path.
pub(crate) fn write_skill_md(dir: &std::path::Path, content: &str) -> std::path::PathBuf {
    let path = dir.join("SKILL.md");
    std::fs::write(&path, content).unwrap();
    path
}
