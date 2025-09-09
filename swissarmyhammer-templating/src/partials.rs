//! Partial template loading system
//!
//! This module provides a trait-based system for loading partial templates
//! from various sources, allowing for flexible integration with different
//! storage backends.

use crate::error::{Result, TemplatingError};
use liquid_core::{Language, ParseTag, Renderable, Runtime, TagReflection, TagTokenIter};
use std::borrow::Cow;
use std::io::Write;

/// Template file extensions for partial support
pub const TEMPLATE_EXTENSIONS: &[&str] = &[".md", ".markdown", ".liquid", ".md.liquid"];

/// Trait for loading partial templates from various sources
pub trait PartialLoader: Send + Sync + std::fmt::Debug {
    /// Check if a partial with the given name exists
    fn contains(&self, name: &str) -> bool;

    /// Get the names of all available partials
    fn names(&self) -> Vec<String>;

    /// Try to load a partial template by name
    fn try_get(&self, name: &str) -> Option<Cow<'_, str>>;

    /// Load a partial template by name, returning an error if not found
    fn get(&self, name: &str) -> Result<String> {
        self.try_get(name)
            .map(|s| s.into_owned())
            .ok_or_else(|| TemplatingError::Partial(format!("Partial '{}' not found", name)))
    }
}

/// Custom partial tag that acts as a no-op marker for liquid partial files
#[derive(Clone, Debug, Default)]
pub struct PartialTag;

impl PartialTag {
    /// Create a new PartialTag
    pub fn new() -> Self {
        Self
    }
}

impl TagReflection for PartialTag {
    fn tag(&self) -> &'static str {
        "partial"
    }

    fn description(&self) -> &'static str {
        "Marks a file as a partial template (no-op)"
    }
}

impl ParseTag for PartialTag {
    fn parse(
        &self,
        mut arguments: TagTokenIter<'_>,
        _options: &Language,
    ) -> liquid_core::Result<Box<dyn Renderable>> {
        // Consume any arguments (though we expect none)
        arguments.expect_nothing()?;

        // Return a no-op renderable
        Ok(Box::new(PartialRenderable))
    }

    fn reflection(&self) -> &dyn TagReflection {
        self
    }
}

/// Renderable for the partial tag (does nothing)
#[derive(Debug, Clone)]
struct PartialRenderable;

impl Renderable for PartialRenderable {
    fn render_to(
        &self,
        _output: &mut dyn Write,
        _context: &dyn Runtime,
    ) -> liquid_core::Result<()> {
        // No-op: this tag doesn't render anything
        Ok(())
    }
}

/// Helper function to normalize partial names by trying different extensions
pub fn normalize_partial_name(requested_name: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    
    // Try exact name first
    candidates.push(requested_name.to_string());

    // Try with various prompt file extensions
    for ext in TEMPLATE_EXTENSIONS {
        let name_with_ext = format!("{requested_name}{ext}");
        candidates.push(name_with_ext);
    }

    // If the name already has an extension, try stripping it and adding others
    if requested_name.contains('.') {
        for ext in TEMPLATE_EXTENSIONS {
            if let Some(name_without_ext) = requested_name.strip_suffix(ext) {
                candidates.push(name_without_ext.to_string());
                // Also try with other extensions
                for other_ext in TEMPLATE_EXTENSIONS {
                    if ext != other_ext {
                        let name_with_other_ext = format!("{name_without_ext}{other_ext}");
                        candidates.push(name_with_other_ext);
                    }
                }
            }
        }
    }

    // Remove duplicates while preserving order
    let mut seen = std::collections::HashSet::new();
    candidates.into_iter().filter(|name| seen.insert(name.clone())).collect()
}

/// Adapter to make any PartialLoader work with Liquid's PartialSource trait
#[derive(Debug)]
pub struct PartialLoaderAdapter<T: PartialLoader> {
    loader: T,
    names_cache: Vec<String>,
}

