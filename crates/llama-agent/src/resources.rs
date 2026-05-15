use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during resource loading operations.
#[derive(Debug, Error)]
pub enum ResourceError {
    /// The requested resource was not found in the resource loader.
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// The resource exists but could not be parsed or processed.
    #[error("Resource parsing failed: {0}")]
    ParseError(String),
}

/// A resource loader that provides access to embedded static resources.
///
/// Resources are embedded at compile time using the `include_str!` macro,
/// making them available without filesystem access at runtime.
pub struct ResourceLoader {
    resources: HashMap<String, &'static str>,
}

impl ResourceLoader {
    /// Creates a new ResourceLoader with all available embedded resources.
    ///
    /// Currently includes:
    /// - `compaction.md`: Default compaction prompt template
    ///
    /// # Example
    /// ```
    /// use llama_agent::ResourceLoader;
    ///
    /// let loader = ResourceLoader::new();
    /// let resource = loader.load_resource("compaction.md").unwrap();
    /// assert!(resource.contains("System Instructions"));
    /// ```
    pub fn new() -> Self {
        let mut resources = HashMap::new();
        resources.insert(
            "compaction.md".to_string(),
            include_str!("../resources/compaction.md"),
        );
        Self { resources }
    }

    /// Loads a resource by name, returning its content as a string slice.
    ///
    /// # Arguments
    /// * `name` - The name of the resource to load (e.g., "compaction.md")
    ///
    /// # Returns
    /// * `Ok(&str)` - The resource content if found
    /// * `Err(ResourceError::NotFound)` - If the resource doesn't exist
    ///
    /// # Example
    /// ```
    /// use llama_agent::ResourceLoader;
    ///
    /// let loader = ResourceLoader::new();
    /// match loader.load_resource("compaction.md") {
    ///     Ok(content) => println!("Resource loaded: {} bytes", content.len()),
    ///     Err(e) => eprintln!("Failed to load resource: {}", e),
    /// }
    /// ```
    pub fn load_resource(&self, name: &str) -> Result<&str, ResourceError> {
        self.resources
            .get(name)
            .copied()
            .ok_or_else(|| ResourceError::NotFound(name.to_string()))
    }

    /// Lists all available resource names.
    ///
    /// # Returns
    /// A vector of resource names that can be loaded with `load_resource()`.
    ///
    /// # Example
    /// ```
    /// use llama_agent::ResourceLoader;
    ///
    /// let loader = ResourceLoader::new();
    /// let available = loader.list_resources();
    /// println!("Available resources: {:?}", available);
    /// ```
    pub fn list_resources(&self) -> Vec<&str> {
        self.resources.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for ResourceLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_loader_creation() {
        let loader = ResourceLoader::new();
        assert!(!loader.resources.is_empty());
    }

    #[test]
    fn test_resource_loader_default() {
        let loader = ResourceLoader::default();
        assert!(!loader.resources.is_empty());
    }

    #[test]
    fn test_load_compaction_resource() {
        let loader = ResourceLoader::new();
        let result = loader.load_resource("compaction.md");

        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("System Instructions"));
        assert!(content.contains("User Prompt Template"));
        assert!(content.contains("{conversation_history}"));
    }

    #[test]
    fn test_load_nonexistent_resource() {
        let loader = ResourceLoader::new();
        let result = loader.load_resource("nonexistent.md");

        assert!(result.is_err());
        match result.unwrap_err() {
            ResourceError::NotFound(name) => assert_eq!(name, "nonexistent.md"),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_list_resources() {
        let loader = ResourceLoader::new();
        let resources = loader.list_resources();

        assert!(resources.contains(&"compaction.md"));
        assert!(!resources.is_empty());
    }

    #[test]
    fn test_integration_resource_to_compaction_prompt() {
        use crate::types::CompactionPrompt;

        // Test the complete integration from resource loading to prompt creation
        let loader = ResourceLoader::new();
        let resource_content = loader.load_resource("compaction.md").unwrap();

        // Verify the resource contains expected structure
        assert!(resource_content.contains("# System Instructions"));
        assert!(resource_content.contains("# User Prompt Template"));

        // Test parsing the resource into a CompactionPrompt
        let prompt = CompactionPrompt::from_resource(resource_content).unwrap();

        // Verify the parsed prompt has valid content
        assert!(!prompt.system_instructions.is_empty());
        assert!(!prompt.user_prompt_template.is_empty());
        assert!(prompt
            .user_prompt_template
            .contains("{conversation_history}"));

        // Test template rendering
        let test_history = "User: Hello\nAssistant: Hi there!";
        let rendered = prompt.render_user_prompt(test_history);

        assert!(rendered.contains("User: Hello"));
        assert!(rendered.contains("Assistant: Hi there!"));
        assert!(!rendered.contains("{conversation_history}"));
    }

    #[test]
    fn test_integration_resource_file_accessibility() {
        // Verify that the resource file is properly embedded and accessible
        let loader = ResourceLoader::new();

        // Should be able to create multiple loaders
        let loader2 = ResourceLoader::new();

        // Both should have access to the same resources
        let content1 = loader.load_resource("compaction.md").unwrap();
        let content2 = loader2.load_resource("compaction.md").unwrap();

        assert_eq!(content1, content2);
        assert!(content1.len() > 100); // Should have substantial content

        // Verify the content structure makes sense
        assert!(content1.lines().count() > 5); // Should have multiple lines
        assert!(content1.contains("compaction")); // Should be about compaction
    }

    #[test]
    fn test_integration_error_handling_workflow() {
        use crate::types::CompactionPrompt;

        let loader = ResourceLoader::new();

        // Test missing resource error handling
        match loader.load_resource("missing.md") {
            Err(ResourceError::NotFound(name)) => assert_eq!(name, "missing.md"),
            _ => panic!("Expected NotFound error"),
        }

        // Test malformed resource content
        let malformed_content = "This is not a valid compaction prompt format";
        match CompactionPrompt::from_resource(malformed_content) {
            Err(ResourceError::ParseError(_)) => {} // Expected
            _ => panic!("Expected ParseError for malformed content"),
        }
    }
}
