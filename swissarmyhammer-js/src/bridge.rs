//! Bidirectional conversion between serde_json::Value and rquickjs Value
//!
//! Provides functions to inject Rust JSON values into a JS context and
//! extract JS values back as JSON.

use crate::error::JsError;
use rquickjs::{Ctx, Value};

/// Convert a serde_json::Value into a rquickjs Value by round-tripping through JSON.parse()
pub fn json_to_js<'js>(
    ctx: &Ctx<'js>,
    value: &serde_json::Value,
) -> std::result::Result<Value<'js>, JsError> {
    let json_str =
        serde_json::to_string(value).map_err(|e| JsError::type_conversion(e.to_string()))?;

    ctx.json_parse(json_str)
        .map_err(|e| JsError::type_conversion(format!("JSON.parse failed: {}", e)))
}

/// Convert a rquickjs Value back to serde_json::Value by round-tripping through JSON.stringify()
///
/// - undefined and functions are converted to null
/// - All other types go through JSON.stringify -> serde_json::from_str
pub fn js_to_json<'js>(
    ctx: &Ctx<'js>,
    value: Value<'js>,
) -> std::result::Result<serde_json::Value, JsError> {
    // Handle undefined and null directly
    if value.is_undefined() || value.is_null() {
        return Ok(serde_json::Value::Null);
    }

    // Functions can't be serialized
    if value.is_function() {
        return Ok(serde_json::Value::Null);
    }

    // Use JSON.stringify for everything else
    match ctx.json_stringify(value) {
        Ok(Some(js_string)) => {
            let s: String = js_string.to_string().map_err(|e| {
                JsError::type_conversion(format!("String conversion failed: {}", e))
            })?;
            serde_json::from_str(&s)
                .map_err(|e| JsError::type_conversion(format!("JSON parse failed: {}", e)))
        }
        Ok(None) => {
            // JSON.stringify returns undefined for functions, symbols, undefined
            Ok(serde_json::Value::Null)
        }
        Err(e) => Err(JsError::type_conversion(format!(
            "JSON.stringify failed: {}",
            e
        ))),
    }
}

/// Set of JS builtin global names to skip when scanning for user variables
pub const JS_BUILTINS: &[&str] = &[
    // Standard JS constructors and objects
    "Object",
    "Function",
    "Array",
    "Number",
    "parseFloat",
    "parseInt",
    "Infinity",
    "NaN",
    "undefined",
    "Boolean",
    "String",
    "Symbol",
    "Date",
    "Promise",
    "RegExp",
    "Error",
    "AggregateError",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
    "JSON",
    "Math",
    "Atomics",
    "console",
    "Reflect",
    "Proxy",
    "Map",
    "BigInt",
    "Set",
    "WeakMap",
    "WeakSet",
    "ArrayBuffer",
    "SharedArrayBuffer",
    "DataView",
    "Int8Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "Int16Array",
    "Uint16Array",
    "Int32Array",
    "Uint32Array",
    "BigInt64Array",
    "BigUint64Array",
    "Float32Array",
    "Float64Array",
    "escape",
    "unescape",
    "eval",
    "isFinite",
    "isNaN",
    "globalThis",
    "decodeURI",
    "decodeURIComponent",
    "encodeURI",
    "encodeURIComponent",
    // Our injected globals
    "env",
    "process",
    // QuickJS-specific
    "__loadScript",
    "print",
    "scriptArgs",
    "gc",
];

