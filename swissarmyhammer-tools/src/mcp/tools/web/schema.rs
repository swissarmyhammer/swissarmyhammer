//! Schema generation for the unified web tool using the Operation pattern

use serde_json::{json, Value};
use swissarmyhammer_operations::{generate_mcp_schema, Operation, SchemaConfig};

/// Generate the MCP schema for the web tool from operation metadata
pub fn generate_web_mcp_schema(operations: &[&dyn Operation]) -> Value {
    let config = SchemaConfig::new(
        "Web operations for searching and fetching content. Use 'search url' to search the web via DuckDuckGo, and 'fetch url' to retrieve a specific page as markdown.",
    )
    .with_examples(generate_web_examples());

    generate_mcp_schema(operations, config)
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

    #[test]
    fn test_generate_web_schema_structure() {
        let search = SearchUrl::default();
        let fetch = FetchUrl::default();
        let ops: Vec<&dyn Operation> = vec![&search, &fetch];
        let schema = generate_web_mcp_schema(&ops);

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], true);
        assert!(schema["properties"].is_object());
        assert!(schema["properties"]["op"].is_object());
        assert!(schema["x-operation-schemas"].is_array());
        assert!(schema["x-operation-groups"].is_object());
    }

    #[test]
    fn test_schema_has_op_enum() {
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
        let schema = generate_web_mcp_schema(&ops);

        let obj = schema.as_object().unwrap();
        assert!(!obj.contains_key("oneOf"));
        assert!(!obj.contains_key("allOf"));
        assert!(!obj.contains_key("anyOf"));
    }

    #[test]
    fn test_schema_has_examples() {
        let search = SearchUrl::default();
        let fetch = FetchUrl::default();
        let ops: Vec<&dyn Operation> = vec![&search, &fetch];
        let schema = generate_web_mcp_schema(&ops);

        assert!(schema["examples"].is_array());
        assert_eq!(schema["examples"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_schema_has_all_parameters() {
        let search = SearchUrl::default();
        let fetch = FetchUrl::default();
        let ops: Vec<&dyn Operation> = vec![&search, &fetch];
        let schema = generate_web_mcp_schema(&ops);

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