impl<T: PartialLoader> PartialLoaderAdapter<T> {
    /// Create a new adapter around a PartialLoader
    pub fn new(loader: T) -> Self {
        let names_cache = loader.names();
        Self {
            loader,
            names_cache,
        }
    }

    /// Get a reference to the underlying loader
    pub fn loader(&self) -> &T {
        &self.loader
    }
}

impl<T: PartialLoader> liquid::partials::PartialSource for PartialLoaderAdapter<T> {
    fn contains(&self, name: &str) -> bool {
        tracing::debug!("PartialLoaderAdapter::contains called with name: '{}'", name);

        // Try exact name and normalized variants
        let candidates = normalize_partial_name(name);
        for candidate in candidates {
            if self.loader.contains(&candidate) {
                tracing::debug!("Found partial: '{}'", candidate);
                return true;
            }
        }

        tracing::debug!("No match found for partial '{}'", name);
        false
    }

    fn names(&self) -> Vec<&str> {
        self.names_cache.iter().map(|s| s.as_str()).collect()
    }

    fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
        tracing::debug!("PartialLoaderAdapter::try_get called with name: '{}'", name);

        // Try exact name and normalized variants
        let candidates = normalize_partial_name(name);
        for candidate in candidates {
            if let Some(content) = self.loader.try_get(&candidate) {
                tracing::debug!("Loaded partial: '{}'", candidate);
                return Some(content);
            }
        }

        tracing::debug!("No match found for partial '{}'", name);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquid::partials::PartialSource;
    use std::collections::HashMap;

    /// Test implementation of PartialLoader for testing
    #[derive(Debug)]
    struct TestPartialLoader {
        partials: HashMap<String, String>,
    }

    impl TestPartialLoader {
        fn new() -> Self {
            let mut partials = HashMap::new();
            partials.insert("header".to_string(), "# Header".to_string());
            partials.insert("footer.md".to_string(), "Footer content".to_string());
            partials.insert("sidebar.liquid".to_string(), "Sidebar {{ title }}".to_string());
            
            Self { partials }
        }
    }

    impl PartialLoader for TestPartialLoader {
        fn contains(&self, name: &str) -> bool {
            self.partials.contains_key(name)
        }

        fn names(&self) -> Vec<String> {
            self.partials.keys().cloned().collect()
        }

        fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
            self.partials.get(name).map(|s| Cow::Borrowed(s.as_str()))
        }
    }

    #[test]
    fn test_partial_loader_basic() {
        let loader = TestPartialLoader::new();
        
        assert!(loader.contains("header"));
        assert!(loader.contains("footer.md"));
        assert!(!loader.contains("nonexistent"));
        
        let header = loader.get("header").unwrap();
        assert_eq!(header, "# Header");
        
        let result = loader.get("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_partial_name() {
        let candidates = normalize_partial_name("header");
        assert!(candidates.contains(&"header".to_string()));
        assert!(candidates.contains(&"header.md".to_string()));
        assert!(candidates.contains(&"header.liquid".to_string()));

        let candidates = normalize_partial_name("footer.md");
        assert!(candidates.contains(&"footer.md".to_string()));
        assert!(candidates.contains(&"footer".to_string()));
        assert!(candidates.contains(&"footer.liquid".to_string()));
    }

    #[test]
    fn test_partial_loader_adapter() {
        let loader = TestPartialLoader::new();
        let adapter = PartialLoaderAdapter::new(loader);

        // Direct matches
        assert!(adapter.contains("header"));
        assert!(adapter.contains("footer.md"));
        
        // Should find content
        let header = adapter.try_get("header").unwrap();
        assert_eq!(header, "# Header");
        
        let footer = adapter.try_get("footer.md").unwrap();
        assert_eq!(footer, "Footer content");
        
        // Non-existent partial
        assert!(!adapter.contains("missing"));
        assert!(adapter.try_get("missing").is_none());
    }

    #[test]
    fn test_partial_tag() {
        let tag = PartialTag::new();
        assert_eq!(tag.tag(), "partial");
        assert!(!tag.description().is_empty());
    }
}