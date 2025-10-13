//! Per-file rule ignore directive parsing and matching
//!
//! This module provides functionality to parse ignore directives from source files
//! and check if rules should be skipped based on those directives.
//!
//! # Ignore Directive Syntax
//!
//! Ignore directives can appear anywhere in a file using the format:
//! ```text
//! <comment-syntax> sah rule ignore <rule-name-glob>
//! ```
//!
//! Supported comment syntaxes:
//! - `//` (C-style line comments)
//! - `#` (Shell/Python-style comments)
//! - `/*` (C-style block comments)
//! - `<!--` (HTML comments)
//!
//! # Examples
//!
//! ```
//! use swissarmyhammer_rules::ignore::{parse_ignore_directives, should_ignore_rule};
//!
//! let content = "// sah rule ignore no-unwrap\n// sah rule ignore test-*\nfn main() {}";
//! let patterns = parse_ignore_directives(content);
//! assert_eq!(patterns.len(), 2);
//!
//! assert!(should_ignore_rule("no-unwrap", &patterns));
//! assert!(should_ignore_rule("test-something", &patterns));
//! assert!(!should_ignore_rule("other-rule", &patterns));
//! ```

/// Parse ignore directives from file content
///
/// Scans the content for lines matching the ignore directive pattern and
/// extracts the rule name/glob patterns.
///
/// # Arguments
///
/// * `content` - The file content to parse
///
/// # Returns
///
/// A vector of rule name patterns to ignore (may contain glob patterns)
///
/// # Examples
///
/// ```
/// use swissarmyhammer_rules::ignore::parse_ignore_directives;
///
/// let content = "// sah rule ignore no-unwrap\n# sah rule ignore test-*\n";
/// let patterns = parse_ignore_directives(content);
/// assert_eq!(patterns, vec!["no-unwrap", "test-*"]);
/// ```
pub fn parse_ignore_directives(content: &str) -> Vec<String> {
    // Simple line-by-line parsing to avoid regex complexity with special characters
    let mut patterns = Vec::new();

    for line in content.lines() {
        // Look for "sah" followed by "rule" and "ignore", handling cases where
        // comment syntax might be attached (like "//sah" instead of "// sah")
        let words: Vec<&str> = line.split_whitespace().collect();

        // Look for the pattern "sah rule ignore <pattern>"
        for i in 0..words.len().saturating_sub(3) {
            // Check if word ends with "sah" (handles "//sah", "/*sah", etc.)
            let is_sah = words[i] == "sah" || words[i].ends_with("sah");

            if is_sah && words[i + 1] == "rule" && words[i + 2] == "ignore" {
                // The next word (if it exists) is the pattern
                if let Some(&pattern_word) = words.get(i + 3) {
                    let pattern = pattern_word.to_string();
                    tracing::trace!("Parsed ignore directive: '{}'", pattern);
                    patterns.push(pattern);
                }
                break; // Only match once per line
            }
        }
    }

    tracing::debug!("Found {} ignore directives in content", patterns.len());
    patterns
}

