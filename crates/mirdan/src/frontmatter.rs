//! Shared YAML frontmatter reading for mirdan package manifests.
//!
//! Every manifest mirdan reads -- `SKILL.md`, `VALIDATOR.md`, `TOOL.md`,
//! `AGENT.md` -- opens with a YAML frontmatter block, and every reader of one
//! makes the same three moves before it looks at anything: read the file,
//! split the block off the body, parse the block as YAML. Those three moves
//! live here once. What differs between readers is only what they ask for
//! afterwards and how they report a failure, so each keeps a wrapper that
//! states its own contract and holds no parsing of its own.
//!
//! [`split_frontmatter_body`] makes the split, so only a line that is exactly
//! three hyphens delimits the block. A three-hyphen run inside a value -- a
//! table separator, a horizontal rule indented in a `description: >-` block
//! scalar, a `--` flag in a TOOL.md command line -- stays in the frontmatter
//! instead of cutting it short, and an opening line of `----` or `---x` opens
//! nothing.

use std::path::Path;

use serde_yaml_ng::Value;
use swissarmyhammer_common::frontmatter::split_frontmatter_body;

use crate::registry::RegistryError;

#[cfg(test)]
pub(crate) mod fixtures;

/// The key that holds the nested block [`metadata_field`] reads first.
const METADATA_KEY: &str = "metadata";

/// Parse the YAML frontmatter block that opens `content`.
///
/// Returns `None` when `content` carries no frontmatter block, or when the
/// block holds YAML the parser rejects. Callers that read one optional field
/// need no distinction between the two; callers that report a failure to a
/// user want [`read_file`] instead.
pub(crate) fn parse(content: &str) -> Option<Value> {
    let (frontmatter, _body) = split_frontmatter_body(content)?;
    serde_yaml_ng::from_str(frontmatter).ok()
}

/// Read `path` and parse the YAML frontmatter block that opens it.
///
/// Returns `None` when the file will not read, and for every reason
/// [`parse`] returns `None`.
pub(crate) fn parse_file(path: &Path) -> Option<Value> {
    parse(&std::fs::read_to_string(path).ok()?)
}

/// Read `path` and parse the YAML frontmatter block that opens it, naming
/// what went wrong.
///
/// # Errors
///
/// Returns an error when the file cannot be read, when the frontmatter block
/// is missing or unterminated, or when the YAML does not parse.
pub(crate) fn read_file(path: &Path) -> Result<Value, RegistryError> {
    let content = std::fs::read_to_string(path)?;

    let (frontmatter, _body) = split_frontmatter_body(&content).ok_or_else(|| {
        RegistryError::Validation(format!(
            "{} must open and close YAML frontmatter with a line of exactly three hyphens",
            path.display()
        ))
    })?;

    serde_yaml_ng::from_str(frontmatter)
        .map_err(|e| RegistryError::Validation(format!("invalid YAML frontmatter: {}", e)))
}

/// The string value of the top-level `name` key in `yaml`.
pub(crate) fn field<'a>(yaml: &'a Value, name: &str) -> Option<&'a str> {
    yaml.get(name).and_then(Value::as_str)
}

/// The string value of `metadata.<name>` in `yaml`, falling back to the
/// top-level `name`.
///
/// A manifest written to the agentskills.io spec nests its version under
/// `metadata`; an older one carries it at the top level, so both are read and
/// the nested one wins.
pub(crate) fn metadata_field<'a>(yaml: &'a Value, name: &str) -> Option<&'a str> {
    yaml.get(METADATA_KEY)
        .and_then(|metadata| metadata.get(name))
        .and_then(Value::as_str)
        .or_else(|| field(yaml, name))
}

/// The string value of the top-level `name` key in the frontmatter of the
/// file at `path`.
pub(crate) fn file_field(path: &Path, name: &str) -> Option<String> {
    field(&parse_file(path)?, name).map(str::to_string)
}

