//! Prompt visibility utilities for determining which prompts should be exposed as slash commands.
//!
//! This module provides a centralized, single source of truth for determining whether
//! a prompt should be visible to users as a slash command. Both llama-agent and claude-agent
//! use this module to filter prompts consistently.
//!
//! # Design
//!
//! The visibility check uses primitive parameters (`&str`, `Option<&serde_json::Value>`) rather
//! than specific prompt types, allowing it to work with any prompt representation (MCP, internal, etc.).
//!
//! # Example
//!
//! ```rust
//! use swissarmyhammer_common::prompt_visibility::is_prompt_visible;
//!
//! // Regular prompt - visible
//! assert!(is_prompt_visible("test", Some("Run tests"), None));
//!
//! // Partial prompt by underscore convention - hidden
//! assert!(!is_prompt_visible("_header", Some("Header partial"), None));
//!
//! // System prompt by dot convention - hidden
//! assert!(!is_prompt_visible(".system", Some("System prompt"), None));
//!
//! // Partial prompt by description - hidden
//! assert!(!is_prompt_visible("utils", Some("Partial template for reuse in other prompts"), None));
//! ```

use serde_json::Value;

/// Checks if a prompt should be visible as a slash command.
///
/// This is the single source of truth for prompt visibility. A prompt is hidden if ANY
/// of the following conditions are met:
///
/// 1. Name starts with underscore (`_`) - convention for partials
/// 2. Name starts with dot (`.`) - convention for system prompts
/// 3. Name contains "partial" (case-insensitive)
/// 4. Description equals "Partial template for reuse in other prompts"
/// 5. Metadata contains `partial: true` or `hidden: true`
///
/// # Arguments
///
/// * `name` - The prompt name
/// * `description` - Optional prompt description
/// * `meta` - Optional metadata (typically from MCP `Prompt.meta` field)
///
/// # Returns
///
/// `true` if the prompt should be visible as a slash command, `false` if it should be hidden.
pub fn is_prompt_visible(name: &str, description: Option<&str>, meta: Option<&Value>) -> bool {
    !is_prompt_partial(name, description, meta)
}

