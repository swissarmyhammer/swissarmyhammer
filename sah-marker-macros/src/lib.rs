//! # SAH Marker Macros
//!
//! Procedural macros for SwissArmyHammer marker attributes, providing infrastructure 
//! for marking MCP tools with metadata that can be used by CLI generation systems
//! and other tooling.
//!
//! ## Overview
//!
//! This crate provides the `#[cli_exclude]` attribute macro for marking MCP tools
//! that should be excluded from CLI generation. The macro serves as a compile-time
//! marker that can be detected by build systems and documented for developers.
//!
//! ## Usage
//!
//! ```rust
//! use sah_marker_macros::cli_exclude;
//!
//! /// Tool designed for MCP workflow operations only
//! #[cli_exclude]
//! #[derive(Default)]
//! pub struct WorkflowOrchestrationTool;
//! ```
//!
//! ## Integration with CLI Exclusion System
//!
//! Tools marked with `#[cli_exclude]` should also implement the `CliExclusionMarker` 
//! trait from `swissarmyhammer_tools::cli` to provide runtime queryable exclusion status:
//!
//! ```rust
//! use sah_marker_macros::cli_exclude;
//! use swissarmyhammer_tools::cli::CliExclusionMarker;
//!
//! #[cli_exclude]
//! #[derive(Default)]
//! pub struct MyCLIExcludedTool;
//!
//! impl CliExclusionMarker for MyCLIExcludedTool {
//!     fn is_cli_excluded(&self) -> bool {
//!         true
//!     }
//!
//!     fn exclusion_reason(&self) -> Option<&'static str> {
//!         Some("Designed for MCP workflow orchestration only")
//!     }
//! }
//! ```
//!
//! ## Design Philosophy
//!
//! The attribute macro approach provides:
//! - **Compile-time Marking**: Clear indication of exclusion intent
//! - **Documentation**: Self-documenting code with visible markers
//! - **Future Extensibility**: Foundation for CLI generation systems
//! - **No Runtime Overhead**: Pure marker with no functional impact
//!
//! For complete documentation on the CLI exclusion system, see the
//! [CLI Exclusion System Documentation](https://docs.rs/swissarmyhammer-tools/latest/swissarmyhammer_tools/cli/index.html).

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Item};

/// Marks an MCP tool as excluded from CLI generation.
///
/// This attribute macro marks tools that are designed specifically for MCP workflow
/// operations and should not be exposed as direct CLI commands. The macro serves as
/// a compile-time marker that can be detected by build systems and provides clear
/// documentation of the tool's intended usage context.
///
/// ## Usage
///
/// ```rust
/// use sah_marker_macros::cli_exclude;
///
/// /// Tool for managing workflow state transitions
/// #[cli_exclude]
/// #[derive(Default)]
/// pub struct WorkflowStateManager;
///
/// /// Tool that uses abort file patterns for error handling  
/// #[cli_exclude]
/// #[derive(Default)]
/// pub struct WorkflowTerminator;
/// ```
///
/// ## When to Use
///
/// Mark tools with `#[cli_exclude]` when they:
///
/// - **Require MCP Context**: Need specific MCP protocol context for proper operation
/// - **Use Workflow Patterns**: Part of larger workflow orchestrations
/// - **Handle Complex State**: Coordinate state between multiple systems
/// - **Use Abort Patterns**: Employ MCP-specific error handling like abort files
///
/// ## Examples from SwissArmyHammer
///
/// Real tools from the codebase that use this attribute:
///
/// ```rust
/// /// Git branch operations within issue workflows
/// #[cli_exclude]
/// #[derive(Default)]
/// pub struct WorkIssueTool;
///
/// /// Coordinated merge operations with issue completion
/// #[cli_exclude]
/// #[derive(Default)]  
/// pub struct MergeIssueTool;
/// ```
///
/// ## Integration with Runtime Detection
///
/// Tools marked with this attribute should also implement `CliExclusionMarker`
/// for runtime queryability:
///
/// ```rust
/// use sah_marker_macros::cli_exclude;
/// use swissarmyhammer_tools::cli::CliExclusionMarker;
///
/// #[cli_exclude]
/// #[derive(Default)]
/// pub struct MyTool;
///
/// impl CliExclusionMarker for MyTool {
///     fn is_cli_excluded(&self) -> bool {
///         true
///     }
///
///     fn exclusion_reason(&self) -> Option<&'static str> {
///         Some("Designed for MCP workflow orchestration - requires specific context")
///     }
/// }
/// ```
///
/// ## Design Philosophy
///
/// This attribute-based approach provides:
///
/// - **Explicit Intent**: Clear marking of tools not suitable for CLI
/// - **Self-Documentation**: Code documents its own usage constraints  
/// - **Compile-time Safety**: Exclusion decision made at build time
/// - **Future Compatibility**: Foundation for CLI generation systems
///
/// ## No-op Implementation
///
/// This is a no-op attribute macro that serves purely as a marker. It does not
/// modify the item's behavior or functionality in any way. The actual exclusion
/// logic is handled by the runtime trait system in `swissarmyhammer_tools::cli`.
///
/// ## Future CLI Generation
///
/// Future CLI generation systems should:
/// 1. Detect this attribute on tool structs
/// 2. Exclude marked tools from CLI command generation
/// 3. Continue allowing tool registration in MCP servers
/// 4. Optionally document excluded tools for developers
#[proc_macro_attribute]
pub fn cli_exclude(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input item to ensure it's valid syntax
    let input = parse_macro_input!(item as Item);

    // Return the item unchanged - this is a no-op marker attribute
    quote! {
        #input
    }
    .into()
}
