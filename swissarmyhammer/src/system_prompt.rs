//! System prompt rendering infrastructure
//!
//! This module provides the core infrastructure to render the `.system.md` file with all template
//! processing resolved, preparing it for integration with Claude Code's `--append-system-prompt` parameter.
//!
//! ## Features
//!
//! - **Template Resolution**: Render `.system.md` with comprehensive coding standards and guidelines
//! - **Fresh Rendering**: Always re-renders system prompt to ensure up-to-date content
//! - **Error Handling**: Comprehensive error handling for template rendering failures
//! - **Configuration Integration**: Full integration with sah.toml configuration variables
//!
//! ## Usage
//!
//! ```rust,no_run
//! use swissarmyhammer::system_prompt::render_system_prompt;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Render the system prompt with all template processing resolved
//! let rendered_prompt = render_system_prompt()?;
//! println!("{}", rendered_prompt);
//! # Ok(())
//! # }
//! ```

use crate::{template::Template, PromptLibrary, Result, SwissArmyHammerError};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// System prompt file name
const SYSTEM_PROMPT_FILE: &str = ".system.md";

/// Default system prompt search paths
const DEFAULT_SYSTEM_PROMPT_PATHS: &[&str] = &[
    "builtin/prompts/.system.md",
    ".swissarmyhammer/prompts/.system.md",
    "prompts/.system.md",
    ".system.md",
];

/// Errors that can occur during system prompt rendering
#[derive(Debug, thiserror::Error)]
pub enum SystemPromptError {
    /// System prompt file not found
    #[error("System prompt file not found: {0}")]
    FileNotFound(String),

    /// Template rendering failed
    #[error("Template rendering failed: {0}")]
    RenderingFailed(String),

    /// Partial template not found
    #[error("Partial template not found: {0}")]
    PartialNotFound(String),

    /// IO error occurred
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Generic system prompt error
    #[error("System prompt error: {0}")]
    Generic(String),
}

impl From<SwissArmyHammerError> for SystemPromptError {
    fn from(err: SwissArmyHammerError) -> Self {
        match err {
            SwissArmyHammerError::Template(msg) => SystemPromptError::RenderingFailed(msg),
            SwissArmyHammerError::Io(io_err) => SystemPromptError::IoError(io_err),
            _ => SystemPromptError::Generic(err.to_string()),
        }
    }
}

/// System prompt renderer with caching capabilities
pub struct SystemPromptRenderer {
    /// Prompt library for accessing partials
    prompt_library: Arc<PromptLibrary>,
}

impl SystemPromptRenderer {
    /// Create a new system prompt renderer
    pub fn new() -> Result<Self> {
        let mut prompt_library = PromptLibrary::new();

        // Add builtin prompts directory if it exists
        let builtin_prompts = Path::new("builtin/prompts");
        if builtin_prompts.exists() {
            prompt_library.add_directory(builtin_prompts)?;
        }

        // Add user prompts directory if it exists
        let user_prompts = Path::new(".swissarmyhammer/prompts");
        if user_prompts.exists() {
            prompt_library.add_directory(user_prompts)?;
        }

        // Add legacy prompts directory if it exists
        let legacy_prompts = Path::new("prompts");
        if legacy_prompts.exists() {
            prompt_library.add_directory(legacy_prompts)?;
        }

        let prompt_library = Arc::new(prompt_library);

        Ok(Self { prompt_library })
    }

    /// Find the system prompt file in default locations
    fn find_system_prompt_file(&self) -> std::result::Result<PathBuf, SystemPromptError> {
        for path_str in DEFAULT_SYSTEM_PROMPT_PATHS {
            let path = Path::new(path_str);
            if path.exists() {
                return Ok(path.to_path_buf());
            }
        }

        Err(SystemPromptError::FileNotFound(format!(
            "System prompt file '{}' not found in any of these locations: {}",
            SYSTEM_PROMPT_FILE,
            DEFAULT_SYSTEM_PROMPT_PATHS.join(", ")
        )))
    }

    /// Render the system prompt without caching
    pub fn render(&self) -> std::result::Result<String, SystemPromptError> {
        // Find the system prompt file
        let system_prompt_path = self.find_system_prompt_file()?;

        // Read the system prompt content
        let content = std::fs::read_to_string(&system_prompt_path)?;

        // Create template with partial support
        let template = Template::with_partials(&content, Arc::clone(&self.prompt_library))?;

        // Render with configuration support
        let args = std::collections::HashMap::new();
        let rendered_content = template.render_with_config(&args)?;

        Ok(rendered_content)
    }
}

impl Default for SystemPromptRenderer {
    fn default() -> Self {
        Self::new().expect("Failed to create default SystemPromptRenderer")
    }
}

/// Render the system prompt with all template processing resolved
///
/// This function provides a convenient way to render the `.system.md` file with all template
/// content fully processed. The system prompt is rendered fresh each time to ensure
/// any changes to the system prompt file or its partials are immediately reflected.
///
/// # Examples
///
/// ```rust,no_run
/// use swissarmyhammer::system_prompt::render_system_prompt;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let rendered = render_system_prompt()?;
/// println!("System prompt: {}", rendered);
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns `SystemPromptError` if:
/// - System prompt file is not found
/// - Template rendering fails
/// - Required partial templates are missing
/// - IO errors occur during file operations
pub fn render_system_prompt() -> std::result::Result<String, SystemPromptError> {
    let renderer = SystemPromptRenderer::new()?;
    renderer.render()
}

/// Clear the system prompt cache
///
/// This function is now a no-op since system prompt caching has been disabled.
/// It is kept for backward compatibility with existing code that may call it.
pub fn clear_cache() {
    // No-op: caching has been disabled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_system_prompt_file_not_found() {
        // Clear cache to ensure clean test
        clear_cache();

        let renderer = SystemPromptRenderer::new().expect("Failed to create renderer");
        let result = renderer.find_system_prompt_file();

        // Should return error if no system prompt file is found
        assert!(matches!(result, Err(SystemPromptError::FileNotFound(_))));
    }

    #[test]
    fn test_render_system_prompt_function() {
        // This test will fail if no system prompt file exists, which is expected
        // in a clean test environment
        let result = render_system_prompt();

        // Should either succeed if system prompt exists, or fail with FileNotFound
        match result {
            Ok(_) => (),                                   // Success case
            Err(SystemPromptError::FileNotFound(_)) => (), // Expected failure
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[test]
    fn test_clear_cache() {
        // This should not panic and is now a no-op
        clear_cache();
        // No assertions needed since function is now a no-op
    }

    #[test]
    fn test_system_prompt_error_from_swiss_army_hammer_error() {
        let template_error = SwissArmyHammerError::Template("Test error".to_string());
        let system_prompt_error: SystemPromptError = template_error.into();

        assert!(matches!(
            system_prompt_error,
            SystemPromptError::RenderingFailed(_)
        ));
    }
}
