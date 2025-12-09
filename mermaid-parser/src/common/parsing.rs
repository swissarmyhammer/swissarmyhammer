//! Shared parsing utilities to eliminate code duplication across diagram parsers
//!
//! This module provides reusable functions for common parsing operations like string handling,
//! bracket matching, and data extraction. These utilities help ensure consistent behavior
//! across all parsers while reducing code duplication.

use crate::error::{ParseError, Result};

/// Utilities for parsing quoted strings
pub mod quoted_strings {

    /// Extract content from a quoted string, handling both single and double quotes
    /// Returns the unquoted content, or the original string if not quoted
    pub fn unquote(input: &str) -> String {
        let trimmed = input.trim();

        if trimmed.len() >= 2 {
            if (trimmed.starts_with('"') && trimmed.ends_with('"'))
                || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
            {
                return trimmed[1..trimmed.len() - 1].to_string();
            }
        }

        trimmed.to_string()
    }

    /// Check if a string is properly quoted
    pub fn is_quoted(input: &str) -> bool {
        let trimmed = input.trim();
        trimmed.len() >= 2
            && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
                || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    }

    /// Parse a field that may or may not be quoted
    /// Returns (content, was_quoted)
    pub fn parse_field(input: &str) -> (String, bool) {
        let trimmed = input.trim();
        if is_quoted(trimmed) {
            (unquote(trimmed), true)
        } else {
            (trimmed.to_string(), false)
        }
    }
}

/// Utilities for key-value pair parsing
pub mod key_value {

    /// Parse a line containing key-value pairs separated by a delimiter
    pub fn parse_separated(line: &str, delimiter: char) -> Option<(String, String)> {
        line.find(delimiter).map(|pos| {
            let key = line[..pos].trim().to_string();
            let value = line[pos + 1..].trim().to_string();
            (key, value)
        })
    }

    /// Parse colon-separated key-value pair
    pub fn parse_colon_separated(line: &str) -> Option<(String, String)> {
        parse_separated(line, ':')
    }

    /// Parse equals-separated key-value pair
    pub fn parse_equals_separated(line: &str) -> Option<(String, String)> {
        parse_separated(line, '=')
    }

    /// Parse a line with multiple possible separators
    pub fn parse_multi_separator(line: &str, separators: &[char]) -> Option<(String, String)> {
        for &sep in separators {
            if let Some(result) = parse_separated(line, sep) {
                return Some(result);
            }
        }
        None
    }
}

/// Utilities for bracket and parentheses handling
pub mod brackets {

    /// Extract content between brackets [content]
    pub fn extract_square_bracket_content(input: &str) -> Option<String> {
        extract_bracket_content(input, '[', ']')
    }

    /// Extract content between parentheses (content)
    pub fn extract_paren_content(input: &str) -> Option<String> {
        extract_bracket_content(input, '(', ')')
    }

    /// Extract content between curly braces {content}
    pub fn extract_curly_bracket_content(input: &str) -> Option<String> {
        extract_bracket_content(input, '{', '}')
    }

    /// Generic bracket content extraction
    pub fn extract_bracket_content(input: &str, open: char, close: char) -> Option<String> {
        let trimmed = input.trim();
        if let Some(start) = trimmed.find(open) {
            if let Some(end) = trimmed.rfind(close) {
                if start < end {
                    return Some(trimmed[start + 1..end].to_string());
                }
            }
        }
        None
    }

    /// Parse content with optional ID prefix: "id[content]" or "[content]"
    pub fn parse_id_bracket_content(input: &str) -> (Option<String>, String) {
        let trimmed = input.trim();
        if let Some(bracket_start) = trimmed.find('[') {
            if trimmed.ends_with(']') {
                let id_part = trimmed[..bracket_start].trim();
                let content = trimmed[bracket_start + 1..trimmed.len() - 1].to_string();

                if id_part.is_empty() {
                    (None, content)
                } else {
                    (Some(id_part.to_string()), content)
                }
            } else {
                (None, trimmed.to_string())
            }
        } else {
            (None, trimmed.to_string())
        }
    }

