//! Schema generation for the code_context tool using the Operation pattern

use serde_json::{json, Value};
use swissarmyhammer_operations::{generate_mcp_schema, Operation, SchemaConfig};

/// Generate the MCP schema for the code_context tool from operation metadata.
pub fn generate_code_context_schema(operations: &[&dyn Operation]) -> Value {
    let config = SchemaConfig::new(
        "Code context operations for symbol lookup, search, grep, call graph, and blast radius analysis. Use 'get symbol' for symbol lookup with locations and source text, 'search symbol' for fuzzy search, 'list symbols' for file-level listing, 'grep code' for regex search, 'get callgraph' for call graph traversal, 'get blastradius' for impact analysis, and status operations for index management.",
    )
    .with_examples(generate_code_context_examples());

    generate_mcp_schema(operations, config)
}

fn generate_code_context_examples() -> Vec<Value> {
    vec![
        json!({
            "description": "Get symbol locations and source text with fuzzy matching",
            "value": {"op": "get symbol", "query": "MyStruct::new", "max_results": 5}
        }),
        json!({
            "description": "Fuzzy search for symbols by kind",
            "value": {"op": "search symbol", "query": "handler", "kind": "function"}
        }),
        json!({
            "description": "List all symbols in a file",
            "value": {"op": "list symbols", "file_path": "src/main.rs"}
        }),
        json!({
            "description": "Regex search across code chunks",
            "value": {"op": "grep code", "pattern": "TODO|FIXME", "max_results": 20}
        }),
        json!({
            "description": "Semantic similarity search across code",
            "value": {"op": "search code", "query": "authentication handler", "top_k": 5}
        }),
        json!({
            "description": "Find duplicated code in a file",
            "value": {"op": "find duplicates", "file_path": "src/handlers.rs", "min_similarity": 0.85}
        }),
        json!({
            "description": "Find all function definitions in Rust files using S-expression query",
            "value": {"op": "query ast", "query": "(function_item name: (identifier) @name)", "language": "rust"}
        }),
        json!({
            "description": "Traverse call graph from a symbol",
            "value": {"op": "get callgraph", "symbol": "process_request", "direction": "outbound"}
        }),
        json!({
            "description": "Analyze blast radius of a file change",
            "value": {"op": "get blastradius", "file_path": "src/server.rs", "max_hops": 3}
        }),
        json!({
            "description": "Check index status",
            "value": {"op": "get status"}
        }),
        json!({
            "description": "Trigger re-indexing",
            "value": {"op": "build status", "layer": "both"}
        }),
        json!({
            "description": "Clear all index data",
            "value": {"op": "clear status"}
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::code_context::{
        BuildStatus, ClearStatus, FindDuplicates, GetBlastradius, GetCallgraph, GetCodeStatus,
        GetSymbol, GrepCode, ListSymbols, QueryAst, SearchCode, SearchSymbol,
    };

    fn test_operations() -> Vec<&'static dyn Operation> {
        vec![
            &GetSymbol as &dyn Operation,
            &SearchSymbol as &dyn Operation,
            &ListSymbols as &dyn Operation,
            &GrepCode as &dyn Operation,
            &SearchCode as &dyn Operation,
            &FindDuplicates as &dyn Operation,
            &QueryAst as &dyn Operation,
            &GetCallgraph as &dyn Operation,
            &GetBlastradius as &dyn Operation,
            &GetCodeStatus as &dyn Operation,
            &BuildStatus as &dyn Operation,
            &ClearStatus as &dyn Operation,
        ]
    }

    #[test]
    fn test_schema_structure() {
        let ops = test_operations();
        let schema = generate_code_context_schema(&ops);

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
        let schema = generate_code_context_schema(&ops);

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert_eq!(op_enum.len(), 12);
        assert!(op_enum.contains(&json!("get symbol")));
        assert!(op_enum.contains(&json!("search symbol")));
        assert!(op_enum.contains(&json!("list symbols")));
        assert!(op_enum.contains(&json!("grep code")));
        assert!(op_enum.contains(&json!("get callgraph")));
        assert!(op_enum.contains(&json!("get blastradius")));
        assert!(op_enum.contains(&json!("get status")));
        assert!(op_enum.contains(&json!("build status")));
        assert!(op_enum.contains(&json!("search code")));
        assert!(op_enum.contains(&json!("find duplicates")));
        assert!(op_enum.contains(&json!("query ast")));
        assert!(op_enum.contains(&json!("clear status")));
    }

    #[test]
    fn test_schema_has_operation_schemas() {
        let ops = test_operations();
        let schema = generate_code_context_schema(&ops);

        let op_schemas = schema["x-operation-schemas"]
            .as_array()
            .expect("should have x-operation-schemas");
        assert_eq!(op_schemas.len(), 12);
    }

    #[test]
    fn test_schema_has_all_parameters() {
        let ops = test_operations();
        let schema = generate_code_context_schema(&ops);

        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("query"));
        assert!(props.contains_key("file_path"));
        assert!(props.contains_key("pattern"));
        assert!(props.contains_key("symbol"));
        assert!(props.contains_key("direction"));
        assert!(props.contains_key("max_depth"));
        assert!(props.contains_key("max_hops"));
        assert!(props.contains_key("max_results"));
        assert!(props.contains_key("kind"));
        assert!(props.contains_key("layer"));
        assert!(props.contains_key("language"));
        assert!(props.contains_key("files"));
        assert!(props.contains_key("top_k"));
        assert!(props.contains_key("min_similarity"));
        assert!(props.contains_key("file_pattern"));
        assert!(props.contains_key("min_chunk_bytes"));
        assert!(props.contains_key("max_per_chunk"));
    }

    #[test]
    fn test_schema_has_examples() {
        let ops = test_operations();
        let schema = generate_code_context_schema(&ops);

        assert!(schema["examples"].is_array());
        assert_eq!(schema["examples"].as_array().unwrap().len(), 12);
    }

    #[test]
    fn test_no_top_level_oneof() {
        let ops = test_operations();
        let schema = generate_code_context_schema(&ops);

        let obj = schema.as_object().unwrap();
        assert!(!obj.contains_key("oneOf"));
        assert!(!obj.contains_key("allOf"));
        assert!(!obj.contains_key("anyOf"));
    }
}
