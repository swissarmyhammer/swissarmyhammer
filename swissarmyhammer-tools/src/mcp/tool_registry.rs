//! Tool registry for MCP operations
//!
//! This module provides a registry pattern for managing MCP tools, replacing
//! the large match statement with a flexible, extensible system.
//!
//! # Architecture Overview
//!
//! The tool registry pattern enables a modular, extensible approach to MCP tool management:
//!
//! 1. **McpTool Trait**: Defines the interface that all tools must implement
//! 2. **ToolRegistry**: Central registry that stores and manages tool instances
//! 3. **ToolContext**: Shared context providing access to storage and services
//! 4. **BaseToolImpl**: Common utility methods for tool implementations
//!
//! # Migration from Legacy System
//!
//! This registry pattern replaces the previous delegation-based approach where all
//! tools were routed through `ToolHandlers` with a large match statement. The new
//! pattern offers:
//!
//! - **Modularity**: Each tool is self-contained in its own module
//! - **Extensibility**: New tools can be added without modifying existing code
//! - **Testability**: Tools can be unit tested independently
//! - **Performance**: Direct access to storage eliminates delegation overhead
//!
//! # Creating New Tools
//!
//! To create a new MCP tool:
//!
//! 1. Create a struct implementing the `McpTool` trait
//! 2. Define the tool's schema using JSON Schema
//! 3. Implement the execute method with your business logic
//! 4. Register the tool with the appropriate registry function
//!
//! ```rust,ignore
//! use async_trait::async_trait;
//! use crate::mcp::tool_registry::{McpTool, ToolContext, BaseToolImpl};
//!
//! #[derive(Default)]
//! pub struct MyTool;
//!
//! #[async_trait]
//! impl McpTool for MyTool {
//!     fn name(&self) -> &'static str {
//!         "my_tool_name"
//!     }
//!
//!     fn description(&self) -> &'static str {
//!         include_str!("description.md")
//!     }
//!
//!     fn schema(&self) -> serde_json::Value {
//!         serde_json::json!({
//!             "type": "object",
//!             "properties": {
//!                 "param": {"type": "string", "description": "Parameter description"}
//!             },
//!             "required": ["param"]
//!         })
//!     }
//!
//!     async fn execute(
//!         &self,
//!         arguments: serde_json::Map<String, serde_json::Value>,
//!         context: &ToolContext,
//!     ) -> std::result::Result<CallToolResult, McpError> {
//!         let request: MyRequest = BaseToolImpl::parse_arguments(arguments)?;
//!         // Tool implementation here
//!         Ok(BaseToolImpl::create_success_response("Success!"))
//!     }
//! }
//! ```

use super::tool_handlers::ToolHandlers;
use rmcp::model::{Annotated, CallToolResult, RawContent, RawTextContent, Tool};
use rmcp::Error as McpError;
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer::common::rate_limiter::RateLimitChecker;
use swissarmyhammer::git::GitOperations;
use swissarmyhammer::issues::IssueStorage;
use swissarmyhammer::memoranda::MemoStorage;
use tokio::sync::{Mutex, RwLock};

/// Context shared by all tools during execution
///
/// The `ToolContext` provides tools with access to all necessary storage backends
/// and services required for their operation. It serves as the dependency injection
/// mechanism for the tool registry pattern.
///
/// # Architecture Notes
///
/// The context maintains both legacy `tool_handlers` for backward compatibility
/// and direct storage access for new tool implementations. This dual approach
/// allows for gradual migration from the old delegation pattern to the new
/// direct access pattern.
///
/// # Thread Safety
///
/// All storage backends are wrapped in appropriate synchronization primitives:
/// - `RwLock` for storage that supports concurrent reads
/// - `Mutex` for exclusive access operations
/// - `Arc` for shared ownership across async tasks
///
/// # Usage Patterns
///
/// New tools should prefer direct access to storage backends:
///
/// ```rust,ignore
/// async fn execute(&self, args: Args, context: &ToolContext) -> Result<CallToolResult> {
///     let memo_storage = context.memo_storage.write().await;
///     let memo = memo_storage.create_memo(title, content).await?;
///     // Process memo...
/// }
/// ```
#[derive(Clone)]
pub struct ToolContext {
    /// The tool handlers instance containing the business logic (for backward compatibility)
    ///
    /// This field exists to support legacy tools that haven't been migrated to the
    /// new registry pattern. New tools should prefer direct storage access.
    pub tool_handlers: Arc<ToolHandlers>,

