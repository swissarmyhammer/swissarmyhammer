//! Schema generation for the unified treesitter tool using the Operation pattern

use serde_json::{json, Value};
use swissarmyhammer_operations::{generate_mcp_schema, Operation, SchemaConfig};

/// Generate the MCP schema for the treesitter tool from operation metadata
pub fn generate_treesitter_mcp_schema(operations: &[&dyn Operation]) -> Value {
    let config = SchemaConfig::new(
        "Tree-sitter code intelligence operations. Use 'search code' for semantic similarity search, 'query ast' for structural pattern matching, 'find duplicates' for duplicate detection, and 'get status' to check index readiness.",
    )
    .with_examples(generate_treesitter_examples());

    generate_mcp_schema(operations, config)
}

fn generate_treesitter_examples() -> Vec<Value> {
    vec![
        json!({
            "description": "Semantic search for similar code",
            "value": {"op": "search code", "query": "fn process_request(req: Request) -> Response", "top_k": 5}
        }),
        json!({
            "description": "Find all function definitions in Rust",
            "value": {"op": "query ast", "query": "(function_item name: (identifier) @name)", "language": "rust"}
        }),
        json!({
            "description": "Detect duplicate code clusters",
            "value": {"op": "find duplicates", "min_similarity": 0.9}
        }),
        json!({
            "description": "Check index status",
            "value": {"op": "get status"}
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::treesitter::duplicates::FindDuplicates;
    use crate::mcp::tools::treesitter::query::QueryAst;
    use crate::mcp::tools::treesitter::search::SearchCode;
    use crate::mcp::tools::treesitter::status::GetStatus;

    fn test_operations() -> Vec<&'static dyn Operation> {
        vec![
            &SearchCode as &dyn Operation,
            &QueryAst as &dyn Operation,
            &FindDuplicates as &dyn Operation,
            &GetStatus as &dyn Operation,
        ]
    }

    #[test]
    fn test_generate_treesitter_schema_structure() {
        let ops = test_operations();
        let schema = generate_treesitter_mcp_schema(&ops);

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], true);
        assert!(schema["properties"].is_object());
        assert!(schema["properties"]["op"].is_object());
        assert!(schema["x-operation-schemas"].is_array());
        assert!(schema["x-operation-groups"].is_object());
    }

    #[test]
    fn test_schema_has_op_enum() {
        let ops = test_operations();
        let schema = generate_treesitter_mcp_schema(&ops);

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert_eq!(op_enum.len(), 4);
        assert!(op_enum.contains(&json!("search code")));
        assert!(op_enum.contains(&json!("query ast")));
        assert!(op_enum.contains(&json!("find duplicates")));
        assert!(op_enum.contains(&json!("get status")));
    }

    #[test]
    fn test_no_top_level_oneof() {
        let ops = test_operations();
        let schema = generate_treesitter_mcp_schema(&ops);

        let obj = schema.as_object().unwrap();
        assert!(!obj.contains_key("oneOf"));
        assert!(!obj.contains_key("allOf"));
        assert!(!obj.contains_key("anyOf"));
    }

    #[test]
    fn test_schema_has_examples() {
        let ops = test_operations();
        let schema = generate_treesitter_mcp_schema(&ops);

        assert!(schema["examples"].is_array());
        assert_eq!(schema["examples"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn test_schema_has_all_parameters() {
        let ops = test_operations();
        let schema = generate_treesitter_mcp_schema(&ops);

        let props = schema["properties"].as_object().unwrap();
        // Search params
        assert!(props.contains_key("query"));
        assert!(props.contains_key("top_k"));
        assert!(props.contains_key("min_similarity"));
        // Query params
        assert!(props.contains_key("files"));
        assert!(props.contains_key("language"));
        // Duplicates params
        assert!(props.contains_key("min_chunk_bytes"));
        assert!(props.contains_key("file"));
        // Common
        assert!(props.contains_key("path"));
    }

    #[test]
    fn test_schema_has_operation_schemas() {
        let ops = test_operations();
        let schema = generate_treesitter_mcp_schema(&ops);

        let op_schemas = schema["x-operation-schemas"]
            .as_array()
            .expect("should have x-operation-schemas");
        assert_eq!(op_schemas.len(), 4);
    }
}
