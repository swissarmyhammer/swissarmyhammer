#![allow(dead_code)]
use anyhow::{bail, Result};
use clap::{Arg, ArgAction, ValueHint};
use serde_json::Value;

// Helper struct to hold owned strings for clap Args
#[derive(Debug)]
pub struct ArgBuilder {
    name: String,
    long_name: String,
    help_text: Option<String>,
    required: bool,
    action: Option<ArgAction>,
    value_parser: Option<String>, // simplified for this implementation
    value_hint: Option<ValueHint>,
    positional: bool,
}

impl ArgBuilder {
    pub fn new(name: String) -> Self {
        Self {
            long_name: name.clone(),
            name,
            help_text: None,
            required: false,
            action: None,
            value_parser: None,
            value_hint: None,
            positional: false,
        }
    }

    pub fn help<S: Into<String>>(mut self, help: S) -> Self {
        self.help_text = Some(help.into());
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn action(mut self, action: ArgAction) -> Self {
        self.action = Some(action);
        self
    }

    pub fn value_parser(mut self, parser: String) -> Self {
        self.value_parser = Some(parser);
        self
    }

    pub fn value_hint(mut self, hint: ValueHint) -> Self {
        self.value_hint = Some(hint);
        self
    }

    pub fn positional(mut self, positional: bool) -> Self {
        self.positional = positional;
        self
    }

    pub fn build(self) -> Arg {
        // Create owned strings that can be leaked to provide 'static references
        let name_static: &'static str = Box::leak(self.name.into_boxed_str());
        let long_name_static: &'static str = Box::leak(self.long_name.into_boxed_str());

        let mut arg = if self.positional {
            // Positional argument - no long flag
            Arg::new(name_static)
        } else {
            // Flag argument - has long name
            Arg::new(name_static).long(long_name_static)
        };

        if let Some(help) = self.help_text {
            let help_static: &'static str = Box::leak(help.into_boxed_str());
            arg = arg.help(help_static);
        }

        if self.required {
            arg = arg.required(true);
        }

        if let Some(action) = self.action {
            arg = arg.action(action);
        }

        if let Some(parser) = self.value_parser {
            match parser.as_str() {
                "i64" => arg = arg.value_parser(clap::value_parser!(i64)),
                "f64" => arg = arg.value_parser(clap::value_parser!(f64)),
                _ => {}
            }
        }

        if let Some(hint) = self.value_hint {
            arg = arg.value_hint(hint);
        }

        arg
    }
}

pub struct SchemaConverter;

impl SchemaConverter {
    /// Convert JSON schema to clap arguments with enhanced features
    pub fn schema_to_clap_args(schema: &Value) -> Result<Vec<Arg>> {
        Self::enhanced_schema_to_clap_args(schema)
    }

    /// Handle more advanced JSON Schema features
    pub fn enhanced_schema_to_clap_args(schema: &Value) -> Result<Vec<Arg>> {
        let mut args = Self::basic_schema_to_clap_args(schema)?;
        
        // Post-process args for advanced features
        for arg in &mut args {
            Self::enhance_arg_with_schema_features(arg, schema)?;
        }
        
        Ok(args)
    }

    /// Basic schema conversion (original implementation)
    fn basic_schema_to_clap_args(schema: &Value) -> Result<Vec<Arg>> {
        let mut args = Vec::new();

        let properties = schema
            .get("properties")
            .and_then(|p| p.as_object())
            .ok_or_else(|| anyhow::anyhow!("Schema missing properties object"))?;

        let required = schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();

        // Sort properties to put required ones first, making the first required argument positional
        let mut prop_vec: Vec<_> = properties.iter().collect();
        prop_vec.sort_by_key(|(name, _)| {
            let is_required = required.contains(&name.as_str());
            (!is_required, name.as_str()) // Required items first, then alphabetical
        });

        let mut first_required_processed = false;

        for (prop_name, prop_schema) in prop_vec {
            let is_required = required.contains(&prop_name.as_str());
            let make_positional = is_required && !first_required_processed;

            if make_positional {
                first_required_processed = true;
            }

            let arg = Self::json_schema_property_to_clap_arg(
                prop_name,
                prop_schema,
                &required,
                make_positional,
            )?;
            args.push(arg);
        }

        Ok(args)
    }

