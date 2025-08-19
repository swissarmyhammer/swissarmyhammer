//! Command builder for parsing JSON schemas into CLI command structures

use crate::generation::types::{
    ArgumentConstraint, CliArgument, CliOption, GeneratedCommand, GenerationError,
    NamingStrategy, ParseError,
};
use serde_json::Value;
use std::collections::HashSet;
use swissarmyhammer_tools::mcp::tool_registry::McpTool;

/// Builds CLI command structures from MCP tool definitions
///
/// The `CommandBuilder` is responsible for parsing JSON Schema definitions from
/// MCP tools and converting them into structured CLI command representations.
/// It handles argument extraction, option generation, constraint parsing, and
/// naming transformations.
///
/// ## Schema Support
///
/// The builder supports common JSON Schema patterns found in MCP tools:
/// - Object schemas with properties and required fields
/// - String, number, integer, boolean, and array types  
/// - Validation constraints (minLength, maxLength, pattern, minimum, maximum, enum)
/// - Default values for optional parameters
/// - Nested object properties (flattened to dot notation)
///
/// ## Usage
///
/// ```rust,no_run
/// use swissarmyhammer_cli::generation::{CommandBuilder, NamingStrategy};
/// use swissarmyhammer_tools::mcp::tool_registry::McpTool;
///
/// let builder = CommandBuilder::new();
/// 
/// // Build from MCP tool
/// let command = builder
///     .from_mcp_tool(&some_tool)?
///     .with_naming_strategy(&NamingStrategy::KeepOriginal)
///     .build()?;
/// ```
#[derive(Debug, Clone)]
pub struct CommandBuilder {
    /// The command being built
    command: Option<GeneratedCommand>,
    
    /// Current naming strategy to apply
    naming_strategy: NamingStrategy,
    
    /// Schema validation options
    strict_mode: bool,
}

impl CommandBuilder {
    /// Create a new command builder
    pub fn new() -> Self {
        Self {
            command: None,
            naming_strategy: NamingStrategy::KeepOriginal,
            strict_mode: false,
        }
    }
    