    /// Direct access to issue storage for new tool implementations
    ///
    /// Provides thread-safe access to issue storage operations. Use `read()` for
    /// read operations and `write()` for write operations.
    pub issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,

    /// Direct access to git operations for new tool implementations
    ///
    /// Git operations are wrapped in `Option` to handle cases where git is not
    /// available or not initialized. Always check for `None` before use.
    pub git_ops: Arc<Mutex<Option<GitOperations>>>,

    /// Direct access to memo storage for new tool implementations
    ///
    /// Provides thread-safe access to memoranda storage operations. Use `read()` for
    /// read operations and `write()` for write operations.
    pub memo_storage: Arc<RwLock<Box<dyn MemoStorage>>>,

    /// Rate limiter for preventing denial of service attacks
    ///
    /// Provides configurable rate limiting for MCP operations. The trait-based
    /// design allows for easy testing with mock implementations.
    pub rate_limiter: Arc<dyn RateLimitChecker>,
}

impl ToolContext {
    /// Create a new tool context
    pub fn new(
        tool_handlers: Arc<ToolHandlers>,
        issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,
        git_ops: Arc<Mutex<Option<GitOperations>>>,
        memo_storage: Arc<RwLock<Box<dyn MemoStorage>>>,
        rate_limiter: Arc<dyn RateLimitChecker>,
    ) -> Self {
        Self {
            tool_handlers,
            issue_storage,
            git_ops,
            memo_storage,
            rate_limiter,
        }
    }
}

/// Trait defining the interface for all MCP tools
///
/// The `McpTool` trait provides a standardized interface for implementing MCP tools
/// within the registry pattern. All tools must implement this trait to be usable
/// with the tool registry system.
///
/// # Design Principles
///
/// - **Stateless**: Tools should be stateless and derive all context from the `ToolContext`
/// - **Thread-Safe**: Tools must be `Send + Sync` to work in async environments
/// - **Self-Describing**: Tools provide their own schema and documentation
/// - **Error Handling**: Tools use structured error handling via `McpError`
///
/// # Implementation Guidelines
///
/// ## Tool Names
/// Tool names should follow the pattern `{domain}_{action}` (e.g., `memo_create`, `issue_list`).
/// Names must be unique within the registry and should be stable across versions.
///
/// ## Descriptions
/// Use `include_str!("description.md")` to load descriptions from separate Markdown files.
/// This improves maintainability and allows for rich documentation.
///
/// ## Schemas
/// Define comprehensive JSON schemas using the `serde_json::json!` macro. Include:
/// - Parameter types and descriptions
/// - Required vs optional parameters
/// - Validation constraints
/// - Examples in the description
///
/// ## Error Handling
/// Use `McpErrorHandler::handle_error()` to convert domain errors to MCP errors:
///
/// ```rust,ignore
/// match storage.create_memo(title, content).await {
///     Ok(memo) => Ok(BaseToolImpl::create_success_response(format!("Created: {}", memo.id))),
///     Err(e) => Err(McpErrorHandler::handle_error(e, "create memo")),
/// }
/// ```
///
/// ## Testing
/// Each tool should have comprehensive unit tests covering:
/// - Schema validation
/// - Success cases
/// - Error conditions
/// - Edge cases
#[async_trait::async_trait]
pub trait McpTool: Send + Sync {
    /// Get the tool's unique identifier name
    ///
    /// The name must be unique within the registry and should follow the
    /// `{domain}_{action}` pattern (e.g., `memo_create`, `issue_list`).
    /// Names should be stable across versions.
    fn name(&self) -> &'static str;

