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

    /// Get the CLI category for grouping tools
    ///
    /// Returns the category name used to organize tools into CLI subcommands.
    /// For example, "issue" groups issue-related tools, "memo" groups memo-related tools.
    ///
    /// # Returns
    ///
    /// * `Some(category)` - Tool belongs to the specified category subcommand
    /// * `None` - Tool appears at the root level (default)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// fn cli_category(&self) -> Option<&'static str> { Some("issue") }  // sah issue create
    /// fn cli_category(&self) -> Option<&'static str> { Some("memo") }   // sah memo list  
    /// fn cli_category(&self) -> Option<&'static str> { None }           // sah my_tool
    /// ```
    fn cli_category(&self) -> Option<&'static str> {
        None
    }

    /// Get the CLI command name
    ///
    /// Returns the command name to use in the CLI interface. By default, this
    /// returns the MCP tool name, but can be overridden to provide CLI-specific
    /// naming that follows kebab-case conventions.
    ///
    /// # Returns
    ///
    /// The command name as a static string reference
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// fn cli_name(&self) -> &'static str { "create" }      // Use custom CLI name
    /// fn cli_name(&self) -> &'static str { self.name() }   // Use MCP tool name (default)
    /// ```
    fn cli_name(&self) -> &'static str {
        self.name()
    }

    /// Get CLI-specific help text
    ///
    /// Returns help text specifically tailored for CLI usage. If not provided,
    /// the CLI will fall back to using the tool's description().
    ///
    /// # Returns
    ///
    /// * `Some(help_text)` - Custom CLI help text
    /// * `None` - Use description() for CLI help (default)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// fn cli_about(&self) -> Option<&'static str> {
    ///     Some("Create a new issue with automatic numbering")
    /// }
    /// ```
    fn cli_about(&self) -> Option<&'static str> {
        None
    }

    /// Control visibility in CLI
    ///
    /// Returns whether this tool should be hidden from CLI command generation.
    /// Useful for MCP-only tools, internal tools, or tools that shouldn't be
    /// exposed directly to CLI users.
    ///
    /// # Returns
    ///
    /// * `true` - Hide from CLI (tool is MCP-only)
    /// * `false` - Show in CLI (default)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// fn hidden_from_cli(&self) -> bool { true }   // MCP-only tool
    /// fn hidden_from_cli(&self) -> bool { false }  // Available in CLI (default)
    /// ```
    fn hidden_from_cli(&self) -> bool {
        false
    }

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

    /// Get all CLI categories from registered tools
    ///
    /// Returns a sorted list of all unique categories from tools that are visible in the CLI.
    /// Hidden tools are excluded from the results.
    ///
    /// # Returns
    ///
    /// A vector of category names sorted alphabetically
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let categories = registry.get_cli_categories();
    /// // Returns: ["issue", "memo", "search"]
    /// ```
    pub fn get_cli_categories(&self) -> Vec<String> {
        let mut categories = std::collections::HashSet::new();

        for tool in self.tools.values() {
            if let Some(category) = tool.cli_category() {
                if !tool.hidden_from_cli() {
                    categories.insert(category.to_string());
                }
            }
        }

        let mut result: Vec<String> = categories.into_iter().collect();
        result.sort();
        result
    }

    /// Get all tools for a specific CLI category
    ///
    /// Returns all tools that belong to the specified category and are visible in the CLI.
    /// Hidden tools are excluded from the results.
    ///
    /// # Arguments
    ///
    /// * `category` - The category name to filter by
    ///
    /// # Returns
    ///
    /// A vector of tool references for the specified category
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let issue_tools = registry.get_tools_for_category("issue");
    /// // Returns all issue-related tools like issue_create, issue_list, etc.
    /// ```
    pub fn get_tools_for_category(&self, category: &str) -> Vec<&dyn McpTool> {
        self.tools
            .values()
            .filter(|tool| tool.cli_category() == Some(category) && !tool.hidden_from_cli())
            .map(|tool| tool.as_ref())
            .collect()
    }

    /// Get all CLI-visible tools without a category (root level tools)
    ///
    /// Returns all tools that don't belong to a specific category and are visible in the CLI.
    /// These tools appear at the root command level rather than under a category subcommand.
    ///
    /// # Returns
    ///
    /// A vector of tool references for root-level tools
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let root_tools = registry.get_root_cli_tools();
    /// // Returns tools that are called like: sah tool_name
    /// ```
    pub fn get_root_cli_tools(&self) -> Vec<&dyn McpTool> {
        self.tools
            .values()
            .filter(|tool| tool.cli_category().is_none() && !tool.hidden_from_cli())
            .map(|tool| tool.as_ref())
            .collect()
    }

    /// Get a tool by CLI path (category/name or just name for root tools)
    ///
    /// Resolves a CLI path to a tool reference. Supports both categorized tools
    /// (category/name format) and root-level tools (name only).
    ///
    /// # Arguments
    ///
    /// * `cli_path` - The CLI path to resolve (e.g., "issue/create" or "my_tool")
    ///
    /// # Returns
    ///
    /// * `Some(tool)` - The tool if found and visible in CLI
    /// * `None` - Tool not found or hidden from CLI
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let tool = registry.get_tool_by_cli_path("issue/create");  // Categorized tool
    /// let tool = registry.get_tool_by_cli_path("my_tool");       // Root tool
    /// ```
    pub fn get_tool_by_cli_path(&self, cli_path: &str) -> Option<&dyn McpTool> {
        // Handle category/name format
        if let Some((category, name)) = cli_path.split_once('/') {
            return self
                .get_tools_for_category(category)
                .into_iter()
                .find(|tool| tool.cli_name() == name);
        }

        // Handle root-level tools
        self.get_root_cli_tools()
            .into_iter()
            .find(|tool| tool.cli_name() == cli_path)
    }

    /// Collect CLI metadata for all visible tools
    ///
    /// Returns a vector of CliToolMetadata structs containing all the information
    /// needed to integrate tools with CLI systems. Hidden tools are excluded.
    ///
    /// # Returns
    ///
    /// A vector of metadata structs for CLI-visible tools
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let metadata = registry.get_cli_metadata();
    /// for meta in metadata {
    ///     println!("Tool: {}, Category: {:?}", meta.name, meta.category);
    /// }
    /// ```
    pub fn get_cli_metadata(&self) -> Vec<CliToolMetadata> {
        self.tools
            .values()
            .filter(|tool| !tool.hidden_from_cli())
            .map(|tool| CliToolMetadata {
                name: tool.cli_name().to_string(),
                category: tool.cli_category().map(|s| s.to_string()),
                about: tool
                    .cli_about()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| tool.description().to_string()),
                schema: tool.schema(),
                mcp_name: tool.name().to_string(),
            })
            .collect()
    }
}

