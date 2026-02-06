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
            let s: String = js_string
                .to_string()
                .map_err(|e| JsError::type_conversion(format!("String conversion failed: {}", e)))?;
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
