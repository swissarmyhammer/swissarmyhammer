//! Project type detection library
//!
//! Automatically detects project types by walking the directory tree
//! and finding project marker files (Cargo.toml, package.json, etc.)
//!
//! This crate is used by the SwissArmyHammer prompt system to populate
//! JS context with detected project types, making them available to all
//! prompts without requiring LLM tool calls.

mod detect;
mod types;

pub use detect::{detect_projects, ProjectDetectionError};
pub use types::{
    project_type_specs, should_skip_directory, spec_for, DetectedProject, ProjectDetectionConfig,
    ProjectSymbols, ProjectType, ProjectTypeSpec, WorkspaceInfo, BUILTIN_CONFIG_YAML,
    SKIP_DIRECTORIES,
};
