//! Shared helpers for the operation-dispatched MCP tools.
//!
//! The op-dispatched tools (`review`, `code_context`, `diagnostics`) share a
//! single dispatch shape: they read scalar/array arguments out of the JSON
//! `arguments` map and serialize a typed response into a JSON-text
//! [`CallToolResult`]. This module is the one home for those primitives so each
//! tool imports them rather than carrying a byte-identical private copy.
//!
//! `McpError` is `rmcp::ErrorData`; these signatures match what every op-tool's
//! `execute` returns.

use rmcp::model::{CallToolResult, Content};
use rmcp::ErrorData as McpError;

/// Read an optional string argument.
///
/// Returns `None` when the key is absent or is not a JSON string.
pub(crate) fn string_arg(
    args: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

/// Read an optional string-array argument (empty when absent or wrong-typed).
///
/// Non-string array elements are silently skipped.
pub(crate) fn string_array_arg(
    args: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Read an optional boolean argument, falling back to `default`.
///
/// An MCP caller is usually an agent, and agents routinely send the JSON string
/// `"true"` where the schema declares a boolean. Coerce that instead of
/// dropping it: a flag that is accepted and ignored returns a success-shaped
/// response missing the thing the caller asked for, with nothing to show why.
///
/// A value that is neither a boolean nor `"true"`/`"false"` is an error, not a
/// fall back to `default`, for the same reason. An absent or null value means
/// "not set", so `default` stands.
pub(crate) fn bool_arg(
    args: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    default: bool,
) -> Result<bool, String> {
    match args.get(key) {
        None | Some(serde_json::Value::Null) => Ok(default),
        Some(serde_json::Value::Bool(value)) => Ok(*value),
        Some(serde_json::Value::String(value)) => {
            match value.trim().to_ascii_lowercase().as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(format!(
                    "`{key}` must be a boolean; got the string \"{value}\""
                )),
            }
        }
        Some(other) => Err(format!("`{key}` must be a boolean; got {other}")),
    }
}

/// The glob metacharacters that make a target a pattern rather than a literal path.
const GLOB_METACHARACTERS: [char; 3] = ['*', '?', '['];

/// Whether a path-shaped argument is a glob pattern rather than a concrete path.
pub(crate) fn is_glob_pattern(target: &str) -> bool {
    target.contains(GLOB_METACHARACTERS)
}

/// Read an optional non-negative integer argument as a `usize`.
///
/// Returns `None` when the key is absent or is not a JSON unsigned integer (a
/// negative or fractional number is treated as absent, deferring to the caller's
/// default).
pub(crate) fn usize_arg(
    args: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<usize> {
    args.get(key).and_then(|v| v.as_u64()).map(|n| n as usize)
}

/// Parse a forgiving list-of-strings value.
///
/// Three shapes yield a list: an array of strings, a stringified JSON array of
/// strings, and a single string (a one-element list) — the shape tolerance sah
/// ops give list params. Returns `None` for any other shape, and for an array
/// holding a non-string element.
pub(crate) fn forgiving_string_list(value: &serde_json::Value) -> Option<Vec<String>> {
    if let Some(items) = value.as_array() {
        return items
            .iter()
            .map(|item| item.as_str().map(str::to_string))
            .collect();
    }
    let text = value.as_str()?;
    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(text) {
        return Some(parsed);
    }
    Some(vec![text.to_string()])
}

/// Serialize a value into a JSON-text [`CallToolResult`].
///
/// The value is pretty-printed; a serialization failure (effectively
/// unreachable for the well-typed response structs the op-tools pass) maps to an
/// `internal_error`.
pub(crate) fn json_result<T: serde::Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| McpError::internal_error(format!("failed to serialize: {e}"), None))?;
    Ok(CallToolResult::success(vec![Content::text(text)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn string_arg_reads_present_and_absent() {
        let args = map(serde_json::json!({"a": "x", "n": 1}));
        assert_eq!(string_arg(&args, "a"), Some("x".to_string()));
        assert_eq!(string_arg(&args, "missing"), None);
        // Wrong-typed (number) is treated as absent.
        assert_eq!(string_arg(&args, "n"), None);
    }

    #[test]
    fn forgiving_string_list_accepts_the_three_shapes_and_rejects_the_rest() {
        // An array of strings.
        assert_eq!(
            forgiving_string_list(&serde_json::json!(["a", "b"])),
            Some(vec!["a".to_string(), "b".to_string()])
        );
        // A stringified JSON array.
        assert_eq!(
            forgiving_string_list(&serde_json::json!("[\"a\", \"b\"]")),
            Some(vec!["a".to_string(), "b".to_string()])
        );
        // A single string is a one-element list.
        assert_eq!(
            forgiving_string_list(&serde_json::json!("a")),
            Some(vec!["a".to_string()])
        );
        // A non-string array element and a non-list shape are malformed.
        assert_eq!(forgiving_string_list(&serde_json::json!(["a", 1])), None);
        assert_eq!(forgiving_string_list(&serde_json::json!(1)), None);
    }

    #[test]
    fn string_array_arg_collects_strings_only() {
        let args = map(serde_json::json!({
            "xs": ["a", 1, "b", null],
            "scalar": "a"
        }));
        assert_eq!(
            string_array_arg(&args, "xs"),
            vec!["a".to_string(), "b".to_string()]
        );
        // Non-array and absent both yield empty.
        assert!(string_array_arg(&args, "scalar").is_empty());
        assert!(string_array_arg(&args, "missing").is_empty());
    }

    #[test]
    fn bool_arg_reads_booleans_and_falls_back_to_the_default() {
        let args = map(serde_json::json!({"yes": true, "no": false, "null": null}));
        assert!(bool_arg(&args, "yes", false).expect("boolean"));
        assert!(!bool_arg(&args, "no", true).expect("boolean"));
        // Absent and null both mean "not set", so the caller's default stands.
        assert!(bool_arg(&args, "missing", true).expect("absent"));
        assert!(!bool_arg(&args, "missing", false).expect("absent"));
        assert!(bool_arg(&args, "null", true).expect("null"));
    }

    #[test]
    fn bool_arg_coerces_string_booleans_an_agent_sends() {
        let args = map(serde_json::json!({
            "lower": "true", "upper": "TRUE", "mixed": "False", "padded": " true "
        }));
        for key in ["lower", "upper", "padded"] {
            assert!(bool_arg(&args, key, false).expect(key), "{key} is true");
        }
        assert!(!bool_arg(&args, "mixed", true).expect("mixed"));
    }

    #[test]
    fn bool_arg_rejects_a_value_it_cannot_read_rather_than_dropping_it() {
        let args = map(serde_json::json!({"word": "yes", "num": 1, "arr": []}));
        for key in ["word", "num", "arr"] {
            let error = bool_arg(&args, key, false).expect_err("must not silently default");
            assert!(
                error.contains(key) && error.contains("must be a boolean"),
                "{key} error names the key and the expected type: {error}"
            );
        }
    }

    #[test]
    fn is_glob_pattern_separates_patterns_from_paths() {
        for pattern in ["*.rs", "**/*.ts", "src/a?.rs", "src/[ab].rs"] {
            assert!(is_glob_pattern(pattern), "{pattern} is a glob");
        }
        for path in ["src/lib.rs", "crates/a/src/mod.rs", "Cargo.toml"] {
            assert!(!is_glob_pattern(path), "{path} is a concrete path");
        }
    }

    #[test]
    fn usize_arg_reads_unsigned_ints_only() {
        let args = map(serde_json::json!({
            "n": 32768,
            "neg": -1,
            "frac": 1.5,
            "str": "8"
        }));
        assert_eq!(usize_arg(&args, "n"), Some(32768));
        // Absent, negative, fractional, and string are all treated as absent so
        // the caller falls back to its default.
        assert_eq!(usize_arg(&args, "missing"), None);
        assert_eq!(usize_arg(&args, "neg"), None);
        assert_eq!(usize_arg(&args, "frac"), None);
        assert_eq!(usize_arg(&args, "str"), None);
    }

    #[test]
    fn json_result_wraps_pretty_json_text() {
        #[derive(serde::Serialize)]
        struct R {
            ok: bool,
        }
        let result = json_result(&R { ok: true }).expect("serialize");
        assert!(!result.is_error.unwrap_or(false));
        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            other => panic!("expected text content, got {other:?}"),
        };
        assert!(text.contains("\"ok\": true"));
    }
}
