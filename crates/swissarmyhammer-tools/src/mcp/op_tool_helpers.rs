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

/// Read an optional boolean flag (`false` when absent or wrong-typed).
///
/// Accepts a real JSON `true`/`false` or the strings `"true"`/`"false"` so a
/// forgiving caller can pass either shape.
pub(crate) fn bool_arg(args: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    match args.get(key) {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => s.eq_ignore_ascii_case("true"),
        _ => false,
    }
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
    fn bool_arg_accepts_json_bool_and_string() {
        let args = map(serde_json::json!({
            "t": true,
            "f": false,
            "st": "TrUe",
            "sf": "no",
            "n": 1
        }));
        assert!(bool_arg(&args, "t"));
        assert!(!bool_arg(&args, "f"));
        assert!(bool_arg(&args, "st"));
        assert!(!bool_arg(&args, "sf"));
        assert!(!bool_arg(&args, "n"));
        assert!(!bool_arg(&args, "missing"));
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
