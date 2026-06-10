//! Schema generation for the unified web tool using the Operation pattern

use serde_json::{json, Value};
use swissarmyhammer_operations::{
    generate_mcp_schema_full, generate_mcp_schema_wire, Operation, SchemaConfig,
};

/// Build the shared schema config (description + examples) for the web tool, so
/// the wire and full generators stay in lockstep.
fn web_schema_config() -> SchemaConfig {
    SchemaConfig::new(
        "Web operations for searching and fetching content. Use 'search url' to search the web via Brave Search, and 'fetch url' to retrieve a specific page as markdown.",
    )
    .with_examples(generate_web_examples())
}

/// Generate the slim WIRE MCP schema for the web tool.
///
/// Model-facing surface: carries only the op enum and per-op required-field
/// signatures, dropping the heavy CLI-facing keys. In-process consumers needing
/// the full per-op detail must call [`generate_web_mcp_schema_full`].
pub fn generate_web_mcp_schema(operations: &[&dyn Operation]) -> Value {
    generate_mcp_schema_wire(operations, web_schema_config())
}

/// Generate the FULL CLI-facing MCP schema for the web tool.
pub fn generate_web_mcp_schema_full(operations: &[&dyn Operation]) -> Value {
    generate_mcp_schema_full(operations, web_schema_config())
}

fn generate_web_examples() -> Vec<Value> {
    vec![
        json!({
            "description": "Search the web",
            "value": {"op": "search url", "query": "rust async programming", "results_count": 10}
        }),
        json!({
            "description": "Fetch a specific URL as markdown",
            "value": {"op": "fetch url", "url": "https://example.com/page"}
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::web::fetch::FetchUrl;
    use crate::mcp::tools::web::search::SearchUrl;
    use swissarmyhammer_operations::WIRE_DROPPED_KEYS;

    #[test]
    fn test_wire_schema_structure_omits_heavy_keys() {
        let search = SearchUrl::default();
        let fetch = FetchUrl::default();
        let ops: Vec<&dyn Operation> = vec![&search, &fetch];
        let schema = generate_web_mcp_schema(&ops);
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
        let search = SearchUrl::default();
        let fetch = FetchUrl::default();
        let ops: Vec<&dyn Operation> = vec![&search, &fetch];
        let schema = generate_web_mcp_schema_full(&ops);

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
        let search = SearchUrl::default();
        let fetch = FetchUrl::default();
        let ops: Vec<&dyn Operation> = vec![&search, &fetch];
        let schema = generate_web_mcp_schema(&ops);

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert_eq!(op_enum.len(), 2);
        assert!(op_enum.contains(&json!("search url")));
        assert!(op_enum.contains(&json!("fetch url")));
    }

    #[test]
    fn test_no_top_level_oneof() {
        let search = SearchUrl::default();
        let fetch = FetchUrl::default();
        let ops: Vec<&dyn Operation> = vec![&search, &fetch];
        for schema in [
            generate_web_mcp_schema(&ops),
            generate_web_mcp_schema_full(&ops),
        ] {
            let obj = schema.as_object().unwrap();
            assert!(!obj.contains_key("oneOf"));
            assert!(!obj.contains_key("allOf"));
            assert!(!obj.contains_key("anyOf"));
        }
    }

    #[test]
    fn test_full_schema_has_examples() {
        let search = SearchUrl::default();
        let fetch = FetchUrl::default();
        let ops: Vec<&dyn Operation> = vec![&search, &fetch];
        let schema = generate_web_mcp_schema_full(&ops);

        assert!(schema["examples"].is_array());
        assert_eq!(schema["examples"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_full_schema_has_all_parameters() {
        let search = SearchUrl::default();
        let fetch = FetchUrl::default();
        let ops: Vec<&dyn Operation> = vec![&search, &fetch];
        let schema = generate_web_mcp_schema_full(&ops);

        let props = schema["properties"].as_object().unwrap();
        // Search params
        assert!(props.contains_key("query"));
        assert!(props.contains_key("results_count"));
        assert!(props.contains_key("category"));
        // Fetch params
        assert!(props.contains_key("url"));
        assert!(props.contains_key("timeout"));
        assert!(props.contains_key("follow_redirects"));
    }
}
