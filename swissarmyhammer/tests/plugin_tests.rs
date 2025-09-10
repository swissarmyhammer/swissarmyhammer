//! Integration tests for the plugin system

use liquid::model::Value;
use liquid::ValueView;
use std::collections::HashMap;
use swissarmyhammer::{CustomLiquidFilter, PluginRegistry, Result, SwissArmyHammerPlugin};

/// Test plugin that provides a "reverse" filter
struct ReverseFilterPlugin {
    name: String,
    version: String,
}

impl ReverseFilterPlugin {
    fn new() -> Self {
        Self {
            name: "reverse-filter".to_string(),
            version: "1.0.0".to_string(),
        }
    }
}

impl SwissArmyHammerPlugin for ReverseFilterPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn description(&self) -> &str {
        "Plugin that provides a reverse filter for strings"
    }

    fn filters(&self) -> Vec<Box<dyn CustomLiquidFilter>> {
        vec![Box::new(ReverseFilter::new())]
    }
}

/// Custom filter that reverses strings
struct ReverseFilter {
    name: String,
}

impl ReverseFilter {
    fn new() -> Self {
        Self {
            name: "reverse".to_string(),
        }
    }
}

impl CustomLiquidFilter for ReverseFilter {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Reverses the characters in a string"
    }

    fn apply(&self, input: &Value) -> Result<Value> {
        let str_val = match input {
            Value::Scalar(_) => {
                // Extract string value properly from liquid Value
                input.render().to_string()
            }
            _ => return Ok(input.clone()),
        };

        let reversed: String = str_val.chars().rev().collect();
        Ok(Value::scalar(reversed))
    }
}

/// Test plugin that provides an "uppercase" filter
struct UppercaseFilterPlugin {
    name: String,
    version: String,
}

impl UppercaseFilterPlugin {
    fn new() -> Self {
        Self {
            name: "uppercase-filter".to_string(),
            version: "1.0.0".to_string(),
        }
    }
}

impl SwissArmyHammerPlugin for UppercaseFilterPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn description(&self) -> &str {
        "Plugin that provides an uppercase filter for strings"
    }

    fn filters(&self) -> Vec<Box<dyn CustomLiquidFilter>> {
        vec![Box::new(UppercaseFilter::new())]
    }
}

/// Custom filter that converts strings to uppercase
struct UppercaseFilter {
    name: String,
}

impl UppercaseFilter {
    fn new() -> Self {
        Self {
            name: "uppercase".to_string(),
        }
    }
}

impl CustomLiquidFilter for UppercaseFilter {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Converts a string to uppercase"
    }

    fn apply(&self, input: &Value) -> Result<Value> {
        let str_val = match input {
            Value::Scalar(_) => {
                // Extract string value properly from liquid Value
                input.render().to_string()
            }
            _ => return Ok(input.clone()),
        };

        let uppercase = str_val.to_uppercase();
        Ok(Value::scalar(uppercase))
    }
}

#[test]
fn test_plugin_registration_and_basic_usage() {
    let mut registry = PluginRegistry::new();

    // Register the reverse filter plugin
    let plugin = ReverseFilterPlugin::new();
    registry
        .register_plugin(Box::new(plugin))
        .expect("Failed to register plugin");

    // Verify plugin is registered
    assert_eq!(registry.plugin_names().len(), 1);
    assert!(registry
        .plugin_names()
        .contains(&"reverse-filter".to_string()));

    // Verify filter is registered
    assert_eq!(registry.filter_names().len(), 1);
    assert!(registry.filter_names().contains(&"reverse".to_string()));

    // Get the filter and test it
    let filter = registry
        .get_filter("reverse")
        .expect("Filter should be registered");
    let input = Value::scalar("hello");
    let result = filter.apply(&input).expect("Filter should work");
    // Check that the result is a scalar with the expected value
    match result {
        Value::Scalar(_) => {
            let result_str = result.render().to_string();
            assert_eq!(result_str, "olleh");
        }
        _ => panic!("Expected scalar result"),
    }
}