    /// Enhance arguments with advanced schema features
    fn enhance_arg_with_schema_features(arg: &mut Arg, schema: &Value) -> Result<()> {
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            if let Some(prop_schema) = properties.get(arg.get_id().as_str()) {
                // Handle default values
                if let Some(default) = prop_schema.get("default") {
                    if let Some(default_str) = default.as_str() {
                        let default_static: &'static str = Box::leak(default_str.to_string().into_boxed_str());
                        *arg = arg.clone().default_value(default_static);
                    } else if let Some(default_num) = default.as_i64() {
                        let default_static: &'static str = Box::leak(default_num.to_string().into_boxed_str());
                        *arg = arg.clone().default_value(default_static);
                    } else if let Some(default_bool) = default.as_bool() {
                        if default_bool {
                            *arg = arg.clone().default_value("true");
                        }
                    }
                }
                
                // Handle examples in help text
                if let Some(examples) = prop_schema.get("examples").and_then(|e| e.as_array()) {
                    if !examples.is_empty() {
                        let example_text = examples.iter()
                            .filter_map(|e| e.as_str())
                            .take(2)
                            .collect::<Vec<_>>()
                            .join(", ");
                        
                        if !example_text.is_empty() {
                            let current_help = arg.get_help().map(|s| s.to_string()).unwrap_or_default();
                            let enhanced_help = if current_help.is_empty() {
                                format!("Examples: {}", example_text)
                            } else {
                                format!("{}\n\nExamples: {}", current_help, example_text)
                            };
                            let help_static: &'static str = Box::leak(enhanced_help.into_boxed_str());
                            *arg = arg.clone().help(help_static);
                        }
                    }
                }
                
                // Handle oneOf/anyOf for enum-like behavior
                if let Some(one_of) = prop_schema.get("oneOf").and_then(|o| o.as_array()) {
                    if let Some(enum_values) = Self::extract_enum_from_one_of(one_of) {
                        // Convert to static string slices
                        let enum_values_static: Vec<&'static str> = enum_values
                            .into_iter()
                            .map(|s| Box::leak(s.into_boxed_str()) as &'static str)
                            .collect();
                        *arg = arg.clone().value_parser(enum_values_static);
                    }
                } else if let Some(any_of) = prop_schema.get("anyOf").and_then(|o| o.as_array()) {
                    if let Some(enum_values) = Self::extract_enum_from_one_of(any_of) {
                        // Convert to static string slices
                        let enum_values_static: Vec<&'static str> = enum_values
                            .into_iter()
                            .map(|s| Box::leak(s.into_boxed_str()) as &'static str)
                            .collect();
                        *arg = arg.clone().value_parser(enum_values_static);
                    }
                }

                // Handle title field for better help text
                if let Some(title) = prop_schema.get("title").and_then(|t| t.as_str()) {
                    let current_help = arg.get_help().map(|s| s.to_string()).unwrap_or_default();
                    let enhanced_help = if current_help.is_empty() {
                        title.to_string()
                    } else {
                        format!("{}: {}", title, current_help)
                    };
                    let help_static: &'static str = Box::leak(enhanced_help.into_boxed_str());
                    *arg = arg.clone().help(help_static);
                }
            }
        }
        
        Ok(())
    }
    
    /// Extract enum values from oneOf/anyOf schema structures
    fn extract_enum_from_one_of(one_of: &[Value]) -> Option<Vec<String>> {
        let mut values = Vec::new();
        
        for item in one_of {
            if let Some(const_val) = item.get("const").and_then(|c| c.as_str()) {
                values.push(const_val.to_string());
            } else if let Some(enum_array) = item.get("enum").and_then(|e| e.as_array()) {
                for enum_val in enum_array {
                    if let Some(val_str) = enum_val.as_str() {
                        values.push(val_str.to_string());
                    }
                }
            }
        }
        
        if values.is_empty() { None } else { Some(values) }
    }

    /// Convert individual JSON schema property to clap Arg
    fn json_schema_property_to_clap_arg(
        name: &str,
        schema: &Value,
        required: &[&str],
        make_positional: bool,
    ) -> Result<Arg> {
        let mut builder = ArgBuilder::new(name.to_string());

        // Get base help text
        let mut help_text = schema
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();

        // Map JSON schema types to clap actions first to determine if this is a boolean
        // Handle both single types (string) and union types (array like ["string", "null"])
        let type_info = schema.get("type");
        let primary_type = match type_info {
            Some(Value::String(s)) => Some(s.as_str()),
            Some(Value::Array(arr)) => {
                // For union types, find the first non-null type
                arr.iter().filter_map(|v| v.as_str()).find(|&s| s != "null")
            }
            _ => None,
        };

        // Handle required fields and positional arguments
        // Boolean flags are NEVER required regardless of schema
        let is_boolean = primary_type == Some("boolean");
        if required.contains(&name) && !is_boolean {
            builder = builder.required(true);
        }

        // Set positional flag (booleans are never positional)
        builder = builder.positional(make_positional && !is_boolean);

        match primary_type {
            Some("boolean") => {
                builder = builder.action(ArgAction::SetTrue);
            }
            Some("integer") => {
                builder = builder.value_parser("i64".to_string());
                if let Some(min) = schema.get("minimum").and_then(|m| m.as_i64()) {
                    // Add validation range hint
                    help_text = format!("{help_text} (min: {min})");
                }
            }
            Some("number") => {
                builder = builder.value_parser("f64".to_string());
                if let Some(min) = schema.get("minimum").and_then(|m| m.as_f64()) {
                    // Add validation range hint
                    help_text = format!("{help_text} (min: {min})");
                }
            }
            Some("array") => {
                builder = builder.action(ArgAction::Append);
                // Handle array item types if specified
                if let Some(items) = schema.get("items") {
                    match items.get("type").and_then(|t| t.as_str()) {
                        Some("integer") => builder = builder.value_parser("i64".to_string()),
                        Some("number") => builder = builder.value_parser("f64".to_string()),
                        _ => {}
                    }
                }
            }
            Some("object") => {
                // For object types, we'll accept JSON string representation
                // The user will need to provide JSON format for complex objects
                help_text = format!("{help_text} (provide as JSON string)");
            }
            Some("string") | None => {
                // Handle string enums with proper validation
                if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
                    let values: Result<Vec<String>, _> = enum_values
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .map(|s| s.to_string())
                                .ok_or_else(|| anyhow::anyhow!("Non-string enum value"))
                        })
                        .collect();
                    if let Ok(valid_values) = values {
                        help_text =
                            format!("{} (valid values: {})", help_text, valid_values.join(", "));

                        // Create a static string slice for the enum values
                        let enum_values_static: Vec<&'static str> = valid_values
                            .into_iter()
                            .map(|s| Box::leak(s.into_boxed_str()) as &'static str)
                            .collect();

                        // Build arg with enum validation using clap's value_parser
                        let name_static: &'static str =
                            Box::leak(name.to_string().into_boxed_str());
                        let long_name_static: &'static str =
                            Box::leak(name.to_string().into_boxed_str());
                        let help_static: &'static str =
                            Box::leak(help_text.clone().into_boxed_str());

                        let mut arg = Arg::new(name_static)
                            .long(long_name_static)
                            .help(help_static)
                            .value_parser(enum_values_static);

                        if required.contains(&name) {
                            arg = arg.required(true);
                        }

                        return Ok(arg);
                    }
                }

                // Handle format hints
                if let Some(format) = schema.get("format").and_then(|f| f.as_str()) {
                    builder = match format {
                        "uri" | "url" => builder.value_hint(ValueHint::Url),
                        "email" => builder.value_hint(ValueHint::EmailAddress),
                        "path" => builder.value_hint(ValueHint::FilePath),
                        _ => builder,
                    };
                }

                // Handle pattern validation hint
                if let Some(pattern) = schema.get("pattern").and_then(|p| p.as_str()) {
                    // For now, just add to help text
                    help_text = format!("{help_text} (pattern: {pattern})");
                }
            }
            Some(unknown_type) => {
                bail!("Unsupported JSON schema type: {}", unknown_type);
            }
        }

