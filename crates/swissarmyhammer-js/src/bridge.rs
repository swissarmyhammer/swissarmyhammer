//! Conversion helpers between `serde_json::Value` and V8 values.
//!
//! `deno_core` exposes the underlying V8 engine. Values produced by script
//! evaluation are `v8::Value` handles; this module converts them to
//! `serde_json::Value` honoring the historical contract:
//!
//! - `undefined`, `null`, functions and symbols all become JSON `null`
//! - everything else round-trips through `JSON.stringify` so nested objects,
//!   arrays and primitives are preserved exactly as the engine sees them.

use crate::error::JsError;

/// Convert a V8 value to a `serde_json::Value` within an active V8 scope.
///
/// `undefined`, `null` and functions are mapped directly to JSON `null`
/// without invoking `JSON.stringify`. Any other value is serialized via the
/// engine's own `JSON.stringify`.
///
/// `JSON.stringify` throws a `TypeError` for some inputs (symbols, `BigInt`,
/// circular structures). A [`deno_core::v8::TryCatch`] wraps the call so any
/// such exception is caught and consumed here — leaving it pending would
/// poison the isolate for the next script. Caught exceptions and `undefined`
/// results both collapse to JSON `null`, matching the prior engine's behavior.
///
/// # Arguments
///
/// * `scope` - An active, mutable V8 scope tied to the runtime's main context
/// * `value` - The V8 value to convert
///
/// # Errors
///
/// Returns [`JsError::TypeConversion`] if `JSON.stringify` produces a string
/// that cannot be parsed back as JSON.
pub fn v8_to_json(
    scope: &mut deno_core::v8::PinScope,
    value: deno_core::v8::Local<deno_core::v8::Value>,
) -> std::result::Result<serde_json::Value, JsError> {
    // undefined / null map straight to JSON null.
    if value.is_undefined() || value.is_null() {
        return Ok(serde_json::Value::Null);
    }

    // Functions cannot be serialized; preserve the historical null mapping.
    if value.is_function() {
        return Ok(serde_json::Value::Null);
    }

    // Stringify under a TryCatch so a thrown TypeError (BigInt, circular
    // refs) is caught here and does not leak into the isolate.
    let rust_string = {
        deno_core::v8::tc_scope!(let tc, &mut *scope);
        match deno_core::v8::json::stringify(tc, value) {
            Some(s) => s.to_rust_string_lossy(tc),
            // `JSON.stringify` threw — map to JSON null.
            None => return Ok(serde_json::Value::Null),
        }
    };

    // V8's `JSON::Stringify` yields the bare token `undefined` (or an empty
    // string) for inputs that stringify to the JS value `undefined` — e.g. a
    // standalone `Symbol`. A JS string whose contents are the word
    // "undefined" stringifies to the quoted `"undefined"`, so an unquoted
    // match is unambiguous. Map it to JSON null, matching the prior engine's
    // `Ok(None)` branch.
    if rust_string.is_empty() || rust_string == "undefined" {
        return Ok(serde_json::Value::Null);
    }

    serde_json::from_str(&rust_string)
        .map_err(|e| JsError::type_conversion(format!("JSON parse failed: {e}")))
}

/// Set of JS builtin global names to skip when scanning for user variables.
///
/// V8 exposes a large set of standard globals; only names absent from this
/// list are treated as user-defined variables during auto-capture.
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
    "WeakRef",
    "FinalizationRegistry",
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
    "Float16Array",
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
    // deno_core / V8-specific globals
    "Deno",
    "queueMicrotask",
    "structuredClone",
    "Temporal",
    "WebAssembly",
];

/// Check if a global variable name is a JS builtin that should be skipped.
///
/// # Arguments
///
/// * `name` - The global property name to test
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
    fn test_is_builtin_engine_specific() {
        // The deno_core engine exposes a `Deno` namespace and web globals.
        assert!(is_builtin("Deno"));
        assert!(is_builtin("queueMicrotask"));
        assert!(is_builtin("structuredClone"));
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
}