/// Check if a global variable name is a JS builtin that should be skipped
pub fn is_builtin(name: &str) -> bool {
    JS_BUILTINS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_builtin_standard_globals() {
        assert!(is_builtin("Object"));
        assert!(is_builtin("Array"));
        assert!(is_builtin("Function"));
        assert!(is_builtin("Math"));
        assert!(is_builtin("JSON"));
        assert!(is_builtin("Promise"));
        assert!(is_builtin("console"));
    }

    #[test]
    fn test_is_builtin_injected_globals() {
        assert!(is_builtin("env"));
        assert!(is_builtin("process"));
    }

    #[test]
    fn test_is_builtin_quickjs_specific() {
        assert!(is_builtin("__loadScript"));
        assert!(is_builtin("print"));
        assert!(is_builtin("scriptArgs"));
        assert!(is_builtin("gc"));
    }

    #[test]
    fn test_is_builtin_user_vars_not_builtin() {
        assert!(!is_builtin("myVar"));
        assert!(!is_builtin("x"));
        assert!(!is_builtin("counter"));
        assert!(!is_builtin(""));
    }

    #[test]
    fn test_is_builtin_typed_arrays() {
        assert!(is_builtin("Int8Array"));
        assert!(is_builtin("Uint8Array"));
        assert!(is_builtin("Float64Array"));
        assert!(is_builtin("BigInt64Array"));
        assert!(is_builtin("BigUint64Array"));
    }

    #[test]
    fn test_is_builtin_error_types() {
        assert!(is_builtin("Error"));
        assert!(is_builtin("TypeError"));
        assert!(is_builtin("RangeError"));
        assert!(is_builtin("ReferenceError"));
        assert!(is_builtin("SyntaxError"));
        assert!(is_builtin("EvalError"));
        assert!(is_builtin("URIError"));
        assert!(is_builtin("AggregateError"));
    }

    #[test]
    fn test_is_builtin_encoding_functions() {
        assert!(is_builtin("decodeURI"));
        assert!(is_builtin("decodeURIComponent"));
        assert!(is_builtin("encodeURI"));
        assert!(is_builtin("encodeURIComponent"));
        assert!(is_builtin("escape"));
        assert!(is_builtin("unescape"));
    }

    #[test]
    fn test_is_builtin_global_functions() {
        assert!(is_builtin("eval"));
        assert!(is_builtin("isFinite"));
        assert!(is_builtin("isNaN"));
        assert!(is_builtin("parseFloat"));
        assert!(is_builtin("parseInt"));
    }

    #[test]
    fn test_is_builtin_collections() {
        assert!(is_builtin("Map"));
        assert!(is_builtin("Set"));
        assert!(is_builtin("WeakMap"));
        assert!(is_builtin("WeakSet"));
    }

    #[test]
    fn test_is_builtin_special_values() {
        assert!(is_builtin("Infinity"));
        assert!(is_builtin("NaN"));
        assert!(is_builtin("undefined"));
        assert!(is_builtin("globalThis"));
    }

    #[test]
    fn test_is_builtin_concurrency() {
        assert!(is_builtin("Atomics"));
        assert!(is_builtin("SharedArrayBuffer"));
    }

    #[test]
    fn test_is_builtin_proxy_reflect() {
        assert!(is_builtin("Proxy"));
        assert!(is_builtin("Reflect"));
    }

    #[test]
    fn test_js_builtins_list_not_empty() {
        assert!(!JS_BUILTINS.is_empty());
    }

    #[test]
    fn test_json_to_js_number() {
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let value = serde_json::json!(42);
            let js_val = json_to_js(&ctx, &value).unwrap();
            let result = js_to_json(&ctx, js_val).unwrap();
            assert_eq!(result, serde_json::json!(42));
        });
    }

    #[test]
    fn test_json_to_js_string() {
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let value = serde_json::json!("hello");
            let js_val = json_to_js(&ctx, &value).unwrap();
            let result = js_to_json(&ctx, js_val).unwrap();
            assert_eq!(result, serde_json::json!("hello"));
        });
    }

    #[test]
    fn test_json_to_js_bool() {
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let value = serde_json::json!(true);
            let js_val = json_to_js(&ctx, &value).unwrap();
            let result = js_to_json(&ctx, js_val).unwrap();
            assert_eq!(result, serde_json::json!(true));
        });
    }

    #[test]
    fn test_json_to_js_null() {
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let value = serde_json::json!(null);
            let js_val = json_to_js(&ctx, &value).unwrap();
            let result = js_to_json(&ctx, js_val).unwrap();
            assert_eq!(result, serde_json::Value::Null);
        });
    }

    #[test]
    fn test_json_to_js_object() {
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let value = serde_json::json!({"name": "test", "count": 5});
            let js_val = json_to_js(&ctx, &value).unwrap();
            let result = js_to_json(&ctx, js_val).unwrap();
            assert_eq!(result["name"], "test");
            assert_eq!(result["count"], 5);
        });
    }

    #[test]
    fn test_json_to_js_array() {
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let value = serde_json::json!([1, "two", 3]);
            let js_val = json_to_js(&ctx, &value).unwrap();
            let result = js_to_json(&ctx, js_val).unwrap();
            assert_eq!(result, serde_json::json!([1, "two", 3]));
        });
    }

    #[test]
    fn test_js_to_json_undefined() {
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val = Value::new_undefined(ctx.clone());
            let result = js_to_json(&ctx, val).unwrap();
            assert_eq!(result, serde_json::Value::Null);
        });
    }

    #[test]
    fn test_js_to_json_null_value() {
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val = Value::new_null(ctx.clone());
            let result = js_to_json(&ctx, val).unwrap();
            assert_eq!(result, serde_json::Value::Null);
        });
    }

    #[test]
    fn test_js_to_json_function() {
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val: Value = ctx.eval(b"(function() {})").unwrap();
            let result = js_to_json(&ctx, val).unwrap();
            assert_eq!(result, serde_json::Value::Null);
        });
    }

    #[test]
    fn test_json_to_js_nested_object() {
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let value = serde_json::json!({"a": {"b": [1, 2]}});
            let js_val = json_to_js(&ctx, &value).unwrap();
            let result = js_to_json(&ctx, js_val).unwrap();
            assert_eq!(result, serde_json::json!({"a": {"b": [1, 2]}}));
        });
    }

    #[test]
    fn test_json_to_js_empty_object() {
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let value = serde_json::json!({});
            let js_val = json_to_js(&ctx, &value).unwrap();
            let result = js_to_json(&ctx, js_val).unwrap();
            assert_eq!(result, serde_json::json!({}));
        });
    }

    #[test]
    fn test_json_to_js_float() {
        let rt = rquickjs::Runtime::new().unwrap();
        let ctx = rquickjs::Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let value = serde_json::json!(2.72);
            let js_val = json_to_js(&ctx, &value).unwrap();
            let result = js_to_json(&ctx, js_val).unwrap();
            assert!((result.as_f64().unwrap() - 2.72).abs() < 0.001);
        });
    }
}
