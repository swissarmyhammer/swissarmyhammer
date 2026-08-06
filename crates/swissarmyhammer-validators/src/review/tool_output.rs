//! Parser for the tool-rule stdout contract.
//!
//! A tool rule's `run` script reports findings on stdout, one finding per
//! line, in either shape (see `builtin/validators/README.md`):
//!
//! - `path:line: message` — the common linter line convention.
//! - `{"file": ..., "line": ..., "message": ...}` — a JSON object per line,
//!   what `jq -c` emits.
//!
//! Empty stdout means clean. A line in neither shape is a broken contract and
//! parses to an error — never a silently dropped finding.

use std::sync::LazyLock;

use serde::Deserialize;

use crate::error::AvpError;
use crate::review::types::Finding;

/// The `path:line: message` linter line shape.
///
/// The file part is non-greedy so the first `:<digits>:` boundary wins; the
/// space after the second colon is optional (`grep -n` output has none).
static LINE_SHAPE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(?P<file>.+?):(?P<line>\d+):\s?(?P<message>.*)$")
        .expect("the linter line shape regex is valid")
});

/// One finding line in the JSON object shape (`jq -c` output).
#[derive(Debug, Deserialize)]
struct JsonFindingLine {
    /// File the finding is about.
    file: String,
    /// 1-based line number the finding points at.
    line: u32,
    /// The finding message.
    message: String,
}

/// Parse a tool rule's stdout into findings.
///
/// Each non-blank line must be one of the two contract shapes. The parsed
/// message becomes the finding's claim, and the raw stdout line becomes its
/// evidence — deterministic tool output is its own proof. The `validator` and
/// `rule` fields are left empty for the engine to tag, the same way fleet
/// findings are re-tagged after parsing.
///
/// Empty (or blank-only) stdout parses to no findings: the tool judged the
/// code clean.
///
/// # Errors
///
/// Returns [`AvpError::Context`] naming the offending line when a non-blank
/// line matches neither shape — a tool that breaks its stdout contract must
/// be reported, not silently trusted.
pub fn parse_tool_stdout(stdout: &str) -> Result<Vec<Finding>, AvpError> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_finding_line)
        .collect()
}

/// Parse one non-blank stdout line in either contract shape.
fn parse_finding_line(line: &str) -> Result<Finding, AvpError> {
    if line.starts_with('{') {
        let parsed: JsonFindingLine = serde_json::from_str(line).map_err(|e| {
            AvpError::Context(format!(
                "tool stdout line is not a {{file, line, message}} object: '{line}' ({e})"
            ))
        })?;
        return Ok(finding(parsed.file, parsed.line, parsed.message, line));
    }

    let captures = LINE_SHAPE.captures(line).ok_or_else(|| {
        AvpError::Context(format!(
            "tool stdout line matches neither 'path:line: message' nor a JSON object: '{line}'"
        ))
    })?;

    let line_number: u32 = captures["line"].parse().map_err(|e| {
        AvpError::Context(format!(
            "tool stdout line number is out of range: '{line}' ({e})"
        ))
    })?;

    Ok(finding(
        captures["file"].to_string(),
        line_number,
        captures["message"].to_string(),
        line,
    ))
}

/// Build a [`Finding`] from parsed stdout fields.
///
/// The raw stdout line is the evidence; validator/rule tagging belongs to the
/// engine.
fn finding(file: String, line: u32, message: String, raw: &str) -> Finding {
    Finding {
        file,
        line,
        validator: String::new(),
        rule: None,
        claim: message,
        evidence: raw.to_string(),
        suggestion: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linter_line_shape_parses_into_finding_fields() {
        let findings = parse_tool_stdout("src/lib.rs:42: missing documentation\n").unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "src/lib.rs");
        assert_eq!(findings[0].line, 42);
        assert_eq!(findings[0].claim, "missing documentation");
        assert_eq!(findings[0].evidence, "src/lib.rs:42: missing documentation");
    }

    #[test]
    fn json_object_shape_parses_into_finding_fields() {
        let findings = parse_tool_stdout(
            r#"{"file": "pkg/tool.py", "line": 7, "message": "D101 missing docstring"}"#,
        )
        .unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "pkg/tool.py");
        assert_eq!(findings[0].line, 7);
        assert_eq!(findings[0].claim, "D101 missing docstring");
    }

    #[test]
    fn mixed_shapes_parse_one_finding_per_line() {
        let stdout = concat!(
            "src/a.rs:1: first\n",
            "\n",
            r#"{"file": "src/b.rs", "line": 2, "message": "second"}"#,
            "\n",
        );
        let findings = parse_tool_stdout(stdout).unwrap();

        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "src/a.rs");
        assert_eq!(findings[1].file, "src/b.rs");
    }

    #[test]
    fn empty_stdout_means_clean() {
        assert!(parse_tool_stdout("").unwrap().is_empty());
        assert!(parse_tool_stdout("\n  \n").unwrap().is_empty());
    }

    #[test]
    fn grep_n_output_without_space_after_colon_parses() {
        let findings = parse_tool_stdout("src/lib.rs:9:TODO left in code").unwrap();

        assert_eq!(findings[0].line, 9);
        assert_eq!(findings[0].claim, "TODO left in code");
    }

    #[test]
    fn line_with_colons_in_message_keeps_first_numeric_boundary() {
        let findings = parse_tool_stdout("src/lib.rs:12: expected `a: b` here").unwrap();

        assert_eq!(findings[0].file, "src/lib.rs");
        assert_eq!(findings[0].line, 12);
        assert_eq!(findings[0].claim, "expected `a: b` here");
    }

    #[test]
    fn contract_breaking_line_is_an_error_naming_the_line() {
        let err = parse_tool_stdout("this is not a finding line").unwrap_err();
        assert!(err.to_string().contains("this is not a finding line"));
    }

    #[test]
    fn malformed_json_object_is_an_error_naming_the_line() {
        let err = parse_tool_stdout(r#"{"file": "a.rs"}"#).unwrap_err();
        assert!(err.to_string().contains(r#"{"file": "a.rs"}"#));
    }
}
