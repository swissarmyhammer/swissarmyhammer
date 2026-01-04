//! Adapter for using prompts as Liquid template partials
//!
//! This module provides the integration between prompt libraries and
//! the Liquid template engine's partial system.

use crate::prompts::PromptLibrary;
use std::borrow::Cow;
use std::sync::Arc;
use swissarmyhammer_common::Pretty;
use swissarmyhammer_templating::partials::normalize_partial_name;

/// Adapter that allows prompts to be used as Liquid template partials
#[derive(Debug)]
pub struct PromptPartialAdapter {
    library: Arc<PromptLibrary>,
}

impl PromptPartialAdapter {
    /// Create a new prompt partial adapter
    pub fn new(library: Arc<PromptLibrary>) -> Self {
        Self { library }
    }

    /// Get the library reference
    pub fn library(&self) -> &PromptLibrary {
        &self.library
    }
}

impl swissarmyhammer_templating::PartialLoader for PromptPartialAdapter {
    fn contains(&self, name: &str) -> bool {
        tracing::trace!(
            "PromptPartialAdapter::contains called with name: '{}'",
            name
        );

        // Try the requested name and all normalized variants
        let candidates = normalize_partial_name(name);
        tracing::trace!("Trying candidates: {}", Pretty(&candidates));

        for candidate in candidates {
            if self.library.get(&candidate).is_ok() {
                tracing::trace!("Found matching partial: '{}'", candidate);
                return true;
            }
        }

        tracing::error!("No matching partial found for: '{}'", name);
        false
    }

    fn names(&self) -> Vec<String> {
        let names = self.library.list_names().unwrap_or_default();
        tracing::trace!("PromptPartialAdapter::names returning: {}", Pretty(&names));
        names
    }

    fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
        tracing::trace!("PromptPartialAdapter::try_get called with name: '{}'", name);

        // Try the requested name and all normalized variants
        let candidates = normalize_partial_name(name);
        tracing::trace!("Trying candidates: {}", Pretty(&candidates));

        for candidate in candidates {
            if let Ok(prompt) = self.library.get(&candidate) {
                tracing::trace!("Found matching partial: '{}'", candidate);
                return Some(Cow::Owned(prompt.template.clone()));
            }
        }

        tracing::error!("No matching partial found for: '{}'", name);
        None
    }
}

impl liquid::partials::PartialSource for PromptPartialAdapter {
    fn contains(&self, name: &str) -> bool {
        // Try the requested name and all normalized variants
        let candidates = normalize_partial_name(name);
        for candidate in candidates {
            if self.library.get(&candidate).is_ok() {
                return true;
            }
        }
        false
    }

    fn names(&self) -> Vec<&str> {
        // Return empty slice for now to avoid lifetime issues
        Vec::new()
    }

    fn try_get(&self, name: &str) -> Option<std::borrow::Cow<'_, str>> {
        // Try the requested name and all normalized variants
        let candidates = normalize_partial_name(name);
        for candidate in candidates {
            if let Ok(prompt) = self.library.get(&candidate) {
                return Some(std::borrow::Cow::Owned(prompt.template.clone()));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::{Prompt, PromptLibrary};
    use swissarmyhammer_templating::PartialLoader;

    #[test]
    fn test_prompt_partial_adapter() {
        let mut library = PromptLibrary::new();
        let prompt = Prompt::new("test_partial", "Hello from partial!");
        library.add(prompt).unwrap();

        let adapter = PromptPartialAdapter::new(Arc::new(library));

        assert!(PartialLoader::contains(&adapter, "test_partial"));
        assert!(!PartialLoader::contains(&adapter, "nonexistent"));

        let names = PartialLoader::names(&adapter);
        assert_eq!(names.len(), 1);
        assert!(names.contains(&"test_partial".to_string()));

        let content = PartialLoader::try_get(&adapter, "test_partial");
        assert!(content.is_some());

        let nonexistent_content = PartialLoader::try_get(&adapter, "nonexistent");
        assert!(nonexistent_content.is_none());
    }

    #[test]
    fn test_adapter_with_multiple_prompts() {
        let mut library = PromptLibrary::new();
        library.add(Prompt::new("partial1", "Content 1")).unwrap();
        library.add(Prompt::new("partial2", "Content 2")).unwrap();
        library.add(Prompt::new("partial3", "Content 3")).unwrap();

        let adapter = PromptPartialAdapter::new(Arc::new(library));

        assert_eq!(PartialLoader::names(&adapter).len(), 3);
        assert!(PartialLoader::contains(&adapter, "partial1"));
        assert!(PartialLoader::contains(&adapter, "partial2"));
        assert!(PartialLoader::contains(&adapter, "partial3"));
    }
}