/// Metadata for CLI tool integration
///
/// Contains all the information needed to integrate an MCP tool with the CLI system.
/// This struct captures both MCP-specific and CLI-specific metadata in a unified format.
///
/// # Fields
///
/// * `name` - CLI command name (from cli_name())
/// * `category` - Optional CLI category for grouping (from cli_category())  
/// * `about` - CLI-specific help text (from cli_about() or description())
/// * `schema` - JSON schema for argument validation (from schema())
/// * `mcp_name` - Original MCP tool name for internal lookups (from name())
///
/// # Usage
///
/// This metadata is primarily used by CLI command generators to create dynamic
/// command structures based on registered MCP tools.
#[derive(Debug, Clone)]
pub struct CliToolMetadata {
    /// CLI command name (may differ from MCP name)
    pub name: String,

    /// Optional category for CLI subcommand grouping
    pub category: Option<String>,

    /// CLI help text (uses cli_about() if available, falls back to description())
    pub about: String,

    /// JSON schema for argument validation
    pub schema: serde_json::Value,

    /// Original MCP tool name for registry lookups
    pub mcp_name: String,
}

/// Builder pattern for easier CLI integration with ToolRegistry
///
/// The `CliRegistryBuilder` provides a convenient interface for CLI systems to
/// interact with the tool registry. It wraps a ToolRegistry and provides
/// methods specifically designed for CLI use cases.
///
/// # Design Goals
///
/// - **Convenience**: Simplified interface for common CLI operations
/// - **Encapsulation**: Hides ToolRegistry complexity from CLI systems
/// - **Consistency**: Provides uniform access patterns for CLI builders
/// - **Performance**: Delegates to ToolRegistry methods without additional overhead
///
/// # Usage Patterns
///
/// ```rust,ignore
/// let builder = CliRegistryBuilder::new(registry);
///
/// // Get all categories for subcommand generation
/// let categories = builder.categories();
///
/// // Generate commands for a specific category
/// for category in &categories {
///     let tools = builder.tools_in_category(category);
///     // Generate CLI commands for tools...
/// }
///
/// // Handle root-level commands
/// let root_tools = builder.root_tools();
/// ```
pub struct CliRegistryBuilder<'a> {
    /// Reference to the underlying tool registry
    registry: &'a ToolRegistry,
}