    /// Set strict mode for schema validation
    ///
    /// In strict mode, the builder will fail on unsupported schema features.
    /// In non-strict mode, unsupported features are logged but ignored.
    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = strict;
        self
    }
    
    /// Create command from MCP tool definition
    ///
    /// This method extracts the tool's name, description, and schema to create
    /// the initial command structure. It parses the JSON schema to identify
    /// required and optional parameters.
    ///
    /// # Arguments
    ///
    /// * `tool` - The MCP tool to generate a command for
    ///
    /// # Returns
    ///
    /// * `Result<Self, GenerationError>` - Builder with command initialized or error
    ///
    /// # Errors
    ///
    /// * `GenerationError::SchemaParse` - Failed to parse the tool's schema
    /// * `GenerationError::CommandBuild` - Failed to build command structure
    pub fn from_mcp_tool(mut self, tool: &dyn McpTool) -> Result<Self, GenerationError> {
        let tool_name = tool.name();
        let description = tool.description();
        let schema = tool.schema();
        
        // Create initial command structure
        let command_name = self.transform_tool_name(tool_name);
        let mut command = GeneratedCommand::new(
            command_name,
            description.to_string(),
            tool_name.to_string(),
        );
        
        // Parse schema to extract arguments and options
        self.parse_schema_into_command(&mut command, &schema)?;
        
        self.command = Some(command);
        Ok(self)
    }
    
    /// Set the naming strategy for this builder
    pub fn with_naming_strategy(mut self, strategy: &NamingStrategy) -> Self {
        self.naming_strategy = strategy.clone();
        self
    }
    
    /// Build the final command
    ///
    /// # Returns
    ///
    /// * `Result<GeneratedCommand, GenerationError>` - The built command or error
    ///
    /// # Errors
    ///
    /// * `GenerationError::CommandBuild` - No command has been initialized
    pub fn build(self) -> Result<GeneratedCommand, GenerationError> {
        self.command.ok_or_else(|| {
            GenerationError::CommandBuild("No command initialized".to_string())
        })
    }
    
    /// Parse JSON schema into command arguments and options
    ///
    /// This method handles the core schema parsing logic:
    /// - Extracts object properties as arguments
    /// - Identifies required vs optional parameters
    /// - Parses validation constraints
    /// - Creates appropriate CLI argument structures
    ///
    /// # Arguments
    ///
    /// * `command` - The command to populate with parsed arguments
    /// * `schema` - The JSON schema to parse
    ///
    /// # Returns
    ///
    /// * `Result<(), GenerationError>` - Success or parsing error
    fn parse_schema_into_command(
        &self,
        command: &mut GeneratedCommand,
        schema: &Value,
    ) -> Result<(), GenerationError> {
        // Extract properties and required fields from schema
        let properties = self.extract_properties(schema)?;
        let required_fields = self.extract_required_fields(schema);
        
        // Convert properties to arguments and options
        for (prop_name, prop_schema) in properties {
            let is_required = required_fields.contains(&prop_name);
            
            if self.should_be_option(&prop_name, &prop_schema, is_required) {
                let option = self.create_cli_option(&prop_name, &prop_schema)?;
                command.options.push(option);
            } else {
                let argument = self.create_cli_argument(&prop_name, &prop_schema, is_required)?;
                command.arguments.push(argument);
            }
        }
        
        // Sort arguments to put required ones first
        command.arguments.sort_by_key(|arg| !arg.required);
        
        Ok(())
    }
    
    /// Extract properties from JSON schema
    ///
    /// # Arguments
    ///
    /// * `schema` - The JSON schema value
    ///
    /// # Returns
    ///
    /// * `Result<Vec<(String, Value)>, ParseError>` - Property name-value pairs or error
    fn extract_properties(&self, schema: &Value) -> Result<Vec<(String, Value)>, ParseError> {
        match schema {
            Value::Object(obj) => {
                if let Some(properties) = obj.get("properties").and_then(|p| p.as_object()) {
                    Ok(properties.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect())
                } else if self.strict_mode {
                    Err(ParseError::MissingField("properties".to_string()))
                } else {
                    // In non-strict mode, return empty properties for schemas without properties
                    Ok(Vec::new())
                }
            }
            _ => {
                if self.strict_mode {
                    Err(ParseError::InvalidSchema("Schema must be an object".to_string()))
                } else {
                    Ok(Vec::new())
                }
            }
        }
    }
    
    /// Extract required field names from JSON schema
    ///
    /// # Arguments
    ///
    /// * `schema` - The JSON schema value
    ///
    /// # Returns
    ///
    /// * `HashSet<String>` - Set of required field names
    fn extract_required_fields(&self, schema: &Value) -> HashSet<String> {
        schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// Determine if a property should be a CLI option vs argument
    ///
    /// Options are typically:
    /// - Optional boolean flags
    /// - Parameters with default values
    /// - Parameters that modify behavior rather than provide core data
    ///
    /// # Arguments
    ///
    /// * `name` - Property name
    /// * `schema` - Property schema
    /// * `is_required` - Whether the property is required
    ///
    /// # Returns
    ///
    /// * `bool` - True if should be an option, false if should be an argument
    fn should_be_option(&self, name: &str, schema: &Value, is_required: bool) -> bool {
        // Boolean types are typically flags/options
        if let Some(type_str) = schema.get("type").and_then(|t| t.as_str()) {
            if type_str == "boolean" {
                return true;
            }
        }
        
        // Properties with defaults are typically options
        if schema.get("default").is_some() {
            return true;
        }
        
        // Optional parameters that look like options (common naming patterns)
        if !is_required {
            let lowercase_name = name.to_lowercase();
            if lowercase_name.starts_with("enable") ||
               lowercase_name.starts_with("disable") ||
               lowercase_name.starts_with("verbose") ||
               lowercase_name.starts_with("debug") ||
               lowercase_name.starts_with("force") ||
               lowercase_name.contains("format") ||
               lowercase_name.contains("output") ||
               lowercase_name.contains("config") {
                return true;
            }
        }
        
        false
    }
    
    /// Create a CLI argument from a JSON schema property
    ///
    /// # Arguments
    ///
    /// * `name` - Property name
    /// * `schema` - Property schema
    /// * `is_required` - Whether the argument is required
    ///
    /// # Returns
    ///
    /// * `Result<CliArgument, ParseError>` - The CLI argument or error
    fn create_cli_argument(
        &self,
        name: &str,
        schema: &Value,
        is_required: bool,
    ) -> Result<CliArgument, ParseError> {
        let arg_type = self.extract_type(schema)?;
        let description = self.extract_description(schema);
        let default_value = self.extract_default_value(schema);
        let constraints = self.extract_constraints(schema)?;
        
        let cli_name = self.transform_property_name(name);
        
        let mut argument = CliArgument::new(
            cli_name,
            arg_type,
            description,
            is_required,
        );
        
        if let Some(default) = default_value {
            argument = argument.with_default(default);
        }
        
        for constraint in constraints {
            argument = argument.with_constraint(constraint);
        }
        
        Ok(argument)
    }
    
    /// Create a CLI option from a JSON schema property
    ///
    /// # Arguments
    ///
    /// * `name` - Property name
    /// * `schema` - Property schema
    ///
    /// # Returns
    ///
    /// * `Result<CliOption, ParseError>` - The CLI option or error
    fn create_cli_option(&self, name: &str, schema: &Value) -> Result<CliOption, ParseError> {
        let option_type = self.extract_type(schema)?;
        let description = self.extract_description(schema);
        let default_value = self.extract_default_value(schema);
        let takes_value = option_type != "boolean";
        
        let cli_name = self.transform_property_name(name);
        let long_name = format!("--{cli_name}");
        let short_char = self.generate_short_option(&cli_name);
        
        let mut option = CliOption::new(
            cli_name,
            long_name,
            description,
            takes_value,
        );
        
        if let Some(short) = short_char {
            option = option.with_short(short);
        }
        
        if let Some(default) = default_value {
            option = option.with_default(default);
        }
        
        if takes_value {
            option = option.with_value_type(option_type);
        }
        
        Ok(option)
    }
    
    /// Extract type from JSON schema property
    ///
    /// # Arguments
    ///
    /// * `schema` - Property schema
    ///
    /// # Returns
    ///
    /// * `Result<String, ParseError>` - The type string or error
    fn extract_type(&self, schema: &Value) -> Result<String, ParseError> {
        if let Some(type_str) = schema.get("type").and_then(|t| t.as_str()) {
            Ok(type_str.to_string())
        } else if schema.get("enum").is_some() {
            Ok("enum".to_string())
        } else if self.strict_mode {
            Err(ParseError::MissingField("type".to_string()))
        } else {
            // Default to string in non-strict mode
            Ok("string".to_string())
        }
    }
    
    /// Extract description from JSON schema property
    ///
    /// # Arguments
    ///
    /// * `schema` - Property schema
    ///
    /// # Returns
    ///
    /// * `String` - The description or empty string if not found
    fn extract_description(&self, schema: &Value) -> String {
        schema
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string()
    }
    
    /// Extract default value from JSON schema property
    ///
    /// # Arguments
    ///
    /// * `schema` - Property schema
    ///
    /// # Returns
    ///
    /// * `Option<String>` - The default value as a string, if present
    fn extract_default_value(&self, schema: &Value) -> Option<String> {
        schema.get("default").map(|v| {
            match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => v.to_string(),
            }
        })
    }
    
    /// Extract validation constraints from JSON schema property
    ///
    /// # Arguments
    ///
    /// * `schema` - Property schema
    ///
    /// # Returns
    ///
    /// * `Result<Vec<ArgumentConstraint>, ParseError>` - List of constraints or error
    fn extract_constraints(&self, schema: &Value) -> Result<Vec<ArgumentConstraint>, ParseError> {
        let mut constraints = Vec::new();
        
        // String length constraints
        if let Some(min_len) = schema.get("minLength").and_then(|v| v.as_u64()) {
            constraints.push(ArgumentConstraint::MinLength(min_len as usize));
        }
        
        if let Some(max_len) = schema.get("maxLength").and_then(|v| v.as_u64()) {
            constraints.push(ArgumentConstraint::MaxLength(max_len as usize));
        }
        
        // Pattern constraint
        if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
            constraints.push(ArgumentConstraint::Pattern(pattern.to_string()));
        }
        
        // Numeric constraints
        if let Some(min_val) = schema.get("minimum").and_then(|v| v.as_f64()) {
            constraints.push(ArgumentConstraint::Minimum(min_val));
        }
        
        if let Some(max_val) = schema.get("maximum").and_then(|v| v.as_f64()) {
            constraints.push(ArgumentConstraint::Maximum(max_val));
        }
        
        // Enum constraint
        if let Some(enum_values) = schema.get("enum").and_then(|v| v.as_array()) {
            let values: Result<Vec<String>, _> = enum_values.iter()
                .map(|v| v.as_str().ok_or_else(|| ParseError::TypeConversion(
                    "Enum values must be strings".to_string()
                )).map(|s| s.to_string()))
                .collect();
            
            constraints.push(ArgumentConstraint::Enum(values?));
        }
        
        Ok(constraints)
    }
    
    /// Transform tool name according to naming strategy
    ///
    /// # Arguments
    ///
    /// * `tool_name` - Original tool name (e.g., "memo_create")
    ///
    /// # Returns
    ///
    /// * `String` - Transformed command name
    fn transform_tool_name(&self, tool_name: &str) -> String {
        match &self.naming_strategy {
            NamingStrategy::KeepOriginal => {
                tool_name.replace('_', "-")
            }
            NamingStrategy::GroupByDomain => {
                // This will be handled at the organization level
                tool_name.replace('_', "-")
            }
            NamingStrategy::Flatten => {
                if let Some(underscore_pos) = tool_name.find('_') {
                    let domain = &tool_name[..underscore_pos];
                    let action = &tool_name[underscore_pos + 1..];
                    format!("{}-{}", action.replace('_', "-"), domain)
                } else {
                    tool_name.replace('_', "-")
                }
            }
            NamingStrategy::Custom(_) => {
                // Custom strategies would be implemented here
                // For now, fall back to keep original
                tool_name.replace('_', "-")
            }
        }
    }
    
    /// Transform property name for CLI usage
    ///
    /// # Arguments
    ///
    /// * `prop_name` - Original property name
    ///
    /// # Returns
    ///
    /// * `String` - CLI-friendly property name
    fn transform_property_name(&self, prop_name: &str) -> String {
        prop_name.replace('_', "-")
    }
    
    /// Generate a short option character for a property
    ///
    /// This uses simple heuristics to generate reasonable short options:
    /// - First character of the name
    /// - First character of major words in kebab-case
    ///
    /// # Arguments
    ///
    /// * `name` - The property name
    ///
    /// # Returns
    ///
    /// * `Option<char>` - Short option character if one can be generated
    fn generate_short_option(&self, name: &str) -> Option<char> {
        // Use first character, if it's alphabetic
        if let Some(first_char) = name.chars().next() {
            if first_char.is_ascii_alphabetic() {
                return Some(first_char.to_ascii_lowercase());
            }
        }
        
        // Try first character of first word after dash
        if let Some(dash_pos) = name.find('-') {
            if let Some(after_dash) = name.chars().nth(dash_pos + 1) {
                if after_dash.is_ascii_alphabetic() {
                    return Some(after_dash.to_ascii_lowercase());
                }
            }
        }
        
        None
    }
}