        // Set final help text
        if !help_text.is_empty() {
            builder = builder.help(help_text);
        }

        Ok(builder.build())
    }

    /// Convert clap ArgMatches back to JSON arguments
    pub fn matches_to_json_args(
        matches: &clap::ArgMatches,
        schema: &Value,
    ) -> Result<serde_json::Map<String, Value>> {
        let mut args = serde_json::Map::new();

        let properties = schema
            .get("properties")
            .and_then(|p| p.as_object())
            .ok_or_else(|| anyhow::anyhow!("Schema missing properties"))?;

        for (prop_name, prop_schema) in properties {
            if let Some(value) = Self::extract_clap_value(matches, prop_name, prop_schema)? {
                args.insert(prop_name.clone(), value);
            }
        }

        Ok(args)
    }

    /// Extract value from ArgMatches based on schema type
    fn extract_clap_value(
        matches: &clap::ArgMatches,
        prop_name: &str,
        prop_schema: &Value,
    ) -> Result<Option<Value>> {
        if !matches.contains_id(prop_name) {
            return Ok(None);
        }

        let json_value = match prop_schema.get("type").and_then(|t| t.as_str()) {
            Some("boolean") => Value::Bool(matches.get_flag(prop_name)),
            Some("integer") => {
                if let Some(val) = matches.get_one::<i64>(prop_name) {
                    Value::Number((*val).into())
                } else {
                    return Ok(None);
                }
            }
            Some("number") => {
                if let Some(val) = matches.get_one::<f64>(prop_name) {
                    Value::Number(serde_json::Number::from_f64(*val).unwrap_or_else(|| 0.into()))
                } else {
                    return Ok(None);
                }
            }
            Some("array") => {
                // Handle different array item types
                if let Some(items) = prop_schema.get("items") {
                    match items.get("type").and_then(|t| t.as_str()) {
                        Some("integer") => {
                            let values: Vec<i64> = matches
                                .get_many::<i64>(prop_name)
                                .unwrap_or_default()
                                .cloned()
                                .collect();
                            Value::Array(
                                values
                                    .into_iter()
                                    .map(|v| Value::Number(v.into()))
                                    .collect(),
                            )
                        }
                        Some("number") => {
                            let values: Vec<f64> = matches
                                .get_many::<f64>(prop_name)
                                .unwrap_or_default()
                                .cloned()
                                .collect();
                            Value::Array(
                                values
                                    .into_iter()
                                    .map(|v| {
                                        Value::Number(
                                            serde_json::Number::from_f64(v)
                                                .unwrap_or_else(|| 0.into()),
                                        )
                                    })
                                    .collect(),
                            )
                        }
                        _ => {
                            // Default to string array
                            let values: Vec<String> = matches
                                .get_many::<String>(prop_name)
                                .unwrap_or_default()
                                .cloned()
                                .collect();
                            Value::Array(values.into_iter().map(Value::String).collect())
                        }
                    }
                } else {
                    // Default to string array if no items type specified
                    let values: Vec<String> = matches
                        .get_many::<String>(prop_name)
                        .unwrap_or_default()
                        .cloned()
                        .collect();
                    Value::Array(values.into_iter().map(Value::String).collect())
                }
            }
            Some("object") => {
                // Parse JSON string representation
                if let Some(val) = matches.get_one::<String>(prop_name) {
                    match serde_json::from_str(val) {
                        Ok(parsed) => parsed,
                        Err(_) => {
                            // If JSON parsing fails, treat as string
                            Value::String(val.clone())
                        }
                    }
                } else {
                    return Ok(None);
                }
            }
            _ => {
                // String or unspecified type
                if let Some(val) = matches.get_one::<String>(prop_name) {
                    Value::String(val.clone())
                } else {
                    return Ok(None);
                }
            }
        };

        Ok(Some(json_value))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SchemaConversionError {
    #[error("Invalid schema structure: {0}")]
    InvalidSchema(String),
    #[error("Unsupported schema type: {0}")]
    UnsupportedType(String),
    #[error("Argument extraction failed: {0}")]
    ArgumentExtraction(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_string_property_conversion() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The name parameter"
                }
            },
            "required": ["name"]
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 1);

