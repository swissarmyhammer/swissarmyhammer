//! Outline generation tool for MCP operations
//!
//! This module provides the OutlineGenerateTool for generating structured code overviews
//! using Tree-sitter parsing through the MCP protocol.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde::{Deserialize, Serialize};

/// Request structure for outline generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineRequest {
    /// Glob patterns to match files against
    pub patterns: Vec<String>,
    /// Output format (defaults to "yaml")
    pub output_format: Option<String>,
}

/// Response structure for outline generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineResponse {
    /// Generated outline nodes
    pub outline: Vec<OutlineNode>,
    /// Total number of files processed
    pub files_processed: usize,
    /// Total number of symbols found
    pub symbols_found: usize,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Individual symbol representation in the outline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineNode {
    /// Symbol name
    pub name: String,
    /// Symbol type/kind
    pub kind: OutlineKind,
    /// Line number where symbol is defined
    pub line: u32,
    /// Optional signature (for functions, methods, etc.)
    pub signature: Option<String>,
    /// Optional documentation string
    pub doc: Option<String>,
    /// Optional type information
    pub type_info: Option<String>,
    /// Nested children symbols
    pub children: Option<Vec<OutlineNode>>,
}

/// Enum representing different types of code symbols
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutlineKind {
    /// Class definition
    Class,
    /// Interface definition
    Interface,
    /// Struct definition
    Struct,
    /// Enum definition
    Enum,
    /// Function definition
    Function,
    /// Method definition (function within a class/struct)
    Method,
    /// Constructor function
    Constructor,
    /// Property or field
    Property,
    /// Field definition
    Field,
    /// Variable declaration
    Variable,
    /// Module or namespace
    Module,
    /// Namespace definition
    Namespace,
    /// Type alias
    TypeAlias,
    /// Trait definition (Rust) or protocol (Swift)
    Trait,
    /// Constant definition
    Constant,
    /// Import/use statement
    Import,
    /// Other/unknown symbol type
    Other,
}

/// Tool for generating code outlines using Tree-sitter parsing
#[derive(Default)]
pub struct OutlineGenerateTool;

