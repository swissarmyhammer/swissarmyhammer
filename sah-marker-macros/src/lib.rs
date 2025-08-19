//! # SAH Marker Macros
//!
//! Procedural macros for SwissArmyHammer marker attributes.
//!
//! This crate provides attribute macros for marking MCP tools with metadata
//! that can be used by CLI generation systems and other tooling.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Item};

/// Marks an MCP tool as excluded from CLI generation.
///
/// Tools marked with this attribute are designed specifically for MCP workflow
/// operations and should not be exposed as direct CLI commands.
///
/// This is a no-op attribute macro that serves as a marker for future CLI
/// generation systems. The attribute does not modify the item's behavior
/// or functionality in any way.
///
/// # Example
///
/// ```rust
/// use sah_marker_macros::cli_exclude;
///
/// #[cli_exclude]
/// #[derive(Default, Debug)]
/// pub struct IssueWorkTool;
///
/// #[cli_exclude]
/// #[derive(Default)]
/// pub struct IssueMergeTool;
/// ```
///
/// # Design Philosophy
///
/// Some MCP tools are designed for specific workflow contexts:
/// - They expect specific MCP context and state management
/// - They use MCP-specific error handling patterns (abort files, etc.)
/// - They are part of larger MCP workflow orchestrations
/// - Direct CLI usage could bypass important workflow validation
///
/// # Future Usage
///
/// CLI generation systems should detect this attribute and exclude marked
/// tools from CLI command generation while still allowing their registration
/// in the MCP tool registry.
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
