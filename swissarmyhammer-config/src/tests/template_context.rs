//! Integration tests for TemplateContext

use crate::types::TemplateContext;

#[test]
fn test_template_context_liquid_conversion() {
    let mut ctx = TemplateContext::new();

    // Add various types of data
    ctx.set(
        "string_val".to_string(),
        serde_json::Value::String("hello".to_string()),
    );
    ctx.set(
        "number_val".to_string(),
        serde_json::Value::Number(42.into()),
    );
    ctx.set("bool_val".to_string(), serde_json::Value::Bool(true));
    ctx.set(
        "array_val".to_string(),
        serde_json::Value::Array(vec![
            serde_json::Value::String("item1".to_string()),
            serde_json::Value::Number(2.into()),
        ]),
    );
    ctx.set(
        "object_val".to_string(),
        serde_json::json!({
            "nested": "value",
            "count": 10
        }),
    );

    let liquid_obj = ctx.to_liquid_object();

    // Verify conversion worked
    assert!(liquid_obj.contains_key("string_val"));
    assert!(liquid_obj.contains_key("number_val"));
    assert!(liquid_obj.contains_key("bool_val"));
    assert!(liquid_obj.contains_key("array_val"));
    assert!(liquid_obj.contains_key("object_val"));
}

#[test]
fn test_template_context_complex_merge() {
    let mut base_ctx = TemplateContext::new();
    base_ctx.set(
        "app_name".to_string(),
        serde_json::Value::String("SwissArmyHammer".to_string()),
    );
    base_ctx.set(
        "version".to_string(),
        serde_json::Value::String("1.0.0".to_string()),
    );
    base_ctx.set(
        "features".to_string(),
        serde_json::Value::Array(vec![
            serde_json::Value::String("workflows".to_string()),
            serde_json::Value::String("prompts".to_string()),
        ]),
    );

    let mut override_ctx = TemplateContext::new();
    override_ctx.set(
        "version".to_string(),
        serde_json::Value::String("2.0.0".to_string()),
    );
    override_ctx.set(
        "features".to_string(),
        serde_json::Value::Array(vec![
            serde_json::Value::String("workflows".to_string()),
            serde_json::Value::String("prompts".to_string()),
            serde_json::Value::String("mcp".to_string()),
        ]),
    );
    override_ctx.set("debug".to_string(), serde_json::Value::Bool(true));

    base_ctx.merge(&override_ctx);

    // Check that override values took precedence
    assert_eq!(
        base_ctx.get("app_name"),
        Some(&serde_json::Value::String("SwissArmyHammer".to_string()))
    );
    assert_eq!(
        base_ctx.get("version"),
        Some(&serde_json::Value::String("2.0.0".to_string()))
    );
    assert_eq!(base_ctx.get("debug"), Some(&serde_json::Value::Bool(true)));

    // Array should be completely replaced, not merged
    let features = base_ctx.get("features").unwrap();
    if let serde_json::Value::Array(arr) = features {
        assert_eq!(arr.len(), 3);
        assert!(arr.contains(&serde_json::Value::String("mcp".to_string())));
    } else {
        panic!("features should be an array");
    }
}

#[test]
fn test_template_context_deeply_nested_env_substitution() {
    std::env::set_var("DEEP_TEST_VAR", "deep_value");
    std::env::set_var("ANOTHER_VAR", "another_value");

    let mut ctx = TemplateContext::new();
    ctx.set(
        "config".to_string(),
        serde_json::json!({
            "database": {
                "url": "postgresql://${DEEP_TEST_VAR}:5432/db",
                "options": {
                    "pool_size": 10,
                    "timeout": "${ANOTHER_VAR:-30}"
                }
            },
            "services": [
                {
                    "name": "api",
                    "host": "${DEEP_TEST_VAR}.example.com"
                },
                {
                    "name": "worker",
                    "env": "${NONEXISTENT:-production}"
                }
            ]
        }),
    );

    ctx.substitute_env_vars().unwrap();

    // Verify deep substitution worked
    let config = ctx.get("config").unwrap();
    let db_url = config["database"]["url"].as_str().unwrap();
    assert_eq!(db_url, "postgresql://deep_value:5432/db");

    let timeout = config["database"]["options"]["timeout"].as_str().unwrap();
    assert_eq!(timeout, "another_value");

    let api_host = config["services"][0]["host"].as_str().unwrap();
    assert_eq!(api_host, "deep_value.example.com");

    let worker_env = config["services"][1]["env"].as_str().unwrap();
    assert_eq!(worker_env, "production");

    // Clean up
    std::env::remove_var("DEEP_TEST_VAR");
    std::env::remove_var("ANOTHER_VAR");
}

