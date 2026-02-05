//! Generic MCP tool schema generation from operation metadata
//!
//! This module provides reusable schema generation for any MCP tool built on
//! the Operation trait. The schema is derived from operation metadata and
//! stays automatically synchronized with operation definitions.

use serde_json::{json, Map, Value};
use std::collections::HashMap;

use crate::{Operation, ParamType};

/// Configuration for schema generation
pub struct SchemaConfig {
    /// Tool description
    pub description: String,
    /// Usage examples
    pub examples: Vec<Value>,
    /// Verb aliases (e.g., "add" -> ["create", "new"])
    pub verb_aliases: Map<String, Value>,
    /// Additional extension fields to include in schema
    pub extensions: Map<String, Value>,
}

impl SchemaConfig {
    /// Create minimal schema config
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            examples: Vec::new(),
            verb_aliases: Map::new(),
            extensions: Map::new(),
        }
    }

    /// Add examples
    pub fn with_examples(mut self, examples: Vec<Value>) -> Self {
        self.examples = examples;
        self
    }

    /// Add verb aliases
    pub fn with_verb_aliases(mut self, aliases: Map<String, Value>) -> Self {
        self.verb_aliases = aliases;
        self
    }

    /// Add custom extension field
    pub fn with_extension(mut self, key: String, value: Value) -> Self {
        self.extensions.insert(key, value);
        self
    }
}

/// Generate complete MCP tool schema from operation metadata
///
/// Creates a Claude API-compatible JSON Schema (no top-level oneOf) that
/// documents all operations and their parameters.
///
/// # Arguments
///
/// * `operations` - Slice of operation trait objects
/// * `config` - Schema configuration (description, examples, aliases)
///
/// # Returns
///
/// JSON Schema with:
/// - Flat properties containing all parameters
/// - Op field with enum of all operations
/// - x-operation-schemas for operation-specific documentation
/// - x-operation-groups categorizing operations by noun
/// - Custom extension fields from config
///
/// # Example
///
/// ```ignore
/// let config = SchemaConfig::new("My tool operations")
///     .with_examples(vec![...])
///     .with_verb_aliases(aliases);
///
/// let schema = generate_mcp_schema(&MY_OPERATIONS, config);
/// ```
pub fn generate_mcp_schema(operations: &[&dyn Operation], config: SchemaConfig) -> Value {
    // Collect all unique parameters across all operations
    let all_properties = collect_all_parameters(operations);

    // Generate operation-specific schemas for documentation (extension field)
    let operation_schemas: Vec<Value> = operations
        .iter()
        .map(|op| operation_to_schema(*op))
        .collect();

    // Build operation groups
    let operation_groups = group_operations_by_noun(operations);

    // Build base schema
    let mut schema = json!({
        "type": "object",
        "additionalProperties": true,
        "description": config.description,
        "properties": all_properties,
        "x-operation-schemas": operation_schemas,
        "x-operation-groups": operation_groups,
    });

    // Add examples if provided
    if !config.examples.is_empty() {
        schema["examples"] = json!(config.examples);
    }

    // Add forgiving input documentation if verb aliases provided
    if !config.verb_aliases.is_empty() {
        schema["x-forgiving-input"] = json!({
            "description": "The tool accepts multiple input formats for maximum flexibility",
            "formats": [
                "Explicit op: { \"op\": \"add task\", \"title\": \"Fix bug\" }",
                "Verb+noun fields: { \"verb\": \"add\", \"noun\": \"task\", \"title\": \"Fix bug\" }",
                "Shorthand: { \"add\": \"task\", \"title\": \"Fix bug\" }",
                "Inferred: { \"title\": \"Fix bug\" } (infers operation from parameters)"
            ],
            "verb_aliases": config.verb_aliases,
        });
    }

    // Add custom extensions
    for (key, value) in config.extensions {
        schema[key] = value;
    }

    schema
}

/// Collect all unique parameters across all operations
///
/// Returns a properties object with all parameters that appear in any operation.
/// The op field is included with enum of all operations.
fn collect_all_parameters(operations: &[&dyn Operation]) -> Map<String, Value> {
    let mut properties = Map::new();
    let mut seen_params: HashMap<String, (ParamType, String)> = HashMap::new();

    // Collect all unique parameters
    for op in operations {
        for param in op.parameters() {
            let key = param.name.to_string();
            // Keep the first description we see for each parameter name
            seen_params
                .entry(key)
                .or_insert((param.param_type, param.description.to_string()));
        }
    }

    // Add op field first with enum of all operations
    properties.insert(
        "op".to_string(),
        json!({
            "type": "string",
            "description": "Operation to perform (verb noun format). See x-operation-schemas for operation-specific parameter requirements.",
            "enum": operations.iter().map(|op| op.op_string()).collect::<Vec<_>>(),
        }),
    );

    // Add all collected parameters
    for (name, (param_type, description)) in seen_params {
        let json_type = param_type_to_json_schema_type(param_type);

        let mut prop = Map::new();
        prop.insert("type".to_string(), json!(json_type));

        if !description.is_empty() {
            prop.insert("description".to_string(), json!(description));
        }

        if param_type == ParamType::Array {
            prop.insert("items".to_string(), json!({"type": "string"}));
        }

        properties.insert(name, Value::Object(prop));
    }

    properties
}

