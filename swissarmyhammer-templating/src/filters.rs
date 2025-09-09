//! Custom template filters for text processing
//!
//! This module provides custom Liquid filters for common text transformations
//! used in templates.

use regex::Regex;
use std::collections::HashMap;

/// Convert string to URL-friendly slug
pub fn slugify_string(input: &str) -> String {
    input
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c.is_whitespace() || c == '-' || c == '_' {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Count lines in a string
pub fn count_lines_in_string(input: &str) -> i32 {
    if input.is_empty() {
        0
    } else {
        input.lines().count() as i32
    }
}

/// Indent each line of a string with the specified number of spaces
pub fn indent_string(input: &str, indent_count: usize) -> String {
    let indent = " ".repeat(indent_count);
    input
        .lines()
        .map(|line| format!("{indent}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Preprocess template string to handle custom filters
///
/// This function handles custom filters that aren't natively supported by Liquid
/// by preprocessing the template string and replacing the filter expressions
/// with their computed values.
pub fn preprocess_custom_filters(template_str: &str, args: &HashMap<String, String>) -> String {
    let mut processed = template_str.to_string();

    // Handle slugify filter: {{ variable | slugify }}
    let slugify_regex = Regex::new(r"\{\{\s*(\w+)\s*\|\s*slugify\s*\}\}")
        .expect("Failed to compile slugify regex");

    for cap in slugify_regex.captures_iter(template_str) {
        let var_name = &cap[1];
        let full_match = &cap[0];

        if let Some(value) = args.get(var_name) {
            let slugified = slugify_string(value);
            processed = processed.replace(full_match, &slugified);
        }
    }

    // Handle count_lines filter: {{ variable | count_lines }}
    let count_lines_regex = Regex::new(r"\{\{\s*(\w+)\s*\|\s*count_lines\s*\}\}")
        .expect("Failed to compile count_lines regex");

    for cap in count_lines_regex.captures_iter(template_str) {
        let var_name = &cap[1];
        let full_match = &cap[0];

        if let Some(value) = args.get(var_name) {
            let line_count = count_lines_in_string(value);
            processed = processed.replace(full_match, &line_count.to_string());
        }
    }

    // Handle indent filter: {{ variable | indent: N }}
    let indent_regex = Regex::new(r"\{\{\s*(\w+)\s*\|\s*indent:\s*(\d+)\s*\}\}")
        .expect("Failed to compile indent regex");

    for cap in indent_regex.captures_iter(template_str) {
        let var_name = &cap[1];
        let indent_str = &cap[2];
        let full_match = &cap[0];

        if let Some(value) = args.get(var_name) {
            if let Ok(indent_count) = indent_str.parse::<usize>() {
                let indented = indent_string(value, indent_count);
                processed = processed.replace(full_match, &indented);
            }
        }
    }

    processed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_function() {
        assert_eq!(slugify_string("Hello World!"), "hello-world");
        assert_eq!(slugify_string("Test_String-123"), "test-string-123");
        assert_eq!(slugify_string("   Multiple   Spaces   "), "multiple-spaces");
        assert_eq!(slugify_string("Special@#$Characters"), "specialcharacters");
        assert_eq!(slugify_string(""), "");
    }

    #[test]
    fn test_count_lines_function() {
        assert_eq!(count_lines_in_string("line1\nline2\nline3"), 3);
        assert_eq!(count_lines_in_string("single line"), 1);
        assert_eq!(count_lines_in_string(""), 0);
        assert_eq!(count_lines_in_string("line1\n\nline3"), 3);
        assert_eq!(count_lines_in_string("line1\n"), 1);
    }

    #[test]
    fn test_indent_function() {
        assert_eq!(indent_string("line1\nline2", 2), "  line1\n  line2");
        assert_eq!(indent_string("single", 4), "    single");
        assert_eq!(indent_string("", 2), "");
        assert_eq!(indent_string("line1\n\nline3", 1), " line1\n \n line3");
    }

    #[test]
    fn test_preprocess_slugify_filter() {
        let mut args = HashMap::new();
        args.insert("title".to_string(), "Hello World!".to_string());

        let result = preprocess_custom_filters("{{ title | slugify }}", &args);
        assert_eq!(result, "hello-world");
    }

    #[test]
    fn test_preprocess_count_lines_filter() {
        let mut args = HashMap::new();
        args.insert("text".to_string(), "line1\nline2\nline3".to_string());

        let result = preprocess_custom_filters("{{ text | count_lines }}", &args);
        assert_eq!(result, "3");
    }

    #[test]
    fn test_preprocess_indent_filter() {
        let mut args = HashMap::new();
        args.insert("text".to_string(), "line1\nline2".to_string());

        let result = preprocess_custom_filters("{{ text | indent: 2 }}", &args);
        assert_eq!(result, "  line1\n  line2");
    }

    #[test]
    fn test_preprocess_multiple_filters() {
        let mut args = HashMap::new();
        args.insert("title".to_string(), "Hello World".to_string());
        args.insert("content".to_string(), "line1\nline2".to_string());

        let template = "Title: {{ title | slugify }}\nLines: {{ content | count_lines }}";
        let result = preprocess_custom_filters(template, &args);
        assert_eq!(result, "Title: hello-world\nLines: 2");
    }

    #[test]
    fn test_preprocess_no_matching_variables() {
        let args = HashMap::new();
        let template = "{{ unknown | slugify }}";
        let result = preprocess_custom_filters(template, &args);
        // Should remain unchanged if variable doesn't exist
        assert_eq!(result, "{{ unknown | slugify }}");
    }
}