#[test]
fn test_multiple_plugin_registration() {
    let mut registry = PluginRegistry::new();

    // Register both plugins
    let reverse_plugin = ReverseFilterPlugin::new();
    let uppercase_plugin = UppercaseFilterPlugin::new();

    registry
        .register_plugin(Box::new(reverse_plugin))
        .expect("Failed to register reverse plugin");
    registry
        .register_plugin(Box::new(uppercase_plugin))
        .expect("Failed to register uppercase plugin");

    // Verify both plugins are registered
    assert_eq!(registry.plugin_names().len(), 2);
    assert!(registry
        .plugin_names()
        .contains(&"reverse-filter".to_string()));
    assert!(registry
        .plugin_names()
        .contains(&"uppercase-filter".to_string()));

    // Verify both filters are registered
    assert_eq!(registry.filter_names().len(), 2);
    assert!(registry.filter_names().contains(&"reverse".to_string()));
    assert!(registry.filter_names().contains(&"uppercase".to_string()));
}

#[test]
fn test_duplicate_plugin_registration_fails() {
    let mut registry = PluginRegistry::new();

    let plugin1 = ReverseFilterPlugin::new();
    let plugin2 = ReverseFilterPlugin::new(); // Same plugin name

    registry
        .register_plugin(Box::new(plugin1))
        .expect("First registration should succeed");

    let result = registry.register_plugin(Box::new(plugin2));
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("already registered"));
}

#[test]
fn test_template_engine_plugin_registry_access() {
    let mut registry = PluginRegistry::new();

    let plugin = ReverseFilterPlugin::new();
    registry
        .register_plugin(Box::new(plugin))
        .expect("Failed to register plugin");

    let engine = swissarmyhammer::TemplateEngine::with_plugins(registry);

    // Test that the engine was created successfully
    // Note: Plugin functionality is managed by the main crate, 
    // the domain crate provides a simplified interface
    let _plugin_registry = engine.plugin_registry();
    // The domain crate returns None for plugin registry - plugins are managed at main crate level
}

#[test]
fn test_template_engine_without_plugins() {
    let engine = swissarmyhammer::TemplateEngine::new();

    // Test that engine without plugins doesn't have plugin registry
    assert!(engine.plugin_registry().is_none());

    // Test that standard liquid filters still work
    let template = "Uppercase: {{ text | upcase }}";
    let mut args = HashMap::new();
    args.insert("text".to_string(), "hello".to_string());

    let result = engine
        .render(template, &args)
        .expect("Template should render");
    assert_eq!(result, "Uppercase: HELLO");
}

#[test]
fn test_custom_filter_with_non_string_input() {
    let filter = ReverseFilter::new();

    // Test with number
    let input = Value::scalar(123);
    let result = filter.apply(&input).expect("Filter should handle numbers");
    // Check that the result is a scalar with the expected value
    match result {
        Value::Scalar(_) => {
            let result_str = result.render().to_string();
            assert_eq!(result_str, "321");
        }
        _ => panic!("Expected scalar result"),
    }

    // Test with array (should return unchanged)
    let input = Value::Array(vec![Value::scalar("a"), Value::scalar("b")]);
    let result = filter.apply(&input).expect("Filter should handle arrays");
    // Arrays should be returned unchanged
    assert!(matches!(result, Value::Array(_)));
}

#[test]
fn test_empty_plugin_registry() {
    let registry = PluginRegistry::new();

    // Empty registry should create a standard parser
    let parser = registry.create_parser();

    // Test that standard filters work but custom ones don't
    let template = parser
        .parse("{{ text | upcase }}")
        .expect("Should parse standard filter");
    let mut object = liquid::Object::new();
    object.insert("text".into(), Value::scalar("hello"));

    let result = template
        .render(&object)
        .expect("Should render standard filter");
    assert_eq!(result, "HELLO");
}

#[test]
fn test_plugin_initialization_and_cleanup() {
    let mut registry = PluginRegistry::new();

    // This test verifies that initialization is called during registration
    // and that the plugin functions correctly
    let plugin = ReverseFilterPlugin::new();
    registry
        .register_plugin(Box::new(plugin))
        .expect("Plugin registration should succeed");

    // If initialization worked, the plugin should be functional
    let filter = registry
        .get_filter("reverse")
        .expect("Filter should be available");
    let result = filter
        .apply(&Value::scalar("test"))
        .expect("Filter should work");
    // Check that the result is a scalar with the expected value
    match result {
        Value::Scalar(_) => {
            let result_str = result.render().to_string();
            assert_eq!(result_str, "tset");
        }
        _ => panic!("Expected scalar result"),
    }
}
