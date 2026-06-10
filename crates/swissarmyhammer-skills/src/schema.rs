//! MCP schema generation for skill operations

use serde_json::{json, Map, Value};
use swissarmyhammer_operations::{
    generate_mcp_schema_full, generate_mcp_schema_wire, Operation, SchemaConfig,
};

/// Build the shared schema config (description, examples, verb aliases) for the
/// skill tool, so the wire and full generators stay in lockstep.
fn skill_schema_config() -> SchemaConfig {
    SchemaConfig::new(
        "Skill management operations. Use 'use' to activate a skill, 'search' to find skills by keyword, 'list' to see all available skills.",
    )
    .with_examples(generate_skill_examples())
    .with_verb_aliases(get_skill_verb_aliases())
}

/// Generate the slim WIRE MCP schema for skill operations.
///
/// Model-facing surface: carries only the op enum and per-op required-field
/// signatures, dropping the heavy CLI-facing keys. In-process consumers needing
/// the full per-op detail must call [`generate_skill_mcp_schema_full`].
pub fn generate_skill_mcp_schema(operations: &[&dyn Operation]) -> Value {
    generate_mcp_schema_wire(operations, skill_schema_config())
}

/// Generate the FULL CLI-facing MCP schema for skill operations.
pub fn generate_skill_mcp_schema_full(operations: &[&dyn Operation]) -> Value {
    generate_mcp_schema_full(operations, skill_schema_config())
}

/// Generate skill-specific usage examples
fn generate_skill_examples() -> Vec<Value> {
    vec![
        json!({
            "description": "List all available skills",
            "value": {"op": "list skill"}
        }),
        json!({
            "description": "Activate a skill by name",
            "value": {"op": "use skill", "name": "plan"}
        }),
        json!({
            "description": "Activate a skill with arguments",
            "value": {"op": "use skill", "name": "task", "arguments": "fix the login bug"}
        }),
        json!({
            "description": "Search for skills by keyword",
            "value": {"op": "search skill", "query": "commit"}
        }),
        json!({
            "description": "Shorthand: activate skill by name only",
            "value": {"name": "commit"}
        }),
        json!({
            "description": "Shorthand: search by query only",
            "value": {"query": "test"}
        }),
    ]
}

/// Get verb aliases for skill operations
fn get_skill_verb_aliases() -> Map<String, Value> {
    let mut aliases = Map::new();
    aliases.insert(
        "use".to_string(),
        json!(["get", "load", "activate", "invoke"]),
    );
    aliases.insert("list".to_string(), json!(["ls", "show", "available"]));
    aliases.insert("search".to_string(), json!(["find", "lookup"]));
    aliases
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::{ListSkills, SearchSkill, UseSkill};
    use swissarmyhammer_operations::WIRE_DROPPED_KEYS;

    fn test_operations() -> Vec<&'static dyn Operation> {
        vec![
            Box::leak(Box::new(ListSkills::new())) as &dyn Operation,
            Box::leak(Box::new(UseSkill::new(""))) as &dyn Operation,
            Box::leak(Box::new(SearchSkill::new(""))) as &dyn Operation,
        ]
    }

    #[test]
    fn test_wire_schema_omits_heavy_keys() {
        let ops = test_operations();
        let schema = generate_skill_mcp_schema(&ops);
        let obj = schema.as_object().unwrap();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["op"].is_object());
        assert!(schema["x-op-signatures"].is_object());
        for key in WIRE_DROPPED_KEYS {
            assert!(!obj.contains_key(key), "wire schema must omit {key:?}");
        }
    }

    #[test]
    fn test_full_schema_keeps_heavy_keys() {
        let ops = test_operations();
        let schema = generate_skill_mcp_schema_full(&ops);

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["op"].is_object());
        assert!(schema["x-operation-schemas"].is_array());
        assert!(schema["x-operation-groups"].is_object());
    }

    #[test]
    fn test_no_top_level_oneof() {
        let ops = test_operations();
        for schema in [
            generate_skill_mcp_schema(&ops),
            generate_skill_mcp_schema_full(&ops),
        ] {
            assert!(!schema.as_object().unwrap().contains_key("oneOf"));
            assert!(!schema.as_object().unwrap().contains_key("allOf"));
            assert!(!schema.as_object().unwrap().contains_key("anyOf"));
        }
    }
}