impl<'a> CliRegistryBuilder<'a> {
    /// Create a new CLI registry builder
    ///
    /// # Arguments
    ///
    /// * `registry` - Reference to the tool registry to wrap
    ///
    /// # Returns
    ///
    /// A new builder instance for CLI integration
    pub fn new(registry: &'a ToolRegistry) -> Self {
        Self { registry }
    }

    /// Get all CLI categories
    ///
    /// Convenience method that delegates to the registry's get_cli_categories().
    /// Returns a sorted list of all unique categories from CLI-visible tools.
    ///
    /// # Returns
    ///
    /// A vector of category names sorted alphabetically
    pub fn categories(&self) -> Vec<String> {
        self.registry.get_cli_categories()
    }

    /// Get tools for a specific category
    ///
    /// Convenience method that delegates to the registry's get_tools_for_category().
    /// Returns all CLI-visible tools that belong to the specified category.
    ///
    /// # Arguments
    ///
    /// * `category` - The category name to filter by
    ///
    /// # Returns
    ///
    /// A vector of tool references for the specified category
    pub fn tools_in_category(&self, category: &str) -> Vec<&dyn McpTool> {
        self.registry.get_tools_for_category(category)
    }

    /// Get root-level tools (no category)
    ///
    /// Convenience method that delegates to the registry's get_root_cli_tools().
    /// Returns all CLI-visible tools that don't belong to a specific category.
    ///
    /// # Returns
    ///
    /// A vector of tool references for root-level tools
    pub fn root_tools(&self) -> Vec<&dyn McpTool> {
        self.registry.get_root_cli_tools()
    }

    /// Get all CLI metadata
    ///
    /// Convenience method that delegates to the registry's get_cli_metadata().
    /// Returns metadata for all CLI-visible tools.
    ///
    /// # Returns
    ///
    /// A vector of CLI tool metadata structs
    pub fn metadata(&self) -> Vec<CliToolMetadata> {
        self.registry.get_cli_metadata()
    }

