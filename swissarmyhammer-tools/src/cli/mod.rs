//! # CLI Exclusion System for SwissArmyHammer Tools
//!
//! This module provides infrastructure for marking MCP tools that should be excluded
//! from CLI generation, along with utilities for CLI generation systems to detect
//! and respect these exclusions.
//!
//! ## Overview
//!
//! Some MCP tools are designed exclusively for MCP workflow operations and should
//! not be exposed as CLI commands. The CLI exclusion system provides a clean,
//! trait-based approach for tools to declare their CLI eligibility status.
//!
//! ## Key Components
//!
//! - [`CliExclusionMarker`]: Trait for tools to declare CLI exclusion status
//! - [`CliExclusionDetector`]: Interface for querying tool exclusion status  
//! - [`ToolCliMetadata`]: Structured metadata about tool CLI eligibility
//! - [`RegistryCliExclusionDetector`]: Registry-based detector implementation
//!
//! ## Usage Example
//!
//! ```rust
//! use swissarmyhammer_tools::cli::{CliExclusionMarker, CliExclusionDetector};
//!
//! // Mark a tool for CLI exclusion
//! #[sah_marker_macros::cli_exclude]
//! #[derive(Default)]
//! pub struct WorkflowTool;
//!
//! impl CliExclusionMarker for WorkflowTool {
//!     fn is_cli_excluded(&self) -> bool {
//!         true
//!     }
//!
//!     fn exclusion_reason(&self) -> Option<&'static str> {
//!         Some("MCP workflow orchestration only")
//!     }
//! }
//!
//! // Query exclusion status from a detector
//! fn check_tool_eligibility<T: CliExclusionDetector>(detector: &T) {
//!     let excluded_tools = detector.get_excluded_tools();
//!     let eligible_tools = detector.get_cli_eligible_tools();
//!     
//!     println!("Excluded tools: {:?}", excluded_tools);
//!     println!("CLI-eligible tools: {:?}", eligible_tools);
//! }
//! ```
//!
//! ## When to Use CLI Exclusion
//!
//! Mark tools with `#[cli_exclude]` when they are:
//! - Designed for MCP workflow orchestration
//! - Use MCP-specific error handling patterns (abort files)
//! - Require coordinated state between multiple systems
//! - Not intended for direct user invocation
//!
//! ## Integration
//!
//! CLI generation systems should use the [`CliExclusionDetector`] trait to
//! determine which tools to include in generated CLI commands. The detector
//! can be obtained from tool registries that implement the appropriate
//! extension methods.
//!
//! For complete usage patterns and examples, see the
//! [CLI Exclusion System Documentation](https://docs.rs/swissarmyhammer-tools/latest/swissarmyhammer_tools/cli/index.html).

pub mod attribute_detection;

/// Comprehensive examples demonstrating the CLI exclusion system
pub mod examples;

/// CLI exclusion validation system
pub mod validator;

#[cfg(test)]
mod integration_tests;

pub use attribute_detection::{
    CliExclusionDetector, CliExclusionMarker, RegistryCliExclusionDetector, ToolCliMetadata,
};
pub use validator::{
    DevUtilities, DocumentationGenerator, ExclusionValidator, ToolAnalysis, ToolSuggestion, 
    ValidationConfig, ValidationIssue, ValidationReport, ValidationSummary, ValidationWarning,
};