#[test]
fn test_template_context_multiple_env_vars_in_single_string() {
    std::env::set_var("HOST", "localhost");
    std::env::set_var("PORT", "3000");

    let mut ctx = TemplateContext::new();
    ctx.set(
        "connection_string".to_string(),
        serde_json::Value::String("http://${HOST}:${PORT}/api/v1".to_string()),
    );

    ctx.substitute_env_vars().unwrap();

    assert_eq!(
        ctx.get("connection_string"),
        Some(&serde_json::Value::String(
            "http://localhost:3000/api/v1".to_string()
        ))
    );

    // Clean up
    std::env::remove_var("HOST");
    std::env::remove_var("PORT");
}

#[test]
fn test_template_context_bidirectional_liquid_conversion() {
    // Create a TemplateContext with various data types
    let mut original_ctx = TemplateContext::new();
    original_ctx.set(
        "string_val",
        serde_json::Value::String("hello world".to_string()),
    );
    original_ctx.set("int_val", serde_json::Value::Number(42.into()));
    original_ctx.set("float_val", serde_json::json!(2.71)); // Using e approximation instead of PI
    original_ctx.set("bool_val", serde_json::Value::Bool(true));
    original_ctx.set("null_val", serde_json::Value::Null);
    original_ctx.set("array_val", serde_json::json!(["item1", 2, true]));
    original_ctx.set(
        "object_val",
        serde_json::json!({
            "nested_string": "nested value",
            "nested_number": 123,
            "nested_array": [1, 2, 3]
        }),
    );

    // Convert to liquid::Object
    let liquid_obj = original_ctx.to_liquid_object();
    assert_eq!(liquid_obj.len(), 7);

    // Convert back to TemplateContext
    let roundtrip_ctx = TemplateContext::from_liquid_object(liquid_obj);

    // Verify all values are preserved
    assert_eq!(
        roundtrip_ctx.get_string("string_val"),
        Some("hello world".to_string())
    );
    assert_eq!(roundtrip_ctx.get_number("int_val"), Some(42.0));
    assert_eq!(roundtrip_ctx.get_number("float_val"), Some(2.71));
    assert_eq!(roundtrip_ctx.get_bool("bool_val"), Some(true));
    assert_eq!(
        roundtrip_ctx.get("null_val"),
        Some(&serde_json::Value::Null)
    );

    // Verify array conversion
    let array_val = roundtrip_ctx.get("array_val").unwrap();
    assert_eq!(array_val, &serde_json::json!(["item1", 2, true]));

    // Verify object conversion
    let object_val = roundtrip_ctx.get("object_val").unwrap();
    assert_eq!(object_val["nested_string"], "nested value");
    assert_eq!(object_val["nested_number"], 123);
    assert_eq!(object_val["nested_array"], serde_json::json!([1, 2, 3]));
}

#[test]
fn test_template_context_from_trait_liquid_object() {
    // Test the From trait implementation
    let mut original_ctx = TemplateContext::new();
    original_ctx.set(
        "test_key",
        serde_json::Value::String("test_value".to_string()),
    );

    // Convert to liquid::Object using Into trait
    let liquid_obj: liquid::Object = original_ctx.clone().into();

    // Convert back using From trait
    let ctx_from_liquid: TemplateContext = liquid_obj.into();

    assert_eq!(
        ctx_from_liquid.get_string("test_key"),
        Some("test_value".to_string())
    );
}
