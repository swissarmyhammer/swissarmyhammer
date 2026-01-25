//! Adapter for using rules as Liquid template partials
//!
//! This module provides the integration between rule libraries and
//! the Liquid template engine's partial system.
//!
//! Note: This is a thin wrapper around the generic [`LibraryPartialAdapter`] from
//! `swissarmyhammer_templating`. The rules library implements [`TemplateContentProvider`],
//! which enables it to work with the unified partial adapter system.

use crate::rules::RuleLibrary;
use std::sync::Arc;
use swissarmyhammer_templating::partials::LibraryPartialAdapter;

/// Adapter that allows rules to be used as Liquid template partials.
///
/// This is a type alias for the generic [`LibraryPartialAdapter`] specialized
/// for [`RuleLibrary`]. The underlying library implements [`TemplateContentProvider`],
/// enabling it to work with the unified partial system.
pub type RulePartialAdapter = LibraryPartialAdapter<RuleLibrary>;

/// Create a new rule partial adapter from a library Arc.
///
/// This is a convenience function that creates a `RulePartialAdapter`
/// (which is a `LibraryPartialAdapter<RuleLibrary>`).
pub fn new_rule_partial_adapter(library: Arc<RuleLibrary>) -> RulePartialAdapter {
    LibraryPartialAdapter::new(library)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rule, RuleLibrary, Severity};
    use swissarmyhammer_templating::partials::TemplateContentProvider;
    use swissarmyhammer_templating::PartialLoader;

    #[test]
    fn test_rule_partial_adapter() {
        let mut library = RuleLibrary::new();
        let rule = Rule::new(
            "test_partial".to_string(),
            "{% partial %}\n\nHello from partial!".to_string(),
            Severity::Info,
        );
        library.add(rule).unwrap();

        let adapter = RulePartialAdapter::new(Arc::new(library));

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
    fn test_adapter_with_multiple_rules() {
        let mut library = RuleLibrary::new();
        library
            .add(Rule::new(
                "partial1".to_string(),
                "{% partial %}\n\nContent 1".to_string(),
                Severity::Info,
            ))
            .unwrap();
        library
            .add(Rule::new(
                "partial2".to_string(),
                "{% partial %}\n\nContent 2".to_string(),
                Severity::Info,
            ))
            .unwrap();
        library
            .add(Rule::new(
                "partial3".to_string(),
                "{% partial %}\n\nContent 3".to_string(),
                Severity::Info,
            ))
            .unwrap();

        let adapter = RulePartialAdapter::new(Arc::new(library));

        assert_eq!(PartialLoader::names(&adapter).len(), 3);
        assert!(PartialLoader::contains(&adapter, "partial1"));
        assert!(PartialLoader::contains(&adapter, "partial2"));
        assert!(PartialLoader::contains(&adapter, "partial3"));
    }

    #[test]
    fn test_adapter_filters_partials_from_normal_rules() {
        let mut library = RuleLibrary::new();
        library
            .add(Rule::new(
                "_partials/pass-response".to_string(),
                "{% partial %}\n\nIf no issues found, respond with \"PASS\".".to_string(),
                Severity::Info,
            ))
            .unwrap();
        library
            .add(Rule::new(
                "normal-rule".to_string(),
                "Check for issues".to_string(),
                Severity::Error,
            ))
            .unwrap();

        let adapter = RulePartialAdapter::new(Arc::new(library));

        // Both should be accessible
        assert!(PartialLoader::contains(&adapter, "_partials/pass-response"));
        assert!(PartialLoader::contains(&adapter, "normal-rule"));

        // Get names
        let names = PartialLoader::names(&adapter);
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_template_content_provider_impl() {
        let mut library = RuleLibrary::new();
        library
            .add(Rule::new(
                "test".to_string(),
                "Template content".to_string(),
                Severity::Info,
            ))
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
        let mut library = RuleLibrary::new();
        library
            .add(Rule::new(
                "test".to_string(),
                "Content".to_string(),
                Severity::Info,
            ))
            .unwrap();

        let adapter = new_rule_partial_adapter(Arc::new(library));
        assert!(PartialLoader::contains(&adapter, "test"));
    }
}
