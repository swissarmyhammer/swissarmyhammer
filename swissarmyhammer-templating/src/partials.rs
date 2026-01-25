//! Partial template loading system
//!
//! This module provides a trait-based system for loading partial templates
//! from various sources, allowing for flexible integration with different
//! storage backends.
//!
//! ## Unified Partial Adapter
//!
//! The [`LibraryPartialAdapter`] provides a unified way to use any library
//! (prompts, rules, validators, etc.) as a partial source. Libraries just need
//! to implement the [`TemplateContentProvider`] trait:
//!
//! ```rust,ignore
//! use swissarmyhammer_templating::partials::{TemplateContentProvider, LibraryPartialAdapter};
//! use std::sync::Arc;
//!
//! // Your library implements TemplateContentProvider
//! impl TemplateContentProvider for MyLibrary {
//!     fn get_template_content(&self, name: &str) -> Option<String> {
//!         self.get(name).ok().map(|item| item.template.clone())
//!     }
//!     fn list_template_names(&self) -> Vec<String> {
//!         self.list_names().unwrap_or_default()
//!     }
//! }
//!
//! // Then wrap it with LibraryPartialAdapter
//! let adapter = LibraryPartialAdapter::new(Arc::new(my_library));
//! // adapter implements both PartialLoader and liquid::partials::PartialSource
//! ```

use crate::error::{Result, TemplatingError};
use liquid_core::{Language, ParseTag, Renderable, Runtime, TagReflection, TagTokenIter};
use std::borrow::Cow;
use std::io::Write;
use std::sync::Arc;

/// Template file extensions for partial support
pub const TEMPLATE_EXTENSIONS: &[&str] = &[".md", ".markdown", ".liquid", ".md.liquid"];

/// Trait for types that can provide template content by name.
///
/// This is the unified interface that libraries (prompts, rules, validators, etc.)
/// implement to participate in the partial template system. By implementing this
/// trait, a library can be wrapped with [`LibraryPartialAdapter`] to work as
/// both a [`PartialLoader`] and a `liquid::partials::PartialSource`.
///
/// # Example
///
/// ```rust,ignore
/// use swissarmyhammer_templating::partials::TemplateContentProvider;
///
/// impl TemplateContentProvider for MyLibrary {
///     fn get_template_content(&self, name: &str) -> Option<String> {
///         self.get(name).ok().map(|item| item.template.clone())
///     }
///
///     fn list_template_names(&self) -> Vec<String> {
///         self.list_names().unwrap_or_default()
///     }
/// }
/// ```
pub trait TemplateContentProvider: Send + Sync + std::fmt::Debug {
    /// Get the template content for a given name.
    ///
    /// Returns `Some(content)` if the name exists, `None` otherwise.
    fn get_template_content(&self, name: &str) -> Option<String>;

    /// List all available template names.
    fn list_template_names(&self) -> Vec<String>;
}

/// A generic partial adapter that works with any [`TemplateContentProvider`].
///
/// This eliminates the need for separate `PromptPartialAdapter`, `RulePartialAdapter`,
/// etc. by providing a single unified adapter that can wrap any library implementing
/// the `TemplateContentProvider` trait.
///
/// # Example
///
/// ```rust,ignore
/// use swissarmyhammer_templating::partials::{LibraryPartialAdapter, TemplateContentProvider};
/// use std::sync::Arc;
///
/// let library = MyLibrary::new();
/// let adapter = LibraryPartialAdapter::new(Arc::new(library));
///
/// // Use with Liquid templates
/// let template = Template::with_partials("{% include 'header' %}", adapter)?;
/// ```
#[derive(Debug, Clone)]
pub struct LibraryPartialAdapter<T: TemplateContentProvider> {
    library: Arc<T>,
}

impl<T: TemplateContentProvider> LibraryPartialAdapter<T> {
    /// Create a new adapter wrapping a library.
    pub fn new(library: Arc<T>) -> Self {
        Self { library }
    }

    /// Get a reference to the underlying library.
    pub fn library(&self) -> &T {
        &self.library
    }

    /// Get the library Arc for cloning.
    pub fn library_arc(&self) -> &Arc<T> {
        &self.library
    }
}

impl<T: TemplateContentProvider> PartialLoader for LibraryPartialAdapter<T> {
    fn contains(&self, name: &str) -> bool {
        tracing::trace!(
            "LibraryPartialAdapter::contains called with name: '{}'",
            name
        );

        // Try the requested name and all normalized variants
        let candidates = normalize_partial_name(name);
        tracing::trace!("Trying candidates: {:?}", candidates);

        for candidate in candidates {
            if self.library.get_template_content(&candidate).is_some() {
                tracing::trace!("Found matching partial: '{}'", candidate);
                return true;
            }
        }

        tracing::warn!("No matching partial found for: '{}'", name);
        false
    }

    fn names(&self) -> Vec<String> {
        let names = self.library.list_template_names();
        tracing::trace!("LibraryPartialAdapter::names returning: {:?}", names);
        names
    }

    fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
        tracing::trace!(
            "LibraryPartialAdapter::try_get called with name: '{}'",
            name
        );