/// Check if a rule should be ignored based on ignore patterns
///
/// Matches the rule name against each pattern using glob matching.
/// Supports wildcards: `*` (matches any sequence) and `?` (matches single char).
///
/// # Arguments
///
/// * `rule_name` - The name of the rule to check
/// * `patterns` - The ignore patterns from the file
///
/// # Returns
///
/// `true` if the rule matches any ignore pattern, `false` otherwise
///
/// # Examples
///
/// ```
/// use swissarmyhammer_rules::ignore::should_ignore_rule;
///
/// let patterns = vec!["no-*".to_string(), "specific-rule".to_string()];
///
/// assert!(should_ignore_rule("no-unwrap", &patterns));
/// assert!(should_ignore_rule("no-panic", &patterns));
/// assert!(should_ignore_rule("specific-rule", &patterns));
/// assert!(!should_ignore_rule("other-rule", &patterns));
/// ```
pub fn should_ignore_rule(rule_name: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| {
        // Try glob pattern matching first
        if pattern.contains('*') || pattern.contains('?') {
            glob::Pattern::new(pattern)
                .map(|glob_pattern| glob_pattern.matches(rule_name))
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        "Invalid glob pattern '{}' in ignore directive: {}",
                        pattern,
                        e
                    );
                    false
                })
        } else {
            // Exact match for non-glob patterns
            rule_name == pattern
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ignore_directives_empty() {
        let content = "fn main() {}";
        let patterns = parse_ignore_directives(content);
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_parse_ignore_directives_line_comment() {
        let content = "// sah rule ignore no-unwrap\nfn main() {}";
        let patterns = parse_ignore_directives(content);
        assert_eq!(patterns, vec!["no-unwrap"]);
    }

    #[test]
    fn test_parse_ignore_directives_hash_comment() {
        let content = "# sah rule ignore no-print\ndef main(): pass";
        let patterns = parse_ignore_directives(content);
        assert_eq!(patterns, vec!["no-print"]);
    }

    #[test]
    fn test_parse_ignore_directives_block_comment() {
        let content = "/* sah rule ignore no-unwrap */\nfn main() {}";
        let patterns = parse_ignore_directives(content);
        assert_eq!(patterns, vec!["no-unwrap"]);
    }

    #[test]
    fn test_parse_ignore_directives_html_comment() {
        let content = "<!-- sah rule ignore no-inline-styles -->\n<div>Content</div>";
        let patterns = parse_ignore_directives(content);
        assert_eq!(patterns, vec!["no-inline-styles"]);
    }

    #[test]
    fn test_parse_ignore_directives_multiple() {
        let content = "// sah rule ignore no-unwrap\n// sah rule ignore test-*\nfn main() {}";
        let patterns = parse_ignore_directives(content);
        assert_eq!(patterns, vec!["no-unwrap", "test-*"]);
    }

    #[test]
    fn test_parse_ignore_directives_glob_patterns() {
        let content =
            "// sah rule ignore no-*\n// sah rule ignore *-unwrap\n// sah rule ignore test-?-rule";
        let patterns = parse_ignore_directives(content);
        assert_eq!(patterns, vec!["no-*", "*-unwrap", "test-?-rule"]);
    }

    #[test]
    fn test_parse_ignore_directives_whitespace_variations() {
        let content = "//sah rule ignore no-spaces\n//  sah  rule  ignore  extra-spaces";
        let patterns = parse_ignore_directives(content);
        assert_eq!(patterns, vec!["no-spaces", "extra-spaces"]);
    }

    #[test]
    fn test_should_ignore_rule_exact_match() {
        let patterns = vec!["no-unwrap".to_string(), "specific-rule".to_string()];

        assert!(should_ignore_rule("no-unwrap", &patterns));
        assert!(should_ignore_rule("specific-rule", &patterns));
        assert!(!should_ignore_rule("other-rule", &patterns));
    }

    #[test]
    fn test_should_ignore_rule_glob_wildcard() {
        let patterns = vec!["no-*".to_string()];

        assert!(should_ignore_rule("no-unwrap", &patterns));
        assert!(should_ignore_rule("no-panic", &patterns));
        assert!(should_ignore_rule("no-anything", &patterns));
        assert!(!should_ignore_rule("yes-something", &patterns));
    }

    #[test]
    fn test_should_ignore_rule_glob_suffix() {
        let patterns = vec!["*-unwrap".to_string()];

        assert!(should_ignore_rule("no-unwrap", &patterns));
        assert!(should_ignore_rule("allow-unwrap", &patterns));
        assert!(!should_ignore_rule("unwrap-test", &patterns));
    }

    #[test]
    fn test_should_ignore_rule_glob_question_mark() {
        let patterns = vec!["test-?-rule".to_string()];

        assert!(should_ignore_rule("test-a-rule", &patterns));
        assert!(should_ignore_rule("test-1-rule", &patterns));
        assert!(!should_ignore_rule("test-ab-rule", &patterns));
        assert!(!should_ignore_rule("test--rule", &patterns));
    }

    #[test]
    fn test_should_ignore_rule_multiple_patterns() {
        let patterns = vec![
            "no-*".to_string(),
            "specific-rule".to_string(),
            "*-test".to_string(),
        ];

        assert!(should_ignore_rule("no-unwrap", &patterns));
        assert!(should_ignore_rule("specific-rule", &patterns));
        assert!(should_ignore_rule("unit-test", &patterns));
        assert!(!should_ignore_rule("other-rule", &patterns));
    }

    #[test]
    fn test_should_ignore_rule_invalid_glob_pattern() {
        // Invalid glob pattern with unclosed bracket
        let patterns = vec!["test-[".to_string()];

        // Should return false and log warning (not crash)
        assert!(!should_ignore_rule("test-something", &patterns));
    }

    #[test]
    fn test_should_ignore_rule_empty_patterns() {
        let patterns: Vec<String> = vec![];

        assert!(!should_ignore_rule("any-rule", &patterns));
    }

    #[test]
    fn test_parse_ignore_directives_mixed_comment_styles() {
        let content = r#"
// sah rule ignore no-unwrap
# sah rule ignore no-print
/* sah rule ignore no-panic */
<!-- sah rule ignore no-inline-styles -->
fn main() {}
"#;
        let patterns = parse_ignore_directives(content);
        assert_eq!(
            patterns,
            vec!["no-unwrap", "no-print", "no-panic", "no-inline-styles"]
        );
    }

    #[test]
    fn test_parse_ignore_directives_ignores_non_matching() {
        let content = r#"
// This is a regular comment
// sah rule ignore no-unwrap
// Another regular comment
// TODO: fix this
fn main() {}
"#;
        let patterns = parse_ignore_directives(content);
        assert_eq!(patterns, vec!["no-unwrap"]);
    }
}