    /// Find matching bracket position with proper nesting
    pub fn find_matching_bracket(
        input: &str,
        start_pos: usize,
        open: char,
        close: char,
    ) -> Option<usize> {
        let chars: Vec<char> = input.chars().collect();
        if start_pos >= chars.len() || chars[start_pos] != open {
            return None;
        }

        let mut depth = 1;
        for (i, &ch) in chars.iter().enumerate().skip(start_pos + 1) {
            if ch == open {
                depth += 1;
            } else if ch == close {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
        None
    }
}

/// Utilities for CSV-like field parsing
pub mod fields {
    use super::*;

    /// Parse a line of comma-separated fields, handling quoted fields
    pub fn parse_csv_line(line: &str) -> Vec<String> {
        parse_delimited_fields(line, ',')
    }

    /// Parse fields separated by any delimiter, respecting quotes
    pub fn parse_delimited_fields(line: &str, delimiter: char) -> Vec<String> {
        let mut fields = Vec::new();
        let mut current_field = String::new();
        let mut in_quotes = false;
        let mut quote_char = '"';

        for ch in line.chars() {
            if ch == '"' || ch == '\'' {
                if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                } else if ch == quote_char {
                    in_quotes = false;
                }
                current_field.push(ch);
            } else if ch == delimiter && !in_quotes {
                fields.push(quoted_strings::unquote(&current_field));
                current_field.clear();
            } else {
                current_field.push(ch);
            }
        }

        if !current_field.is_empty() {
            fields.push(quoted_strings::unquote(&current_field));
        }

        fields
    }

    /// Clean and normalize field content
    pub fn clean_field(field: &str) -> String {
        field.trim().replace("\\n", "\n").replace("\\t", "\t")
    }
}

/// Utilities for line processing and filtering
pub mod lines {

    /// Common comment prefixes used in Mermaid diagrams
    const COMMENT_PREFIXES: &[&str] = &["//", "%%"];

    /// Check if a line should be skipped (empty, whitespace, comments)
    pub fn should_skip_line(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.is_empty()
            || COMMENT_PREFIXES
                .iter()
                .any(|prefix| trimmed.starts_with(prefix))
    }

    /// Clean a line by trimming and removing common escape sequences
    pub fn clean_line(line: &str) -> String {
        line.trim().replace("\\t", "").to_string()
    }

    /// Split a line into meaningful parts, skipping empty parts
    pub fn split_line_parts(line: &str, delimiters: &[char]) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current_part = String::new();

        for ch in line.chars() {
            if delimiters.contains(&ch) {
                if !current_part.trim().is_empty() {
                    parts.push(current_part.trim().to_string());
                    current_part.clear();
                }
            } else {
                current_part.push(ch);
            }
        }

        if !current_part.trim().is_empty() {
            parts.push(current_part.trim().to_string());
        }

        parts
    }
}

/// Utilities for numeric parsing
pub mod numbers {
    use super::*;

    /// Parse a string that might contain a number with units (e.g., "10px", "5%", "3d")
    pub fn parse_number_with_unit(input: &str) -> Option<(f64, String)> {
        let trimmed = input.trim();
        let mut number_part = String::new();
        let mut unit_part = String::new();
        let mut in_number = true;

        for ch in trimmed.chars() {
            if in_number && (ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+') {
                number_part.push(ch);
            } else {
                in_number = false;
                unit_part.push(ch);
            }
        }

        if let Ok(number) = number_part.parse::<f64>() {
            Some((number, unit_part.trim().to_string()))
        } else {
            None
        }
    }

