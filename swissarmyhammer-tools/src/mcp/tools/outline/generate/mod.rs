//! Outline generation tool for MCP operations
//!
//! This module provides the OutlineGenerateTool for generating structured code overviews
//! using Tree-sitter parsing through the MCP protocol.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
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

        let start_time = std::time::Instant::now();

        // Use the new file discovery functionality
        let file_discovery =
            match swissarmyhammer::outline::FileDiscovery::new(request.patterns.clone()) {
                Ok(discovery) => discovery,
                Err(e) => {
                    return Err(McpError::invalid_params(
                        format!("Failed to create file discovery: {e}"),
                        None,
                    ));
                }
            };

        let (discovered_files, discovery_report) = match file_discovery.discover_files() {
            Ok(result) => result,
            Err(e) => {
                return Err(McpError::internal_error(
                    format!("File discovery failed: {e}"),
                    None,
                ));
            }
        };

        tracing::info!("File discovery report: {}", discovery_report.summary());

        // Filter to only supported files for outline generation
        let supported_files =
            swissarmyhammer::outline::FileDiscovery::filter_supported_files(discovered_files);

        // Process all supported files and generate outline
        let outline_parser = swissarmyhammer::outline::OutlineParser::new(
            swissarmyhammer::outline::OutlineParserConfig::default(),
        )
        .map_err(|e| {
            McpError::internal_error(format!("Failed to create outline parser: {e}"), None)
        })?;

        // Build hierarchical structure using HierarchyBuilder
        let mut hierarchy_builder = swissarmyhammer::outline::HierarchyBuilder::new();
        let mut total_symbols = 0;

        for discovered_file in &supported_files {
            tracing::debug!("Processing file: {}", discovered_file.path.display());

            // Read file content
            let content = match std::fs::read_to_string(&discovered_file.path) {
                Ok(content) => content,
                Err(e) => {
                    tracing::warn!(
                        "Failed to read file {}: {}",
                        discovered_file.path.display(),
                        e
                    );
                    continue;
                }
            };

            match outline_parser.parse_file(&discovered_file.path, &content) {
                Ok(outline_tree) => {
                    total_symbols += outline_tree.symbols.len();
                    hierarchy_builder
                        .add_file_outline(outline_tree)
                        .map_err(|e| {
                            McpError::internal_error(
                                format!("Failed to add file to hierarchy: {e}"),
                                None,
                            )
                        })?;
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse file {}: {}",
                        discovered_file.path.display(),
                        e
                    );
                    // Continue processing other files
                }
            }
        }

        // Build the complete hierarchy
        let hierarchy = hierarchy_builder.build_hierarchy().map_err(|e| {
            McpError::internal_error(format!("Failed to build hierarchy: {e}"), None)
        })?;

        // Format output based on requested format
        let output_format = request.output_format.as_deref().unwrap_or("yaml");
        let formatted_output = match output_format {
            "json" => {
                // For JSON, convert to legacy format for compatibility
                let mut outline_nodes = Vec::new();
                for file in hierarchy.all_files() {
                    for symbol in &file.symbols {
                        let converted = convert_outline_node_with_children(symbol.clone())?;
                        outline_nodes.push(converted);
                    }
                }

                let response = OutlineResponse {
                    outline: outline_nodes,
                    files_processed: supported_files.len(),
                    symbols_found: total_symbols,
                    execution_time_ms: start_time.elapsed().as_millis() as u64,
                };

                serde_json::to_string_pretty(&response).map_err(|e| {
                    McpError::internal_error(format!("JSON serialization error: {e}"), None)
                })?
            }
            "yaml" => {
                // Use the new YamlFormatter for proper hierarchical YAML output
                let formatter = swissarmyhammer::outline::YamlFormatter::with_defaults();
                formatter.format_hierarchy(&hierarchy).map_err(|e| {
                    McpError::internal_error(format!("YAML formatting error: {e}"), None)
                })?
            }
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

/// Convert internal OutlineNode to MCP tool OutlineNode (legacy - no children support)
#[allow(dead_code)]
fn convert_outline_node(
    internal_node: swissarmyhammer::outline::types::OutlineNode,
    _file_path: &std::path::Path,
) -> Result<OutlineNode, McpError> {
    let kind = convert_outline_node_type(&internal_node.node_type);

    Ok(OutlineNode {
        name: internal_node.name,
        kind,
        line: internal_node.start_line as u32,
        signature: internal_node.signature,
        doc: internal_node.documentation,
        type_info: internal_node.visibility.map(|v| format!("{v:?}")),
        children: None, // Legacy function - no children support
    })
}

/// Convert internal OutlineNode to MCP tool OutlineNode with children support
fn convert_outline_node_with_children(
    internal_node: swissarmyhammer::outline::types::OutlineNode,
) -> Result<OutlineNode, McpError> {
    let kind = convert_outline_node_type(&internal_node.node_type);

    // Convert children recursively
    let children = if internal_node.children.is_empty() {
        None
    } else {
        let mut converted_children = Vec::new();
        for child in internal_node.children {
            converted_children.push(convert_outline_node_with_children(*child)?);
        }
        Some(converted_children)
    };

    Ok(OutlineNode {
        name: internal_node.name,
        kind,
        line: internal_node.start_line as u32,
        signature: internal_node.signature,
        doc: internal_node.documentation,
        type_info: internal_node.visibility.map(|v| format!("{v:?}")),
        children,
    })
}

/// Convert internal OutlineNodeType to MCP tool OutlineKind
fn convert_outline_node_type(
    node_type: &swissarmyhammer::outline::types::OutlineNodeType,
) -> OutlineKind {
    use swissarmyhammer::outline::types::OutlineNodeType;

    match node_type {
        OutlineNodeType::Class => OutlineKind::Class,
        OutlineNodeType::Interface => OutlineKind::Interface,
        OutlineNodeType::Struct => OutlineKind::Struct,
        OutlineNodeType::Enum => OutlineKind::Enum,
        OutlineNodeType::Function => OutlineKind::Function,
        OutlineNodeType::Method => OutlineKind::Method,
        OutlineNodeType::Property => OutlineKind::Property,
        OutlineNodeType::Variable => OutlineKind::Variable,
        OutlineNodeType::Module => OutlineKind::Module,
        OutlineNodeType::TypeAlias => OutlineKind::TypeAlias,
        OutlineNodeType::Trait => OutlineKind::Trait,
        OutlineNodeType::Constant => OutlineKind::Constant,
        OutlineNodeType::Import => OutlineKind::Import,
        OutlineNodeType::Impl => OutlineKind::Other, // Map Impl to Other as no direct equivalent
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
        use crate::mcp::tool_handlers::ToolHandlers;
        use std::path::PathBuf;
        use std::sync::Arc;
        use swissarmyhammer::git::GitOperations;
        use swissarmyhammer::issues::IssueStorage;
        use swissarmyhammer::memoranda::{mock_storage::MockMemoStorage, MemoStorage};
        use tokio::sync::{Mutex, RwLock};

        // Create mock context
        let issue_storage: Arc<RwLock<Box<dyn IssueStorage>>> = Arc::new(RwLock::new(Box::new(
            swissarmyhammer::issues::FileSystemIssueStorage::new(PathBuf::from("./test_issues"))
                .unwrap(),
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
            Arc::new(swissarmyhammer::common::rate_limiter::MockRateLimiter),
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
        use crate::mcp::tool_handlers::ToolHandlers;
        use std::path::PathBuf;
        use std::sync::Arc;
        use swissarmyhammer::git::GitOperations;
        use swissarmyhammer::issues::IssueStorage;
        use swissarmyhammer::memoranda::{mock_storage::MockMemoStorage, MemoStorage};
        use tokio::sync::{Mutex, RwLock};

        // Create mock context
        let issue_storage: Arc<RwLock<Box<dyn IssueStorage>>> = Arc::new(RwLock::new(Box::new(
            swissarmyhammer::issues::FileSystemIssueStorage::new(PathBuf::from("./test_issues"))
                .unwrap(),
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
            Arc::new(swissarmyhammer::common::rate_limiter::MockRateLimiter),
        );

        let tool = OutlineGenerateTool::new();
        let mut args = serde_json::Map::new();
        // Use a more targeted pattern to avoid scanning thousands of files
        args.insert(
            "patterns".to_string(),
            json!(["swissarmyhammer-tools/src/lib.rs"]),
        );
        args.insert("output_format".to_string(), json!("yaml"));

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }
}
