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
//! This module re-exports the single shared JSONC primitive,
//! [`parse_jsonc`](swissarmyhammer_common::parse_jsonc), owned by
//! `swissarmyhammer-common`. Mirdan keeps `crate::parse_jsonc` as the canonical
//! call-site path; the implementation (and the `jsonc_parser` dependency) lives
//! in the common crate so the Claude hook-settings loader can reuse it without
//! depending on mirdan.

pub use swissarmyhammer_common::parse_jsonc;

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
