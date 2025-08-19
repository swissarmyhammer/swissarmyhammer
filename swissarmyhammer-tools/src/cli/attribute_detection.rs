//! Attribute detection utilities for MCP tools
//!
//! This module provides utilities to detect and process CLI exclusion markers
//! from MCP tool definitions, creating the foundation for CLI generation systems
//! to identify which tools should be included or excluded.
//!
//! # Architecture Overview
//!
//! The detection system uses a trait-based approach that integrates cleanly with
//! Rust's type system and the existing MCP tool registry pattern:
//!
//! 1. **CliExclusionMarker Trait**: Optional trait that tools can implement to
//!    declare their CLI exclusion status
//! 2. **ToolCliMetadata**: Structured metadata about tool CLI eligibility
//! 3. **CliExclusionDetector**: Main interface for querying exclusion status
//!
//! # Design Philosophy
//!
//! Rather than attempting runtime attribute parsing (which is complex in Rust),
//! this system uses compile-time trait implementations that tools can opt into.
//! This provides:
//!
//! - Type safety and compile-time validation
//! - No runtime reflection or parsing overhead
//! - Easy integration with existing tool patterns
//! - Clear documentation of exclusion rationale
//!
//! # Example Usage
//!
//! ```rust
//! use swissarmyhammer_tools::cli::{CliExclusionMarker, CliExclusionDetector};
//! use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
//!
//! // Tool that should be excluded from CLI
//! #[derive(Default)]
//! pub struct WorkflowTool;
//!
//! impl CliExclusionMarker for WorkflowTool {
//!     fn is_cli_excluded(&self) -> bool {
//!         true
//!     }
//!
//!     fn exclusion_reason(&self) -> Option<&'static str> {
//!         Some("Designed for MCP workflow orchestration only")
//!     }
//! }
//!
//! // Query exclusion status from registry
//! let registry = ToolRegistry::new();
//! let detector = registry.as_exclusion_detector();
//! let excluded = detector.get_excluded_tools();
//! ```

use std::collections::HashMap;

/// Metadata about a tool's CLI eligibility status
///
/// This structure contains all information needed by CLI generation systems
/// to make decisions about whether to include a tool in generated CLI commands.
///
/// # Design Notes
///
/// The metadata is designed to be:
/// - **Lightweight**: Minimal memory footprint for registry storage
/// - **Informative**: Includes human-readable exclusion reasons
/// - **Extensible**: Can be enhanced with additional metadata in the future
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCliMetadata {
    /// The unique name of the tool
    pub name: String,

    /// Whether this tool is marked for CLI exclusion
    pub is_cli_excluded: bool,

    /// Optional human-readable reason for exclusion
    ///
    /// This provides context for why a tool is excluded from CLI generation,
    /// which can be useful for documentation and debugging.
    pub exclusion_reason: Option<String>,
}

impl ToolCliMetadata {
    /// Create metadata for a CLI-eligible tool
    ///
    /// # Arguments
    ///
    /// * `name` - The unique tool name
    ///
    /// # Example
    ///
    /// ```rust
    /// # use swissarmyhammer_tools::cli::ToolCliMetadata;
    /// let metadata = ToolCliMetadata::included("memo_create");
    /// assert!(!metadata.is_cli_excluded);
    /// assert_eq!(metadata.name, "memo_create");
    /// ```
    pub fn included<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            is_cli_excluded: false,
            exclusion_reason: None,
        }
    }

    /// Create metadata for a CLI-excluded tool
    ///
    /// # Arguments
    ///
    /// * `name` - The unique tool name
    /// * `reason` - Human-readable exclusion reason
    ///
    /// # Example
    ///
    /// ```rust
    /// # use swissarmyhammer_tools::cli::ToolCliMetadata;
    /// let metadata = ToolCliMetadata::excluded(
    ///     "issue_work",
    ///     "MCP workflow orchestration only"
    /// );
    /// assert!(metadata.is_cli_excluded);
    /// assert_eq!(metadata.exclusion_reason.unwrap(), "MCP workflow orchestration only");
    /// ```
    pub fn excluded<S: Into<String>, R: Into<String>>(name: S, reason: R) -> Self {
        Self {
            name: name.into(),
            is_cli_excluded: true,
            exclusion_reason: Some(reason.into()),
        }
    }
}

