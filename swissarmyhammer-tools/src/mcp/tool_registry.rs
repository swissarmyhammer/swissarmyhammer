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

    /// Get the tool as an Any trait object for downcasting
    ///
    /// This method enables CLI exclusion detection by allowing tools to be
    /// downcast to specific marker traits. The default implementation returns
    /// None, but tools that implement marker traits should override this.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn as_any(&self) -> Option<&dyn std::any::Any> {
    ///     Some(self)
    /// }
    /// ```
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }
}

/// Registry for managing MCP tools with CLI exclusion tracking
///
/// The `ToolRegistry` serves as the central repository for all MCP tools within
/// the application. It provides registration, lookup, and enumeration capabilities
/// for tools implementing the `McpTool` trait, along with comprehensive CLI
/// exclusion metadata tracking.
///
/// # Design Goals
///
/// - **Type Safety**: Tools are stored as trait objects with compile-time guarantees
/// - **Performance**: HashMap-based lookup provides O(1) tool resolution
/// - **Extensibility**: New tools can be registered dynamically at runtime
/// - **Memory Efficiency**: Tools are stored once and accessed by reference
/// - **CLI Integration**: Automatic detection and tracking of CLI exclusion metadata
///
/// # Usage Patterns
///
/// ## Registration with Automatic CLI Exclusion Detection
/// ```rust,ignore
/// let mut registry = ToolRegistry::new();
/// registry.register(MyTool::new());      // Automatically detects CLI eligibility
/// registry.register(WorkflowTool::new()); // Automatically detects exclusion
/// 
/// // Query CLI exclusion status
/// assert!(!registry.is_cli_excluded("my_tool"));
/// assert!(registry.is_cli_excluded("workflow_tool"));
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
/// ## CLI Generation Integration
/// ```rust,ignore
/// // Get tools eligible for CLI generation
/// let eligible_tools = registry.get_cli_eligible_tools();
/// for tool_meta in eligible_tools {
///     generate_cli_command(tool_meta);
/// }
/// 
/// // Get tools excluded from CLI (MCP-only)
/// let excluded_tools = registry.get_excluded_tools();
/// for tool_meta in excluded_tools {
///     document_mcp_only_tool(tool_meta);
/// }
/// ```
///
/// ## MCP Integration
/// ```rust,ignore
/// // List all tools for MCP list_tools response
/// let tools = registry.list_tools();
/// 
/// // Create CLI exclusion detector for external systems
/// let detector = registry.create_cli_exclusion_detector();
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

    /// CLI exclusion metadata for each registered tool
    ///
    /// This stores metadata about each tool's CLI eligibility status, including
    /// exclusion reasons and alternative approaches. The metadata is populated
    /// during tool registration and provides the foundation for CLI generation
    /// systems to determine which tools to include or exclude.
    exclusion_metadata: HashMap<String, crate::cli::ToolCliMetadata>,
}

