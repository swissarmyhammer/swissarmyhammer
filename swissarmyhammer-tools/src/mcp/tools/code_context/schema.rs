//! Schema generation for the code_context tool.
//!
//! Produces a flat JSON Schema with an `"op"` enum and `x-operation-schemas`
//! following the same pattern as the treesitter tool.

use serde_json::{json, Value};

/// Generate the MCP schema for the code_context tool.
///
/// Builds a flat schema with all parameters merged at the top level,
/// an `"op"` enum for operation dispatch, and `x-operation-schemas`
/// for per-operation parameter documentation.
pub fn generate_code_context_schema() -> Value {
    let op_names = vec![
        "find symbol",
        "get symbol",
        "search symbol",
        "list symbols",
        "grep code",
        "get callgraph",
        "get blastradius",
        "get status",
        "build status",
        "clear status",
    ];

    let op_schemas = vec![
        json!({
            "title": "find symbol",
            "type": "object",
            "description": "Find symbol locations by exact name match. Returns file, line, char coordinates.",
            "required": ["op", "name"],
            "properties": {
                "op": { "const": "find symbol" },
                "name": {
                    "type": "string",
                    "description": "The symbol name to search for"
                }
            }
        }),
        json!({
            "title": "get symbol",
            "type": "object",
            "description": "Get symbol source text with multi-tier fuzzy matching (exact, suffix, case-insensitive, fuzzy)",
            "required": ["op", "query"],
            "properties": {
                "op": { "const": "get symbol" },
                "query": {
                    "type": "string",
                    "description": "The symbol name or qualified path to search for"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return"
                }
            }
        }),
        json!({
            "title": "search symbol",
            "type": "object",
            "description": "Fuzzy search across all indexed symbols with optional kind filter",
            "required": ["op", "query"],
            "properties": {
                "op": { "const": "search symbol" },
                "query": {
                    "type": "string",
                    "description": "The text to fuzzy-match against symbol names"
                },
                "kind": {
                    "type": "string",
                    "description": "Filter by symbol kind: function, method, struct, class, interface, module, etc."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return"
                }
            }
        }),
        json!({
            "title": "list symbols",
            "type": "object",
            "description": "List all symbols in a specific file, sorted by start line",
            "required": ["op", "file_path"],
            "properties": {
                "op": { "const": "list symbols" },
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to list symbols from"
                }
            }
        }),
        json!({
            "title": "grep code",
            "type": "object",
            "description": "Regex search across stored code chunks. Returns complete semantic blocks that match.",
            "required": ["op", "pattern"],
            "properties": {
                "op": { "const": "grep code" },
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "language": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only search chunks from files with these extensions (e.g. [\"rs\", \"py\"])"
                },
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only search chunks from these specific file paths"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matching chunks to return"
                }
            }
        }),
        json!({
            "title": "get callgraph",
            "type": "object",
            "description": "Traverse call graph from a starting symbol. Returns edges with source provenance.",
            "required": ["op", "symbol"],
            "properties": {
                "op": { "const": "get callgraph" },
                "symbol": {
                    "type": "string",
                    "description": "Symbol identifier -- either a name or a file:line:char locator"
                },
                "direction": {
                    "type": "string",
                    "enum": ["inbound", "outbound", "both"],
                    "description": "Traversal direction (default: outbound)"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum traversal depth, 1-5 (default: 2)"
                }
            }
        }),
        json!({
            "title": "get blastradius",
            "type": "object",
            "description": "Analyze blast radius of changes to a file or symbol. Finds transitive inbound callers.",
            "required": ["op", "file_path"],
            "properties": {
                "op": { "const": "get blastradius" },
                "file_path": {
                    "type": "string",
                    "description": "File path to analyze"
                },
                "symbol": {
                    "type": "string",
                    "description": "Optional symbol name within the file to narrow the starting set"
                },
                "max_hops": {
                    "type": "integer",
                    "description": "Maximum number of hops to follow, 1-10 (default: 3)"
                }
            }
        }),
        json!({
            "title": "get status",
            "type": "object",
            "description": "Health report with file counts, indexing progress, chunk/edge counts",
            "required": ["op"],
            "properties": {
                "op": { "const": "get status" }
            }
        }),
        json!({
            "title": "build status",
            "type": "object",
            "description": "Mark files for re-indexing by resetting indexed flags",
            "required": ["op"],
            "properties": {
                "op": { "const": "build status" },
                "layer": {
                    "type": "string",
                    "enum": ["treesitter", "lsp", "both"],
                    "description": "Which indexing layer to reset (default: both)"
                }
            }
        }),
        json!({
            "title": "clear status",
            "type": "object",
            "description": "Wipe all index data (edges, symbols, chunks, files) and return stats",
            "required": ["op"],
            "properties": {
                "op": { "const": "clear status" }
            }
        }),
    ];

    // Collect all unique properties across all operations
    let mut all_properties = serde_json::Map::new();

    // op field
    all_properties.insert(
        "op".to_string(),
        json!({
            "type": "string",
            "description": "Operation to perform (verb noun format). See x-operation-schemas for operation-specific parameter requirements.",
            "enum": op_names
        }),
    );

    // Merge all properties from all operation schemas
    for schema in &op_schemas {
        if let Some(props) = schema["properties"].as_object() {
            for (key, value) in props {
                if key != "op" && !all_properties.contains_key(key) {
                    all_properties.insert(key.clone(), value.clone());
                }
            }
        }
    }

    // Build operation groups
    let operation_groups = json!({
        "symbol": ["find symbol", "get symbol", "search symbol", "list symbols"],
        "code": ["grep code"],
        "graph": ["get callgraph", "get blastradius"],
        "status": ["get status", "build status", "clear status"]
    });

    // Build examples
    let examples = vec![
        json!({
            "description": "Find symbol locations by name",
            "value": {"op": "find symbol", "name": "process_request"}
        }),
        json!({
            "description": "Get symbol source text with fuzzy matching",
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
    ];

    json!({
        "type": "object",
        "additionalProperties": true,
        "description": "Code context operations for symbol lookup, search, grep, call graph, and blast radius analysis. Use 'find symbol' for exact name lookup, 'get symbol' for source text retrieval, 'search symbol' for fuzzy search, 'list symbols' for file-level listing, 'grep code' for regex search, 'get callgraph' for call graph traversal, 'get blastradius' for impact analysis, and status operations for index management.",
        "properties": all_properties,
        "required": ["op"],
        "examples": examples,
        "x-operation-schemas": op_schemas,
        "x-operation-groups": operation_groups
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_structure() {
        let schema = generate_code_context_schema();

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], true);
        assert!(schema["properties"].is_object());
        assert!(schema["properties"]["op"].is_object());
        assert!(schema["x-operation-schemas"].is_array());
        assert!(schema["x-operation-groups"].is_object());
    }

    #[test]
    fn test_schema_has_op_enum() {
        let schema = generate_code_context_schema();

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert_eq!(op_enum.len(), 10);
        assert!(op_enum.contains(&json!("find symbol")));
        assert!(op_enum.contains(&json!("get symbol")));
        assert!(op_enum.contains(&json!("search symbol")));
        assert!(op_enum.contains(&json!("list symbols")));
        assert!(op_enum.contains(&json!("grep code")));
        assert!(op_enum.contains(&json!("get callgraph")));
        assert!(op_enum.contains(&json!("get blastradius")));
        assert!(op_enum.contains(&json!("get status")));
        assert!(op_enum.contains(&json!("build status")));
        assert!(op_enum.contains(&json!("clear status")));
    }

    #[test]
    fn test_schema_has_operation_schemas() {
        let schema = generate_code_context_schema();

        let op_schemas = schema["x-operation-schemas"]
            .as_array()
            .expect("should have x-operation-schemas");
        assert_eq!(op_schemas.len(), 10);
    }

    #[test]
    fn test_schema_has_all_parameters() {
        let schema = generate_code_context_schema();

        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("name"));
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
    }

    #[test]
    fn test_schema_has_examples() {
        let schema = generate_code_context_schema();

        assert!(schema["examples"].is_array());
        assert_eq!(schema["examples"].as_array().unwrap().len(), 10);
    }

    #[test]
    fn test_no_top_level_oneof() {
        let schema = generate_code_context_schema();

        let obj = schema.as_object().unwrap();
        assert!(!obj.contains_key("oneOf"));
        assert!(!obj.contains_key("allOf"));
        assert!(!obj.contains_key("anyOf"));
    }
}