/// Checks if a prompt is a partial template (internal use only).
///
/// This is the inverse of `is_prompt_visible`. Use this when you need to explicitly
/// check if something is a partial rather than checking visibility.
///
/// # Arguments
///
/// * `name` - The prompt name
/// * `description` - Optional prompt description
/// * `meta` - Optional metadata (typically from MCP `Prompt.meta` field)
///
/// # Returns
///
/// `true` if the prompt is a partial template, `false` otherwise.
pub fn is_prompt_partial(name: &str, description: Option<&str>, meta: Option<&Value>) -> bool {
    // Check 1: Name starts with underscore (convention for partials)
    if name.starts_with('_') {
        return true;
    }

    // Check 2: Name starts with dot (convention for system prompts like .system, .check)
    if name.starts_with('.') {
        return true;
    }

    // Check 3: Name contains "partial" (case-insensitive)
    if name.to_lowercase().contains("partial") {
        return true;
    }

    // Check 4: Description matches partial template description
    if let Some(desc) = description {
        if desc == "Partial template for reuse in other prompts" {
            return true;
        }
    }

    // Check 5: Metadata indicates partial or hidden
    if let Some(meta_value) = meta {
        // Check for partial: true
        if let Some(partial) = meta_value.get("partial") {
            if partial.as_bool() == Some(true) {
                return true;
            }
        }

        // Check for hidden: true
        if let Some(hidden) = meta_value.get("hidden") {
            if hidden.as_bool() == Some(true) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_regular_prompt_is_visible() {
        assert!(is_prompt_visible("test", Some("Run tests"), None));
        assert!(is_prompt_visible(
            "commit",
            Some("Create a git commit"),
            None
        ));
        assert!(is_prompt_visible("review", None, None));
    }

    #[test]
    fn test_underscore_prefix_is_hidden() {
        assert!(!is_prompt_visible("_header", Some("Header partial"), None));
        assert!(!is_prompt_visible("_footer", None, None));
        assert!(!is_prompt_visible(
            "_utils",
            Some("Utility functions"),
            None
        ));
    }

    #[test]
    fn test_dot_prefix_is_hidden() {
        assert!(!is_prompt_visible(".system", Some("System prompt"), None));
        assert!(!is_prompt_visible(".check", None, None));
        assert!(!is_prompt_visible(
            ".internal",
            Some("Internal use only"),
            None
        ));
    }

    #[test]
    fn test_partial_in_name_is_hidden() {
        assert!(!is_prompt_visible("tool_partial", Some("A partial"), None));
        assert!(!is_prompt_visible("my-partial-template", None, None));
        assert!(!is_prompt_visible(
            "PARTIAL",
            Some("Uppercase partial"),
            None
        ));
        assert!(!is_prompt_visible("somePartialThing", None, None));
    }

    #[test]
    fn test_partial_description_is_hidden() {
        assert!(!is_prompt_visible(
            "utils",
            Some("Partial template for reuse in other prompts"),
            None
        ));
    }

    #[test]
    fn test_similar_description_is_visible() {
        // Similar but not exact match - should be visible
        assert!(is_prompt_visible(
            "utils",
            Some("A partial template for reuse"),
            None
        ));
        assert!(is_prompt_visible(
            "utils",
            Some("partial template for reuse in other prompts"), // lowercase 'p'
            None
        ));
    }

    #[test]
    fn test_meta_partial_true_is_hidden() {
        let meta = json!({"partial": true});
        assert!(!is_prompt_visible("test", Some("Test prompt"), Some(&meta)));
    }

    #[test]
    fn test_meta_partial_false_is_visible() {
        let meta = json!({"partial": false});
        assert!(is_prompt_visible("test", Some("Test prompt"), Some(&meta)));
    }

    #[test]
    fn test_meta_hidden_true_is_hidden() {
        let meta = json!({"hidden": true});
        assert!(!is_prompt_visible("test", Some("Test prompt"), Some(&meta)));
    }

    #[test]
    fn test_meta_hidden_false_is_visible() {
        let meta = json!({"hidden": false});
        assert!(is_prompt_visible("test", Some("Test prompt"), Some(&meta)));
    }

    #[test]
    fn test_meta_without_partial_or_hidden_is_visible() {
        let meta = json!({"author": "test", "version": "1.0"});
        assert!(is_prompt_visible("test", Some("Test prompt"), Some(&meta)));
    }

    #[test]
    fn test_is_prompt_partial_inverse_of_visible() {
        // Verify that is_prompt_partial is the inverse of is_prompt_visible
        let test_cases = vec![
            ("test", Some("Test prompt"), None),
            ("_header", Some("Header"), None),
            ("my_partial", None, None),
            (
                "utils",
                Some("Partial template for reuse in other prompts"),
                None,
            ),
        ];

        for (name, desc, meta) in test_cases {
            assert_eq!(
                is_prompt_visible(name, desc, meta),
                !is_prompt_partial(name, desc, meta),
                "Mismatch for name={}, desc={:?}",
                name,
                desc
            );
        }
    }

    #[test]
    fn test_empty_name_is_visible() {
        // Edge case: empty name should be visible (not a partial)
        assert!(is_prompt_visible("", None, None));
    }

    #[test]
    fn test_multiple_conditions_any_hides() {
        // If any condition matches, prompt should be hidden
        let meta = json!({"hidden": true});

        // Both underscore prefix AND hidden meta
        assert!(!is_prompt_visible("_test", Some("Test"), Some(&meta)));

        // Normal name but hidden meta
        assert!(!is_prompt_visible("test", Some("Test"), Some(&meta)));
    }
}
