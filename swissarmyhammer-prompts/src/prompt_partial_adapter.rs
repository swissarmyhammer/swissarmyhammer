//! Adapter for using prompts as Liquid template partials
//!
//! This module provides the integration between prompt libraries and
//! the Liquid template engine's partial system.

use crate::prompts::PromptLibrary;
use std::sync::Arc;
use std::borrow::Cow;

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
        self.library.get(name).is_ok()
    }

    fn names(&self) -> Vec<String> {
        self.library.list_names().unwrap_or_default()
    }

    fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
        match self.library.get(name) {
            Ok(prompt) => Some(Cow::Owned(prompt.template.clone())),
            Err(_) => None,
        }
    }
}

impl liquid::partials::PartialSource for PromptPartialAdapter {
    fn contains(&self, name: &str) -> bool {
        self.library.get(name).is_ok()
    }

    fn names(&self) -> Vec<&str> {
        // Return empty slice for now to avoid lifetime issues
        Vec::new()
    }

    fn try_get(&self, name: &str) -> Option<std::borrow::Cow<'_, str>> {
        match self.library.get(name) {
            Ok(prompt) => Some(std::borrow::Cow::Owned(prompt.template.clone())),
            Err(_) => None,
        }
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