//! Prompt utility functions for common prompt operations
//!
//! This module provides shared functionality for rendering prompts,
//! including specialized functions for system prompt handling.

use crate::{PromptLibrary, Result};
use std::collections::HashMap;
use std::sync::Arc;

/// Render the system prompt using standard PromptLibrary infrastructure
/// 
/// This function loads the .system prompt from standard prompt directories and renders it
/// using the same pipeline as other prompts. It searches the following directories in order:
/// 1. builtin/prompts (relative to current directory)
/// 2. .swissarmyhammer/prompts
/// 3. prompts
///
/// Returns the rendered system prompt content as a string.
pub fn render_system_prompt() -> Result<String> {
    let mut library = PromptLibrary::new();
    
    // Add builtin prompts directory
    if let Ok(builtin_path) = std::env::current_dir().map(|p| p.join("builtin/prompts")) {
        if builtin_path.exists() {
            library.add_directory(builtin_path)?;
        }
    }
    
    // Add other standard prompt directories
    let standard_paths = [
        ".swissarmyhammer/prompts",
        "prompts",
    ];
    
    for path_str in &standard_paths {
        let path = std::path::Path::new(path_str);
        if path.exists() {
            library.add_directory(path)?;
        }
    }
    
    // Get and render the .system prompt
    let system_prompt = library.get(".system")?;
    let args = HashMap::new();
    let rendered = system_prompt.render_with_partials(&args, Arc::new(library))?;
    
    Ok(rendered)
}