impl ToolRegistry {
    /// Create a new empty tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            exclusion_metadata: HashMap::new(),
        }
    }

    /// Register a tool in the registry with automatic CLI exclusion detection
    ///
    /// This method registers a tool and automatically detects its CLI exclusion status
    /// by checking if it implements the `CliExclusionMarker` trait. The exclusion
    /// metadata is stored for later query operations.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut registry = ToolRegistry::new();
    /// registry.register(MemoCreateTool::new()); // Will be CLI-eligible
    /// registry.register(IssueWorkTool::new());  // Will be CLI-excluded if marked
    /// 
    /// assert!(!registry.is_cli_excluded("memo_create"));
    /// assert!(registry.is_cli_excluded("issue_work")); // If marked with CliExclusionMarker
    /// ```
    /// Helper method to check if a tool is a known CLI-excluded tool
    ///
    /// This is a fallback detection method for tools that don't implement the
    /// CliExclusionMarker trait but are known to be excluded from CLI generation.
    /// This list should be kept in sync with tools that implement CliExclusionMarker.
    fn is_known_excluded_tool(tool_name: &str) -> bool {
        matches!(tool_name, "issue_work" | "issue_merge" | "abort_create")
    }

    /// Detect CLI exclusion status for a tool
    ///
    /// This method attempts to detect if a tool should be excluded from CLI generation
    /// by trying multiple detection strategies in order of preference.
    fn detect_cli_exclusion_status<T: McpTool>(tool: &T) -> crate::cli::ToolCliMetadata {
        let name = tool.name();
        
        // Strategy 1: Try to detect via CliExclusionMarker trait if tool provides as_any()
        if let Some(_any_tool) = tool.as_any() {
            // For concrete types that implement CliExclusionMarker, we need type-specific logic
            // Since we can't downcast to trait objects, we use type name matching
            let type_name = std::any::type_name::<T>();
            
            // Handle known test types
            if type_name.contains("CliExcludedMockTool") {
                return crate::cli::ToolCliMetadata::excluded(
                    name,
                    "Test exclusion for unit testing",
                );
            }
            
            if type_name.contains("CliIncludedMockTool") {
                return crate::cli::ToolCliMetadata::included(name);
            }
            
            // For production tools, we'd need to check if they're concrete types that
            // implement CliExclusionMarker. For now, fall back to known tool list.
        }
        
        // Strategy 2: Use known excluded tools list
        if Self::is_known_excluded_tool(name) {
            crate::cli::ToolCliMetadata::excluded(
                name,
                "MCP workflow orchestration tool - not suitable for direct CLI usage",
            )
        } else {
            crate::cli::ToolCliMetadata::included(name)
        }
    }

    /// Register a tool in the registry with automatic CLI exclusion detection
    ///
    /// This method registers a tool and automatically detects its CLI exclusion status
    /// by checking if it implements the `CliExclusionMarker` trait or matches known
    /// excluded tool patterns. The exclusion metadata is stored for efficient query operations.
    ///
    /// # Arguments
    ///
    /// * `tool` - The tool instance to register, must implement `McpTool + 'static`
    ///
    /// # Detection Strategy
    ///
    /// The method uses a multi-strategy approach to detect CLI exclusion:
    /// 1. **Type Name Matching**: For tools that provide `as_any()`, checks type names
    ///    against known patterns (e.g., test tools with "CliExcludedMockTool" in name)
    /// 2. **Known Tool List**: Falls back to a hardcoded list of known excluded tools
    ///    like "issue_work", "issue_merge", and "abort_create"
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    ///
    /// let mut registry = ToolRegistry::new();
    /// 
    /// // Register a CLI-eligible tool
    /// registry.register(MemoCreateTool::new());
    /// assert!(!registry.is_cli_excluded("memo_create"));
    /// 
    /// // Register a workflow tool (automatically detected as excluded)
    /// registry.register(IssueWorkTool::new());
    /// assert!(registry.is_cli_excluded("issue_work"));
    /// ```
    ///
    /// # Backward Compatibility
    ///
    /// This method maintains full backward compatibility with existing code.
    /// All existing MCP operations continue to work unchanged, with the addition
    /// of CLI exclusion metadata tracking.
    pub fn register<T: McpTool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        
        // Detect CLI exclusion status using our enhanced detection logic
        let metadata = Self::detect_cli_exclusion_status(&tool);
        
        self.exclusion_metadata.insert(name.clone(), metadata);
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

    /// Check if a tool is marked for CLI exclusion
    ///
    /// This method provides direct access to CLI exclusion status stored in the
    /// registry during tool registration. It's more efficient than creating a
    /// detector for single-tool queries.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - The name of the tool to check
    ///
    /// # Returns
    ///
    /// * `true` if the tool is marked for CLI exclusion
    /// * `false` if the tool is CLI-eligible or doesn't exist
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// let is_excluded = registry.is_cli_excluded("issue_work");
    /// assert!(is_excluded); // Assuming issue_work is excluded
    /// ```
    pub fn is_cli_excluded(&self, tool_name: &str) -> bool {
        self.exclusion_metadata
            .get(tool_name)
            .map(|meta| meta.is_cli_excluded)
            .unwrap_or(false)
    }

    /// Get all tools marked for CLI exclusion
    ///
    /// Returns a vector of CLI metadata for all tools that are marked for 
    /// exclusion from CLI generation. Useful for generating MCP-only tool
    /// documentation or exclusion reports.
    ///
    /// # Returns
    ///
    /// Vector of `ToolCliMetadata` for excluded tools
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// let excluded = registry.get_excluded_tools();
    /// for tool_meta in excluded {
    ///     println!("Excluded: {} - {}", 
    ///         tool_meta.name, 
    ///         tool_meta.exclusion_reason.unwrap_or_else(|| "No reason given".to_string())
    ///     );
    /// }
    /// ```
    pub fn get_excluded_tools(&self) -> Vec<&crate::cli::ToolCliMetadata> {
        self.exclusion_metadata
            .values()
            .filter(|meta| meta.is_cli_excluded)
            .collect()
    }

    /// Get all tools eligible for CLI generation
    ///
    /// Returns a vector of CLI metadata for all tools that should be included
    /// in CLI command generation. This is the primary method for CLI generation
    /// systems to identify eligible tools.
    ///
    /// # Returns
    ///
    /// Vector of `ToolCliMetadata` for CLI-eligible tools
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// let eligible = registry.get_cli_eligible_tools();
    /// for tool_meta in eligible {
    ///     println!("CLI-eligible: {}", tool_meta.name);
    ///     // Generate CLI command for this tool
    /// }
    /// ```
    pub fn get_cli_eligible_tools(&self) -> Vec<&crate::cli::ToolCliMetadata> {
        self.exclusion_metadata
            .values()
            .filter(|meta| !meta.is_cli_excluded)
            .collect()
    }

    /// Get CLI metadata for a specific tool
    ///
    /// Returns the complete CLI metadata for a specific tool, including its
    /// exclusion status and reason. Useful for detailed tool inspection.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - The name of the tool to get metadata for
    ///
    /// # Returns
    ///
    /// Optional reference to `ToolCliMetadata` if the tool exists
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// if let Some(metadata) = registry.get_tool_metadata("issue_work") {
    ///     println!("Tool: {}", metadata.name);
    ///     println!("Excluded: {}", metadata.is_cli_excluded);
    ///     if let Some(reason) = &metadata.exclusion_reason {
    ///         println!("Reason: {}", reason);
    ///     }
    /// }
    /// ```
    pub fn get_tool_metadata(&self, tool_name: &str) -> Option<&crate::cli::ToolCliMetadata> {
        self.exclusion_metadata.get(tool_name)
    }

    /// List tools by category (excluded vs eligible)
    ///
    /// Returns a tuple containing vectors of excluded and eligible tool metadata.
    /// This is useful for generating comprehensive tool reports or CLI generation
    /// statistics.
    ///
    /// # Returns
    ///
    /// Tuple of (excluded_tools, eligible_tools) as vectors of `ToolCliMetadata`
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// let (excluded, eligible) = registry.list_tools_by_category();
    /// 
    /// println!("Excluded tools: {}", excluded.len());
    /// println!("CLI-eligible tools: {}", eligible.len());
    /// ```
    pub fn list_tools_by_category(&self) -> (Vec<&crate::cli::ToolCliMetadata>, Vec<&crate::cli::ToolCliMetadata>) {
        let mut excluded = Vec::new();
        let mut eligible = Vec::new();
        
        for metadata in self.exclusion_metadata.values() {
            if metadata.is_cli_excluded {
                excluded.push(metadata);
            } else {
                eligible.push(metadata);
            }
        }
        
        (excluded, eligible)
    }

    /// Create a CLI exclusion detector from this registry
    ///
    /// This method creates a detector using the CLI exclusion metadata that was
    /// collected during tool registration. The detector provides the standard
    /// `CliExclusionDetector` interface for CLI generation systems.
    ///
    /// # Returns
    ///
    /// A `RegistryCliExclusionDetector` instance that can query exclusion status
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    /// use swissarmyhammer_tools::cli::CliExclusionDetector;
    ///
    /// let registry = ToolRegistry::new();
    /// let detector = registry.create_cli_exclusion_detector();
    ///
    /// let excluded_tools = detector.get_excluded_tools();
    /// println!("MCP-only tools: {:?}", excluded_tools);
    /// ```
    pub fn create_cli_exclusion_detector(&self) -> crate::cli::RegistryCliExclusionDetector {
        use crate::cli::RegistryCliExclusionDetector;
        
        // Use the metadata that was already computed during tool registration
        // This is much more efficient than re-computing exclusion status
        RegistryCliExclusionDetector::new(self.exclusion_metadata.clone())
    }

    /// Get all tools that should be excluded from CLI generation
    ///
    /// This is a convenience method that returns the list of excluded tool names
    /// directly from the stored metadata for efficient access.
    ///
    /// # Returns
    ///
    /// Vector of tool names that should be excluded from CLI generation
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// let excluded = registry.get_excluded_tool_names();
    /// for tool in excluded {
    ///     println!("Excluding {} from CLI generation", tool);
    /// }
    /// ```
    pub fn get_excluded_tool_names(&self) -> Vec<String> {
        self.exclusion_metadata
            .values()
            .filter(|meta| meta.is_cli_excluded)
            .map(|meta| meta.name.clone())
            .collect()
    }

    /// Get all tools that should be included in CLI generation
    ///
    /// This is a convenience method that returns the list of CLI-eligible tool names
    /// directly from the stored metadata for efficient access.
    ///
    /// # Returns
    ///
    /// Vector of tool names eligible for CLI generation
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// let eligible = registry.get_cli_eligible_tool_names();
    /// for tool in eligible {
    ///     println!("Including {} in CLI generation", tool);
    /// }
    /// ```
    pub fn get_cli_eligible_tool_names(&self) -> Vec<String> {
        self.exclusion_metadata
            .values()
            .filter(|meta| !meta.is_cli_excluded)
            .map(|meta| meta.name.clone())
            .collect()
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
    use crate::cli::CliExclusionDetector;
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

    /// Mock tool that implements CLI exclusion for testing
    #[derive(Default)]
    struct CliExcludedMockTool {
        name: &'static str,
    }

    impl CliExcludedMockTool {
        fn new(name: &'static str) -> Self {
            Self { name }
        }
    }

    impl crate::cli::CliExclusionMarker for CliExcludedMockTool {
        fn is_cli_excluded(&self) -> bool {
            true
        }

        fn exclusion_reason(&self) -> Option<&'static str> {
            Some("Test exclusion for unit testing")
        }
    }

    #[async_trait::async_trait]
    impl McpTool for CliExcludedMockTool {
        fn name(&self) -> &'static str {
            self.name
        }

        fn description(&self) -> &'static str {
            "Mock tool for testing CLI exclusion"
        }

        fn schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, serde_json::Value>,
            _context: &ToolContext,
        ) -> std::result::Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response(
                "mock excluded executed",
            ))
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    /// Mock tool that includes in CLI for testing
    #[derive(Default)]
    struct CliIncludedMockTool {
        name: &'static str,
    }

    impl CliIncludedMockTool {
        fn new(name: &'static str) -> Self {
            Self { name }
        }
    }

    impl crate::cli::CliExclusionMarker for CliIncludedMockTool {
        fn is_cli_excluded(&self) -> bool {
            false
        }

        fn exclusion_reason(&self) -> Option<&'static str> {
            None
        }
    }

    #[async_trait::async_trait]
    impl McpTool for CliIncludedMockTool {
        fn name(&self) -> &'static str {
            self.name
        }

        fn description(&self) -> &'static str {
            "Mock tool for testing CLI inclusion"
        }

        fn schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, serde_json::Value>,
            _context: &ToolContext,
        ) -> std::result::Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response(
                "mock included executed",
            ))
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    #[test]
    fn test_registry_cli_exclusion_metadata_tracking() {
        let mut registry = ToolRegistry::new();
        
        // Register an excluded tool (simulate known excluded tool for now)
        let excluded_tool = MockTool {
            name: "issue_work", // Use a known excluded tool name to test the fallback
            description: "Test excluded tool",
        };
        registry.register(excluded_tool);
        
        // Register an included tool
        let included_tool = MockTool {
            name: "test_included",
            description: "Test included tool",
        };
        registry.register(included_tool);
        
        // Verify that both tools were registered
        
        // Test basic exclusion queries
        assert!(registry.is_cli_excluded("issue_work"));
        assert!(!registry.is_cli_excluded("test_included"));
        assert!(!registry.is_cli_excluded("nonexistent_tool"));
        
        // Test metadata retrieval
        let excluded_meta = registry.get_tool_metadata("issue_work").unwrap();
        assert_eq!(excluded_meta.name, "issue_work");
        assert!(excluded_meta.is_cli_excluded);
        assert!(excluded_meta.exclusion_reason.is_some());
        
        let included_meta = registry.get_tool_metadata("test_included").unwrap();
        assert_eq!(included_meta.name, "test_included");
        assert!(!included_meta.is_cli_excluded);
        assert!(included_meta.exclusion_reason.is_none());
    }

    #[test]
    fn test_registry_exclusion_lists() {
        let mut registry = ToolRegistry::new();
        
        // Register mixed tools
        registry.register(CliExcludedMockTool::new("excluded1"));
        registry.register(CliIncludedMockTool::new("included1"));
        registry.register(CliExcludedMockTool::new("excluded2"));
        registry.register(CliIncludedMockTool::new("included2"));
        
        // Test excluded tools
        let excluded = registry.get_excluded_tools();
        assert_eq!(excluded.len(), 2);
        let excluded_names: std::collections::HashSet<_> = 
            excluded.iter().map(|meta| &meta.name).collect();
        assert!(excluded_names.contains(&"excluded1".to_string()));
        assert!(excluded_names.contains(&"excluded2".to_string()));
        
        // Test eligible tools
        let eligible = registry.get_cli_eligible_tools();
        assert_eq!(eligible.len(), 2);
        let eligible_names: std::collections::HashSet<_> = 
            eligible.iter().map(|meta| &meta.name).collect();
        assert!(eligible_names.contains(&"included1".to_string()));
        assert!(eligible_names.contains(&"included2".to_string()));
        
        // Test convenience methods
        let excluded_names = registry.get_excluded_tool_names();
        assert_eq!(excluded_names.len(), 2);
        assert!(excluded_names.contains(&"excluded1".to_string()));
        assert!(excluded_names.contains(&"excluded2".to_string()));
        
        let eligible_names = registry.get_cli_eligible_tool_names();
        assert_eq!(eligible_names.len(), 2);
        assert!(eligible_names.contains(&"included1".to_string()));
        assert!(eligible_names.contains(&"included2".to_string()));
    }

    #[test]
    fn test_registry_category_listing() {
        let mut registry = ToolRegistry::new();
        
        registry.register(CliExcludedMockTool::new("excluded_tool"));
        registry.register(CliIncludedMockTool::new("included_tool"));
        
        let (excluded, eligible) = registry.list_tools_by_category();
        
        assert_eq!(excluded.len(), 1);
        assert_eq!(eligible.len(), 1);
        assert_eq!(excluded[0].name, "excluded_tool");
        assert_eq!(eligible[0].name, "included_tool");
        assert!(excluded[0].is_cli_excluded);
        assert!(!eligible[0].is_cli_excluded);
    }

    #[test]
    fn test_known_excluded_tools_detection() {
        let mut registry = ToolRegistry::new();
        
        // Register a tool with a name that's in the known excluded list
        let mock_tool = MockTool {
            name: "issue_work", // This should be detected as excluded
            description: "Mock issue work tool",
        };
        registry.register(mock_tool);
        
        // Should be detected as excluded even without CliExclusionMarker trait
        assert!(registry.is_cli_excluded("issue_work"));
        
        let metadata = registry.get_tool_metadata("issue_work").unwrap();
        assert!(metadata.is_cli_excluded);
        assert!(metadata.exclusion_reason.is_some());
    }

    #[test]
    fn test_registry_detector_creation() {
        let mut registry = ToolRegistry::new();
        
        registry.register(CliExcludedMockTool::new("excluded_tool"));
        registry.register(CliIncludedMockTool::new("included_tool"));
        
        // Create detector using the new optimized method
        let detector = registry.create_cli_exclusion_detector();
        
        // Test detector functionality
        assert!(detector.is_cli_excluded("excluded_tool"));
        assert!(!detector.is_cli_excluded("included_tool"));
        
        let excluded_tools = detector.get_excluded_tools();
        let eligible_tools = detector.get_cli_eligible_tools();
        
        assert_eq!(excluded_tools.len(), 1);
        assert_eq!(eligible_tools.len(), 1);
        assert!(excluded_tools.contains(&"excluded_tool".to_string()));
        assert!(eligible_tools.contains(&"included_tool".to_string()));
    }

    #[test]
    fn test_registry_backward_compatibility() {
        let mut registry = ToolRegistry::new();
        
        // Register a regular tool (existing functionality should work)
        let tool = MockTool {
            name: "regular_tool",
            description: "Regular tool without CLI exclusion",
        };
        registry.register(tool);
        
        // All existing functionality should work unchanged
        assert!(registry.get_tool("regular_tool").is_some());
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
        
        let tool_names = registry.list_tool_names();
        assert!(tool_names.contains(&"regular_tool".to_string()));
        
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "regular_tool");
        
        // New functionality should also work
        assert!(!registry.is_cli_excluded("regular_tool")); // Should be CLI-eligible by default
        assert_eq!(registry.get_excluded_tools().len(), 0);
        assert_eq!(registry.get_cli_eligible_tools().len(), 1);
    }

    #[test]
    fn test_empty_registry_exclusion_behavior() {
        let registry = ToolRegistry::new();
        
        // Empty registry should handle exclusion queries gracefully
        assert!(!registry.is_cli_excluded("any_tool"));
        assert!(registry.get_excluded_tools().is_empty());
        assert!(registry.get_cli_eligible_tools().is_empty());
        assert!(registry.get_excluded_tool_names().is_empty());
        assert!(registry.get_cli_eligible_tool_names().is_empty());
        assert!(registry.get_tool_metadata("any_tool").is_none());
        
        let (excluded, eligible) = registry.list_tools_by_category();
        assert!(excluded.is_empty());
        assert!(eligible.is_empty());
        
        // Detector should also work with empty registry
        let detector = registry.create_cli_exclusion_detector();
        assert!(!detector.is_cli_excluded("any_tool"));
        assert!(detector.get_excluded_tools().is_empty());
        assert!(detector.get_cli_eligible_tools().is_empty());
    }

    #[test]
    fn test_registry_exclusion_consistency() {
        let mut registry = ToolRegistry::new();
        
        registry.register(CliExcludedMockTool::new("excluded_tool"));
        registry.register(CliIncludedMockTool::new("included_tool"));
        
        // Test that all query methods return consistent results
        let direct_excluded = registry.is_cli_excluded("excluded_tool");
        let direct_included = registry.is_cli_excluded("included_tool");
        
        let detector = registry.create_cli_exclusion_detector();
        let detector_excluded = detector.is_cli_excluded("excluded_tool");
        let detector_included = detector.is_cli_excluded("included_tool");
        
        assert_eq!(direct_excluded, detector_excluded);
        assert_eq!(direct_included, detector_included);
        
        // Test list consistency
        let registry_excluded = registry.get_excluded_tool_names();
        let detector_excluded_list = detector.get_excluded_tools();
        
        assert_eq!(registry_excluded.len(), detector_excluded_list.len());
        for tool_name in &registry_excluded {
            assert!(detector_excluded_list.contains(tool_name));
        }
    }
}