/// Trait for MCP tools to declare their CLI exclusion status
///
/// Tools that should be excluded from CLI generation can implement this trait
/// to provide their exclusion status and reasoning. This is an optional trait
/// that provides a clean way for tools to self-declare their CLI eligibility.
///
/// # Design Rationale
///
/// This trait-based approach avoids the complexity of runtime attribute parsing
/// while providing a clean, type-safe way for tools to declare their status.
/// Tools marked with `#[cli_exclude]` should implement this trait to provide
/// runtime queryable exclusion status.
///
/// # Implementation Guidelines
///
/// Tools should implement this trait when they are designed specifically for
/// MCP protocol operations and should not be exposed as CLI commands:
///
/// - Workflow orchestration tools
/// - Internal state management tools
/// - Tools that require specific MCP context
/// - Tools that use MCP-specific error handling
///
/// # Example
///
/// ```rust
/// use swissarmyhammer_tools::cli::CliExclusionMarker;
///
/// #[derive(Default)]
/// pub struct IssueWorkTool;
///
/// impl CliExclusionMarker for IssueWorkTool {
///     fn is_cli_excluded(&self) -> bool {
///         true
///     }
///
///     fn exclusion_reason(&self) -> Option<&'static str> {
///         Some("Designed for MCP workflow state transitions")
///     }
/// }
/// ```
pub trait CliExclusionMarker: Send + Sync {
    /// Returns true if this tool should be excluded from CLI generation
    ///
    /// The default implementation returns `false`, meaning tools are included
    /// in CLI generation unless they explicitly declare otherwise.
    fn is_cli_excluded(&self) -> bool {
        false
    }

    /// Returns an optional human-readable reason for CLI exclusion
    ///
    /// This should provide context about why the tool is excluded from CLI
    /// generation. Common reasons include:
    /// - "MCP workflow orchestration only"
    /// - "Requires MCP protocol context"
    /// - "Internal tool for state management"
    /// - "Uses MCP-specific error handling patterns"
    fn exclusion_reason(&self) -> Option<&'static str> {
        None
    }
}

/// Main interface for detecting CLI exclusion attributes on MCP tools
///
/// This trait defines the interface that CLI generation systems should use
/// to determine which tools should be included or excluded from CLI command
/// generation. The trait is implemented by types that can examine tool
/// collections and extract exclusion metadata.
///
/// # Design Goals
///
/// - **Simple Interface**: Easy-to-use methods for common queries
/// - **Efficient Lookup**: Optimized for CLI generation performance
/// - **Comprehensive Results**: Provides both individual and bulk queries
/// - **Future-Proof**: Extensible interface for additional metadata
///
/// # Usage Patterns
///
/// CLI generation systems typically use this interface in two ways:
///
/// 1. **Bulk Processing**: Get all excluded/eligible tools at once
/// 2. **Individual Queries**: Check specific tools during processing
///
/// # Example
///
/// ```rust
/// use swissarmyhammer_tools::cli::CliExclusionDetector;
///
/// fn generate_cli_commands<T: CliExclusionDetector>(detector: &T) {
///     let eligible_tools = detector.get_cli_eligible_tools();
///     
///     for tool_name in eligible_tools {
///         // Generate CLI command for this tool
///         println!("Generating CLI command for: {}", tool_name);
///     }
/// }
/// ```
pub trait CliExclusionDetector {
    /// Check if a specific tool has the CLI exclusion marker
    ///
    /// # Arguments
    ///
    /// * `tool_name` - The name of the tool to check
    ///
    /// # Returns
    ///
    /// * `true` if the tool is marked for CLI exclusion
    /// * `false` if the tool should be included in CLI generation
    ///
    /// # Example
    ///
    /// ```rust
    /// # use swissarmyhammer_tools::cli::CliExclusionDetector;
    /// # fn example<T: CliExclusionDetector>(detector: &T) {
    /// if detector.is_cli_excluded("issue_work") {
    ///     println!("Tool is excluded from CLI generation");
    /// }
    /// # }
    /// ```
    fn is_cli_excluded(&self, tool_name: &str) -> bool;

    /// Get all tools marked for CLI exclusion
    ///
    /// Returns a vector of tool names that should be excluded from CLI
    /// generation. This is useful for generating exclusion lists or
    /// documentation about MCP-only tools.
    ///
    /// # Returns
    ///
    /// Vector of tool names marked for exclusion
    ///
    /// # Example
    ///
    /// ```rust
    /// # use swissarmyhammer_tools::cli::CliExclusionDetector;
    /// # fn example<T: CliExclusionDetector>(detector: &T) {
    /// let excluded = detector.get_excluded_tools();
    /// println!("MCP-only tools: {:?}", excluded);
    /// # }
    /// ```
    fn get_excluded_tools(&self) -> Vec<String>;

    /// Get all tools eligible for CLI generation
    ///
    /// Returns a vector of tool names that should be included in CLI
    /// command generation. This is the primary method used by CLI
    /// generation systems.
    ///
    /// # Returns
    ///
    /// Vector of tool names eligible for CLI generation
    ///
    /// # Example
    ///
    /// ```rust
    /// # use swissarmyhammer_tools::cli::CliExclusionDetector;
    /// # fn example<T: CliExclusionDetector>(detector: &T) {
    /// let eligible = detector.get_cli_eligible_tools();
    /// for tool in eligible {
    ///     // Generate CLI command for this tool
    /// }
    /// # }
    /// ```
    fn get_cli_eligible_tools(&self) -> Vec<String>;