impl OutlineGenerateTool {
    /// Creates a new instance of the OutlineGenerateTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for OutlineGenerateTool {
    fn name(&self) -> &'static str {
        "outline_generate"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "patterns": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Glob patterns to match files against (e.g., \"**/*.rs\", \"src/**/*.py\")"
                },
                "output_format": {
                    "type": "string",
                    "enum": ["yaml", "json"],
                    "default": "yaml",
                    "description": "Output format for the outline (default: yaml)"
                }
            },
            "required": ["patterns"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: OutlineRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Validate patterns are not empty
        if request.patterns.is_empty() {
            return Err(McpError::invalid_params(
                "At least one pattern must be provided".to_string(),
                None,
            ));
        }

        tracing::debug!("Generating outline for patterns: {:?}", request.patterns);

        // TODO: Implement actual Tree-sitter parsing and outline generation
        // For now, return a placeholder response
        let response = OutlineResponse {
            outline: vec![],
            files_processed: 0,
            symbols_found: 0,
            execution_time_ms: 0,
        };

        // Format output based on requested format
        let output_format = request.output_format.as_deref().unwrap_or("yaml");
        let formatted_output = match output_format {
            "json" => serde_json::to_string_pretty(&response).map_err(|e| {
                McpError::internal_error(format!("JSON serialization error: {e}"), None)
            })?,
            "yaml" => serde_yaml::to_string(&response).map_err(|e| {
                McpError::internal_error(format!("YAML serialization error: {e}"), None)
            })?,
            _ => {
                return Err(McpError::invalid_params(
                    format!("Unsupported output format: {output_format}"),
                    None,
                ))
            }
        };

        Ok(BaseToolImpl::create_success_response(formatted_output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_outline_request_serialization() {
        let request = OutlineRequest {
            patterns: vec!["**/*.rs".to_string(), "src/**/*.py".to_string()],
            output_format: Some("yaml".to_string()),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: OutlineRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(request.patterns, deserialized.patterns);
        assert_eq!(request.output_format, deserialized.output_format);
    }

    #[test]
    fn test_outline_response_serialization() {
        let response = OutlineResponse {
            outline: vec![OutlineNode {
                name: "test_function".to_string(),
                kind: OutlineKind::Function,
                line: 42,
                signature: Some("fn test_function() -> String".to_string()),
                doc: Some("A test function".to_string()),
                type_info: None,
                children: None,
            }],
            files_processed: 1,
            symbols_found: 1,
            execution_time_ms: 123,
        };

        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: OutlineResponse = serde_json::from_str(&serialized).unwrap();

        assert_eq!(response.outline.len(), deserialized.outline.len());
        assert_eq!(response.files_processed, deserialized.files_processed);
        assert_eq!(response.symbols_found, deserialized.symbols_found);
        assert_eq!(response.execution_time_ms, deserialized.execution_time_ms);
    }

    #[test]
    fn test_outline_node_serialization() {
        let node = OutlineNode {
            name: "MyClass".to_string(),
            kind: OutlineKind::Class,
            line: 10,
            signature: Some("class MyClass:".to_string()),
            doc: Some("A sample class".to_string()),
            type_info: Some("class".to_string()),
            children: Some(vec![OutlineNode {
                name: "method".to_string(),
                kind: OutlineKind::Method,
                line: 15,
                signature: Some("def method(self):".to_string()),
                doc: None,
                type_info: None,
                children: None,
            }]),
        };

        let serialized = serde_json::to_string(&node).unwrap();
        let deserialized: OutlineNode = serde_json::from_str(&serialized).unwrap();

        assert_eq!(node.name, deserialized.name);
        assert_eq!(node.kind, deserialized.kind);
        assert_eq!(node.line, deserialized.line);
        assert_eq!(node.signature, deserialized.signature);
        assert_eq!(node.doc, deserialized.doc);
        assert_eq!(node.type_info, deserialized.type_info);
        assert!(deserialized.children.is_some());
        assert_eq!(
            node.children.as_ref().unwrap().len(),
            deserialized.children.as_ref().unwrap().len()
        );
    }

    #[test]
    fn test_outline_kind_serialization() {
        let kinds = vec![
            OutlineKind::Class,
            OutlineKind::Interface,
            OutlineKind::Struct,
            OutlineKind::Enum,
            OutlineKind::Function,
            OutlineKind::Method,
            OutlineKind::Constructor,
            OutlineKind::Property,
            OutlineKind::Field,
            OutlineKind::Variable,
            OutlineKind::Module,
            OutlineKind::Namespace,
            OutlineKind::TypeAlias,
            OutlineKind::Trait,
            OutlineKind::Constant,
            OutlineKind::Import,
            OutlineKind::Other,
        ];

        for kind in kinds {
            let serialized = serde_json::to_string(&kind).unwrap();
            let deserialized: OutlineKind = serde_json::from_str(&serialized).unwrap();
            assert_eq!(kind, deserialized);
        }
    }

    #[test]
    fn test_tool_schema() {
        let tool = OutlineGenerateTool::new();
        let schema = tool.schema();

        assert!(schema.is_object());
        let obj = schema.as_object().unwrap();
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("properties"));
        assert!(obj.contains_key("required"));

        let properties = obj["properties"].as_object().unwrap();
        assert!(properties.contains_key("patterns"));
        assert!(properties.contains_key("output_format"));

        let required = obj["required"].as_array().unwrap();
        assert!(required.contains(&json!("patterns")));
    }

    #[test]
    fn test_tool_name_and_description() {
        let tool = OutlineGenerateTool::new();
        assert_eq!(tool.name(), "outline_generate");
        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn test_tool_execution_invalid_empty_patterns() {
        use crate::git::GitOperations;
        use crate::issues::IssueStorage;
        use crate::mcp::tool_handlers::ToolHandlers;
        use crate::memoranda::{mock_storage::MockMemoStorage, MemoStorage};
        use std::path::PathBuf;
        use std::sync::Arc;
        use tokio::sync::{Mutex, RwLock};

        // Create mock context
        let issue_storage: Arc<RwLock<Box<dyn IssueStorage>>> = Arc::new(RwLock::new(Box::new(
            crate::issues::FileSystemIssueStorage::new(PathBuf::from("./test_issues")).unwrap(),
        )));
        let git_ops: Arc<Mutex<Option<GitOperations>>> = Arc::new(Mutex::new(None));
        let memo_storage: Arc<RwLock<Box<dyn MemoStorage>>> =
            Arc::new(RwLock::new(Box::new(MockMemoStorage::new())));
        let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));
        let context = ToolContext::new(
            tool_handlers,
            issue_storage,
            git_ops,
            memo_storage,
            Arc::new(crate::common::rate_limiter::MockRateLimiter),
        );

        let tool = OutlineGenerateTool::new();
        let mut args = serde_json::Map::new();
        args.insert("patterns".to_string(), json!([])); // Empty patterns array

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("At least one pattern must be provided"));
    }

    #[tokio::test]
    async fn test_tool_execution_valid_patterns() {
        use crate::git::GitOperations;
        use crate::issues::IssueStorage;
        use crate::mcp::tool_handlers::ToolHandlers;
        use crate::memoranda::{mock_storage::MockMemoStorage, MemoStorage};
        use std::path::PathBuf;
        use std::sync::Arc;
        use tokio::sync::{Mutex, RwLock};

        // Create mock context
        let issue_storage: Arc<RwLock<Box<dyn IssueStorage>>> = Arc::new(RwLock::new(Box::new(
            crate::issues::FileSystemIssueStorage::new(PathBuf::from("./test_issues")).unwrap(),
        )));
        let git_ops: Arc<Mutex<Option<GitOperations>>> = Arc::new(Mutex::new(None));
        let memo_storage: Arc<RwLock<Box<dyn MemoStorage>>> =
            Arc::new(RwLock::new(Box::new(MockMemoStorage::new())));
        let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));
        let context = ToolContext::new(
            tool_handlers,
            issue_storage,
            git_ops,
            memo_storage,
            Arc::new(crate::common::rate_limiter::MockRateLimiter),
        );

        let tool = OutlineGenerateTool::new();
        let mut args = serde_json::Map::new();
        args.insert("patterns".to_string(), json!(["**/*.rs"]));
        args.insert("output_format".to_string(), json!("yaml"));

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }
}
