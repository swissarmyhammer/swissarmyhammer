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
//! Prompts are hidden only through explicit metadata flags: `partial: true` or `hidden: true`.
//!
//! # Example
//!
//! ```rust
//! use swissarmyhammer_common::prompt_visibility::is_prompt_visible;
//! use serde_json::json;
//!
//! // Regular prompt - visible
//! assert!(is_prompt_visible("test", Some("Run tests"), None));
//!
//! // Prompt with hidden: true - hidden
//! let meta = json!({"hidden": true});
//! assert!(!is_prompt_visible(".check", Some("System prompt"), Some(&meta)));
//!
//! // Prompt with partial: true - hidden
//! let meta = json!({"partial": true});
//! assert!(!is_prompt_visible("_header", Some("Header partial"), Some(&meta)));
//! ```

use serde_json::Value;

/// Checks if a prompt should be visible as a slash command.
///
/// This is the single source of truth for prompt visibility. A prompt is hidden if
/// its metadata contains `partial: true` or `hidden: true`.
///
/// # Arguments
///
/// * `name` - The prompt name (unused but kept for backward compatibility)
/// * `description` - Optional prompt description (unused but kept for backward compatibility)
/// * `meta` - Optional metadata (typically from MCP `Prompt.meta` field)
///
/// # Returns
///
/// `true` if the prompt should be visible as a slash command, `false` if it should be hidden.
pub fn is_prompt_visible(name: &str, description: Option<&str>, meta: Option<&Value>) -> bool {
    !is_prompt_partial(name, description, meta)
}

/// Checks if a prompt is a partial template or hidden.
///
/// This checks the metadata for `partial: true` or `hidden: true` flags.
/// Note: This cannot check for `{% partial %}` directive since it only has access
/// to metadata. Use `Prompt.is_partial_template()` to also check the template directive.
///
/// # Arguments
///
/// * `name` - The prompt name (unused but kept for backward compatibility)
/// * `description` - Optional prompt description (unused but kept for backward compatibility)
/// * `meta` - Optional metadata (typically from MCP `Prompt.meta` field)
///
/// # Returns
///
/// `true` if the prompt is a partial or hidden, `false` otherwise.
pub fn is_prompt_partial(_name: &str, _description: Option<&str>, meta: Option<&Value>) -> bool {
    // Check metadata for explicit flags
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
        // Names with special prefixes are visible without metadata flags
        assert!(is_prompt_visible("_header", Some("Header"), None));
        assert!(is_prompt_visible(".check", Some("System prompt"), None));
        assert!(is_prompt_visible("my-partial-template", None, None));
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
        use serde_json::json;

        let test_cases = vec![
            ("test", Some("Test prompt"), None),
            ("_header", Some("Header"), None),
            ("my_partial", None, None),
            (".check", Some("System prompt"), None),
            ("test", Some("Test"), Some(json!({"hidden": true}))),
            ("test", Some("Test"), Some(json!({"partial": true}))),
        ];

        for (name, desc, meta) in test_cases {
            let meta_ref = meta.as_ref();
            assert_eq!(
                is_prompt_visible(name, desc, meta_ref),
                !is_prompt_partial(name, desc, meta_ref),
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