        // Try the requested name and all normalized variants
        let candidates = normalize_partial_name(name);
        tracing::trace!("Trying candidates: {:?}", candidates);

        for candidate in candidates {
            if let Some(content) = self.library.get_template_content(&candidate) {
                tracing::trace!("Found matching partial: '{}'", candidate);
                return Some(Cow::Owned(content));
            }
        }

        tracing::warn!("No matching partial found for: '{}'", name);
        None
    }
}

impl<T: TemplateContentProvider> liquid::partials::PartialSource for LibraryPartialAdapter<T> {
    fn contains(&self, name: &str) -> bool {
        // Try the requested name and all normalized variants
        let candidates = normalize_partial_name(name);
        for candidate in candidates {
            if self.library.get_template_content(&candidate).is_some() {
                return true;
            }
        }
        false
    }

    fn names(&self) -> Vec<&str> {
        // Return empty slice to avoid lifetime issues
        // (the underlying data isn't stored in this adapter)
        Vec::new()
    }

    fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
        // Try the requested name and all normalized variants
        let candidates = normalize_partial_name(name);
        for candidate in candidates {
            if let Some(content) = self.library.get_template_content(&candidate) {
                return Some(Cow::Owned(content));
            }
        }
        None
    }
}

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

    // If the request ends with .liquid (added by liquid engine), try without it
    if let Some(name_without_liquid) = requested_name.strip_suffix(".liquid") {
        candidates.push(name_without_liquid.to_string());

        // Also try with other extensions after stripping .liquid
        for ext in TEMPLATE_EXTENSIONS {
            let name_with_ext = format!("{name_without_liquid}{ext}");
            candidates.push(name_with_ext);
        }
    }

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
    candidates
        .into_iter()
        .filter(|name| seen.insert(name.clone()))
        .collect()
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
        tracing::trace!(
            "PartialLoaderAdapter::contains called with name: '{}'",
            name
        );

        // Try exact name and normalized variants
        let candidates = normalize_partial_name(name);
        for candidate in candidates {
            if self.loader.contains(&candidate) {
                tracing::trace!("Found partial: '{}'", candidate);
                return true;
            }
        }

        tracing::error!("No match found for partial '{}'", name);
        false
    }

    fn names(&self) -> Vec<&str> {
        self.names_cache.iter().map(|s| s.as_str()).collect()
    }

    fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
        tracing::trace!("PartialLoaderAdapter::try_get called with name: '{}'", name);

        // Try exact name and normalized variants
        let candidates = normalize_partial_name(name);
        for candidate in candidates {
            if let Some(content) = self.loader.try_get(&candidate) {
                tracing::trace!("Loaded partial: '{}'", candidate);
                return Some(content);
            }
        }

        tracing::error!("No match found for partial '{}'", name);
        None
    }
}

/// A simple PartialLoader implementation backed by a HashMap.
///
/// This provides a reusable way to create partial loaders from any collection
/// of string content. Useful for validators, rules, and other systems that
/// need templating support.
///
/// # Example
///
/// ```rust
/// use swissarmyhammer_templating::partials::HashMapPartialLoader;
/// use swissarmyhammer_templating::PartialLoader;
/// use std::collections::HashMap;
///
/// let mut partials = HashMap::new();
/// partials.insert("header".to_string(), "# My Header".to_string());
/// partials.insert("footer".to_string(), "---\nFooter".to_string());
///
/// let loader = HashMapPartialLoader::new(partials);
/// assert!(loader.contains("header"));
/// assert_eq!(loader.get("header").unwrap(), "# My Header");
/// ```
#[derive(Debug, Clone)]
pub struct HashMapPartialLoader {
    partials: std::collections::HashMap<String, String>,
}

impl HashMapPartialLoader {
    /// Create a new HashMapPartialLoader from a HashMap of partials.
    pub fn new(partials: std::collections::HashMap<String, String>) -> Self {
        Self { partials }
    }

    /// Create an empty HashMapPartialLoader.
    pub fn empty() -> Self {
        Self {
            partials: std::collections::HashMap::new(),
        }
    }

    /// Add a partial to the loader.
    pub fn add(&mut self, name: impl Into<String>, content: impl Into<String>) {
        self.partials.insert(name.into(), content.into());
    }

    /// Get the number of partials.
    pub fn len(&self) -> usize {
        self.partials.len()
    }

    /// Check if the loader is empty.
    pub fn is_empty(&self) -> bool {
        self.partials.is_empty()
    }
}

impl PartialLoader for HashMapPartialLoader {
    fn contains(&self, name: &str) -> bool {
        // Try exact name and normalized variants
        let candidates = normalize_partial_name(name);
        candidates.iter().any(|c| self.partials.contains_key(c))
    }

    fn names(&self) -> Vec<String> {
        self.partials.keys().cloned().collect()
    }

    fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
        // Try exact name first
        if let Some(content) = self.partials.get(name) {
            return Some(Cow::Borrowed(content.as_str()));
        }

