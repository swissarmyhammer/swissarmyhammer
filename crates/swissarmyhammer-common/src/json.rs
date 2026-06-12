//! Lenient JSONC parsing for user-written configuration files.
//!
//! Claude `.claude/settings.json` files — and editor settings files shipped by
//! Zed and VS Code — are routinely JSONC (JSON with `//` and `/* */` comments
//! and trailing commas) even when the file extension is `.json`. Strict
//! `serde_json::from_str` rejects these.
//!
//! Per Postel's law — *be liberal in what we accept* — every read path that
//! ingests user-written JSON config should accept JSONC. Writing remains strict
//! JSON via `serde_json::to_string_pretty`.
//!
//! This module is the single shared JSONC primitive for the SwissArmyHammer
//! ecosystem. [`parse_jsonc`] wraps `jsonc_parser` and converts the result into
//! a `serde_json::Value`; [`read_json_file`] layers the "empty object for a
//! missing or blank file" convention on top. Both mirdan's install/deploy
//! settings primitives and the Claude hook-settings loader delegate here so
//! there is exactly one JSONC implementation.

use std::fs;
use std::io;
use std::path::Path;

use serde::de::Error as _;
use serde_json::{Map, Value};

/// Parse a string of JSONC (JSON with `//` and `/* */` comments and trailing
/// commas) into a [`serde_json::Value`].
///
/// Plain JSON is fully backward compatible: any input that
/// `serde_json::from_str` would parse, `parse_jsonc` parses to the same value.
/// The lenient extensions accepted on top are line comments, block comments,
/// and trailing commas.
///
/// # Errors
///
/// Returns a [`serde_json::Error`] when the input is neither valid JSON nor
/// valid JSONC. Empty or whitespace-only input is rejected with an EOF error,
/// matching `serde_json::from_str`. The error's `Display` impl preserves the
/// parser's original line/column information so user-facing messages stay
/// informative.
pub fn parse_jsonc(content: &str) -> Result<Value, serde_json::Error> {
    // Empty or whitespace-only input — match serde_json's behavior, which
    // rejects empty input with an EOF error. `jsonc_parser` would otherwise
    // deserialize empty input as `null`.
    if content.trim().is_empty() {
        return serde_json::from_str(content);
    }
    jsonc_parser::parse_to_serde_value(content, &Default::default())
        .map_err(|e| serde_json::Error::custom(e.to_string()))
        .and_then(|opt: Option<Value>| {
            opt.ok_or_else(|| serde_json::Error::custom("empty JSONC input"))
        })
}

/// Error returned by [`read_json_file`].
///
/// Distinguishes an I/O failure (the file exists but could not be read) from a
/// parse failure (the file was read but its contents are neither valid JSON nor
/// valid JSONC), so callers can map each variant onto their own error domain
/// while keeping the offending path in the message.
#[derive(Debug)]
pub enum JsonFileError {
    /// The file exists but could not be read.
    Io(io::Error),
    /// The file was read but its contents could not be parsed as JSON or JSONC.
    /// Carries the path for context and the underlying parse error.
    Parse {
        /// Path of the file that failed to parse.
        path: std::path::PathBuf,
        /// The underlying parse error, whose `Display` preserves line/column.
        source: serde_json::Error,
    },
}

impl std::fmt::Display for JsonFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{}", e),
            Self::Parse { path, source } => {
                write!(f, "Invalid JSON in {}: {}", path.display(), source)
            }
        }
    }
}

impl std::error::Error for JsonFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

