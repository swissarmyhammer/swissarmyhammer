//! MCP schema generation for agent operations

use serde_json::{json, Map, Value};
use swissarmyhammer_operations::{
    generate_mcp_schema_full, generate_mcp_schema_wire, Operation, SchemaConfig,
};

/// Build the shared schema config (description, examples, verb aliases) for the
/// agent tool, so the wire and full generators stay in lockstep.
fn agent_schema_config() -> SchemaConfig {
    SchemaConfig::new(
        "Agent management operations. Use 'use' to get an agent's full definition, 'search' to find agents by keyword, 'list' to see all available agents.",
    )
    .with_examples(generate_agent_examples())
    .with_verb_aliases(get_agent_verb_aliases())
}

/// Generate the slim WIRE MCP schema for agent operations.
///
/// Model-facing surface: carries only the op enum and per-op required-field
/// signatures, dropping the heavy CLI-facing keys. In-process consumers needing
/// the full per-op detail must call [`generate_agent_mcp_schema_full`].
pub fn generate_agent_mcp_schema(operations: &[&dyn Operation]) -> Value {
    generate_mcp_schema_wire(operations, agent_schema_config())
}

/// Generate the FULL CLI-facing MCP schema for agent operations.
pub fn generate_agent_mcp_schema_full(operations: &[&dyn Operation]) -> Value {
    generate_mcp_schema_full(operations, agent_schema_config())
}

/// Generate agent-specific usage examples
fn generate_agent_examples() -> Vec<Value> {
    vec![
        json!({
            "description": "List all available agents",
            "value": {"op": "list agent"}
        }),
        json!({
            "description": "Get an agent's full definition",
            "value": {"op": "use agent", "name": "test"}
        }),
        json!({
            "description": "Search for agents by keyword",
            "value": {"op": "search agent", "query": "test"}
        }),
        json!({
            "description": "Shorthand: get agent by name only",
            "value": {"name": "tester"}
        }),
        json!({
            "description": "Shorthand: search by query only",
            "value": {"query": "review"}
        }),
    ]
}

/// Get verb aliases for agent operations
fn get_agent_verb_aliases() -> Map<String, Value> {
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
    use crate::operations::{ListAgents, SearchAgent, UseAgent};
    use swissarmyhammer_operations::WIRE_DROPPED_KEYS;

    fn test_operations() -> Vec<&'static dyn Operation> {
        vec![
            Box::leak(Box::new(ListAgents::new())) as &dyn Operation,
            Box::leak(Box::new(UseAgent::new(""))) as &dyn Operation,
            Box::leak(Box::new(SearchAgent::new(""))) as &dyn Operation,
        ]
    }

    #[test]
    fn test_wire_schema_omits_heavy_keys() {
        let ops = test_operations();
        let schema = generate_agent_mcp_schema(&ops);
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
        let schema = generate_agent_mcp_schema_full(&ops);

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["op"].is_object());
        assert!(schema["x-operation-schemas"].is_array());
        assert!(schema["x-operation-groups"].is_object());
    }

    #[test]
    fn test_no_top_level_oneof() {
        let ops = test_operations();
        for schema in [
            generate_agent_mcp_schema(&ops),
            generate_agent_mcp_schema_full(&ops),
        ] {
            assert!(!schema.as_object().unwrap().contains_key("oneOf"));
            assert!(!schema.as_object().unwrap().contains_key("allOf"));
            assert!(!schema.as_object().unwrap().contains_key("anyOf"));
        }
    }
}
