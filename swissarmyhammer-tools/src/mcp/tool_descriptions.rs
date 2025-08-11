//! Tool descriptions registry for MCP operations
//!
//! This module provides access to tool descriptions that are generated at build time
//! from the description.md files in each tool directory.

use std::collections::HashMap;

// Include the generated tool descriptions from build.rs
include!(concat!(env!("OUT_DIR"), "/tool_descriptions.rs"));

/// Get description for a specific tool path
///
/// # Arguments
/// * `tool_path` - The tool path (e.g., "issues_create")
///
/// # Returns
/// * `Some(&str)` - The description if found
/// * `None` - If the tool path is not found
pub fn get_description(tool_path: &str) -> Option<&'static str> {
    let descriptions = get_tool_descriptions();
    descriptions.get(tool_path).copied()
}

/// List all available tool descriptions
///
/// # Returns
/// * `Vec<(&str, &str)>` - Vector of (tool_path, description) tuples
pub fn list_all_descriptions() -> Vec<(&'static str, &'static str)> {
    let descriptions = get_tool_descriptions();
    descriptions.iter().map(|(&k, &v)| (k, v)).collect()
}

/// Get description for a tool by noun and verb
///
/// # Arguments
/// * `noun` - The tool noun (e.g., "issues")
/// * `verb` - The tool verb (e.g., "create")
///
/// # Returns
/// * `Some(&str)` - The description if found
/// * `None` - If the tool is not found
///
/// # Example
/// ```rust
/// use crate::mcp::tool_descriptions::get_tool_description;
/// let desc = get_tool_description("issues", "create");
/// assert!(desc.is_some());
/// ```
pub fn get_tool_description(noun: &str, verb: &str) -> Option<&'static str> {
    let tool_path = format!("{noun}_{verb}");
    get_description(&tool_path)
}

/// Check if a tool description exists
///
/// # Arguments
/// * `noun` - The tool noun (e.g., "issues")
/// * `verb` - The tool verb (e.g., "create")
///
/// # Returns
/// * `bool` - True if the description exists, false otherwise
pub fn has_tool_description(noun: &str, verb: &str) -> bool {
    get_tool_description(noun, verb).is_some()
}

/// Get all tool descriptions grouped by noun
///
/// # Returns
/// * `HashMap<String, Vec<(String, &str)>>` - Map of noun to list of (verb, description) pairs
pub fn get_descriptions_by_noun() -> HashMap<String, Vec<(String, &'static str)>> {
    let descriptions = get_tool_descriptions();
    let mut grouped = HashMap::new();

    for (tool_path, description) in descriptions {
        if let Some((noun, verb)) = tool_path.split_once('_') {
            grouped
                .entry(noun.to_string())
                .or_insert_with(Vec::new)
                .push((verb.to_string(), description));
        }
    }

    // Sort verbs within each noun group
    for verbs in grouped.values_mut() {
        verbs.sort_by_key(|(verb, _)| verb.clone());
    }

    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_descriptions_available() {
        let descriptions = list_all_descriptions();
        assert!(!descriptions.is_empty(), "No tool descriptions found");
    }

    #[test]
    fn test_get_tool_description() {
        // Test that we can get descriptions for known tools
        assert!(get_tool_description("issues", "create").is_some());
        assert!(get_tool_description("memoranda", "create").is_some());
    }

    #[test]
    fn test_has_tool_description() {
        assert!(has_tool_description("issues", "create"));
        assert!(has_tool_description("memoranda", "create"));
        assert!(!has_tool_description("nonexistent", "tool"));
    }

    #[test]
    fn test_get_descriptions_by_noun() {
        let grouped = get_descriptions_by_noun();
        assert!(grouped.contains_key("issues"));
        assert!(grouped.contains_key("memoranda"));

        // Check that issues has expected verbs
        let issues = &grouped["issues"];
        let verbs: Vec<&str> = issues.iter().map(|(v, _)| v.as_str()).collect();
        assert!(verbs.contains(&"create"));
    }

    #[test]
    fn test_description_content_quality() {
        if let Some(create_issue_desc) = get_tool_description("issues", "create") {
            assert!(
                create_issue_desc.len() > 10,
                "Description should be substantial"
            );
            assert!(
                !create_issue_desc.trim().is_empty(),
                "Description should not be empty"
            );
        }
    }
}
