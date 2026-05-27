//! Lenient JSONC parsing for user-written configuration files.
//!
//! Agents like Zed and VS Code routinely ship `settings.json` files that are
//! JSONC (JSON with `//` and `/* */` comments and trailing commas) even when
//! the file extension is `.json`. Strict `serde_json::from_str` rejects these.
//!
//! Per Postel's law — *be liberal in what we accept* — every read path that
//! ingests user-written JSON config should accept JSONC. Writing remains
//! strict JSON via `serde_json::to_string_pretty`.
//!
//! This module exposes a single helper, [`parse_jsonc`], that wraps
//! `jsonc_parser::parse_to_value` and converts the result into a
//! `serde_json::Value`. Parse failures surface as a `serde_json::Error` whose
//! `Display` impl carries the original line/column information, so existing
//! call-site error contexts (`"Invalid JSON in {path}: {e}"`) keep working
//! unchanged.

use serde::de::Error as _;
use serde_json::Value;

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
/// valid JSONC. The error's `Display` impl preserves the parser's original
/// line/column information so user-facing messages stay informative.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_jsonc_plain_json() {
        let result = parse_jsonc(r#"{"x": 1, "y": [2, 3]}"#).unwrap();
        assert_eq!(result, json!({"x": 1, "y": [2, 3]}));
    }

    #[test]
    fn test_parse_jsonc_line_comments() {
        let result = parse_jsonc("// leading comment\n{\"x\": 1}").unwrap();
        assert_eq!(result, json!({"x": 1}));

        let result = parse_jsonc("{\"x\": 1 // trailing line comment\n}").unwrap();
        assert_eq!(result, json!({"x": 1}));
    }

    #[test]
    fn test_parse_jsonc_block_comments() {
        let result = parse_jsonc("/* block */ {\"x\": 1}").unwrap();
        assert_eq!(result, json!({"x": 1}));

        let result = parse_jsonc("{\n  /* mid */ \"x\": 1\n}").unwrap();
        assert_eq!(result, json!({"x": 1}));
    }

    #[test]
    fn test_parse_jsonc_trailing_commas() {
        let result = parse_jsonc(r#"{"x": 1,}"#).unwrap();
        assert_eq!(result, json!({"x": 1}));

        let result = parse_jsonc(r#"[1, 2, 3,]"#).unwrap();
        assert_eq!(result, json!([1, 2, 3]));
    }

    #[test]
    fn test_parse_jsonc_invalid_returns_error() {
        let err = parse_jsonc("not json").unwrap_err();
        // Error message should be non-empty and informative (line/column
        // style from the underlying parser).
        let msg = err.to_string();
        assert!(!msg.is_empty(), "expected non-empty error message");
    }

    #[test]
    fn test_parse_jsonc_comments_and_trailing_commas_combined() {
        let input = "// Settings\n{\n  \"x\": 1,\n}";
        let result = parse_jsonc(input).unwrap();
        assert_eq!(result, json!({"x": 1}));
    }
}