/// Generate JSON Schema for a single operation
///
/// Creates a schema entry with:
/// - Const "op" field for this specific operation
/// - All parameter properties with correct types
/// - Required field list
///
/// This is stored in x-operation-schemas for documentation purposes.
fn operation_to_schema(op: &dyn Operation) -> Value {
    let params = op.parameters();

    let mut properties = Map::new();
    let mut required = vec!["op".to_string()];

    // Op field is always const for this specific operation
    properties.insert("op".to_string(), json!({"const": op.op_string()}));

    // Add each parameter
    for param in params {
        let json_type = param_type_to_json_schema_type(param.param_type);

        let mut prop_schema = Map::new();
        prop_schema.insert("type".to_string(), json!(json_type));

        // Add description if present
        if !param.description.is_empty() {
            prop_schema.insert("description".to_string(), json!(param.description));
        }

        // For array types, add items schema
        if param.param_type == ParamType::Array {
            prop_schema.insert("items".to_string(), json!({"type": "string"}));
        }

        properties.insert(param.name.to_string(), Value::Object(prop_schema));

        if param.required {
            required.push(param.name.to_string());
        }
    }

    json!({
        "title": op.op_string(),
        "description": op.description(),
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

/// Convert ParamType to JSON Schema type string
fn param_type_to_json_schema_type(param_type: ParamType) -> &'static str {
    match param_type {
        ParamType::String => "string",
        ParamType::Integer => "integer",
        ParamType::Number => "number",
        ParamType::Boolean => "boolean",
        ParamType::Array => "array",
    }
}

/// Group operations by noun for categorization
///
/// Returns a map of noun → array of operation strings
fn group_operations_by_noun(operations: &[&dyn Operation]) -> Map<String, Value> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();

    for op in operations {
        let noun = op.noun().to_string();
        let op_string = op.op_string();

        groups.entry(noun).or_default().push(op_string);
    }

    // Convert to Map<String, Value>
    groups.into_iter().map(|(k, v)| (k, json!(v))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParamMeta;

    // Mock operation for testing
    struct MockAddTask;

    static MOCK_ADD_TASK_PARAMS: &[ParamMeta] = &[
        ParamMeta::new("title")
            .description("Task title")
            .param_type(ParamType::String)
            .required(),
        ParamMeta::new("description")
            .description("Task description")
            .param_type(ParamType::String),
    ];

    impl Operation for MockAddTask {
        fn verb(&self) -> &'static str {
            "add"
        }
        fn noun(&self) -> &'static str {
            "task"
        }
        fn description(&self) -> &'static str {
            "Create a new task"
        }
        fn parameters(&self) -> &'static [ParamMeta] {
            MOCK_ADD_TASK_PARAMS
        }
    }

    struct MockGetTask;

    static MOCK_GET_TASK_PARAMS: &[ParamMeta] = &[ParamMeta::new("id")
        .description("Task ID")
        .param_type(ParamType::String)
        .required()];

    impl Operation for MockGetTask {
        fn verb(&self) -> &'static str {
            "get"
        }
        fn noun(&self) -> &'static str {
            "task"
        }
        fn description(&self) -> &'static str {
            "Get a task"
        }
        fn parameters(&self) -> &'static [ParamMeta] {
            MOCK_GET_TASK_PARAMS
        }
    }

    struct MockListTasks;

    static MOCK_LIST_TASKS_PARAMS: &[ParamMeta] = &[
        ParamMeta::new("assignee")
            .description("Filter by assignee")
            .param_type(ParamType::String),
        ParamMeta::new("ready")
            .description("Filter by ready status")
            .param_type(ParamType::Boolean),
    ];

    impl Operation for MockListTasks {
        fn verb(&self) -> &'static str {
            "list"
        }
        fn noun(&self) -> &'static str {
            "tasks"
        }
        fn description(&self) -> &'static str {
            "List all tasks"
        }
        fn parameters(&self) -> &'static [ParamMeta] {
            MOCK_LIST_TASKS_PARAMS
        }
    }

    #[test]
    fn test_param_type_mapping() {
        assert_eq!(param_type_to_json_schema_type(ParamType::String), "string");
        assert_eq!(
            param_type_to_json_schema_type(ParamType::Integer),
            "integer"
        );
        assert_eq!(param_type_to_json_schema_type(ParamType::Number), "number");
        assert_eq!(
            param_type_to_json_schema_type(ParamType::Boolean),
            "boolean"
        );
        assert_eq!(param_type_to_json_schema_type(ParamType::Array), "array");
    }

    #[test]
    fn test_operation_to_schema() {
        let op = MockAddTask;
        let schema = operation_to_schema(&op);

        // Verify structure
        assert_eq!(schema["title"], "add task");
        assert_eq!(schema["description"], "Create a new task");
        assert_eq!(schema["type"], "object");

        // Verify op field is const
        assert_eq!(schema["properties"]["op"]["const"], "add task");

        // Verify required includes op and title
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("op")));
        assert!(required.contains(&json!("title")));

        // Verify title property
        assert_eq!(schema["properties"]["title"]["type"], "string");
        assert_eq!(schema["properties"]["title"]["description"], "Task title");
    }

    #[test]
    fn test_collect_all_parameters() {
        let ops: Vec<&dyn Operation> = vec![&MockAddTask, &MockGetTask, &MockListTasks];
        let properties = collect_all_parameters(&ops);

        // Should have op field plus all unique parameters
        assert!(properties.contains_key("op"));
        assert!(properties.contains_key("title"));
        assert!(properties.contains_key("description"));
        assert!(properties.contains_key("id"));
        assert!(properties.contains_key("assignee"));
        assert!(properties.contains_key("ready"));

        // Verify op field has enum
        assert!(properties["op"]["enum"].is_array());
        let enum_vals = properties["op"]["enum"].as_array().unwrap();
        assert_eq!(enum_vals.len(), 3);
        assert!(enum_vals.contains(&json!("add task")));
        assert!(enum_vals.contains(&json!("get task")));
        assert!(enum_vals.contains(&json!("list tasks")));
    }

    #[test]
    fn test_group_operations_by_noun() {
        let ops: Vec<&dyn Operation> = vec![&MockAddTask, &MockGetTask, &MockListTasks];
        let groups = group_operations_by_noun(&ops);

        // Should have task and tasks groups
        assert!(groups.contains_key("task"));
        assert!(groups.contains_key("tasks"));

        let task_group = groups["task"].as_array().unwrap();
        assert!(task_group.contains(&json!("add task")));
        assert!(task_group.contains(&json!("get task")));

        let tasks_group = groups["tasks"].as_array().unwrap();
        assert!(tasks_group.contains(&json!("list tasks")));
    }

    #[test]
    fn test_generate_mcp_schema_minimal() {
        let ops: Vec<&dyn Operation> = vec![&MockAddTask, &MockGetTask];
        let config = SchemaConfig::new("Test operations");
        let schema = generate_mcp_schema(&ops, config);

        // Verify basic structure
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], true);
        assert_eq!(schema["description"], "Test operations");

        // Verify properties exist
        assert!(schema["properties"].is_object());
        assert!(schema["properties"]["op"].is_object());

        // Verify x-operation-schemas exists
        assert!(schema["x-operation-schemas"].is_array());
        let op_schemas = schema["x-operation-schemas"].as_array().unwrap();
        assert_eq!(op_schemas.len(), 2);

        // Verify x-operation-groups exists
        assert!(schema["x-operation-groups"].is_object());
    }

    #[test]
    fn test_generate_mcp_schema_with_examples() {
        let ops: Vec<&dyn Operation> = vec![&MockAddTask];
        let examples = vec![
            json!({"description": "Add a task", "value": {"op": "add task", "title": "Test"}}),
        ];
        let config = SchemaConfig::new("Test").with_examples(examples);
        let schema = generate_mcp_schema(&ops, config);

        assert!(schema["examples"].is_array());
        assert_eq!(schema["examples"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_generate_mcp_schema_with_aliases() {
        let ops: Vec<&dyn Operation> = vec![&MockAddTask];
        let mut aliases = Map::new();
        aliases.insert("add".to_string(), json!(["create", "new"]));

        let config = SchemaConfig::new("Test").with_verb_aliases(aliases);
        let schema = generate_mcp_schema(&ops, config);

        assert!(schema["x-forgiving-input"].is_object());
        assert!(schema["x-forgiving-input"]["verb_aliases"]["add"].is_array());
    }

    #[test]
    fn test_no_top_level_oneof() {
        // Critical: Claude API doesn't support oneOf/allOf/anyOf at top level
        let ops: Vec<&dyn Operation> = vec![&MockAddTask, &MockGetTask];
        let config = SchemaConfig::new("Test");
        let schema = generate_mcp_schema(&ops, config);

        assert!(!schema.as_object().unwrap().contains_key("oneOf"));
        assert!(!schema.as_object().unwrap().contains_key("allOf"));
        assert!(!schema.as_object().unwrap().contains_key("anyOf"));

        // Operation-specific schemas are in extension field instead
        assert!(schema["x-operation-schemas"].is_array());
    }
}