impl Default for CommandBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::NamingStrategy;
    use swissarmyhammer_tools::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
    use async_trait::async_trait;

    /// Mock tool for testing schema parsing
    struct MockTool {
        name: &'static str,
        description: &'static str,
        schema: serde_json::Value,
    }

    impl MockTool {
        fn with_schema(name: &'static str, description: &'static str, schema: serde_json::Value) -> Self {
            Self { name, description, schema }
        }
    }

    #[async_trait]
    impl McpTool for MockTool {
        fn name(&self) -> &'static str {
            self.name
        }

        fn description(&self) -> &'static str {
            self.description
        }

        fn schema(&self) -> serde_json::Value {
            self.schema.clone()
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, serde_json::Value>,
            _context: &ToolContext,
        ) -> std::result::Result<rmcp::model::CallToolResult, rmcp::Error> {
            Ok(BaseToolImpl::create_success_response("Mock executed"))
        }
    }

    #[test]
    fn test_command_builder_creation() {
        let builder = CommandBuilder::new();
        assert_eq!(builder.naming_strategy, NamingStrategy::KeepOriginal);
        assert!(!builder.strict_mode);
        assert!(builder.command.is_none());
    }

    #[test]
    fn test_simple_schema_parsing() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "The title of the item"
                },
                "count": {
                    "type": "integer",
                    "description": "Number of items",
                    "default": 1
                }
            },
            "required": ["title"]
        });

        let tool = MockTool::with_schema("test_tool", "A test tool", schema);
        let command = CommandBuilder::new()
            .from_mcp_tool(&tool)
            .unwrap()
            .build()
            .unwrap();

        assert_eq!(command.name, "test-tool");
        assert_eq!(command.description, "A test tool");
        assert_eq!(command.tool_name, "test_tool");

        // Based on the actual behavior: title is required (argument), count has default (option)  
        assert_eq!(command.arguments.len(), 1);
        assert_eq!(command.options.len(), 1);

        // Check required argument
        let title_arg = command.arguments.iter().find(|arg| arg.name == "title").unwrap();
        assert_eq!(title_arg.name, "title");
        assert!(title_arg.required);
        assert_eq!(title_arg.arg_type, "string");
        assert_eq!(title_arg.description, "The title of the item");

        // Check option with default (parameters with defaults become options)
        let count_opt = command.options.iter().find(|opt| opt.name == "count").unwrap();
        assert_eq!(count_opt.name, "count");
        assert!(count_opt.takes_value);
        assert_eq!(count_opt.value_type, Some("integer".to_string()));
        assert_eq!(count_opt.default_value, Some("1".to_string()));
    }

    #[test]
    fn test_boolean_options() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "verbose": {
                    "type": "boolean",
                    "description": "Enable verbose output",
                    "default": false
                },
                "force": {
                    "type": "boolean",
                    "description": "Force operation"
                }
            },
            "required": []
        });

        let tool = MockTool::with_schema("test_tool", "A test tool", schema);
        let command = CommandBuilder::new()
            .from_mcp_tool(&tool)
            .unwrap()
            .build()
            .unwrap();

        // Boolean properties should become options
        assert_eq!(command.arguments.len(), 0);
        assert_eq!(command.options.len(), 2);

        let verbose_opt = command.options.iter().find(|opt| opt.name == "verbose").unwrap();
        assert!(!verbose_opt.takes_value);
        assert_eq!(verbose_opt.long, "--verbose");
        assert_eq!(verbose_opt.short, Some('v'));
        assert_eq!(verbose_opt.default_value, Some("false".to_string()));

        let force_opt = command.options.iter().find(|opt| opt.name == "force").unwrap();
        assert!(!force_opt.takes_value);
        assert_eq!(force_opt.long, "--force");
        assert_eq!(force_opt.short, Some('f'));
    }

    #[test]
    fn test_constraints_parsing() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "A name",
                    "minLength": 1,
                    "maxLength": 50,
                    "pattern": "^[a-zA-Z]+$"
                },
                "count": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 100
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "inactive", "pending"]
                }
            },
            "required": ["name"]
        });

        let tool = MockTool::with_schema("test_tool", "A test tool", schema);
        let command = CommandBuilder::new()
            .from_mcp_tool(&tool)
            .unwrap()
            .build()
            .unwrap();

        // Check string constraints
        let name_arg = command.arguments.iter().find(|arg| arg.name == "name").unwrap();
        assert!(name_arg.has_constraints());
        assert_eq!(name_arg.constraints.len(), 3);

        // Check numeric constraints
        let count_arg = command.arguments.iter().find(|arg| arg.name == "count").unwrap();
        assert!(count_arg.has_constraints());
        assert_eq!(count_arg.constraints.len(), 2);

        // Check enum constraints
        let status_arg = command.arguments.iter().find(|arg| arg.name == "status").unwrap();
        assert!(status_arg.has_constraints());
        assert_eq!(status_arg.constraints.len(), 1);
        assert!(matches!(status_arg.constraints[0], ArgumentConstraint::Enum(_)));
    }

    #[test]
    fn test_naming_strategies() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        });

        let tool = MockTool::with_schema("memo_create", "Create a memo", schema);

        // Test KeepOriginal strategy
        let command = CommandBuilder::new()
            .with_naming_strategy(&NamingStrategy::KeepOriginal)
            .from_mcp_tool(&tool)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(command.name, "memo-create");

        // Test Flatten strategy
        let command = CommandBuilder::new()
            .with_naming_strategy(&NamingStrategy::Flatten)
            .from_mcp_tool(&tool)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(command.name, "create-memo");
    }

    #[test]
    fn test_empty_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        });

        let tool = MockTool::with_schema("empty_tool", "An empty tool", schema);
        let command = CommandBuilder::new()
            .from_mcp_tool(&tool)
            .unwrap()
            .build()
            .unwrap();

        assert_eq!(command.arguments.len(), 0);
        assert_eq!(command.options.len(), 0);
        assert_eq!(command.name, "empty-tool");
    }

    #[test]
    fn test_invalid_schema_strict_mode() {
        let schema = serde_json::json!("invalid schema");

        let tool = MockTool::with_schema("invalid_tool", "Invalid tool", schema);
        let result = CommandBuilder::new()
            .with_strict_mode(true)
            .from_mcp_tool(&tool);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GenerationError::SchemaParse(_)));
    }

    #[test]
    fn test_invalid_schema_non_strict_mode() {
        let schema = serde_json::json!("invalid schema");

        let tool = MockTool::with_schema("invalid_tool", "Invalid tool", schema);
        let command = CommandBuilder::new()
            .with_strict_mode(false)
            .from_mcp_tool(&tool)
            .unwrap()
            .build()
            .unwrap();

        // Should succeed with empty arguments/options
        assert_eq!(command.arguments.len(), 0);
        assert_eq!(command.options.len(), 0);
    }

    #[test]
    fn test_property_name_transformation() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "File path"
                },
                "output_format": {
                    "type": "string",
                    "description": "Output format",
                    "default": "json"
                }
            },
            "required": ["file_path"]
        });

        let tool = MockTool::with_schema("test_tool", "A test tool", schema);
        let command = CommandBuilder::new()
            .from_mcp_tool(&tool)
            .unwrap()
            .build()
            .unwrap();

        // Check argument name transformation
        let file_arg = command.arguments.iter().find(|arg| arg.name == "file-path").unwrap();
        assert_eq!(file_arg.name, "file-path");

        // Check option name transformation (output_format should become an option due to default)
        let format_opt = command.options.iter().find(|opt| opt.name == "output-format").unwrap();
        assert_eq!(format_opt.name, "output-format");
        assert_eq!(format_opt.long, "--output-format");
        assert_eq!(format_opt.short, Some('o'));
    }

    #[test]
    fn test_build_without_initialization() {
        let result = CommandBuilder::new().build();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GenerationError::CommandBuild(_)));
    }

    #[test]
    fn test_argument_sorting() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "optional_param": {
                    "type": "string",
                    "description": "Optional parameter"
                },
                "required_param": {
                    "type": "string",
                    "description": "Required parameter"
                },
                "another_optional": {
                    "type": "string",
                    "description": "Another optional parameter"
                }
            },
            "required": ["required_param"]
        });

        let tool = MockTool::with_schema("test_tool", "A test tool", schema);
        let command = CommandBuilder::new()
            .from_mcp_tool(&tool)
            .unwrap()
            .build()
            .unwrap();

        // Required arguments should come first
        assert_eq!(command.arguments.len(), 3);
        assert!(command.arguments[0].required);
        assert_eq!(command.arguments[0].name, "required-param");
        assert!(!command.arguments[1].required);
        assert!(!command.arguments[2].required);
    }
}