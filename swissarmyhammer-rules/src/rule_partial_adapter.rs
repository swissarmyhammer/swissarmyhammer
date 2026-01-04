//! Adapter for using rules as Liquid template partials
//!
//! This module provides the integration between rule libraries and
//! the Liquid template engine's partial system.

use crate::rules::RuleLibrary;
use std::borrow::Cow;
use std::sync::Arc;
use swissarmyhammer_common::Pretty;
use swissarmyhammer_templating::partials::normalize_partial_name;

/// Adapter that allows rules to be used as Liquid template partials
#[derive(Debug)]
pub struct RulePartialAdapter {
    library: Arc<RuleLibrary>,
}

impl RulePartialAdapter {
    /// Create a new rule partial adapter
    pub fn new(library: Arc<RuleLibrary>) -> Self {
        Self { library }
    }

    /// Get the library reference
    pub fn library(&self) -> &RuleLibrary {
        &self.library
    }

    /// Get the library Arc for cloning
    pub fn library_arc(&self) -> &Arc<RuleLibrary> {
        &self.library
    }
}

impl swissarmyhammer_templating::PartialLoader for RulePartialAdapter {
    fn contains(&self, name: &str) -> bool {
        tracing::trace!("RulePartialAdapter::contains called with name: '{}'", name);

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
        tracing::trace!("RulePartialAdapter::names returning: {}", Pretty(&names));
        names
    }

    fn try_get(&self, name: &str) -> Option<Cow<'_, str>> {
        tracing::trace!("RulePartialAdapter::try_get called with name: '{}'", name);

        // Try the requested name and all normalized variants
        let candidates = normalize_partial_name(name);
        tracing::trace!("Trying candidates: {}", Pretty(&candidates));

        for candidate in candidates {
            if let Ok(rule) = self.library.get(&candidate) {
                tracing::trace!("Found matching partial: '{}'", candidate);
                return Some(Cow::Owned(rule.template.clone()));
            }
        }

        tracing::error!("No matching partial found for: '{}'", name);
        None
    }
}

impl liquid::partials::PartialSource for RulePartialAdapter {
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
            if let Ok(rule) = self.library.get(&candidate) {
                return Some(std::borrow::Cow::Owned(rule.template.clone()));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rule, RuleLibrary, Severity};
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
}
