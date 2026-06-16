//! Schema generation for the code_context tool using the Operation pattern

use serde_json::{json, Value};
use swissarmyhammer_operations::{
    generate_mcp_schema_full, generate_mcp_schema_wire, Operation, SchemaConfig,
};

/// Build the shared schema config (description + examples) for the code_context
/// tool, so the wire and full generators stay in lockstep.
fn code_context_schema_config() -> SchemaConfig {
    SchemaConfig::new(
        "Code context operations for symbol lookup, search, grep, call graph, and blast radius analysis. Use 'get symbol' for symbol lookup with locations and source text, 'search symbol' for fuzzy search, 'list symbols' for file-level listing, 'grep code' for regex search, 'get callgraph' for call graph traversal, 'get blastradius' for impact analysis, and status operations for index management.",
    )
    .with_examples(generate_code_context_examples())
}

/// Generate the slim WIRE MCP schema for the code_context tool.
///
/// Model-facing surface: carries only the op enum and per-op required-field
/// signatures, dropping the heavy CLI-facing keys. In-process consumers needing
/// the full per-op detail must call [`generate_code_context_schema_full`].
pub fn generate_code_context_schema(operations: &[&dyn Operation]) -> Value {
    generate_mcp_schema_wire(operations, code_context_schema_config())
}