/// Read a JSONC settings file, returning an empty object when the file does not
/// exist or contains only whitespace.
///
/// Accepts JSONC (JSON with `//` and `/* */` comments and trailing commas)
/// because user-written config files routinely contain them even when the file
/// extension is `.json`.
///
/// # Errors
///
/// Returns [`JsonFileError::Io`] when an existing file cannot be read, and
/// [`JsonFileError::Parse`] (carrying the path) when the file exists but is
/// neither valid JSON nor valid JSONC.
pub fn read_json_file(path: &Path) -> Result<Value, JsonFileError> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let content = fs::read_to_string(path).map_err(JsonFileError::Io)?;
    if content.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    parse_jsonc(&content).map_err(|source| JsonFileError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_jsonc_plain_json() {
        let result = parse_jsonc(r#"{"x": 1, "y": [2, 3]}"#).unwrap();
        assert_eq!(result, json!({"x": 1, "y": [2, 3]}));
    }

    #[test]
    fn parse_jsonc_line_comments() {
        let result = parse_jsonc("// leading comment\n{\"x\": 1}").unwrap();
        assert_eq!(result, json!({"x": 1}));

        let result = parse_jsonc("{\"x\": 1 // trailing line comment\n}").unwrap();
        assert_eq!(result, json!({"x": 1}));
    }

    #[test]
    fn parse_jsonc_block_comments() {
        let result = parse_jsonc("/* block */ {\"x\": 1}").unwrap();
        assert_eq!(result, json!({"x": 1}));

        let result = parse_jsonc("{\n  /* mid */ \"x\": 1\n}").unwrap();
        assert_eq!(result, json!({"x": 1}));
    }

    #[test]
    fn parse_jsonc_trailing_commas() {
        let result = parse_jsonc(r#"{"x": 1,}"#).unwrap();
        assert_eq!(result, json!({"x": 1}));

        let result = parse_jsonc(r#"[1, 2, 3,]"#).unwrap();
        assert_eq!(result, json!([1, 2, 3]));
    }

    #[test]
    fn parse_jsonc_empty_input_is_eof_error_like_serde_json() {
        // Empty input must error the same way serde_json does (EOF), not parse
        // to `null` as jsonc_parser would.
        let err = parse_jsonc("").unwrap_err();
        let serde_err = serde_json::from_str::<Value>("").unwrap_err();
        assert_eq!(err.to_string(), serde_err.to_string());
        assert!(err.is_eof(), "expected an EOF error, got: {err}");

        // Whitespace-only input is likewise an EOF error.
        let err = parse_jsonc("   \n\t").unwrap_err();
        assert!(err.is_eof(), "expected an EOF error, got: {err}");
    }

    #[test]
    fn parse_jsonc_invalid_returns_error_with_line_column() {
        let err = parse_jsonc("not json").unwrap_err();
        let msg = err.to_string();
        assert!(!msg.is_empty(), "expected non-empty error message");
        // The underlying parser preserves position information in Display.
        assert!(
            msg.contains("line") || msg.contains("column"),
            "expected line/column in error Display, got: {msg}"
        );
    }

    #[test]
    fn parse_jsonc_comments_and_trailing_commas_combined() {
        let input = "// Settings\n{\n  \"x\": 1,\n}";
        let result = parse_jsonc(input).unwrap();
        assert_eq!(result, json!({"x": 1}));
    }

    #[test]
    fn read_json_file_returns_empty_object_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.json");
        let value = read_json_file(&path).unwrap();
        assert_eq!(value, json!({}));
    }

    #[test]
    fn read_json_file_returns_empty_object_for_blank_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blank.json");
        fs::write(&path, "   \n").unwrap();
        let value = read_json_file(&path).unwrap();
        assert_eq!(value, json!({}));
    }

    #[test]
    fn read_json_file_accepts_jsonc() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "// header\n{\n  \"foo\": 1,\n}").unwrap();
        let value = read_json_file(&path).unwrap();
        assert_eq!(value, json!({"foo": 1}));
    }

    #[test]
    fn read_json_file_parse_error_preserves_path_and_position() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        fs::write(&path, "not json").unwrap();
        let err = read_json_file(&path).unwrap_err();
        assert!(matches!(err, JsonFileError::Parse { .. }));
        let msg = err.to_string();
        assert!(
            msg.contains("bad.json"),
            "expected path in error Display, got: {msg}"
        );
    }
}