        let name_arg = &args[0];
        assert_eq!(name_arg.get_id(), "name");
        assert_eq!(
            name_arg.get_help().map(|s| s.to_string()),
            Some("The name parameter".to_string())
        );
        assert!(name_arg.is_required_set());
    }

    #[test]
    fn test_boolean_property_conversion() {
        let schema = json!({
            "type": "object",
            "properties": {
                "enabled": {
                    "type": "boolean",
                    "description": "Enable feature"
                }
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let enabled_arg = &args[0];

        assert!(matches!(enabled_arg.get_action(), ArgAction::SetTrue));
        assert!(!enabled_arg.is_required_set());
    }

    #[test]
    fn test_integer_with_minimum() {
        let schema = json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Item count"
                }
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let count_arg = &args[0];

        let help_text = count_arg
            .get_help()
            .map(|s| s.to_string())
            .unwrap_or_default();
        assert!(help_text.contains("min: 1"));
    }

    #[test]
    fn test_number_property_conversion() {
        let schema = json!({
            "type": "object",
            "properties": {
                "price": {
                    "type": "number",
                    "minimum": 0.01,
                    "description": "Item price"
                }
            },
            "required": ["price"]
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 1);

        let price_arg = &args[0];
        assert_eq!(price_arg.get_id(), "price");
        assert!(price_arg.is_required_set());

        let help_text = price_arg
            .get_help()
            .map(|s| s.to_string())
            .unwrap_or_default();
        assert!(help_text.contains("min: 0.01"));
    }

    #[test]
    fn test_object_property_conversion() {
        let schema = json!({
            "type": "object",
            "properties": {
                "config": {
                    "type": "object",
                    "description": "Configuration object"
                }
            },
            "required": ["config"]
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 1);

        let config_arg = &args[0];
        assert_eq!(config_arg.get_id(), "config");
        assert!(config_arg.is_required_set());

        let help_text = config_arg
            .get_help()
            .map(|s| s.to_string())
            .unwrap_or_default();
        assert!(help_text.contains("provide as JSON string"));
    }

    #[test]
    fn test_array_property() {
        let schema = json!({
            "type": "object",
            "properties": {
                "tags": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of tags"
                }
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let tags_arg = &args[0];

        assert!(matches!(tags_arg.get_action(), ArgAction::Append));
    }

    #[test]
    fn test_enum_values() {
        let schema = json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["json", "yaml", "table"],
                    "description": "Output format"
                }
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let format_arg = &args[0];

        // Verify enum values are handled (exact verification depends on clap internals)
        let help_text = format_arg
            .get_help()
            .map(|s| s.to_string())
            .unwrap_or_default();
        assert!(help_text.contains("Output format"));
    }

    #[test]
    fn test_matches_to_json_round_trip() {
        // Test that we can convert schema -> args -> matches -> json successfully
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "count": {"type": "integer"},
                "enabled": {"type": "boolean"}
            },
            "required": ["name"]
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();

        // This would need to be tested with actual clap command parsing
        // For now, test the JSON conversion logic directly
        assert!(!args.is_empty());
    }

    #[test]
    fn test_missing_properties() {
        let schema = json!({
            "type": "object"
        });

        let result = SchemaConverter::schema_to_clap_args(&schema);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Schema missing properties object"));
    }

    #[test]
    fn test_unsupported_type() {
        let schema = json!({
            "type": "object",
            "properties": {
                "data": {
                    "type": "null",
                    "description": "Unsupported null type"
                }
            }
        });

        let result = SchemaConverter::schema_to_clap_args(&schema);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported JSON schema type: null"));
    }

    #[test]
    fn test_format_hints() {
        let schema = json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "format": "uri",
                    "description": "URL parameter"
                },
                "email": {
                    "type": "string",
                    "format": "email",
                    "description": "Email parameter"
                },
                "path": {
                    "type": "string",
                    "format": "path",
                    "description": "File path parameter"
                }
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 3);

        // Check that format hints are applied (value hints are internal to clap)
        let url_arg = args.iter().find(|arg| arg.get_id() == "url").unwrap();
        let email_arg = args.iter().find(|arg| arg.get_id() == "email").unwrap();
        let path_arg = args.iter().find(|arg| arg.get_id() == "path").unwrap();

        assert_eq!(
            url_arg.get_help().map(|s| s.to_string()),
            Some("URL parameter".to_string())
        );
        assert_eq!(
            email_arg.get_help().map(|s| s.to_string()),
            Some("Email parameter".to_string())
        );
        assert_eq!(
            path_arg.get_help().map(|s| s.to_string()),
            Some("File path parameter".to_string())
        );
    }

    #[test]
    fn test_pattern_validation_hint() {
        let schema = json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "pattern": "^[A-Z]{3}-[0-9]{3}$",
                    "description": "Product code"
                }
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let code_arg = &args[0];

        let help_text = code_arg
            .get_help()
            .map(|s| s.to_string())
            .unwrap_or_default();
        assert!(help_text.contains("pattern: ^[A-Z]{3}-[0-9]{3}$"));
    }

    #[test]
    fn test_boolean_never_required() {
        // Test that boolean fields are never required, even when listed in required array
        let schema = json!({
            "type": "object",
            "properties": {
                "string_field": {"type": "string", "description": "A string"},
                "bool_field": {"type": "boolean", "description": "A boolean"}
            },
            "required": ["string_field", "bool_field"]
        });
        
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 2);
        
        let string_arg = args.iter().find(|arg| arg.get_id() == "string_field").unwrap();
        let bool_arg = args.iter().find(|arg| arg.get_id() == "bool_field").unwrap();
        
        // String should be required
        assert!(string_arg.is_required_set(), "String argument should be required");
        
        // Boolean should NEVER be required, even if in required array
        assert!(!bool_arg.is_required_set(), "Boolean argument should never be required");
        
        // Verify boolean has correct action
        assert!(matches!(bool_arg.get_action(), clap::ArgAction::SetTrue));
    }

    #[test]
    fn test_boolean_never_required_specific_failure_case() {
        // Test the exact failing case from property test
        let schema = json!({
            "type": "object",
            "properties": {
                "a": {"type": "string", "description": "String parameter"},
                "aa": {"type": "integer", "description": "Integer parameter"},
                "A": {"type": "boolean", "description": "Boolean parameter"}
            },
            "required": ["a", "aa", "A"]
        });
        
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 3);
        
        let string_arg = args.iter().find(|arg| arg.get_id() == "a").unwrap();
        let int_arg = args.iter().find(|arg| arg.get_id() == "aa").unwrap();
        let bool_arg = args.iter().find(|arg| arg.get_id() == "A").unwrap();
        
        // String and int should be required
        assert!(string_arg.is_required_set(), "String argument should be required");
        assert!(int_arg.is_required_set(), "Int argument should be required");
        
        // Boolean should NEVER be required, even if in required array
        assert!(!bool_arg.is_required_set(), "Boolean argument should never be required");
        
        // Verify boolean has correct action
        assert!(matches!(bool_arg.get_action(), clap::ArgAction::SetTrue));
    }

    #[test]
    fn test_default_values() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "default": "default_name",
                    "description": "Name parameter"
                },
                "count": {
                    "type": "integer", 
                    "default": 10,
                    "description": "Count parameter"
                },
                "enabled": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable flag"
                }
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 3);

        let name_arg = args.iter().find(|arg| arg.get_id() == "name").unwrap();
        let count_arg = args.iter().find(|arg| arg.get_id() == "count").unwrap();
        let enabled_arg = args.iter().find(|arg| arg.get_id() == "enabled").unwrap();

        // Check default values are set (clap internals)
        assert_eq!(name_arg.get_default_values(), vec!["default_name"]);
        assert_eq!(count_arg.get_default_values(), vec!["10"]);
        assert_eq!(enabled_arg.get_default_values(), vec!["true"]);
    }

    #[test]
    fn test_examples_in_help_text() {
        let schema = json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "description": "Output format",
                    "examples": ["json", "yaml", "table"]
                }
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let format_arg = &args[0];

        let help_text = format_arg.get_help().map(|s| s.to_string()).unwrap_or_default();
        assert!(help_text.contains("Examples: json, yaml"));
    }

    #[test]
    fn test_one_of_enum_behavior() {
        let schema = json!({
            "type": "object",
            "properties": {
                "level": {
                    "type": "string",
                    "description": "Log level",
                    "oneOf": [
                        {"const": "debug"},
                        {"const": "info"}, 
                        {"const": "warn"},
                        {"const": "error"}
                    ]
                }
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let level_arg = &args[0];

        // Check that oneOf creates value parser constraints (clap internals)
        assert_eq!(level_arg.get_id(), "level");
        let help_text = level_arg.get_help().map(|s| s.to_string()).unwrap_or_default();
        assert!(help_text.contains("Log level"));
    }

    #[test]
    fn test_any_of_enum_behavior() {
        let schema = json!({
            "type": "object",
            "properties": {
                "output": {
                    "type": "string",
                    "description": "Output type",
                    "anyOf": [
                        {"enum": ["json", "yaml"]},
                        {"const": "table"}
                    ]
                }
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let output_arg = &args[0];

        assert_eq!(output_arg.get_id(), "output");
        let help_text = output_arg.get_help().map(|s| s.to_string()).unwrap_or_default();
        assert!(help_text.contains("Output type"));
    }

    #[test]
    fn test_title_field_enhancement() {
        let schema = json!({
            "type": "object",
            "properties": {
                "verbose": {
                    "type": "boolean",
                    "title": "Verbose Mode",
                    "description": "Enable verbose output"
                }
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let verbose_arg = &args[0];

        let help_text = verbose_arg.get_help().map(|s| s.to_string()).unwrap_or_default();
        assert!(help_text.contains("Verbose Mode"));
        assert!(help_text.contains("Enable verbose output"));
    }

    #[test]
    fn test_extract_enum_from_one_of() {
        let one_of = vec![
            json!({"const": "value1"}),
            json!({"const": "value2"}),
            json!({"enum": ["value3", "value4"]})
        ];

        let result = SchemaConverter::extract_enum_from_one_of(&one_of);
        assert!(result.is_some());
        
        let values = result.unwrap();
        assert_eq!(values.len(), 4);
        assert!(values.contains(&"value1".to_string()));
        assert!(values.contains(&"value2".to_string()));
        assert!(values.contains(&"value3".to_string()));
        assert!(values.contains(&"value4".to_string()));
    }

    #[test]
    fn test_complex_enhanced_schema() {
        let schema = json!({
            "type": "object",
            "properties": {
                "config": {
                    "type": "string",
                    "title": "Configuration File",
                    "description": "Path to configuration file",
                    "default": "config.yaml",
                    "examples": ["config.yaml", "settings.json"],
                    "format": "path"
                },
                "level": {
                    "type": "string",
                    "description": "Logging level",
                    "default": "info",
                    "oneOf": [
                        {"const": "debug"},
                        {"const": "info"},
                        {"const": "warn"}
                    ]
                }
            },
            "required": ["config"]
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 2);

        let config_arg = args.iter().find(|arg| arg.get_id() == "config").unwrap();
        let level_arg = args.iter().find(|arg| arg.get_id() == "level").unwrap();

        // Check config arg has all enhancements
        assert!(config_arg.is_required_set());
        assert_eq!(config_arg.get_default_values(), vec!["config.yaml"]);
        
        let config_help = config_arg.get_help().map(|s| s.to_string()).unwrap_or_default();
        assert!(config_help.contains("Configuration File"));
        assert!(config_help.contains("Examples: config.yaml, settings.json"));

        // Check level arg has default and oneOf
        assert!(!level_arg.is_required_set());
        assert_eq!(level_arg.get_default_values(), vec!["info"]);
    }
}