    /// Get the tool's human-readable description
    ///
    /// This description is shown to users in tool listings and help text.
    /// Consider using `include_str!("description.md")` to load descriptions
    /// from separate Markdown files for better maintainability.
    fn description(&self) -> &'static str;

    /// Get the tool's JSON schema for argument validation
    ///
    /// The schema should be a valid JSON Schema object defining the structure
    /// and validation rules for the tool's arguments. Include detailed
    /// descriptions for all parameters.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn schema(&self) -> serde_json::Value {
    ///     serde_json::json!({
    ///         "type": "object",
    ///         "properties": {
    ///             "title": {
    ///                 "type": "string",
    ///                 "description": "The memo title",
    ///                 "minLength": 1
    ///             }
    ///         },
    ///         "required": ["title"]
    ///     })
    /// }
    /// ```
    fn schema(&self) -> serde_json::Value;

    /// Execute the tool with the given arguments and context
    ///
    /// This is the main entry point for tool execution. The method receives:
    /// - `arguments`: Validated JSON arguments from the MCP client
    /// - `context`: Access to storage backends and services
    ///
    /// # Implementation Pattern
    ///
    /// 1. Parse arguments using `BaseToolImpl::parse_arguments()`
    /// 2. Validate business logic constraints
    /// 3. Perform the operation using context storage
    /// 4. Return structured response using `BaseToolImpl::create_success_response()`
    /// 5. Handle errors using `McpErrorHandler::handle_error()`
    ///
    /// # Error Handling
    ///
    /// Always use `McpErrorHandler::handle_error()` to convert domain errors
    /// to appropriate MCP errors for consistent client experience.
    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError>;
}

/// Registry for managing MCP tools
///
/// The `ToolRegistry` serves as the central repository for all MCP tools within
/// the application. It provides registration, lookup, and enumeration capabilities
/// for tools implementing the `McpTool` trait.
///
/// # Design Goals
///
/// - **Type Safety**: Tools are stored as trait objects with compile-time guarantees
/// - **Performance**: HashMap-based lookup provides O(1) tool resolution
/// - **Extensibility**: New tools can be registered dynamically at runtime
/// - **Memory Efficiency**: Tools are stored once and accessed by reference
///
/// # Usage Patterns
///
/// ## Registration
/// ```rust,ignore
/// let mut registry = ToolRegistry::new();
/// registry.register(MyTool::new());
/// registry.register(AnotherTool::new());
/// ```
///
/// ## Tool Execution
/// ```rust,ignore
/// if let Some(tool) = registry.get_tool("memo_create") {
///     let result = tool.execute(arguments, &context).await?;
///     // Handle result...
/// }
/// ```
///
/// ## MCP Integration
/// ```rust,ignore
/// // List all tools for MCP list_tools response
/// let tools = registry.list_tools();
/// ```
///
/// # Thread Safety
///
/// The registry itself is not thread-safe and should be protected by appropriate
/// synchronization when shared across threads. However, individual tools must
/// implement `Send + Sync` and can be safely called concurrently.
#[derive(Default)]
pub struct ToolRegistry {
    /// Internal storage mapping tool names to trait objects
    ///
    /// Uses HashMap for O(1) lookup performance. Tool names must be unique
    /// and are used as the primary key for tool resolution.
    tools: HashMap<String, Box<dyn McpTool>>,
}

