//! Adapter to make PromptLibrary work with the new templating domain crate
//!
//! This module provides an adapter that implements the PartialLoader trait
//! for PromptLibrary, allowing the existing prompt system to work with the
//! new swissarmyhammer-templating domain crate.

use crate::PromptLibrary;
use std::borrow::Cow;
use std::sync::Arc;
use swissarmyhammer_templating::PartialLoader;

/// Template file extensions for partial support
const TEMPLATE_EXTENSIONS: &[&str] = &[".md", ".markdown", ".liquid", ".md.liquid"];

/// Adapter to make PromptLibrary work with PartialLoader trait
#[derive(Debug)]
pub struct PromptPartialAdapter {
    library: Arc<PromptLibrary>,
    names: Vec<String>,
}

impl PromptPartialAdapter {
    /// Create a new adapter around a PromptLibrary
    pub fn new(library: Arc<PromptLibrary>) -> Self {
        let mut names = Vec::new();
        if let Ok(prompts) = library.list() {
            for prompt in prompts.iter() {
                names.push(prompt.name.clone());

                // Strip common prompt extensions to make them available as partials
                for ext in TEMPLATE_EXTENSIONS {
                    if let Some(name_without_ext) = prompt.name.strip_suffix(ext) {
                        names.push(name_without_ext.to_string());
                    }
                }
            }
        }
        Self { library, names }
    }

    /// Create a new adapter from a storage backend
    pub fn from_storage(storage: &dyn crate::StorageBackend) -> Self {
        let mut names = Vec::new();
        if let Ok(prompts) = storage.list() {
            for prompt in prompts.iter() {
                names.push(prompt.name.clone());

                // Strip common prompt extensions to make them available as partials
                for ext in TEMPLATE_EXTENSIONS {
                    if let Some(name_without_ext) = prompt.name.strip_suffix(ext) {
                        names.push(name_without_ext.to_string());
                    }
                }
            }
        }

        // Create a temporary library wrapper around the storage
        let mut library = PromptLibrary::new();
        if let Ok(prompts) = storage.list() {
            for prompt in prompts {
                let _ = library.add(prompt);
            }
        }

        Self {
            library: Arc::new(library),
            names,
        }
    }
}

impl PartialLoader for PromptPartialAdapter {
    fn contains(&self, name: &str) -> bool {
        tracing::debug!(
            "PromptPartialAdapter::contains called with name: '{}'",
            name
        );

        // Try exact name first
        if self.library.get(name).is_ok() {
            return true;
        }

        // Try with various prompt file extensions
        for ext in TEMPLATE_EXTENSIONS {
            let name_with_ext = format!("{name}{ext}");
            if self.library.get(&name_with_ext).is_ok() {
                return true;
            }
        }

        // If the name already has an extension, try stripping it
        if name.contains('.') {
            // Try stripping each known extension
            for ext in TEMPLATE_EXTENSIONS {
                if let Some(name_without_ext) = name.strip_suffix(ext) {
                    if self.library.get(name_without_ext).is_ok() {
                        return true;
                    }
                    // Also try with other extensions
                    for other_ext in TEMPLATE_EXTENSIONS {
                        if ext != other_ext {
                            let name_with_other_ext = format!("{name_without_ext}{other_ext}");
                            if self.library.get(&name_with_other_ext).is_ok() {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        tracing::debug!("No match found for partial '{}'", name);
        false
    }

    fn names(&self) -> Vec<String> {
        self.names.clone()
    }

    fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
        tracing::debug!("PromptPartialAdapter::try_get called with name: '{}'", name);

        // Try exact name first
        if let Ok(prompt) = self.library.get(name) {
            return Some(Cow::Owned(prompt.template));
        }

        // Try with various prompt file extensions
        for ext in TEMPLATE_EXTENSIONS {
            let name_with_ext = format!("{name}{ext}");
            if let Ok(prompt) = self.library.get(&name_with_ext) {
                return Some(Cow::Owned(prompt.template));
            }
        }

        // If the name already has an extension, try stripping it
        if name.contains('.') {
            // Try stripping each known extension
            for ext in TEMPLATE_EXTENSIONS {
                if let Some(name_without_ext) = name.strip_suffix(ext) {
                    if let Ok(prompt) = self.library.get(name_without_ext) {
                        return Some(Cow::Owned(prompt.template));
                    }
                    // Also try with other extensions
                    for other_ext in TEMPLATE_EXTENSIONS {
                        if ext != other_ext {
                            let name_with_other_ext = format!("{name_without_ext}{other_ext}");
                            if let Ok(prompt) = self.library.get(&name_with_other_ext) {
                                return Some(Cow::Owned(prompt.template));
                            }
                        }
                    }
                }
            }
        }

        tracing::debug!("No match found for partial '{}'", name);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Prompt, PromptLibrary};

    #[test]
    fn test_prompt_partial_adapter() {
        let mut library = PromptLibrary::new();
        let _ = library.add(Prompt {
            name: "header".to_string(),
            template: "# Header".to_string(),
            description: Some("Header template".to_string()),
            category: None,
            tags: vec![],
            parameters: vec![],
            source: None,
            metadata: std::collections::HashMap::new(),
        });
        let _ = library.add(Prompt {
            name: "footer.md".to_string(),
            template: "Footer content".to_string(),
            description: Some("Footer template".to_string()),
            category: None,
            tags: vec![],
            parameters: vec![],
            source: None,
            metadata: std::collections::HashMap::new(),
        });

        let adapter = PromptPartialAdapter::new(Arc::new(library));

        // Test contains
        assert!(adapter.contains("header"));
        assert!(adapter.contains("footer.md"));
        assert!(adapter.contains("footer")); // Should work without extension
        assert!(!adapter.contains("missing"));

        // Test try_get
        let header = adapter.try_get("header").unwrap();
        assert_eq!(header, "# Header");

        let footer = adapter.try_get("footer.md").unwrap();
        assert_eq!(footer, "Footer content");

        let footer_no_ext = adapter.try_get("footer").unwrap();
        assert_eq!(footer_no_ext, "Footer content");

        assert!(adapter.try_get("missing").is_none());
    }
}