    /// Parse percentage string (e.g., "50%" -> 0.5)
    pub fn parse_percentage(input: &str) -> Option<f64> {
        if let Some((number, unit)) = parse_number_with_unit(input) {
            if unit == "%" {
                Some(number / 100.0)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Parse integer from string, returning error with line info
    pub fn parse_int_with_error(input: &str, line_num: usize, expected_desc: &str) -> Result<i32> {
        input.trim().parse().map_err(|_| ParseError::SyntaxError {
            message: format!("Invalid {}", expected_desc),
            expected: vec!["integer".to_string()],
            found: input.to_string(),
            line: line_num + 1,
            column: 1,
        })
    }

    /// Parse float from string, returning error with line info
    pub fn parse_float_with_error(
        input: &str,
        line_num: usize,
        expected_desc: &str,
    ) -> Result<f64> {
        input.trim().parse().map_err(|_| ParseError::SyntaxError {
            message: format!("Invalid {}", expected_desc),
            expected: vec!["number".to_string()],
            found: input.to_string(),
            line: line_num + 1,
            column: 1,
        })
    }
}

/// Utilities for identifier and name validation
pub mod identifiers {

    /// Check if a string is a valid identifier (alphanumeric + underscore, starts with letter)
    pub fn is_valid_identifier(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        let mut chars = name.chars();
        if let Some(first) = chars.next() {
            if !first.is_ascii_alphabetic() && first != '_' {
                return false;
            }
        }

        chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    /// Sanitize a string to create a valid identifier
    pub fn sanitize_identifier(name: &str) -> String {
        let mut result = String::new();
        let mut first = true;

        for ch in name.chars() {
            if first {
                if ch.is_ascii_alphabetic() || ch == '_' {
                    result.push(ch);
                } else if ch.is_ascii_digit() {
                    result.push('_');
                    result.push(ch);
                } else {
                    result.push('_');
                }
                first = false;
            } else if ch.is_ascii_alphanumeric() || ch == '_' {
                result.push(ch);
            } else {
                result.push('_');
            }
        }

        if result.is_empty() {
            result.push('_');
        }

        result
    }
}

/// Pattern matching utilities for common diagram patterns
pub mod patterns {

    /// Match common arrow patterns in diagrams
    /// Returns (source, arrow_type, target) if matched
    pub fn match_arrow_pattern(line: &str) -> Option<(String, String, String)> {
        let arrow_patterns = &[
            "<<-->>", "<<->>", "-->>", "->>", "-->", "->", "--x", "-x", "--)", "-)", "===", "==",
            "--", "-",
        ];

        for &arrow in arrow_patterns {
            if let Some(pos) = line.find(arrow) {
                let source = line[..pos].trim().to_string();
                let rest = &line[pos + arrow.len()..];
                let target = if let Some(colon_pos) = rest.find(':') {
                    rest[..colon_pos].trim().to_string()
                } else {
                    rest.trim().to_string()
                };

                if !source.is_empty() && !target.is_empty() {
                    return Some((source, arrow.to_string(), target));
                }
            }
        }
        None
    }

    /// Match node definition patterns like "A[label]", "A(label)", "A{label}"
    /// Returns (id, shape_indicator, label)
    pub fn match_node_pattern(line: &str) -> Option<(String, Option<String>, Option<String>)> {
        let shapes = &[('[', ']'), ('(', ')'), ('{', '}'), ('<', '>')];

        for &(open, close) in shapes {
            if let Some(open_pos) = line.find(open) {
                if let Some(close_pos) = line.rfind(close) {
                    if open_pos < close_pos {
                        let id = line[..open_pos].trim().to_string();
                        let label = line[open_pos + 1..close_pos].to_string();
                        let shape = format!("{}{}", open, close);

                        if !id.is_empty() {
                            return Some((id, Some(shape), Some(label)));
                        }
                    }
                }
            }
        }

        // No shape found, just return the line as ID
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            Some((trimmed.to_string(), None, None))
        } else {
            None
        }
    }
}

/// Validation utilities for common patterns
pub mod validation {

    /// Validate that all referenced IDs exist
    pub fn validate_references(ids: &[String], references: &[String]) -> Vec<String> {
        references
            .iter()
            .filter(|&ref_id| !ids.contains(ref_id))
            .cloned()
            .collect()
    }

    /// Naming convention types
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum NamingConvention {
        CamelCase,
        SnakeCase,
        KebabCase,
    }

    /// Validate identifier naming conventions
    pub fn validate_naming_convention(name: &str, convention: NamingConvention) -> bool {
        match convention {
            NamingConvention::CamelCase => is_camel_case(name),
            NamingConvention::SnakeCase => is_snake_case(name),
            NamingConvention::KebabCase => is_kebab_case(name),
        }
    }

    /// Check if string follows camelCase convention
    fn is_camel_case(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        let mut chars = name.chars();
        if let Some(first) = chars.next() {
            if !first.is_ascii_lowercase() {
                return false;
            }
        }

        chars.all(|c| c.is_ascii_alphanumeric())
    }

    /// Check if string follows snake_case convention
    fn is_snake_case(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        name.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    }

    /// Check if string follows kebab-case convention
    fn is_kebab_case(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        name.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod quoted_strings_tests {
        use super::*;

        #[test]
        fn test_unquote_double_quotes() {
            assert_eq!(quoted_strings::unquote("\"hello\""), "hello");
            assert_eq!(quoted_strings::unquote("hello"), "hello");
            assert_eq!(quoted_strings::unquote("\"\""), "");
            assert_eq!(quoted_strings::unquote("\"hello world\""), "hello world");
        }

        #[test]
        fn test_unquote_single_quotes() {
            assert_eq!(quoted_strings::unquote("'hello'"), "hello");
            assert_eq!(quoted_strings::unquote("'hello world'"), "hello world");
        }

        #[test]
        fn test_unquote_whitespace() {
            assert_eq!(quoted_strings::unquote("  \"hello\"  "), "hello");
            assert_eq!(quoted_strings::unquote("  'hello'  "), "hello");
        }

        #[test]
        fn test_is_quoted() {
            assert!(quoted_strings::is_quoted("\"hello\""));
            assert!(quoted_strings::is_quoted("'hello'"));
            assert!(!quoted_strings::is_quoted("hello"));
            assert!(!quoted_strings::is_quoted("\"hello"));
            assert!(!quoted_strings::is_quoted("hello\""));
            assert!(!quoted_strings::is_quoted("\""));
        }

        #[test]
        fn test_parse_field() {
            assert_eq!(
                quoted_strings::parse_field("\"hello\""),
                ("hello".to_string(), true)
            );
            assert_eq!(
                quoted_strings::parse_field("hello"),
                ("hello".to_string(), false)
            );
            assert_eq!(
                quoted_strings::parse_field("'world'"),
                ("world".to_string(), true)
            );
        }
    }

    mod key_value_tests {
        use super::*;

        #[test]
        fn test_parse_colon_separated() {
            assert_eq!(
                key_value::parse_colon_separated("key: value"),
                Some(("key".to_string(), "value".to_string()))
            );
            assert_eq!(
                key_value::parse_colon_separated("name:John Doe"),
                Some(("name".to_string(), "John Doe".to_string()))
            );
            assert_eq!(key_value::parse_colon_separated("no_separator"), None);
        }

        #[test]
        fn test_parse_equals_separated() {
            assert_eq!(
                key_value::parse_equals_separated("key=value"),
                Some(("key".to_string(), "value".to_string()))
            );
            assert_eq!(key_value::parse_equals_separated("no_separator"), None);
        }

        #[test]
        fn test_parse_multi_separator() {
            assert_eq!(
                key_value::parse_multi_separator("key:value", &[':', '=']),
                Some(("key".to_string(), "value".to_string()))
            );
            assert_eq!(
                key_value::parse_multi_separator("key=value", &[':', '=']),
                Some(("key".to_string(), "value".to_string()))
            );
            assert_eq!(
                key_value::parse_multi_separator("no_separator", &[':', '=']),
                None
            );
        }
    }

    mod brackets_tests {
        use super::*;

        #[test]
        fn test_extract_square_bracket_content() {
            assert_eq!(
                brackets::extract_square_bracket_content("[content]"),
                Some("content".to_string())
            );
            assert_eq!(
                brackets::extract_square_bracket_content("prefix[content]suffix"),
                Some("content".to_string())
            );
            assert_eq!(
                brackets::extract_square_bracket_content("no_brackets"),
                None
            );
        }

        #[test]
        fn test_extract_paren_content() {
            assert_eq!(
                brackets::extract_paren_content("(content)"),
                Some("content".to_string())
            );
            assert_eq!(
                brackets::extract_paren_content("func(args)"),
                Some("args".to_string())
            );
        }

        #[test]
        fn test_parse_id_bracket_content() {
            assert_eq!(
                brackets::parse_id_bracket_content("id[content]"),
                (Some("id".to_string()), "content".to_string())
            );
            assert_eq!(
                brackets::parse_id_bracket_content("[content]"),
                (None, "content".to_string())
            );
            assert_eq!(
                brackets::parse_id_bracket_content("no_brackets"),
                (None, "no_brackets".to_string())
            );
        }

        #[test]
        fn test_find_matching_bracket() {
            let input = "start[nested[content]more]end";
            // The bracket at position 5 should match the bracket at position 25
            assert_eq!(
                brackets::find_matching_bracket(input, 5, '[', ']'),
                Some(25)
            );

            let input2 = "[simple]";
            assert_eq!(
                brackets::find_matching_bracket(input2, 0, '[', ']'),
                Some(7)
            );

            // No matching bracket
            let input3 = "[unclosed";
            assert_eq!(brackets::find_matching_bracket(input3, 0, '[', ']'), None);
        }
    }

    mod fields_tests {
        use super::*;

        #[test]
        fn test_parse_csv_line() {
            assert_eq!(
                fields::parse_csv_line("a,b,c"),
                vec!["a".to_string(), "b".to_string(), "c".to_string()]
            );
            assert_eq!(
                fields::parse_csv_line("\"quoted field\",normal,\"another quoted\""),
                vec![
                    "quoted field".to_string(),
                    "normal".to_string(),
                    "another quoted".to_string()
                ]
            );
        }

        #[test]
        fn test_parse_delimited_fields() {
            assert_eq!(
                fields::parse_delimited_fields("a|b|c", '|'),
                vec!["a".to_string(), "b".to_string(), "c".to_string()]
            );
            assert_eq!(
                fields::parse_delimited_fields("'quoted'|normal|'more'", '|'),
                vec![
                    "quoted".to_string(),
                    "normal".to_string(),
                    "more".to_string()
                ]
            );
        }

        #[test]
        fn test_clean_field() {
            assert_eq!(fields::clean_field("  content  "), "content");
            assert_eq!(fields::clean_field("line1\\nline2"), "line1\nline2");
            assert_eq!(fields::clean_field("tab\\there"), "tab\there");
        }
    }

    mod lines_tests {
        use super::*;

        #[test]
        fn test_should_skip_line() {
            assert!(lines::should_skip_line(""));
            assert!(lines::should_skip_line("   "));
            assert!(lines::should_skip_line("// comment"));
            assert!(lines::should_skip_line("%% comment"));
            assert!(!lines::should_skip_line("actual content"));
        }

        #[test]
        fn test_clean_line() {
            assert_eq!(lines::clean_line("  content  "), "content");
            assert_eq!(lines::clean_line("\\tcontent"), "content");
        }

        #[test]
        fn test_split_line_parts() {
            assert_eq!(
                lines::split_line_parts("a,b;c", &[',', ';']),
                vec!["a".to_string(), "b".to_string(), "c".to_string()]
            );
            assert_eq!(
                lines::split_line_parts("a,,b", &[',']),
                vec!["a".to_string(), "b".to_string()]
            );
        }
    }

    mod numbers_tests {
        use super::*;

        #[test]
        fn test_parse_number_with_unit() {
            assert_eq!(
                numbers::parse_number_with_unit("10px"),
                Some((10.0, "px".to_string()))
            );
            assert_eq!(
                numbers::parse_number_with_unit("50%"),
                Some((50.0, "%".to_string()))
            );
            assert_eq!(
                numbers::parse_number_with_unit("-3.5em"),
                Some((-3.5, "em".to_string()))
            );
            assert_eq!(numbers::parse_number_with_unit("not_a_number"), None);
        }

        #[test]
        fn test_parse_percentage() {
            assert_eq!(numbers::parse_percentage("50%"), Some(0.5));
            assert_eq!(numbers::parse_percentage("100%"), Some(1.0));
            assert_eq!(numbers::parse_percentage("10px"), None);
        }
    }

    mod identifiers_tests {
        use super::*;

        #[test]
        fn test_is_valid_identifier() {
            assert!(identifiers::is_valid_identifier("valid_name"));
            assert!(identifiers::is_valid_identifier("_underscore"));
            assert!(identifiers::is_valid_identifier("name123"));
            assert!(!identifiers::is_valid_identifier("123invalid"));
            assert!(!identifiers::is_valid_identifier(""));
            assert!(!identifiers::is_valid_identifier("with-dash"));
        }

        #[test]
        fn test_sanitize_identifier() {
            assert_eq!(identifiers::sanitize_identifier("valid_name"), "valid_name");
            assert_eq!(
                identifiers::sanitize_identifier("123invalid"),
                "_123invalid"
            );
            assert_eq!(identifiers::sanitize_identifier("with-dash"), "with_dash");
            assert_eq!(identifiers::sanitize_identifier(""), "_");
            assert_eq!(
                identifiers::sanitize_identifier("special@chars"),
                "special_chars"
            );
        }
    }

    mod patterns_tests {
        use super::*;

        #[test]
        fn test_match_arrow_pattern() {
            assert_eq!(
                patterns::match_arrow_pattern("A --> B"),
                Some(("A".to_string(), "-->".to_string(), "B".to_string()))
            );
            assert_eq!(
                patterns::match_arrow_pattern("A ->> B: message"),
                Some(("A".to_string(), "->>".to_string(), "B".to_string()))
            );
            assert_eq!(patterns::match_arrow_pattern("no_arrow"), None);
        }

        #[test]
        fn test_match_node_pattern() {
            assert_eq!(
                patterns::match_node_pattern("A[label]"),
                Some((
                    "A".to_string(),
                    Some("[]".to_string()),
                    Some("label".to_string())
                ))
            );
            assert_eq!(
                patterns::match_node_pattern("B(round)"),
                Some((
                    "B".to_string(),
                    Some("()".to_string()),
                    Some("round".to_string())
                ))
            );
            assert_eq!(
                patterns::match_node_pattern("just_id"),
                Some(("just_id".to_string(), None, None))
            );
        }
    }

    mod validation_tests {
        use super::*;

        #[test]
        fn test_validate_references() {
            let ids = vec!["A".to_string(), "B".to_string(), "C".to_string()];
            let refs = vec!["A".to_string(), "D".to_string(), "B".to_string()];
            let missing = validation::validate_references(&ids, &refs);
            assert_eq!(missing, vec!["D".to_string()]);
        }

        #[test]
        fn test_validate_naming_convention() {
            assert!(validation::validate_naming_convention(
                "camelCase",
                validation::NamingConvention::CamelCase
            ));
            assert!(validation::validate_naming_convention(
                "snake_case",
                validation::NamingConvention::SnakeCase
            ));
            assert!(validation::validate_naming_convention(
                "kebab-case",
                validation::NamingConvention::KebabCase
            ));

            assert!(!validation::validate_naming_convention(
                "PascalCase",
                validation::NamingConvention::CamelCase
            ));
            assert!(!validation::validate_naming_convention(
                "camelCase",
                validation::NamingConvention::SnakeCase
            ));
            assert!(!validation::validate_naming_convention(
                "snake_case",
                validation::NamingConvention::KebabCase
            ));
        }
    }
}