/// Generate the FULL CLI-facing MCP schema for the code_context tool.
pub fn generate_code_context_schema_full(operations: &[&dyn Operation]) -> Value {
    generate_mcp_schema_full(operations, code_context_schema_config())
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
            "value": {"op": "rebuild index", "layer": "both"}
        }),
        json!({
            "description": "Clear all index data",
            "value": {"op": "clear status"}
        }),
        json!({
            "description": "Check LSP server status for detected languages",
            "value": {"op": "lsp status"}
        }),
        json!({
            "description": "Detect project types in the workspace",
            "value": {"op": "detect projects"}
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::code_context::{
        ClearStatus, DetectProjects, FindDuplicates, GetBlastradius, GetCallgraph, GetCodeStatus,
        GetSymbol, GrepCode, ListSymbols, LspStatus, QueryAst, RebuildIndex, SearchCode,
        SearchSymbol,
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
            &RebuildIndex as &dyn Operation,
            &ClearStatus as &dyn Operation,
            &LspStatus as &dyn Operation,
            &DetectProjects as &dyn Operation,
        ]
    }

    use swissarmyhammer_operations::WIRE_DROPPED_KEYS;

    #[test]
    fn test_wire_schema_structure_omits_heavy_keys() {
        let ops = test_operations();
        let schema = generate_code_context_schema(&ops);
        let obj = schema.as_object().unwrap();

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], true);
        assert!(schema["properties"].is_object());
        assert!(schema["properties"]["op"].is_object());
        assert!(schema["x-op-signatures"].is_object());
        for key in WIRE_DROPPED_KEYS {
            assert!(
                !obj.contains_key(key),
                "wire schema must omit heavy key {key:?}"
            );
        }
    }

    #[test]
    fn test_full_schema_structure_keeps_heavy_keys() {
        let ops = test_operations();
        let schema = generate_code_context_schema_full(&ops);

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], true);
        assert!(schema["properties"].is_object());
        assert!(schema["properties"]["op"].is_object());
        assert!(schema["x-operation-schemas"].is_array());
        assert!(schema["x-operation-groups"].is_object());
    }

    #[test]
    fn test_schema_has_op_enum() {
        // The op enum lives on both surfaces; assert against the wire schema.
        let ops = test_operations();
        let schema = generate_code_context_schema(&ops);

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert_eq!(op_enum.len(), 14);
        assert!(op_enum.contains(&json!("get symbol")));
        assert!(op_enum.contains(&json!("search symbol")));
        assert!(op_enum.contains(&json!("list symbols")));
        assert!(op_enum.contains(&json!("grep code")));
        assert!(op_enum.contains(&json!("get callgraph")));
        assert!(op_enum.contains(&json!("get blastradius")));
        assert!(op_enum.contains(&json!("get status")));
        assert!(op_enum.contains(&json!("rebuild index")));
        assert!(op_enum.contains(&json!("search code")));
        assert!(op_enum.contains(&json!("find duplicates")));
        assert!(op_enum.contains(&json!("query ast")));
        assert!(op_enum.contains(&json!("clear status")));
    }

    #[test]
    fn test_full_schema_has_operation_schemas() {
        let ops = test_operations();
        let schema = generate_code_context_schema_full(&ops);

        let op_schemas = schema["x-operation-schemas"]
            .as_array()
            .expect("should have x-operation-schemas");
        assert_eq!(op_schemas.len(), 14);
    }

    #[test]
    fn test_full_schema_has_all_parameters() {
        let ops = test_operations();
        let schema = generate_code_context_schema_full(&ops);

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
        // `min_similarity` stays a valid union-level prop because `find
        // duplicates` (`FIND_DUPLICATES_PARAMS`) still declares it. It is NOT
        // a `search code` param anymore — see
        // `test_search_code_own_params_are_lean` below.
        assert!(props.contains_key("min_similarity"));
        assert!(props.contains_key("file_pattern"));
        assert!(props.contains_key("min_chunk_bytes"));
        assert!(props.contains_key("max_per_chunk"));
    }

    /// Pull the per-op `x-operation-schemas` entry for one op string and return
    /// its declared parameter names (excluding the always-present `op` const).
    fn op_param_names(schema: &Value, op_string: &str) -> Vec<String> {
        let entry = schema["x-operation-schemas"]
            .as_array()
            .expect("full schema must carry x-operation-schemas")
            .iter()
            .find(|e| e["title"] == json!(op_string))
            .unwrap_or_else(|| panic!("no x-operation-schemas entry for {op_string:?}"));
        let mut names: Vec<String> = entry["properties"]
            .as_object()
            .expect("op schema must have properties")
            .keys()
            .filter(|k| k.as_str() != "op")
            .cloned()
            .collect();
        names.sort();
        names
    }

    /// `search code`'s agent-facing input surface must stay lean: exactly
    /// `query`, `top_k`, `language`, `file_pattern`. No `min_similarity`, no
    /// fusion weight knobs (`w_bm25`/`w_trigram`/`w_cosine`), no
    /// `min_fused_score` floor — those stay internal to `SearchCodeOptions`.
    #[test]
    fn test_search_code_own_params_are_lean() {
        let ops = test_operations();
        let schema = generate_code_context_schema_full(&ops);

        let mut expected = vec![
            "file_pattern".to_string(),
            "language".to_string(),
            "query".to_string(),
            "top_k".to_string(),
        ];
        expected.sort();
        assert_eq!(op_param_names(&schema, "search code"), expected);
    }

    /// The union-level `min_similarity` prop is justified solely by `find
    /// duplicates`, which still declares it as its own param.
    #[test]
    fn test_find_duplicates_still_declares_min_similarity() {
        let ops = test_operations();
        let schema = generate_code_context_schema_full(&ops);

        assert!(
            op_param_names(&schema, "find duplicates").contains(&"min_similarity".to_string()),
            "find duplicates must keep its own min_similarity param"
        );
    }

    #[test]
    fn test_full_schema_has_examples() {
        let ops = test_operations();
        let schema = generate_code_context_schema_full(&ops);

        assert!(schema["examples"].is_array());
        assert_eq!(schema["examples"].as_array().unwrap().len(), 14);
    }

    #[test]
    fn test_no_top_level_oneof() {
        let ops = test_operations();
        for schema in [
            generate_code_context_schema(&ops),
            generate_code_context_schema_full(&ops),
        ] {
            let obj = schema.as_object().unwrap();
            assert!(!obj.contains_key("oneOf"));
            assert!(!obj.contains_key("allOf"));
            assert!(!obj.contains_key("anyOf"));
        }
    }
}
