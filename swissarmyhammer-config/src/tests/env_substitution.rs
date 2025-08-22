//! Tests for environment variable substitution

use crate::types::TemplateContext;

#[test]
fn test_complex_env_substitution_patterns() {
    std::env::set_var("HOST", "api.example.com");
    std::env::set_var("PORT", "8080");
    std::env::set_var("PROTOCOL", "https");
    std::env::set_var("API_KEY", "secret_key_123");

    let mut ctx = TemplateContext::new();
    
    // Test various substitution patterns
    ctx.set("simple_var".to_string(), serde_json::Value::String("${HOST}".to_string()));
    ctx.set("with_default".to_string(), serde_json::Value::String("${MISSING_VAR:-default_value}".to_string()));
    ctx.set("multiple_vars".to_string(), serde_json::Value::String("${PROTOCOL}://${HOST}:${PORT}".to_string()));
    ctx.set("mixed_content".to_string(), serde_json::Value::String("API endpoint: ${PROTOCOL}://${HOST}:${PORT}/api/v1".to_string()));
    ctx.set("nested_structure".to_string(), serde_json::json!({
        "connection": {
            "url": "${PROTOCOL}://${HOST}:${PORT}",
            "timeout": "${TIMEOUT:-30}",
            "credentials": {
                "api_key": "${API_KEY}",
                "token": "${TOKEN:-temp_token}"
            }
        }
    }));

    ctx.substitute_env_vars().unwrap();

    // Verify substitutions
    assert_eq!(ctx.get("simple_var"), Some(&serde_json::Value::String("api.example.com".to_string())));
    assert_eq!(ctx.get("with_default"), Some(&serde_json::Value::String("default_value".to_string())));
    assert_eq!(ctx.get("multiple_vars"), Some(&serde_json::Value::String("https://api.example.com:8080".to_string())));
    assert_eq!(ctx.get("mixed_content"), Some(&serde_json::Value::String("API endpoint: https://api.example.com:8080/api/v1".to_string())));

    // Check nested structure
    if let Some(nested) = ctx.get("nested_structure") {
        assert_eq!(nested["connection"]["url"], serde_json::Value::String("https://api.example.com:8080".to_string()));
        assert_eq!(nested["connection"]["timeout"], serde_json::Value::String("30".to_string()));
        assert_eq!(nested["connection"]["credentials"]["api_key"], serde_json::Value::String("secret_key_123".to_string()));
        assert_eq!(nested["connection"]["credentials"]["token"], serde_json::Value::String("temp_token".to_string()));
    } else {
        panic!("Nested structure should be present");
    }

    // Clean up
    std::env::remove_var("HOST");
    std::env::remove_var("PORT");
    std::env::remove_var("PROTOCOL");
    std::env::remove_var("API_KEY");
}

#[test]
fn test_env_substitution_edge_cases() {
    std::env::set_var("EDGE_TEST", "edge_value");
    
    let mut ctx = TemplateContext::new();
    
    // Edge case patterns
    ctx.set("empty_default".to_string(), serde_json::Value::String("${MISSING:-}".to_string()));
    ctx.set("space_in_default".to_string(), serde_json::Value::String("${MISSING:-default with spaces}".to_string()));
    ctx.set("adjacent_vars".to_string(), serde_json::Value::String("${EDGE_TEST}${EDGE_TEST}".to_string()));
    ctx.set("var_with_text".to_string(), serde_json::Value::String("prefix_${EDGE_TEST}_suffix".to_string()));
    ctx.set("special_chars_in_default".to_string(), serde_json::Value::String("${MISSING:-default:with:colons}".to_string()));

    ctx.substitute_env_vars().unwrap();

    assert_eq!(ctx.get("empty_default"), Some(&serde_json::Value::String("".to_string())));
    assert_eq!(ctx.get("space_in_default"), Some(&serde_json::Value::String("default with spaces".to_string())));
    assert_eq!(ctx.get("adjacent_vars"), Some(&serde_json::Value::String("edge_valueedge_value".to_string())));
    assert_eq!(ctx.get("var_with_text"), Some(&serde_json::Value::String("prefix_edge_value_suffix".to_string())));
    assert_eq!(ctx.get("special_chars_in_default"), Some(&serde_json::Value::String("default:with:colons".to_string())));

    std::env::remove_var("EDGE_TEST");
}

#[test]
fn test_env_substitution_in_arrays() {
    std::env::set_var("ITEM_PREFIX", "test");
    std::env::set_var("ITEM_COUNT", "3");
    
    let mut ctx = TemplateContext::new();
    ctx.set("config_array".to_string(), serde_json::json!([
        "${ITEM_PREFIX}_item_1",
        "${ITEM_PREFIX}_item_2",
        "static_item",
        "${MISSING_ITEM:-default_item}",
        "count_${ITEM_COUNT}"
    ]));

    ctx.substitute_env_vars().unwrap();

    if let Some(serde_json::Value::Array(arr)) = ctx.get("config_array") {
        assert_eq!(arr[0], serde_json::Value::String("test_item_1".to_string()));
        assert_eq!(arr[1], serde_json::Value::String("test_item_2".to_string()));
        assert_eq!(arr[2], serde_json::Value::String("static_item".to_string()));
        assert_eq!(arr[3], serde_json::Value::String("default_item".to_string()));
        assert_eq!(arr[4], serde_json::Value::String("count_3".to_string()));
    } else {
        panic!("Config array should be an array");
    }

    std::env::remove_var("ITEM_PREFIX");
    std::env::remove_var("ITEM_COUNT");
}

#[test]
fn test_env_substitution_error_handling() {
    let mut ctx = TemplateContext::new();
    ctx.set("missing_var".to_string(), serde_json::Value::String("${DEFINITELY_MISSING_VAR}".to_string()));

    let result = ctx.substitute_env_vars();
    assert!(result.is_err());
    
    if let Err(err) = result {
        assert!(err.to_string().contains("DEFINITELY_MISSING_VAR"));
        assert!(err.to_string().contains("not found"));
    }
}

#[test]
fn test_env_substitution_preserves_non_strings() {
    std::env::set_var("STRING_VAR", "substituted");
    
    let mut ctx = TemplateContext::new();
    ctx.set("number".to_string(), serde_json::Value::Number(42.into()));
    ctx.set("boolean".to_string(), serde_json::Value::Bool(true));
    ctx.set("null_val".to_string(), serde_json::Value::Null);
    ctx.set("string_with_sub".to_string(), serde_json::Value::String("value: ${STRING_VAR}".to_string()));
    ctx.set("object".to_string(), serde_json::json!({
        "nested_string": "${STRING_VAR}",
        "nested_number": 123
    }));

    ctx.substitute_env_vars().unwrap();

    // Non-string values should be preserved
    assert_eq!(ctx.get("number"), Some(&serde_json::Value::Number(42.into())));
    assert_eq!(ctx.get("boolean"), Some(&serde_json::Value::Bool(true)));
    assert_eq!(ctx.get("null_val"), Some(&serde_json::Value::Null));

    // String substitution should work
    assert_eq!(ctx.get("string_with_sub"), Some(&serde_json::Value::String("value: substituted".to_string())));

    // Nested string substitution should work, non-strings preserved
    if let Some(obj) = ctx.get("object") {
        assert_eq!(obj["nested_string"], serde_json::Value::String("substituted".to_string()));
        assert_eq!(obj["nested_number"], serde_json::Value::Number(123.into()));
    }

    std::env::remove_var("STRING_VAR");
}