    /// Find a tool by CLI path
    ///
    /// Convenience method that delegates to the registry's get_tool_by_cli_path().
    /// Resolves a CLI path to a tool reference.
    ///
    /// # Arguments
    ///
    /// * `cli_path` - The CLI path to resolve (e.g., "issue/create" or "my_tool")
    ///
    /// # Returns
    ///
    /// * `Some(tool)` - The tool if found and visible in CLI
    /// * `None` - Tool not found or hidden from CLI
    pub fn find_tool(&self, cli_path: &str) -> Option<&dyn McpTool> {
        self.registry.get_tool_by_cli_path(cli_path)
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
/// Register all abort-related tools with the registry
pub fn register_abort_tools(registry: &mut ToolRegistry) {
    use super::tools::abort;
    abort::register_abort_tools(registry);
}

/// Register all file-related tools with the registry
pub fn register_file_tools(registry: &mut ToolRegistry) {
    use super::tools::files;
    files::register_file_tools(registry);
}

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

/// Register all notification-related tools with the registry
pub fn register_notify_tools(registry: &mut ToolRegistry) {
    use super::tools::notify;
    notify::register_notify_tools(registry);
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

/// Register all shell-related tools with the registry
pub fn register_shell_tools(registry: &mut ToolRegistry) {
    use super::tools::shell;
    shell::register_shell_tools(registry);
}

/// Register all todo-related tools with the registry
pub fn register_todo_tools(registry: &mut ToolRegistry) {
    use super::tools::todo;
    todo::register_todo_tools(registry);
}

/// Register all web fetch-related tools with the registry
pub fn register_web_fetch_tools(registry: &mut ToolRegistry) {
    use super::tools::web_fetch;
    web_fetch::register_web_fetch_tools(registry);
}

/// Register all web search-related tools with the registry
pub fn register_web_search_tools(registry: &mut ToolRegistry) {
    use super::tools::web_search;
    web_search::register_web_search_tools(registry);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{Annotated, RawContent, RawTextContent};
    use std::iter::Iterator;

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

    /// Test tool that implements all CLI methods for testing
    struct TestCliTool {
        name: &'static str,
        category: Option<&'static str>,
        cli_name: &'static str,
        cli_about: Option<&'static str>,
        hidden: bool,
    }

    #[async_trait::async_trait]
    impl McpTool for TestCliTool {
        fn name(&self) -> &'static str {
            self.name
        }

        fn description(&self) -> &'static str {
            "Test tool description"
        }

        fn schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            })
        }

        fn cli_category(&self) -> Option<&'static str> {
            self.category
        }

        fn cli_name(&self) -> &'static str {
            self.cli_name
        }

        fn cli_about(&self) -> Option<&'static str> {
            self.cli_about
        }

        fn hidden_from_cli(&self) -> bool {
            self.hidden
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, serde_json::Value>,
            _context: &ToolContext,
        ) -> std::result::Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response("Test executed"))
        }
    }

    #[test]
    fn test_default_cli_methods() {
        let tool = MockTool {
            name: "test_defaults",
            description: "Test tool",
        };

        // Test default implementations
        assert_eq!(tool.cli_category(), None);
        assert_eq!(tool.cli_name(), "test_defaults"); // Should match name()
        assert_eq!(tool.cli_about(), None);
        assert!(!tool.hidden_from_cli());
    }

    #[test]
    fn test_custom_cli_methods() {
        let tool = TestCliTool {
            name: "test_tool",
            category: Some("test"),
            cli_name: "custom-name",
            cli_about: Some("Custom CLI help text"),
            hidden: false,
        };

        // Test custom implementations
        assert_eq!(tool.cli_category(), Some("test"));
        assert_eq!(tool.cli_name(), "custom-name");
        assert_eq!(tool.cli_about(), Some("Custom CLI help text"));
        assert!(!tool.hidden_from_cli());
    }

    #[test]
    fn test_hidden_cli_tool() {
        let tool = TestCliTool {
            name: "hidden_tool",
            category: None,
            cli_name: "hidden",
            cli_about: None,
            hidden: true,
        };

        // Test hidden tool
        assert_eq!(tool.cli_category(), None);
        assert_eq!(tool.cli_name(), "hidden");
        assert_eq!(tool.cli_about(), None);
        assert!(tool.hidden_from_cli());
    }

    #[test]
    fn test_cli_categorized_tool() {
        let tool = TestCliTool {
            name: "categorized_tool",
            category: Some("category"),
            cli_name: "categorized",
            cli_about: Some("Categorized tool help"),
            hidden: false,
        };

        // Test categorized tool
        assert_eq!(tool.cli_category(), Some("category"));
        assert_eq!(tool.cli_name(), "categorized");
        assert_eq!(tool.cli_about(), Some("Categorized tool help"));
        assert!(!tool.hidden_from_cli());
    }

    #[test]
    fn test_cli_name_defaults_to_tool_name() {
        let tool = MockTool {
            name: "mcp_tool_name",
            description: "Test tool",
        };

        // CLI name should default to MCP tool name
        assert_eq!(tool.cli_name(), tool.name());
        assert_eq!(tool.cli_name(), "mcp_tool_name");
    }

    #[tokio::test]
    async fn test_cli_tool_execution() {
        use std::path::PathBuf;
        use swissarmyhammer::git::GitOperations;
        use swissarmyhammer::issues::IssueStorage;
        use swissarmyhammer::memoranda::{mock_storage::MockMemoStorage, MemoStorage};
        use tokio::sync::{Mutex, RwLock};

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

        let tool = TestCliTool {
            name: "exec_test",
            category: Some("test"),
            cli_name: "execute",
            cli_about: Some("Test execution"),
            hidden: false,
        };

        // Test that CLI tools can execute normally
        let result = tool.execute(serde_json::Map::new(), &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));

        if let RawContent::Text(text_content) = &call_result.content[0].raw {
            assert_eq!(text_content.text, "Test executed");
        }
    }

    #[test]
    fn test_cli_method_return_types() {
        let tool = TestCliTool {
            name: "type_test",
            category: Some("types"),
            cli_name: "types",
            cli_about: Some("Type testing"),
            hidden: false,
        };

        // Verify return types match trait signature
        let _category: Option<&'static str> = tool.cli_category();
        let _name: &'static str = tool.cli_name();
        let _about: Option<&'static str> = tool.cli_about();
        let _hidden: bool = tool.hidden_from_cli();

        // Test passes if compilation succeeds - validates trait signature compatibility
    }

    // Tests for CLI integration methods

    fn create_test_registry_with_cli_tools() -> ToolRegistry {
        let mut registry = ToolRegistry::new();

        // Add categorized tools
        registry.register(TestCliTool {
            name: "issue_create",
            category: Some("issue"),
            cli_name: "create",
            cli_about: Some("Create a new issue"),
            hidden: false,
        });

        registry.register(TestCliTool {
            name: "issue_list",
            category: Some("issue"),
            cli_name: "list",
            cli_about: None, // Will use description
            hidden: false,
        });

        registry.register(TestCliTool {
            name: "memo_create",
            category: Some("memo"),
            cli_name: "create",
            cli_about: Some("Create a new memo"),
            hidden: false,
        });

        // Add root-level tool
        registry.register(TestCliTool {
            name: "search_files",
            category: None,
            cli_name: "search",
            cli_about: Some("Search through files"),
            hidden: false,
        });

        // Add hidden tool (should be excluded)
        registry.register(TestCliTool {
            name: "internal_tool",
            category: Some("internal"),
            cli_name: "internal",
            cli_about: Some("Internal tool"),
            hidden: true,
        });

        registry
    }

    #[test]
    fn test_get_cli_categories() {
        let registry = create_test_registry_with_cli_tools();
        let categories = registry.get_cli_categories();

        // Should contain visible categories only
        assert_eq!(categories, vec!["issue", "memo"]);

        // Should be sorted
        let mut expected_sorted = categories.clone();
        expected_sorted.sort();
        assert_eq!(categories, expected_sorted);

        // Should not contain categories from hidden tools
        assert!(!categories.contains(&"internal".to_string()));
    }

    #[test]
    fn test_get_cli_categories_empty_registry() {
        let registry = ToolRegistry::new();
        let categories = registry.get_cli_categories();
        assert!(categories.is_empty());
    }

    #[test]
    fn test_get_cli_categories_only_hidden_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(TestCliTool {
            name: "hidden1",
            category: Some("category1"),
            cli_name: "hidden1",
            cli_about: None,
            hidden: true,
        });

        let categories = registry.get_cli_categories();
        assert!(categories.is_empty());
    }

    #[test]
    fn test_get_tools_for_category() {
        let registry = create_test_registry_with_cli_tools();
        let issue_tools = registry.get_tools_for_category("issue");

        assert_eq!(issue_tools.len(), 2);

        let tool_names: Vec<&str> = issue_tools.iter().map(|tool| tool.cli_name()).collect();
        assert!(tool_names.contains(&"create"));
        assert!(tool_names.contains(&"list"));

        // Verify all tools are in the correct category
        for tool in &issue_tools {
            assert_eq!(tool.cli_category(), Some("issue"));
            assert!(!tool.hidden_from_cli());
        }
    }

    #[test]
    fn test_get_tools_for_category_nonexistent() {
        let registry = create_test_registry_with_cli_tools();
        let tools = registry.get_tools_for_category("nonexistent");
        assert!(tools.is_empty());
    }

    #[test]
    fn test_get_tools_for_category_excludes_hidden() {
        let registry = create_test_registry_with_cli_tools();
        let internal_tools = registry.get_tools_for_category("internal");

        // Should be empty because internal_tool is hidden
        assert!(internal_tools.is_empty());
    }

    #[test]
    fn test_get_root_cli_tools() {
        let registry = create_test_registry_with_cli_tools();
        let root_tools = registry.get_root_cli_tools();

        assert_eq!(root_tools.len(), 1);
        assert_eq!(root_tools[0].cli_name(), "search");
        assert_eq!(root_tools[0].cli_category(), None);
        assert!(!root_tools[0].hidden_from_cli());
    }

    #[test]
    fn test_get_root_cli_tools_empty() {
        let mut registry = ToolRegistry::new();

        // Add only categorized tools
        registry.register(TestCliTool {
            name: "categorized",
            category: Some("category"),
            cli_name: "categorized",
            cli_about: None,
            hidden: false,
        });

        let root_tools = registry.get_root_cli_tools();
        assert!(root_tools.is_empty());
    }

    #[test]
    fn test_get_tool_by_cli_path_categorized() {
        let registry = create_test_registry_with_cli_tools();

        // Test category/name format
        let tool = registry.get_tool_by_cli_path("issue/create");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().cli_name(), "create");
        assert_eq!(tool.unwrap().cli_category(), Some("issue"));

        let tool = registry.get_tool_by_cli_path("memo/create");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().cli_name(), "create");
        assert_eq!(tool.unwrap().cli_category(), Some("memo"));
    }

    #[test]
    fn test_get_tool_by_cli_path_root() {
        let registry = create_test_registry_with_cli_tools();

        // Test root tool lookup
        let tool = registry.get_tool_by_cli_path("search");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().cli_name(), "search");
        assert_eq!(tool.unwrap().cli_category(), None);
    }

    #[test]
    fn test_get_tool_by_cli_path_not_found() {
        let registry = create_test_registry_with_cli_tools();

        // Test nonexistent paths
        assert!(registry.get_tool_by_cli_path("nonexistent/tool").is_none());
        assert!(registry.get_tool_by_cli_path("issue/nonexistent").is_none());
        assert!(registry.get_tool_by_cli_path("nonexistent").is_none());
    }

    #[test]
    fn test_get_tool_by_cli_path_hidden_tool() {
        let registry = create_test_registry_with_cli_tools();

        // Hidden tools should not be found
        assert!(registry.get_tool_by_cli_path("internal/internal").is_none());
    }

    #[test]
    fn test_get_cli_metadata() {
        let registry = create_test_registry_with_cli_tools();
        let metadata = registry.get_cli_metadata();

        // Should have 4 visible tools (excluding hidden)
        assert_eq!(metadata.len(), 4);

        // Find specific tools and verify metadata
        let issue_create = metadata
            .iter()
            .find(|m| m.mcp_name == "issue_create")
            .expect("issue_create should be present");

        assert_eq!(issue_create.name, "create");
        assert_eq!(issue_create.category, Some("issue".to_string()));
        assert_eq!(issue_create.about, "Create a new issue");
        assert_eq!(issue_create.mcp_name, "issue_create");

        // Test tool with no cli_about (should use description)
        let issue_list = metadata
            .iter()
            .find(|m| m.mcp_name == "issue_list")
            .expect("issue_list should be present");

        assert_eq!(issue_list.about, "Test tool description"); // Falls back to description()

        // Test root tool
        let search = metadata
            .iter()
            .find(|m| m.mcp_name == "search_files")
            .expect("search_files should be present");

        assert_eq!(search.category, None);
        assert_eq!(search.name, "search");

        // Verify hidden tools are excluded
        assert!(metadata.iter().all(|m| m.mcp_name != "internal_tool"));
    }

    #[test]
    fn test_get_cli_metadata_empty_registry() {
        let registry = ToolRegistry::new();
        let metadata = registry.get_cli_metadata();
        assert!(metadata.is_empty());
    }

    // Tests for CliRegistryBuilder

    #[test]
    fn test_cli_registry_builder_categories() {
        let registry = create_test_registry_with_cli_tools();
        let builder = CliRegistryBuilder::new(&registry);

        let categories = builder.categories();
        assert_eq!(categories, vec!["issue", "memo"]);
    }

    #[test]
    fn test_cli_registry_builder_tools_in_category() {
        let registry = create_test_registry_with_cli_tools();
        let builder = CliRegistryBuilder::new(&registry);

        let issue_tools = builder.tools_in_category("issue");
        assert_eq!(issue_tools.len(), 2);

        let tool_names: Vec<&str> = issue_tools.iter().map(|tool| tool.cli_name()).collect();
        assert!(tool_names.contains(&"create"));
        assert!(tool_names.contains(&"list"));
    }

    #[test]
    fn test_cli_registry_builder_root_tools() {
        let registry = create_test_registry_with_cli_tools();
        let builder = CliRegistryBuilder::new(&registry);

        let root_tools = builder.root_tools();
        assert_eq!(root_tools.len(), 1);
        assert_eq!(root_tools[0].cli_name(), "search");
    }

    #[test]
    fn test_cli_registry_builder_metadata() {
        let registry = create_test_registry_with_cli_tools();
        let builder = CliRegistryBuilder::new(&registry);

        let metadata = builder.metadata();
        assert_eq!(metadata.len(), 4); // 4 visible tools
    }

    #[test]
    fn test_cli_registry_builder_find_tool() {
        let registry = create_test_registry_with_cli_tools();
        let builder = CliRegistryBuilder::new(&registry);

        // Test categorized tool lookup
        let tool = builder.find_tool("issue/create");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().cli_name(), "create");

        // Test root tool lookup
        let tool = builder.find_tool("search");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().cli_name(), "search");

        // Test not found
        assert!(builder.find_tool("nonexistent").is_none());
    }

    #[test]
    fn test_cli_tool_metadata_structure() {
        let metadata = CliToolMetadata {
            name: "test-name".to_string(),
            category: Some("test-category".to_string()),
            about: "Test about text".to_string(),
            schema: serde_json::json!({"type": "object"}),
            mcp_name: "test_mcp_name".to_string(),
        };

        // Verify all fields are accessible
        assert_eq!(metadata.name, "test-name");
        assert_eq!(metadata.category, Some("test-category".to_string()));
        assert_eq!(metadata.about, "Test about text");
        assert_eq!(metadata.mcp_name, "test_mcp_name");

        // Verify schema is preserved
        if let serde_json::Value::Object(obj) = &metadata.schema {
            assert!(obj.contains_key("type"));
        } else {
            panic!("Schema should be an object");
        }
    }

    #[test]
    fn test_integration_with_existing_registration() {
        let mut registry = ToolRegistry::new();

        // Register tools using existing pattern
        registry.register(MockTool {
            name: "existing_tool",
            description: "Existing tool",
        });

        // Add CLI-aware tools
        registry.register(TestCliTool {
            name: "cli_tool",
            category: Some("test"),
            cli_name: "cli",
            cli_about: Some("CLI tool"),
            hidden: false,
        });

        // Verify both types work together
        assert_eq!(registry.len(), 2);
        assert!(registry.get_tool("existing_tool").is_some());
        assert!(registry.get_tool("cli_tool").is_some());

        // CLI methods should work with both
        let categories = registry.get_cli_categories();
        assert_eq!(categories, vec!["test"]); // Only cli_tool has category

        let root_tools = registry.get_root_cli_tools();
        assert_eq!(root_tools.len(), 1); // existing_tool (no category, not hidden)

        let metadata = registry.get_cli_metadata();
        assert_eq!(metadata.len(), 2); // Both tools visible
    }

    /// Create a full registry with all production tools for testing
    fn create_full_tool_registry() -> ToolRegistry {
        let mut registry = ToolRegistry::new();

        // Register all tool categories
        register_issue_tools(&mut registry);
        register_memo_tools(&mut registry);
        register_file_tools(&mut registry);
        register_search_tools(&mut registry);
        register_shell_tools(&mut registry);
        register_web_search_tools(&mut registry);
        register_outline_tools(&mut registry);
        register_todo_tools(&mut registry);
        register_notify_tools(&mut registry);
        register_abort_tools(&mut registry);
        register_web_fetch_tools(&mut registry);

        registry
    }

    #[test]
    fn test_all_visible_tools_have_cli_categories() {
        let registry = create_full_tool_registry();
        let tools: Vec<&dyn McpTool> = registry.tools.values().map(|t| t.as_ref()).collect();

        for tool in tools {
            if !tool.hidden_from_cli() {
                assert!(
                    tool.cli_category().is_some(),
                    "Tool '{}' is visible in CLI but has no category. All visible tools should have a category for proper CLI organization.",
                    tool.name()
                );
            }
        }
    }

    #[test]
    fn test_hidden_tools_are_properly_marked() {
        let registry = create_full_tool_registry();

        // Tools that should be hidden from CLI (internal/workflow tools)
        let hidden_tool_patterns = [
            "todo_",     // All todo tools are for internal workflow use
            "notify_",   // Notification tools are for internal use
            "abort_",    // Abort tools are for internal workflow control
            "web_fetch", // Web fetch is internal, web_search is user-facing
        ];

        for (name, tool) in &registry.tools {
            let should_be_hidden = hidden_tool_patterns
                .iter()
                .any(|pattern| name.starts_with(pattern));

            if should_be_hidden {
                assert!(
                    tool.hidden_from_cli(),
                    "Tool '{name}' should be hidden from CLI but isn't. Internal tools should not be exposed in CLI."
                );
            }
        }
    }

    #[test]
    fn test_cli_naming_conventions() {
        let registry = create_full_tool_registry();

        for tool in registry.tools.values() {
            if !tool.hidden_from_cli() {
                let cli_name = tool.cli_name();

                // CLI names should not be empty
                assert!(
                    !cli_name.is_empty(),
                    "Tool '{}' has empty CLI name",
                    tool.name()
                );

                // CLI names should not contain underscores (use kebab-case)
                assert!(
                    !cli_name.contains('_'),
                    "Tool '{}' CLI name '{}' contains underscores. Use kebab-case for CLI commands.",
                    tool.name(),
                    cli_name
                );

                // CLI names should be reasonable length
                assert!(
                    cli_name.len() <= 20,
                    "Tool '{}' CLI name '{}' is too long ({}). Keep CLI commands concise.",
                    tool.name(),
                    cli_name,
                    cli_name.len()
                );
            }
        }
    }

    #[test]
    fn test_no_cli_naming_conflicts_within_categories() {
        let registry = create_full_tool_registry();
        let categories = registry.get_cli_categories();

        for category in categories {
            let tools = registry.get_tools_for_category(&category);
            let mut cli_names = std::collections::HashSet::new();

            for tool in tools {
                let cli_name = tool.cli_name();
                assert!(
                    cli_names.insert(cli_name),
                    "Duplicate CLI name '{cli_name}' found in category '{category}'. Each tool in a category must have a unique CLI name."
                );
            }
        }
    }

    #[test]
    fn test_expected_tool_categories_exist() {
        let registry = create_full_tool_registry();
        let categories = registry.get_cli_categories();

        // Expected categories based on our tool organization
        let expected_categories = [
            "issue",
            "memo",
            "file",
            "search",
            "shell",
            "web-search",
            "outline",
        ];

        for expected in &expected_categories {
            assert!(
                categories.contains(&expected.to_string()),
                "Expected CLI category '{expected}' not found. Available categories: {categories:?}"
            );
        }
    }

    #[test]
    fn test_cli_about_text_quality() {
        let registry = create_full_tool_registry();

        for tool in registry.tools.values() {
            if !tool.hidden_from_cli() {
                if let Some(cli_about) = tool.cli_about() {
                    // CLI about text should not be empty
                    assert!(
                        !cli_about.trim().is_empty(),
                        "Tool '{}' has empty CLI about text",
                        tool.name()
                    );

                    // Should be reasonably concise for CLI help
                    assert!(
                        cli_about.len() <= 100,
                        "Tool '{}' CLI about text is too long ({}): '{}'. Keep help text concise for CLI.",
                        tool.name(),
                        cli_about.len(),
                        cli_about
                    );

                    // Should be different from description to add value
                    let description = tool.description();
                    assert!(
                        cli_about != description,
                        "Tool '{}' CLI about text is identical to description. CLI help should be tailored for CLI users.",
                        tool.name()
                    );
                }
            }
        }
    }

    #[test]
    fn test_expected_tool_counts_per_category() {
        let registry = create_full_tool_registry();

        // Verify we have the expected number of tools in each category
        // This helps catch missing tools or incorrectly categorized tools
        let category_expectations = [
            ("issue", 8),  // create, list, show, update, work, merge, mark_complete, all_complete
            ("memo", 7),   // create, list, get, update, delete, search, get_all_context
            ("file", 5),   // read, write, edit, glob, grep
            ("search", 2), // index, query
            ("shell", 1),  // exec
            ("web-search", 1), // search
            ("outline", 1), // generate
        ];

        for (category, expected_count) in &category_expectations {
            let tools = registry.get_tools_for_category(category);
            assert_eq!(
                tools.len(),
                *expected_count,
                "Category '{}' has {} tools, expected {}. Tools: {:?}",
                category,
                tools.len(),
                expected_count,
                tools.iter().map(|t| t.cli_name()).collect::<Vec<_>>()
            );
        }
    }
}
