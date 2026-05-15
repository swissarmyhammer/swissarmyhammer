//! Schema generation for the unified files tool using the Operation pattern

use serde_json::{json, Value};
use swissarmyhammer_operations::{generate_mcp_schema, Operation, SchemaConfig};

/// Generate the MCP schema for the files tool from operation metadata
pub fn generate_files_mcp_schema(operations: &[&dyn Operation]) -> Value {
    let config = SchemaConfig::new(
        "File operations for reading, writing, editing, and searching files. Use 'read file' to read contents, 'write file' to create/overwrite, 'edit file' for string replacements, 'glob files' for pattern matching, and 'grep files' for content search.",
    )
    .with_examples(generate_files_examples());

    generate_mcp_schema(operations, config)
}

fn generate_files_examples() -> Vec<Value> {
    vec![
        json!({
            "description": "Read a file",
            "value": {"op": "read file", "path": "/src/main.rs"}
        }),
        json!({
            "description": "Read with offset and limit",
            "value": {"op": "read file", "path": "/logs/app.log", "offset": 100, "limit": 50}
        }),
        json!({
            "description": "Write a new file",
            "value": {"op": "write file", "file_path": "/src/config.rs", "content": "// config"}
        }),
        json!({
            "description": "Edit a file",
            "value": {"op": "edit file", "file_path": "/src/main.rs", "old_string": "old_fn", "new_string": "new_fn"}
        }),
        json!({
            "description": "Find files by pattern",
            "value": {"op": "glob files", "pattern": "**/*.rs"}
        }),
        json!({
            "description": "Search file contents",
            "value": {"op": "grep files", "pattern": "TODO", "path": "/src"}
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::files::edit::EditFile;
    use crate::mcp::tools::files::glob::GlobFiles;
    use crate::mcp::tools::files::grep::GrepFiles;
    use crate::mcp::tools::files::read::ReadFile;
    use crate::mcp::tools::files::write::WriteFile;

    fn test_operations() -> Vec<&'static dyn Operation> {
        vec![
            &ReadFile as &dyn Operation,
            &WriteFile as &dyn Operation,
            &EditFile as &dyn Operation,
            &GlobFiles as &dyn Operation,
            &GrepFiles as &dyn Operation,
        ]
    }

    #[test]
    fn test_generate_files_schema_structure() {
        let ops = test_operations();
        let schema = generate_files_mcp_schema(&ops);

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
        let schema = generate_files_mcp_schema(&ops);

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert_eq!(op_enum.len(), 5);
        assert!(op_enum.contains(&json!("read file")));
        assert!(op_enum.contains(&json!("write file")));
        assert!(op_enum.contains(&json!("edit file")));
        assert!(op_enum.contains(&json!("glob files")));
        assert!(op_enum.contains(&json!("grep files")));
    }

    #[test]
    fn test_no_top_level_oneof() {
        let ops = test_operations();
        let schema = generate_files_mcp_schema(&ops);

        let obj = schema.as_object().unwrap();
        assert!(!obj.contains_key("oneOf"));
        assert!(!obj.contains_key("allOf"));
        assert!(!obj.contains_key("anyOf"));
    }

    #[test]
    fn test_schema_has_examples() {
        let ops = test_operations();
        let schema = generate_files_mcp_schema(&ops);

        assert!(schema["examples"].is_array());
        assert_eq!(schema["examples"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn test_schema_has_all_parameters() {
        let ops = test_operations();
        let schema = generate_files_mcp_schema(&ops);

        let props = schema["properties"].as_object().unwrap();
        // Read params
        assert!(props.contains_key("path"));
        assert!(props.contains_key("offset"));
        assert!(props.contains_key("limit"));
        // Write params
        assert!(props.contains_key("file_path"));
        assert!(props.contains_key("content"));
        // Edit params
        assert!(props.contains_key("old_string"));
        assert!(props.contains_key("new_string"));
        assert!(props.contains_key("replace_all"));
        // Glob params
        assert!(props.contains_key("pattern"));
        assert!(props.contains_key("case_sensitive"));
        assert!(props.contains_key("respect_git_ignore"));
        // Grep params
        assert!(props.contains_key("glob"));
        assert!(props.contains_key("type"));
        assert!(props.contains_key("case_insensitive"));
        assert!(props.contains_key("output_mode"));
    }

    #[test]
    fn test_schema_has_operation_schemas() {
        let ops = test_operations();
        let schema = generate_files_mcp_schema(&ops);

        let op_schemas = schema["x-operation-schemas"]
            .as_array()
            .expect("should have x-operation-schemas");
        assert_eq!(op_schemas.len(), 5);
    }
}
