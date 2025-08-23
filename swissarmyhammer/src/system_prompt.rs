//! System prompt rendering infrastructure
//!
//! This module provides the core infrastructure to render the `.system.md` file with all template
//! processing resolved, preparing it for integration with Claude Code's `--append-system-prompt` parameter.
//!
//! ## Features
//!
//! - **Template Resolution**: Render `.system.md` with comprehensive coding standards and guidelines
//! - **Caching Strategy**: Cache rendered system prompt to avoid repeated processing
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
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

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

/// Cache entry for rendered system prompt
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Rendered content
    content: String,
    /// System prompt file modification time
    system_prompt_mtime: SystemTime,
    /// Modification times of all referenced partials
    partial_mtimes: Vec<(String, SystemTime)>,
}

/// Global cache for rendered system prompt
static SYSTEM_PROMPT_CACHE: Mutex<Option<CacheEntry>> = Mutex::new(None);

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

    /// Get file modification time
    fn get_mtime(path: &Path) -> std::result::Result<SystemTime, SystemPromptError> {
        let metadata = std::fs::metadata(path)?;
        Ok(metadata.modified()?)
    }

    /// Extract partial names from template content
    fn extract_partial_names(&self, content: &str) -> Vec<String> {
        let render_regex = regex::Regex::new(r#"\{\s*%\s*render\s*"([^"]+)"\s*%\s*\}"#)
            .expect("Failed to compile render regex");

        render_regex
            .captures_iter(content)
            .map(|cap| cap[1].to_string())
            .collect()
    }

    /// Get modification times for all partials referenced in the template
    fn get_partial_mtimes(
        &self,
        content: &str,
    ) -> std::result::Result<Vec<(String, SystemTime)>, SystemPromptError> {
        let partial_names = self.extract_partial_names(content);
        let mut partial_mtimes = Vec::new();

        for partial_name in partial_names {
            // Try to find the partial in the prompt library
            if let Ok(prompt) = self.prompt_library.get(&partial_name) {
                if let Some(path) = &prompt.source {
                    let mtime = Self::get_mtime(path)?;
                    partial_mtimes.push((partial_name.clone(), mtime));
                }
            } else {
                // Try common file extensions
                let extensions = [".md", ".markdown", ".liquid", ".md.liquid"];
                let mut found = false;

                for ext in &extensions {
                    let name_with_ext = format!("{}{}", partial_name, ext);
                    if let Ok(prompt) = self.prompt_library.get(&name_with_ext) {
                        if let Some(path) = &prompt.source {
                            let mtime = Self::get_mtime(path)?;
                            partial_mtimes.push((partial_name.clone(), mtime));
                            found = true;
                            break;
                        }
                    }
                }

                if !found {
                    return Err(SystemPromptError::PartialNotFound(partial_name));
                }
            }
        }

        Ok(partial_mtimes)
    }

    /// Check if cache is still valid
    fn is_cache_valid(&self, cache_entry: &CacheEntry, system_prompt_path: &Path) -> bool {
        // Check system prompt file modification time
        match Self::get_mtime(system_prompt_path) {
            Ok(current_mtime) => {
                if current_mtime != cache_entry.system_prompt_mtime {
                    return false;
                }
            }
            Err(_) => return false,
        }

        // Check all partial modification times
        for (partial_name, cached_mtime) in &cache_entry.partial_mtimes {
            if let Ok(prompt) = self.prompt_library.get(partial_name) {
                if let Some(path) = &prompt.source {
                    match Self::get_mtime(path) {
                        Ok(current_mtime) => {
                            if current_mtime != *cached_mtime {
                                return false;
                            }
                        }
                        Err(_) => return false,
                    }
                }
            }
        }

        true
    }

    /// Render the system prompt with caching
    pub fn render(&self) -> std::result::Result<String, SystemPromptError> {
        // Find the system prompt file
        let system_prompt_path = self.find_system_prompt_file()?;

        // Check cache first
        {
            let cache = SYSTEM_PROMPT_CACHE.lock().unwrap();
            if let Some(ref cache_entry) = *cache {
                if self.is_cache_valid(cache_entry, &system_prompt_path) {
                    return Ok(cache_entry.content.clone());
                }
            }
        }

        // Read the system prompt content
        let content = std::fs::read_to_string(&system_prompt_path)?;

        // Get modification times
        let system_prompt_mtime = Self::get_mtime(&system_prompt_path)?;
        let partial_mtimes = self.get_partial_mtimes(&content)?;

        // Create template with partial support
        let template = Template::with_partials(&content, Arc::clone(&self.prompt_library))?;

        // Render with configuration support
        let args = std::collections::HashMap::new();
        let rendered_content = template.render_with_config(&args)?;

        // Update cache
        {
            let mut cache = SYSTEM_PROMPT_CACHE.lock().unwrap();
            *cache = Some(CacheEntry {
                content: rendered_content.clone(),
                system_prompt_mtime,
                partial_mtimes,
            });
        }

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
/// content fully processed. The result is cached for performance, with automatic cache
/// invalidation when the system prompt file changes.
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
/// This function clears the internal cache, forcing the next call to `render_system_prompt()`
/// to re-read and re-render all template files. This is useful for testing or when you want
/// to ensure fresh rendering.
pub fn clear_cache() {
    let mut cache = SYSTEM_PROMPT_CACHE.lock().unwrap();
    *cache = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_extract_partial_names() {
        let renderer = SystemPromptRenderer::new().expect("Failed to create renderer");

        let content = r#"
            Some content
            {% render "principals" %}
            More content
            {% render "coding_standards" %}
            Final content
        "#;

        let partial_names = renderer.extract_partial_names(content);
        assert_eq!(partial_names.len(), 2);
        assert!(partial_names.contains(&"principals".to_string()));
        assert!(partial_names.contains(&"coding_standards".to_string()));
    }

    #[test]
    fn test_extract_partial_names_various_whitespace() {
        let renderer = SystemPromptRenderer::new().expect("Failed to create renderer");

        // Test various whitespace patterns
        let content = r#"
            {% render "test1" %}
            {%render "test2"%}
            {% render"test3" %}
            {%render"test4"%}
        "#;

        let partial_names = renderer.extract_partial_names(content);
        assert_eq!(partial_names.len(), 4);
        assert!(partial_names.contains(&"test1".to_string()));
        assert!(partial_names.contains(&"test2".to_string()));
        assert!(partial_names.contains(&"test3".to_string()));
        assert!(partial_names.contains(&"test4".to_string()));
    }

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
        // This should not panic
        clear_cache();

        // Verify cache is cleared
        let cache = SYSTEM_PROMPT_CACHE.lock().unwrap();
        assert!(cache.is_none());
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

    #[test]
    fn test_get_mtime() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").expect("Failed to write test file");

        let mtime = SystemPromptRenderer::get_mtime(&test_file);
        assert!(mtime.is_ok());
    }

    #[test]
    fn test_get_mtime_file_not_found() {
        let non_existent = Path::new("/non/existent/file.txt");
        let mtime = SystemPromptRenderer::get_mtime(non_existent);
        assert!(mtime.is_err());
    }
}
