//! Template variable extraction utilities
//!
//! This module provides functionality to extract variable names from Liquid templates
//! for validation, initialization, and analysis purposes.

use regex::Regex;
use std::collections::HashSet;

/// Compiled regex patterns for template variable extraction
struct TemplateVariableExtractor {
    variable_re: Regex,
    tag_re: Regex,
}

impl TemplateVariableExtractor {
    fn new() -> Self {
        Self {
            // Match {{ variable }}, {{ variable.property }}, {{ variable | filter }}, etc.
            variable_re: Regex::new(r"\{\{\s*(\w+)(?:\.\w+)*\s*(?:\|[^\}]+)?\}\}")
                .expect("Failed to compile variable regex"),
            // Check for variables in {% if %}, {% unless %}, {% for %} tags
            tag_re: Regex::new(r"\{%\s*(?:if|unless|for\s+\w+\s+in)\s+(\w+)")
                .expect("Failed to compile tag regex"),
        }
    }

    fn extract(&self, template: &str) -> Vec<String> {
        let mut variables = HashSet::new();

        for cap in self.variable_re.captures_iter(template) {
            variables.insert(cap[1].to_string());
        }

        for cap in self.tag_re.captures_iter(template) {
            variables.insert(cap[1].to_string());
        }

        variables.into_iter().collect()
    }
}

/// Extract all variable names from a liquid template
///
/// This function uses thread_local storage to ensure the regex patterns
/// are compiled only once per thread for performance.
///
/// # Arguments
///
/// * `template` - The template string to analyze
///
/// # Returns
///
/// A vector of unique variable names found in the template
pub fn extract_template_variables(template: &str) -> Vec<String> {
    // Use thread_local to ensure the regex is compiled only once per thread
    thread_local! {
        static EXTRACTOR: TemplateVariableExtractor = TemplateVariableExtractor::new();
    }

    EXTRACTOR.with(|extractor| extractor.extract(template))
}

/// Create well-known template variables that are automatically available in all templates
///
/// These variables represent system-level information that templates commonly need.
/// They have the lowest precedence and can be overridden by configuration,
/// environment variables, or provided arguments.
pub fn create_well_known_variables() -> liquid::model::Object {
    let mut object = liquid::model::Object::new();

    // Add issues_directory - the standard location for issue files
    let issues_directory = determine_issues_directory();
    object.insert(
        "issues_directory".into(),
        liquid::model::Value::scalar(issues_directory.to_string_lossy().to_string()),
    );

    object
}

/// Determine the appropriate issues directory path
///
/// Uses the same logic as the main crate's issue storage to maintain consistency.
/// This will be enhanced in the future to support the swissarmyhammer/issues migration.
fn determine_issues_directory() -> std::path::PathBuf {
    match std::env::current_dir() {
        Ok(current_dir) => {
            // Future enhancement: check for swissarmyhammer/issues directory first
            // For now, use the current hardcoded behavior for backward compatibility
            current_dir.join("issues")
        }
        Err(_) => {
            // Fallback to relative path if current_dir fails
            std::path::PathBuf::from("issues")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_template_variables() {
        // Test the extract_template_variables function
        let template = "Hello {{ name }}, you have {{ count }} messages in {{ language | default: 'English' }}";
        let vars = extract_template_variables(template);

        assert!(vars.contains(&"name".to_string()));
        assert!(vars.contains(&"count".to_string()));
        assert!(vars.contains(&"language".to_string()));
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn test_extract_template_variables_with_conditionals() {
        // Test extraction from conditional tags
        let template =
            "{% if premium %}Premium user{% endif %} {% unless disabled %}Active{% endunless %}";
        let vars = extract_template_variables(template);

        assert!(vars.contains(&"premium".to_string()));
        assert!(vars.contains(&"disabled".to_string()));
        assert_eq!(vars.len(), 2);
    }

    #[test]
    fn test_extract_template_variables_whitespace_variations() {
        // Test whitespace variations in liquid templates
        let templates = vec![
            "{{name}}",
            "{{ name }}",
            "{{  name  }}",
            "{{\tname\t}}",
            "{{ name}}",
            "{{name }}",
        ];

        for template in templates {
            let vars = extract_template_variables(template);
            assert!(
                vars.contains(&"name".to_string()),
                "Failed for template: {template}"
            );
            assert_eq!(vars.len(), 1, "Failed for template: {template}");
        }
    }

    #[test]
    fn test_extract_template_variables_unicode() {
        // Test unicode characters in variable names
        // Note: Rust regex \w matches Unicode word characters by default
        let template = "Hello {{ café }}, {{ 用户名 }}, {{ user_name }}";
        let vars = extract_template_variables(template);

        // All three are valid variable names in Liquid/Rust regex
        assert!(vars.contains(&"café".to_string()));
        assert!(vars.contains(&"用户名".to_string()));
        assert!(vars.contains(&"user_name".to_string()));
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn test_extract_template_variables_long_names() {
        // Test very long template variable names
        let long_var_name = "a".repeat(100);
        let template = format!("Hello {{{{ {long_var_name} }}}}");
        let vars = extract_template_variables(&template);

        assert!(vars.contains(&long_var_name));
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn test_extract_template_variables_no_recursive_parsing() {
        // Test handling of nested/malformed template syntax
        let template = "{{ {{ inner }} }} and {{ var_{{ suffix }} }}";
        let vars = extract_template_variables(template);

        // The regex will find "inner" and "suffix" as they appear within {{ }}
        // even though the overall syntax is malformed
        assert!(vars.contains(&"inner".to_string()));
        assert!(vars.contains(&"suffix".to_string()));
        assert_eq!(vars.len(), 2);
    }

    #[test]
    fn test_extract_template_variables_duplicates() {
        // Test that duplicate variables are only counted once
        let template = "{{ name }} says hello to {{ name }} and {{ name }}";
        let vars = extract_template_variables(template);

        assert!(vars.contains(&"name".to_string()));
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn test_extract_template_variables_for_loops() {
        // Test extraction from for loops
        let template = "{% for item in items %}{{ item.name }}{% endfor %} {% for product in products %}{{ product }}{% endfor %}";
        let vars = extract_template_variables(template);

        assert!(vars.contains(&"items".to_string()));
        assert!(vars.contains(&"item".to_string()));
        assert!(vars.contains(&"products".to_string()));
        assert!(vars.contains(&"product".to_string()));
        assert_eq!(vars.len(), 4);
    }

    #[test]
    fn test_create_well_known_variables() {
        let vars = create_well_known_variables();

        // Should contain issues_directory
        assert!(vars.contains_key("issues_directory"));

        // Should be a string value
        let issues_dir_value = vars.get("issues_directory").unwrap();
        assert!(matches!(issues_dir_value, liquid::model::Value::Scalar(_)));
    }

    #[test]
    fn test_determine_issues_directory() {
        let issues_dir = determine_issues_directory();

        // Should end with "issues"
        assert!(issues_dir.to_string_lossy().ends_with("issues"));

        // Should be an absolute or relative path
        assert!(!issues_dir.to_string_lossy().is_empty());
    }
}