        // Try normalized variants
        let candidates = normalize_partial_name(name);
        for candidate in candidates {
            if let Some(content) = self.partials.get(&candidate) {
                return Some(Cow::Borrowed(content.as_str()));
            }
        }

        None
    }
}

impl liquid::partials::PartialSource for HashMapPartialLoader {
    fn contains(&self, name: &str) -> bool {
        PartialLoader::contains(self, name)
    }

    fn names(&self) -> Vec<&str> {
        self.partials.keys().map(|s| s.as_str()).collect()
    }

    fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
        PartialLoader::try_get(self, name)
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
            partials.insert(
                "sidebar.liquid".to_string(),
                "Sidebar {{ title }}".to_string(),
            );

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

    #[test]
    fn test_hashmap_partial_loader() {
        let mut partials = HashMap::new();
        partials.insert("header".to_string(), "# Header Content".to_string());
        partials.insert("footer.md".to_string(), "Footer text".to_string());

        let loader = HashMapPartialLoader::new(partials);

        assert_eq!(loader.len(), 2);
        assert!(!loader.is_empty());

        // Direct matches (using PartialLoader trait)
        assert!(PartialLoader::contains(&loader, "header"));
        assert!(PartialLoader::contains(&loader, "footer.md"));

        // Get content
        let header = PartialLoader::get(&loader, "header").unwrap();
        assert_eq!(header, "# Header Content");

        // Normalized name lookup (header.md should find header)
        assert!(PartialLoader::contains(&loader, "header.md"));
    }

    #[test]
    fn test_hashmap_partial_loader_empty() {
        let loader = HashMapPartialLoader::empty();
        assert!(loader.is_empty());
        assert_eq!(loader.len(), 0);
        assert!(!PartialLoader::contains(&loader, "anything"));
    }

    #[test]
    fn test_hashmap_partial_loader_add() {
        let mut loader = HashMapPartialLoader::empty();
        loader.add("test", "Test content");

        assert!(PartialLoader::contains(&loader, "test"));
        assert_eq!(PartialLoader::get(&loader, "test").unwrap(), "Test content");
    }

    #[test]
    fn test_hashmap_partial_loader_as_partial_source() {
        let mut partials = HashMap::new();
        partials.insert("snippet".to_string(), "Snippet content".to_string());

        let loader = HashMapPartialLoader::new(partials);

        // Test PartialSource trait methods directly
        assert!(PartialSource::contains(&loader, "snippet"));
        let names = PartialSource::names(&loader);
        assert!(names.contains(&"snippet"));
        let content = PartialSource::try_get(&loader, "snippet").unwrap();
        assert_eq!(content, "Snippet content");
    }

    /// A mock library that implements TemplateContentProvider for testing
    #[derive(Debug)]
    struct MockLibrary {
        items: HashMap<String, String>,
    }

    impl MockLibrary {
        fn new() -> Self {
            let mut items = HashMap::new();
            items.insert("header".to_string(), "# Header".to_string());
            items.insert("footer".to_string(), "---\nFooter".to_string());
            items.insert(
                "_partials/shared".to_string(),
                "{% partial %}\nShared content".to_string(),
            );
            Self { items }
        }
    }

    impl TemplateContentProvider for MockLibrary {
        fn get_template_content(&self, name: &str) -> Option<String> {
            self.items.get(name).cloned()
        }

        fn list_template_names(&self) -> Vec<String> {
            self.items.keys().cloned().collect()
        }
    }

    #[test]
    fn test_library_partial_adapter() {
        let library = MockLibrary::new();
        let adapter = LibraryPartialAdapter::new(Arc::new(library));

        // Test PartialLoader trait
        assert!(PartialLoader::contains(&adapter, "header"));
        assert!(PartialLoader::contains(&adapter, "footer"));
        assert!(PartialLoader::contains(&adapter, "_partials/shared"));
        assert!(!PartialLoader::contains(&adapter, "nonexistent"));

        let header = PartialLoader::try_get(&adapter, "header").unwrap();
        assert_eq!(header, "# Header");

        let names = PartialLoader::names(&adapter);
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn test_library_partial_adapter_as_partial_source() {
        let library = MockLibrary::new();
        let adapter = LibraryPartialAdapter::new(Arc::new(library));

        // Test PartialSource trait
        assert!(PartialSource::contains(&adapter, "header"));
        let content = PartialSource::try_get(&adapter, "header").unwrap();
        assert_eq!(content, "# Header");
    }

    #[test]
    fn test_library_partial_adapter_library_access() {
        let library = MockLibrary::new();
        let adapter = LibraryPartialAdapter::new(Arc::new(library));

        // Test access to underlying library
        let lib_ref = adapter.library();
        assert!(lib_ref.get_template_content("header").is_some());

        // Test Arc access - library_arc returns a reference to the Arc
        let arc_ref = adapter.library_arc();
        assert_eq!(Arc::strong_count(arc_ref), 1); // Only the adapter holds it

        // When we clone it, the count increases
        let _cloned = Arc::clone(arc_ref);
        assert_eq!(Arc::strong_count(arc_ref), 2);
    }
}