    /// Get detailed metadata for all tools
    ///
    /// Returns complete metadata for all registered tools, including both
    /// excluded and eligible tools with their exclusion reasons. This is
    /// useful for generating comprehensive documentation.
    ///
    /// # Returns
    ///
    /// Vector of `ToolCliMetadata` for all tools
    ///
    /// # Example
    ///
    /// ```rust
    /// # use swissarmyhammer_tools::cli::CliExclusionDetector;
    /// # fn example<T: CliExclusionDetector>(detector: &T) {
    /// let all_metadata = detector.get_all_tool_metadata();
    /// for metadata in all_metadata {
    ///     if metadata.is_cli_excluded {
    ///         println!("Excluded: {} - {}",
    ///             metadata.name,
    ///             metadata.exclusion_reason.unwrap_or("No reason given")
    ///         );
    ///     }
    /// }
    /// # }
    /// ```
    fn get_all_tool_metadata(&self) -> Vec<ToolCliMetadata>;
}

/// Registry-based implementation of CLI exclusion detection
///
/// This struct provides CLI exclusion detection capabilities by examining
/// a collection of registered MCP tools. It efficiently caches exclusion
/// metadata and provides fast lookup operations.
///
/// # Performance Characteristics
///
/// - **Lazy Evaluation**: Metadata is computed on first access
/// - **Cached Results**: Subsequent queries use cached metadata
/// - **Memory Efficient**: Stores minimal required information
/// - **Thread Safe**: Immutable after construction, safe for concurrent access
///
/// # Usage
///
/// This type is typically not used directly. Instead, use the extension
/// methods on `ToolRegistry` to get a detector instance:
///
/// ```rust
/// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
///
/// let registry = ToolRegistry::new();
/// let detector = RegistryCliExclusionDetector::new(&registry);
/// ```
pub struct RegistryCliExclusionDetector {
    /// Cached metadata for all tools
    metadata_cache: HashMap<String, ToolCliMetadata>,
}

impl RegistryCliExclusionDetector {
    /// Create a new detector from a tool metadata map
    ///
    /// # Arguments
    ///
    /// * `metadata_map` - HashMap mapping tool names to their metadata
    ///
    /// # Example
    ///
    /// ```rust
    /// # use swissarmyhammer_tools::cli::{RegistryCliExclusionDetector, ToolCliMetadata};
    /// # use std::collections::HashMap;
    /// #
    /// let mut metadata = HashMap::new();
    /// metadata.insert("tool1".to_string(), ToolCliMetadata::included("tool1"));
    /// metadata.insert("tool2".to_string(), ToolCliMetadata::excluded("tool2", "MCP only"));
    ///
    /// let detector = RegistryCliExclusionDetector::new(metadata);
    /// ```
    pub fn new(metadata_cache: HashMap<String, ToolCliMetadata>) -> Self {
        Self { metadata_cache }
    }
}

impl CliExclusionDetector for RegistryCliExclusionDetector {
    fn is_cli_excluded(&self, tool_name: &str) -> bool {
        self.metadata_cache
            .get(tool_name)
            .map(|metadata| metadata.is_cli_excluded)
            .unwrap_or(false)
    }

    fn get_excluded_tools(&self) -> Vec<String> {
        self.metadata_cache
            .values()
            .filter(|metadata| metadata.is_cli_excluded)
            .map(|metadata| metadata.name.clone())
            .collect()
    }

    fn get_cli_eligible_tools(&self) -> Vec<String> {
        self.metadata_cache
            .values()
            .filter(|metadata| !metadata.is_cli_excluded)
            .map(|metadata| metadata.name.clone())
            .collect()
    }

    fn get_all_tool_metadata(&self) -> Vec<ToolCliMetadata> {
        self.metadata_cache.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
    use async_trait::async_trait;
    use rmcp::model::CallToolResult;
    use rmcp::Error as McpError;

    /// Mock tool for testing that implements CLI exclusion
    #[derive(Default)]
    struct ExcludedMockTool;

    impl CliExclusionMarker for ExcludedMockTool {
        fn is_cli_excluded(&self) -> bool {
            true
        }

        fn exclusion_reason(&self) -> Option<&'static str> {
            Some("Test exclusion for mock tool")
        }
    }

    #[async_trait]
    impl McpTool for ExcludedMockTool {
        fn name(&self) -> &'static str {
            "excluded_mock"
        }

        fn description(&self) -> &'static str {
            "Mock tool for testing exclusion"
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

