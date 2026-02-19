//! MCP schema generation for skill operations

use serde_json::{json, Map, Value};
use swissarmyhammer_operations::{generate_mcp_schema, Operation, SchemaConfig};

/// Generate MCP schema for skill operations
pub fn generate_skill_mcp_schema(operations: &[&dyn Operation]) -> Value {
    let config = SchemaConfig::new(
        "Skill management operations. Use 'use' to activate a skill, 'search' to find skills by keyword, 'list' to see all available skills.",
    )
    .with_examples(generate_skill_examples())
    .with_verb_aliases(get_skill_verb_aliases());

    generate_mcp_schema(operations, config)
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
    aliases.insert("use".to_string(), json!(["get", "load", "activate", "invoke"]));
    aliases.insert("list".to_string(), json!(["ls", "show", "available"]));
    aliases.insert("search".to_string(), json!(["find", "lookup"]));
    aliases
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::{ListSkills, SearchSkill, UseSkill};

    fn test_operations() -> Vec<&'static dyn Operation> {
        vec![
            Box::leak(Box::new(ListSkills::new())) as &dyn Operation,
            Box::leak(Box::new(UseSkill::new(""))) as &dyn Operation,
            Box::leak(Box::new(SearchSkill::new(""))) as &dyn Operation,
        ]
    }

    #[test]
    fn test_generate_skill_schema_structure() {
        let ops = test_operations();
        let schema = generate_skill_mcp_schema(&ops);

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["op"].is_object());
    }

    #[test]
    fn test_no_top_level_oneof() {
        let ops = test_operations();
        let schema = generate_skill_mcp_schema(&ops);

        assert!(!schema.as_object().unwrap().contains_key("oneOf"));
        assert!(!schema.as_object().unwrap().contains_key("allOf"));
        assert!(!schema.as_object().unwrap().contains_key("anyOf"));
    }
}
