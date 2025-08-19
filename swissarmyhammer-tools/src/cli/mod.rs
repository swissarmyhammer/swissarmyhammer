//! CLI-related utilities for SwissArmyHammer tools
//!
//! This module provides utilities for CLI generation systems to understand
//! which MCP tools should be included or excluded from CLI command generation.

pub mod attribute_detection;

#[cfg(test)]
mod integration_tests;

pub use attribute_detection::{
    CliExclusionDetector, CliExclusionMarker, RegistryCliExclusionDetector, ToolCliMetadata,
};