/// The string value of `metadata.<name>` in the frontmatter of the file at
/// `path`, falling back to the top-level `name`.
pub(crate) fn file_metadata_field(path: &Path, name: &str) -> Option<String> {
    metadata_field(&parse_file(path)?, name).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::fixtures::{
        write_skill_md, NO_CLOSING_DELIMITER, OPENING_LINE_OF_FOUR_HYPHENS,
        OPENING_LINE_WITH_TRAILING_TEXT, THREE_HYPHEN_RUN_IN_DESCRIPTION,
    };

    #[test]
    fn test_parse_keeps_every_key_past_a_three_hyphen_run() {
        let yaml = parse(THREE_HYPHEN_RUN_IN_DESCRIPTION).unwrap();

        assert_eq!(field(&yaml, "name"), Some("test-skill"));
        assert_eq!(metadata_field(&yaml, "version"), Some("1.2.3"));
    }

    #[test]
    fn test_parse_rejects_an_opening_line_with_trailing_text() {
        assert!(parse(OPENING_LINE_WITH_TRAILING_TEXT).is_none());
    }

    #[test]
    fn test_parse_rejects_an_opening_line_of_four_hyphens() {
        assert!(parse(OPENING_LINE_OF_FOUR_HYPHENS).is_none());
    }

    #[test]
    fn test_parse_rejects_a_file_with_no_closing_delimiter() {
        assert!(parse(NO_CLOSING_DELIMITER).is_none());
    }

    #[test]
    fn test_parse_file_reads_the_file_at_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_skill_md(dir.path(), THREE_HYPHEN_RUN_IN_DESCRIPTION);

        let yaml = parse_file(&path).unwrap();

        assert_eq!(field(&yaml, "name"), Some("test-skill"));
    }

    #[test]
    fn test_parse_file_returns_none_for_a_file_that_will_not_read() {
        let dir = tempfile::tempdir().unwrap();

        assert!(parse_file(&dir.path().join("absent.md")).is_none());
    }

    #[test]
    fn test_read_file_names_a_missing_delimiter() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_skill_md(dir.path(), NO_CLOSING_DELIMITER);

        let message = read_file(&path).unwrap_err().to_string();

        assert!(
            message.contains("exactly three hyphens"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn test_read_file_names_yaml_the_parser_rejects() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_skill_md(dir.path(), "---\nname: [unterminated\n---\n# Test\n");

        let message = read_file(&path).unwrap_err().to_string();

        assert!(
            message.contains("invalid YAML frontmatter"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn test_metadata_field_prefers_the_metadata_block_over_the_top_level() {
        let yaml =
            parse("---\nversion: \"1.0.0\"\nmetadata:\n  version: \"2.0.0\"\n---\n").unwrap();

        assert_eq!(metadata_field(&yaml, "version"), Some("2.0.0"));
    }

    #[test]
    fn test_metadata_field_falls_back_to_the_top_level() {
        let yaml = parse("---\nversion: \"1.0.0\"\n---\n").unwrap();

        assert_eq!(metadata_field(&yaml, "version"), Some("1.0.0"));
    }

    #[test]
    fn test_field_reads_no_value_for_an_absent_key() {
        let yaml = parse("---\nname: test-skill\n---\n").unwrap();

        assert_eq!(field(&yaml, "description"), None);
    }

    #[test]
    fn test_parse_returns_none_for_text_that_opens_no_block() {
        assert!(parse("# Just markdown").is_none());
    }

    #[test]
    fn test_file_field_reads_one_field_out_of_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_skill_md(dir.path(), THREE_HYPHEN_RUN_IN_DESCRIPTION);

        assert_eq!(file_field(&path, "name"), Some("test-skill".to_string()));
        assert_eq!(file_field(&path, "absent"), None);
    }

    #[test]
    fn test_file_metadata_field_reads_a_nested_version_out_of_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_skill_md(dir.path(), THREE_HYPHEN_RUN_IN_DESCRIPTION);

        assert_eq!(
            file_metadata_field(&path, "version"),
            Some("1.2.3".to_string())
        );
    }
}