impl ToolRegistry {
    /// Create a new empty tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool in the registry
    pub fn register<T: McpTool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        self.tools.insert(name, Box::new(tool));
    }

    /// Get a tool by name
    pub fn get_tool(&self, name: &str) -> Option<&dyn McpTool> {
        self.tools.get(name).map(|tool| tool.as_ref())
    }

    /// List all registered tool names
    pub fn list_tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get all registered tools as Tool objects for MCP list_tools response
    pub fn list_tools(&self) -> Vec<Tool> {
        self.tools
            .values()
            .map(|tool| {
                let schema = tool.schema();
                let schema_map = if let serde_json::Value::Object(map) = schema {
                    map
                } else {
                    serde_json::Map::new()
                };

                Tool {
                    name: tool.name().into(),
                    description: Some(tool.description().into()),
                    input_schema: std::sync::Arc::new(schema_map),
                    annotations: None,
                }
            })
            .collect()
    }

    /// Get the number of registered tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

/// Base implementation providing common utility methods for MCP tools
pub struct BaseToolImpl;

impl BaseToolImpl {
    /// Parse tool arguments from a JSON map into a typed struct
    ///
    /// # Arguments
    ///
    /// * `arguments` - The JSON map of arguments from the MCP request
    ///
    /// # Returns
    ///
    /// * `Result<T, McpError>` - The parsed arguments or an error
    pub fn parse_arguments<T: serde::de::DeserializeOwned>(
        arguments: serde_json::Map<String, serde_json::Value>,
    ) -> std::result::Result<T, McpError> {
        serde_json::from_value(serde_json::Value::Object(arguments))
            .map_err(|e| McpError::invalid_request(format!("Invalid arguments: {e}"), None))
    }

    /// Create a success response with serializable content
    ///
    /// # Arguments
    ///
    /// * `content` - The content to include in the response
    ///
    /// # Returns
    ///
    /// * `CallToolResult` - A success response
    pub fn create_success_response<T: Into<String>>(content: T) -> CallToolResult {
        CallToolResult {
            content: vec![Annotated::new(
                RawContent::Text(RawTextContent {
                    text: content.into(),
                }),
                None,
            )],
            is_error: Some(false),
        }
    }

    /// Create an error response with the given error message
    ///
    /// # Arguments
    ///
    /// * `error` - The error message
    /// * `details` - Optional additional details
    ///
    /// # Returns
    ///
    /// * `CallToolResult` - An error response
    pub fn create_error_response<T: Into<String>>(
        error: T,
        details: Option<String>,
    ) -> CallToolResult {
        let error_text = match details {
            Some(details) => format!("{}: {}", error.into(), details),
            None => error.into(),
        };

        CallToolResult {
            content: vec![Annotated::new(
                RawContent::Text(RawTextContent { text: error_text }),
                None,
            )],
            is_error: Some(true),
        }
    }
}

/// Tool registration functions for organizing tools by category
/// Register all issue-related tools with the registry
pub fn register_issue_tools(registry: &mut ToolRegistry) {
    use super::tools::issues;
    issues::register_issue_tools(registry);
}

/// Register all memo-related tools with the registry
pub fn register_memo_tools(registry: &mut ToolRegistry) {
    use super::tools::memoranda;
    memoranda::register_memoranda_tools(registry);
}

/// Register all search-related tools with the registry
pub fn register_search_tools(registry: &mut ToolRegistry) {
    use super::tools::search;
    search::register_search_tools(registry);
}