    #[test]
    fn test_tool_cli_metadata_included() {
        let metadata = ToolCliMetadata::included("test_tool");

        assert_eq!(metadata.name, "test_tool");
        assert!(!metadata.is_cli_excluded);
        assert!(metadata.exclusion_reason.is_none());
    }

    #[test]
    fn test_tool_cli_metadata_excluded() {
        let metadata = ToolCliMetadata::excluded("test_tool", "Test reason");

        assert_eq!(metadata.name, "test_tool");
        assert!(metadata.is_cli_excluded);
        assert_eq!(metadata.exclusion_reason.unwrap(), "Test reason");
    }

    #[test]
    fn test_cli_exclusion_marker_default() {
        #[derive(Default)]
        struct DefaultTool;

        impl CliExclusionMarker for DefaultTool {}

        let tool = DefaultTool;
        assert!(!tool.is_cli_excluded());
        assert!(tool.exclusion_reason().is_none());
    }

    #[test]
    fn test_cli_exclusion_marker_excluded() {
        let tool = ExcludedMockTool;
        assert!(tool.is_cli_excluded());
        assert_eq!(
            tool.exclusion_reason().unwrap(),
            "Test exclusion for mock tool"
        );
    }

    #[test]
    fn test_registry_cli_exclusion_detector_creation() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "included_tool".to_string(),
            ToolCliMetadata::included("included_tool"),
        );
        metadata.insert(
            "excluded_tool".to_string(),
            ToolCliMetadata::excluded("excluded_tool", "Test exclusion"),
        );

        let detector = RegistryCliExclusionDetector::new(metadata);

        assert!(!detector.is_cli_excluded("included_tool"));
        assert!(detector.is_cli_excluded("excluded_tool"));
        assert!(!detector.is_cli_excluded("nonexistent_tool"));
    }

    #[test]
    fn test_registry_detector_get_excluded_tools() {
        let mut metadata = HashMap::new();
        metadata.insert("tool1".to_string(), ToolCliMetadata::included("tool1"));
        metadata.insert(
            "excluded1".to_string(),
            ToolCliMetadata::excluded("excluded1", "Reason 1"),
        );
        metadata.insert(
            "excluded2".to_string(),
            ToolCliMetadata::excluded("excluded2", "Reason 2"),
        );
        metadata.insert("tool2".to_string(), ToolCliMetadata::included("tool2"));

        let detector = RegistryCliExclusionDetector::new(metadata);
        let mut excluded = detector.get_excluded_tools();
        excluded.sort(); // For deterministic testing

        assert_eq!(excluded, vec!["excluded1", "excluded2"]);
    }

    #[test]
    fn test_registry_detector_get_cli_eligible_tools() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "eligible1".to_string(),
            ToolCliMetadata::included("eligible1"),
        );
        metadata.insert(
            "excluded1".to_string(),
            ToolCliMetadata::excluded("excluded1", "Reason 1"),
        );
        metadata.insert(
            "eligible2".to_string(),
            ToolCliMetadata::included("eligible2"),
        );

        let detector = RegistryCliExclusionDetector::new(metadata);
        let mut eligible = detector.get_cli_eligible_tools();
        eligible.sort(); // For deterministic testing

        assert_eq!(eligible, vec!["eligible1", "eligible2"]);
    }

    #[test]
    fn test_registry_detector_get_all_tool_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("tool1".to_string(), ToolCliMetadata::included("tool1"));
        metadata.insert(
            "excluded_tool".to_string(),
            ToolCliMetadata::excluded("excluded_tool", "Test reason"),
        );

        let detector = RegistryCliExclusionDetector::new(metadata);
        let all_metadata = detector.get_all_tool_metadata();

        assert_eq!(all_metadata.len(), 2);

        // Find metadata by tool name for testing
        let tool1_metadata = all_metadata
            .iter()
            .find(|m| m.name == "tool1")
            .expect("tool1 metadata should exist");
        let excluded_metadata = all_metadata
            .iter()
            .find(|m| m.name == "excluded_tool")
            .expect("excluded_tool metadata should exist");

        assert!(!tool1_metadata.is_cli_excluded);
        assert!(tool1_metadata.exclusion_reason.is_none());

        assert!(excluded_metadata.is_cli_excluded);
        assert_eq!(
            excluded_metadata.exclusion_reason.as_deref(),
            Some("Test reason")
        );
    }

    #[test]
    fn test_empty_detector() {
        let detector = RegistryCliExclusionDetector::new(HashMap::new());

        assert!(detector.get_excluded_tools().is_empty());
        assert!(detector.get_cli_eligible_tools().is_empty());
        assert!(detector.get_all_tool_metadata().is_empty());
        assert!(!detector.is_cli_excluded("any_tool"));
    }
}
