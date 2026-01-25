//! Adapter for using prompts as Liquid template partials
//!
//! This module provides the integration between prompt libraries and
//! the Liquid template engine's partial system.
//!
//! Note: This is a thin wrapper around the generic [`LibraryPartialAdapter`] from
//! `swissarmyhammer_templating`. The prompts library implements [`TemplateContentProvider`],
//! which enables it to work with the unified partial adapter system.

use crate::prompts::PromptLibrary;
use std::sync::Arc;
use swissarmyhammer_templating::partials::LibraryPartialAdapter;

/// Adapter that allows prompts to be used as Liquid template partials.
///
/// This is a type alias for the generic [`LibraryPartialAdapter`] specialized
/// for [`PromptLibrary`]. The underlying library implements [`TemplateContentProvider`],
/// enabling it to work with the unified partial system.
pub type PromptPartialAdapter = LibraryPartialAdapter<PromptLibrary>;

/// Create a new prompt partial adapter from a library Arc.
///
/// This is a convenience function that creates a `PromptPartialAdapter`
/// (which is a `LibraryPartialAdapter<PromptLibrary>`).
pub fn new_prompt_partial_adapter(library: Arc<PromptLibrary>) -> PromptPartialAdapter {
    LibraryPartialAdapter::new(library)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::{Prompt, PromptLibrary};
    use swissarmyhammer_templating::partials::TemplateContentProvider;
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

    #[test]
    fn test_template_content_provider_impl() {
        let mut library = PromptLibrary::new();
        library
            .add(Prompt::new("test", "Template content"))
            .unwrap();

        // Test TemplateContentProvider directly
        assert!(library.get_template_content("test").is_some());
        assert_eq!(
            library.get_template_content("test").unwrap(),
            "Template content"
        );
        assert!(library.get_template_content("nonexistent").is_none());

        let names = library.list_template_names();
        assert_eq!(names.len(), 1);
        assert!(names.contains(&"test".to_string()));
    }

    #[test]
    fn test_convenience_function() {
        let mut library = PromptLibrary::new();
        library.add(Prompt::new("test", "Content")).unwrap();

        let adapter = new_prompt_partial_adapter(Arc::new(library));
        assert!(PartialLoader::contains(&adapter, "test"));
    }
}