/// Register all outline-related tools with the registry
pub fn register_outline_tools(registry: &mut ToolRegistry) {
    use super::tools::outline;
    outline::register_outline_tools(registry);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{Annotated, RawContent, RawTextContent};

    /// Mock tool for testing
    struct MockTool {
        name: &'static str,
        description: &'static str,
    }

    #[async_trait::async_trait]
    impl McpTool for MockTool {
        fn name(&self) -> &'static str {
            self.name
        }

        fn description(&self) -> &'static str {
            self.description
        }

        fn schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            })
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, serde_json::Value>,
            _context: &ToolContext,
        ) -> std::result::Result<CallToolResult, McpError> {
            Ok(CallToolResult {
                content: vec![Annotated::new(
                    RawContent::Text(RawTextContent {
                        text: format!("Mock tool {} executed", self.name),
                    }),
                    None,
                )],
                is_error: Some(false),
            })
        }
    }

    #[test]
    fn test_tool_registry_creation() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_tool_registration() {
        let mut registry = ToolRegistry::new();
        let tool = MockTool {
            name: "test_tool",
            description: "A test tool",
        };

        registry.register(tool);

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
        assert!(registry.get_tool("test_tool").is_some());
        assert!(registry.get_tool("nonexistent").is_none());
    }

    #[test]
    fn test_tool_lookup() {
        let mut registry = ToolRegistry::new();
        let tool = MockTool {
            name: "lookup_test",
            description: "A lookup test tool",
        };

        registry.register(tool);

        let retrieved_tool = registry.get_tool("lookup_test").unwrap();
        assert_eq!(retrieved_tool.name(), "lookup_test");
        assert_eq!(retrieved_tool.description(), "A lookup test tool");
    }

    #[test]
    fn test_multiple_tool_registration() {
        let mut registry = ToolRegistry::new();

        let tool1 = MockTool {
            name: "tool1",
            description: "First tool",
        };
        let tool2 = MockTool {
            name: "tool2",
            description: "Second tool",
        };

        registry.register(tool1);
        registry.register(tool2);

        assert_eq!(registry.len(), 2);
        assert!(registry.get_tool("tool1").is_some());
        assert!(registry.get_tool("tool2").is_some());

        let tool_names = registry.list_tool_names();
        assert!(tool_names.contains(&"tool1".to_string()));
        assert!(tool_names.contains(&"tool2".to_string()));
    }

    #[tokio::test]
    async fn test_tool_execution() {
        use std::path::PathBuf;
        use swissarmyhammer::git::GitOperations;
        use swissarmyhammer::issues::IssueStorage;
        use swissarmyhammer::memoranda::{mock_storage::MockMemoStorage, MemoStorage};
        use tokio::sync::{Mutex, RwLock};

        // Create mock storage and handlers for context
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

        let tool = MockTool {
            name: "exec_test",
            description: "Execution test tool",
        };

        let result = tool.execute(serde_json::Map::new(), &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }

    #[test]
    fn test_base_tool_impl_parse_arguments() {
        use serde::Deserialize;

        #[derive(Deserialize, PartialEq, Debug)]
        struct TestArgs {
            name: String,
            count: Option<i32>,
        }

        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("test".to_string()),
        );
        args.insert(
            "count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(42)),
        );

        let parsed: TestArgs = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.count, Some(42));
    }

    #[test]
    fn test_base_tool_impl_parse_arguments_error() {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct TestArgs {
            #[serde(rename = "required_field")]
            _required_field: String,
        }

        let args = serde_json::Map::new(); // Missing required field

        let result: std::result::Result<TestArgs, McpError> = BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_base_tool_impl_create_success_response() {
        let response = BaseToolImpl::create_success_response("Success message");

        assert_eq!(response.is_error, Some(false));
        assert_eq!(response.content.len(), 1);

        if let RawContent::Text(text_content) = &response.content[0].raw {
            assert_eq!(text_content.text, "Success message");
        } else {
            panic!("Expected text content");
        }
    }

    #[test]
    fn test_base_tool_impl_create_error_response() {
        let response = BaseToolImpl::create_error_response("Error message", None);

        assert_eq!(response.is_error, Some(true));
        assert_eq!(response.content.len(), 1);

        if let RawContent::Text(text_content) = &response.content[0].raw {
            assert_eq!(text_content.text, "Error message");
        } else {
            panic!("Expected text content");
        }
    }

    #[test]
    fn test_base_tool_impl_create_error_response_with_details() {
        let response = BaseToolImpl::create_error_response(
            "Error message",
            Some("Additional details".to_string()),
        );

        assert_eq!(response.is_error, Some(true));
        assert_eq!(response.content.len(), 1);

        if let RawContent::Text(text_content) = &response.content[0].raw {
            assert_eq!(text_content.text, "Error message: Additional details");
        } else {
            panic!("Expected text content");
        }
    }
}
