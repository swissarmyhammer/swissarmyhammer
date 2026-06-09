//! Generic MCP tool schema generation from operation metadata
//!
//! This module provides reusable schema generation for any MCP tool built on
//! the Operation trait. The schema is derived from operation metadata and
//! stays automatically synchronized with operation definitions.

use serde_json::{json, Map, Value};
use std::collections::HashMap;

use crate::{Operation, ParamType};

/// The protocol discriminator field name shared by every generated schema.
///
/// This is the single most load-bearing key in the schema contract: it is the
/// property that carries the operation enum and the entry in every `required`
/// list, and the full and wire schemas must agree on it exactly. Defining it
/// once here keeps every insertion / required-list site in lockstep so the name
/// can never drift between surfaces.
const OP_FIELD: &str = "op";

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
///
/// This is a thin alias of [`generate_mcp_schema_full`], kept so the existing
/// in-process / wire callers keep compiling during the full-vs-wire transition.
pub fn generate_mcp_schema(operations: &[&dyn Operation], config: SchemaConfig) -> Value {
    generate_mcp_schema_full(operations, config)
}

/// Generate the complete CLI-facing MCP tool schema from operation metadata.
///
/// Byte-for-byte the historical behavior of [`generate_mcp_schema`]: flat
/// `properties`, the `op` enum, `x-operation-schemas`, `x-operation-groups`,
/// `x-forgiving-input`, `examples`, and any custom extensions. The
/// schema-driven CLI generator ([`crate::cli_gen`]) reads this surface
/// in-process, so it must retain the per-op detail.
///
/// # Arguments
///
/// * `operations` - Slice of operation trait objects
/// * `config` - Schema configuration (description, examples, aliases)
pub fn generate_mcp_schema_full(operations: &[&dyn Operation], config: SchemaConfig) -> Value {
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

/// The heavy CLI-facing keys that the FULL schema carries but the slim WIRE
/// schema deliberately omits.
///
/// [`generate_mcp_schema_full`] adds all of these; [`generate_mcp_schema_wire`]
/// adds none of them. This is the single source of truth for that contract:
/// wire-omission tests across the workspace import this slice instead of
/// re-declaring the literal list, so adding a fifth heavy key here keeps every
/// such test in lockstep.
pub const WIRE_DROPPED_KEYS: [&str; 4] = [
    "x-operation-schemas",
    "x-operation-groups",
    "x-forgiving-input",
    "examples",
];

/// Generate the slim WIRE MCP tool schema from operation metadata.
///
/// This is the model-facing surface: it carries only what the model needs to
/// call the tool correctly — the tool description, the `op` enum of valid op
/// strings, and a compact per-op required-field map under `x-op-signatures`.
/// It deliberately DROPS the heavy CLI-facing detail (`x-operation-schemas`,
/// `x-operation-groups`, `x-forgiving-input`, `examples`) and the per-op
/// property sub-objects, keeping only the `op` property.
///
/// Shape:
/// ```jsonc
/// {
///   "type": "object",
///   "additionalProperties": true,
///   "description": "<tool description>",
///   "properties": { "op": { "type": "string", "enum": [ ...op strings ] } },
///   "required": ["op"],
///   "x-op-signatures": { "<op>": ["<required param>", ...], ... }
/// }
/// ```
///
/// `x-op-signatures` has exactly one key per op in the enum; each value is that
/// op's required parameter names (excluding `op`) in declaration order.
///
/// # Arguments
///
/// * `operations` - Slice of operation trait objects
/// * `config` - Schema configuration (only `description` is used here)
pub fn generate_mcp_schema_wire(operations: &[&dyn Operation], config: SchemaConfig) -> Value {
    // Per-op required-name signatures, keyed by op string, covering every op.
    let signatures: Map<String, Value> = operations
        .iter()
        .map(|op| (op.op_string(), json!(required_param_names_for_op(*op))))
        .collect();

    // Only the `op` property survives, carrying the enum of valid op strings.
    let op_enum: Vec<String> = operations.iter().map(|op| op.op_string()).collect();

    let mut properties = Map::new();
    properties.insert(
        OP_FIELD.to_string(),
        json!({
            "type": "string",
            "enum": op_enum,
        }),
    );

    json!({
        "type": "object",
        "additionalProperties": true,
        "description": config.description,
        "properties": properties,
        "required": [OP_FIELD],
        "x-op-signatures": signatures,
    })
}

/// The required parameter names of a single operation, in declaration order.
///
/// Excludes the synthetic `op` field. Shared by [`operation_to_schema`] (the
/// per-op full schema) and [`generate_mcp_schema_wire`] (the wire signature
/// map) so the required-derivation logic lives in exactly one place. An op with
/// no required parameters yields an empty vec.
fn required_param_names_for_op(op: &dyn Operation) -> Vec<String> {
    op.parameters()
        .iter()
        .filter(|param| param.required)
        .map(|param| param.name.to_string())
        .collect()
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
        OP_FIELD.to_string(),
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

    // Op field is always const for this specific operation, and always required.
    properties.insert(OP_FIELD.to_string(), json!({"const": op.op_string()}));
    let mut required = vec![OP_FIELD.to_string()];
    // Reuse the single source of truth for required-name derivation.
    required.extend(required_param_names_for_op(op));

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
    use crate::test_support::{MockAddTask, MockGetTask, MockListTasks};
    use crate::ParamMeta;

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
    fn test_generate_mcp_schema_with_extensions() {
        let ops: Vec<&dyn Operation> = vec![&MockAddTask];
        let config = SchemaConfig::new("Test")
            .with_extension("x-custom-field".to_string(), json!("custom_value"))
            .with_extension("x-another".to_string(), json!({"nested": true}));
        let schema = generate_mcp_schema(&ops, config);

        assert_eq!(schema["x-custom-field"], "custom_value");
        assert_eq!(schema["x-another"]["nested"], true);
    }

    // Mock operation with an array parameter and empty description
    struct MockWithArrayParam;

    static MOCK_ARRAY_PARAMS: &[ParamMeta] = &[
        ParamMeta::new("tags")
            .description("Tag list")
            .param_type(ParamType::Array)
            .required(),
        ParamMeta::new("silent")
            .description("")
            .param_type(ParamType::Boolean),
    ];

    impl Operation for MockWithArrayParam {
        fn verb(&self) -> &'static str {
            "tag"
        }
        fn noun(&self) -> &'static str {
            "item"
        }
        fn description(&self) -> &'static str {
            "Tag an item"
        }
        fn parameters(&self) -> &'static [ParamMeta] {
            MOCK_ARRAY_PARAMS
        }
    }

    #[test]
    fn test_collect_all_parameters_with_array_type() {
        let ops: Vec<&dyn Operation> = vec![&MockWithArrayParam];
        let properties = collect_all_parameters(&ops);

        // Array param should have items schema
        let tags_prop = &properties["tags"];
        assert_eq!(tags_prop["type"], "array");
        assert_eq!(tags_prop["items"]["type"], "string");
        assert_eq!(tags_prop["description"], "Tag list");
    }

    #[test]
    fn test_collect_all_parameters_empty_description_omitted() {
        let ops: Vec<&dyn Operation> = vec![&MockWithArrayParam];
        let properties = collect_all_parameters(&ops);

        // Empty description should not be included
        let silent_prop = properties["silent"].as_object().unwrap();
        assert!(!silent_prop.contains_key("description"));
        assert_eq!(silent_prop["type"], "boolean");
    }

    #[test]
    fn test_operation_to_schema_with_array_param() {
        let op = MockWithArrayParam;
        let schema = operation_to_schema(&op);

        // Array param should have items in the per-operation schema too
        assert_eq!(schema["properties"]["tags"]["type"], "array");
        assert_eq!(schema["properties"]["tags"]["items"]["type"], "string");

        // Required should include tags
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("tags")));
    }

    #[test]
    fn test_operation_to_schema_empty_description_omitted() {
        let op = MockWithArrayParam;
        let schema = operation_to_schema(&op);

        // Silent param has empty description — should not appear in schema
        let silent_prop = schema["properties"]["silent"].as_object().unwrap();
        assert!(!silent_prop.contains_key("description"));

        // Silent is not required
        let required = schema["required"].as_array().unwrap();
        assert!(!required.contains(&json!("silent")));
    }

    // ------------------------------------------------------------------
    // Full vs wire schema split (card B)
    // ------------------------------------------------------------------

    use crate::test_support::{
        MockAddColumn, MockAddTag, MockGetBoard, MockInitBoard, MockUpdateBoard,
    };

    /// The full multi-noun mock op set, mirroring `cli_gen`'s `mock_schema`.
    fn full_mock_ops() -> Vec<&'static dyn Operation> {
        vec![
            &MockInitBoard,
            &MockGetBoard,
            &MockUpdateBoard,
            &MockAddTask,
            &MockGetTask,
            &MockListTasks,
            &MockAddColumn,
            &MockAddTag,
        ]
    }

    #[test]
    fn full_schema_contains_all_custom_extensions() {
        let ops = full_mock_ops();
        let mut aliases = Map::new();
        aliases.insert("add".to_string(), json!(["create", "new"]));
        let config = SchemaConfig::new("Mock operations")
            .with_examples(vec![json!({"value": {"op": "add task"}})])
            .with_verb_aliases(aliases);
        let schema = generate_mcp_schema_full(&ops, config);

        assert!(schema["x-operation-schemas"].is_array());
        assert!(schema["x-operation-groups"].is_object());
        assert!(schema["x-forgiving-input"].is_object());
        assert!(schema["examples"].is_array());
        // Full schema carries per-op property sub-objects in the flat properties.
        assert!(schema["properties"]["title"].is_object());
    }

    #[test]
    fn wire_schema_omits_dropped_keys() {
        let ops = full_mock_ops();
        let mut aliases = Map::new();
        aliases.insert("add".to_string(), json!(["create", "new"]));
        let config = SchemaConfig::new("Mock operations")
            .with_examples(vec![json!({"value": {"op": "add task"}})])
            .with_verb_aliases(aliases);
        let schema = generate_mcp_schema_wire(&ops, config);
        let obj = schema.as_object().unwrap();

        assert!(!obj.contains_key("x-operation-schemas"));
        assert!(!obj.contains_key("x-operation-groups"));
        assert!(!obj.contains_key("x-forgiving-input"));
        assert!(!obj.contains_key("examples"));
    }

    #[test]
    fn wire_schema_keeps_op_enum_and_top_level_shape() {
        let ops = full_mock_ops();
        let schema = generate_mcp_schema_wire(&ops, SchemaConfig::new("Mock operations"));

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], true);
        assert_eq!(schema["description"], "Mock operations");
        assert_eq!(schema["required"], json!(["op"]));

        // The only property is `op`, with the full op enum.
        let props = schema["properties"].as_object().unwrap();
        assert_eq!(props.len(), 1, "wire properties must hold only `op`");
        let enum_vals = props["op"]["enum"].as_array().unwrap();
        let expected: Vec<Value> = ops.iter().map(|op| json!(op.op_string())).collect();
        assert_eq!(enum_vals.len(), ops.len());
        for op in &ops {
            assert!(enum_vals.contains(&json!(op.op_string())));
        }
        let _ = expected;
    }

    #[test]
    fn wire_schema_signatures_cover_every_op_with_ordered_required_names() {
        let ops = full_mock_ops();
        let schema = generate_mcp_schema_wire(&ops, SchemaConfig::new("Mock operations"));
        let sigs = schema["x-op-signatures"].as_object().unwrap();

        // One key per op in the enum.
        assert_eq!(sigs.len(), ops.len());
        for op in &ops {
            assert!(
                sigs.contains_key(&op.op_string()),
                "missing signature for {}",
                op.op_string()
            );
        }

        // `init board` requires only `name` (excludes `op`).
        assert_eq!(sigs["init board"], json!(["name"]));
        // `add task` requires only `title` (description optional, assignees array).
        assert_eq!(sigs["add task"], json!(["title"]));
        // `get board` has no required params beyond op -> empty array.
        assert_eq!(sigs["get board"], json!([]));
        // `update board` has only an optional `name` -> empty array.
        assert_eq!(sigs["update board"], json!([]));
    }

    #[test]
    fn wire_schema_is_dramatically_smaller_than_full() {
        let ops = full_mock_ops();
        let full = generate_mcp_schema_full(&ops, SchemaConfig::new("Mock operations"));
        let wire = generate_mcp_schema_wire(&ops, SchemaConfig::new("Mock operations"));

        let full_len = serde_json::to_string(&full).unwrap().len();
        let wire_len = serde_json::to_string(&wire).unwrap().len();

        // Wire form for this mock set stays well under a safe ceiling.
        assert!(
            wire_len < 1024,
            "wire schema unexpectedly large: {wire_len} bytes"
        );
        // And it is dramatically smaller than the full form.
        assert!(
            wire_len < full_len / 4,
            "wire ({wire_len}) not < full/4 ({})",
            full_len / 4
        );
    }

    #[test]
    fn alias_returns_full_schema() {
        let ops = full_mock_ops();
        let alias = generate_mcp_schema(&ops, SchemaConfig::new("Mock operations"));
        let full = generate_mcp_schema_full(&ops, SchemaConfig::new("Mock operations"));
        assert_eq!(alias, full);
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
