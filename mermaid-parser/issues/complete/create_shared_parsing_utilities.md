# Create Shared Parsing Utilities

**Priority**: Low  
**Impact**: Code quality and maintainability  
**Effort**: Low to Medium  

## Problem

Parser implementations contain repeated patterns for common parsing operations like string handling, bracket matching, and data extraction. These utilities are implemented inconsistently across different parsers, leading to code duplication and potential bugs.

### Common Parsing Patterns Found

#### 1. Quoted String Parsing
Found in `sankey.rs`, `packet.rs`, `er.rs`, `class.rs`, and others:

```rust
// Pattern 1: Basic quoted string extraction
if field_part.starts_with('"') && field_part.ends_with('"') {
    let name = field_part[1..field_part.len() - 1].to_string();
    (name, false)
}

// Pattern 2: With validation
if content.starts_with('"') && content.ends_with('"') && content.len() >= 2 {
    Some(content[1..content.len()-1].to_string())
}

// Pattern 3: Complex quoted string handling in sankey.rs
fn parse_quoted_field(field: &str) -> String {
    if field.starts_with('"') && field.ends_with('"') && field.len() >= 2 {
        field[1..field.len() - 1].to_string()
    } else {
        field.to_string()
    }
}
```

#### 2. Colon-Separated Key-Value Parsing
Found in `packet.rs`, `quadrant.rs`, `er.rs`, and others:

```rust
// Pattern from packet.rs
if let Some(colon_pos) = trimmed.find(':') {
    let range_part = trimmed[..colon_pos].trim();
    let field_part = trimmed[colon_pos + 1..].trim();
    // ... process parts
}

// Pattern from quadrant.rs  
if let Some(colon_pos) = trimmed[..bracket_start].rfind(':') {
    let name_part = trimmed[..colon_pos].trim();
    let value_part = trimmed[colon_pos + 1..].trim();
    // ... process parts
}
```

#### 3. Bracket Matching and Content Extraction
Found in `kanban.rs`, `flowchart.rs`, `packet.rs`, and others:

```rust
// Pattern 1: Simple bracket content
if let Some(bracket_pos) = content.find('[') {
    if content.ends_with(']') {
        let id = content[..bracket_pos].trim();
        let text = content[bracket_pos + 1..content.len() - 1].to_string();
    }
}

// Pattern 2: Complex bracket matching in flowchart.rs
let mut bracket_count = 0;
let mut found_closing = false;
for j in i + 1..tokens.len() {
    match &tokens[j] {
        FlowToken::LeftBracket => bracket_count += 1,
        FlowToken::RightBracket => {
            if bracket_count == 0 {
                found_closing = true;
                break;
            } else {
                bracket_count -= 1;
            }
        }
        _ => {}
    }
}
```

#### 4. Whitespace and Comment Cleaning
Found in most parser files:

```rust
// Pattern: Skip empty lines and comments
let trimmed = line.trim();
if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("%%") {
    continue;
}
```

#### 5. CSV-like Field Parsing
Found in `sankey.rs`, `gantt.rs`, `xy.rs`:

```rust
// Pattern: Split and clean fields
let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
for part in parts {
    let field = parse_quoted_field(part);
    // ... process field
}
```

## Solution

Create a comprehensive parsing utilities module with reusable functions:

```rust
// src/common/parsing.rs

use crate::error::{ParseError, Result};

/// Utilities for parsing quoted strings
pub mod quoted_strings {
    use super::*;
    
    /// Extract content from a quoted string, handling both single and double quotes
    /// Returns the unquoted content, or the original string if not quoted
    pub fn unquote(input: &str) -> String {
        let trimmed = input.trim();
        
        if trimmed.len() >= 2 {
            if (trimmed.starts_with('"') && trimmed.ends_with('"')) ||
               (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
                return trimmed[1..trimmed.len() - 1].to_string();
            }
        }
        
        trimmed.to_string()
    }
    
    /// Check if a string is properly quoted
    pub fn is_quoted(input: &str) -> bool {
        let trimmed = input.trim();
        trimmed.len() >= 2 && (
            (trimmed.starts_with('"') && trimmed.ends_with('"')) ||
            (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        )
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
    use super::*;
    
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
    use super::*;
    
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
    use super::*;
    use crate::common::constants::comments;
    
    /// Check if a line should be skipped (empty, whitespace, comments)
    pub fn should_skip_line(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.is_empty() || 
        comments::COMMENT_PREFIXES.iter().any(|prefix| trimmed.starts_with(prefix))
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
    pub fn parse_int_with_error(
        input: &str,
        line_num: usize,
        expected_desc: &str,
    ) -> Result<i32> {
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
    use super::*;
    
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
```

## Usage Examples

### Before (Duplicated Code)

```rust
// In packet.rs
if field_part.starts_with('"') && field_part.ends_with('"') {
    let name = field_part[1..field_part.len() - 1].to_string();
    (name, false)
}

// In sankey.rs  
fn parse_quoted_field(field: &str) -> String {
    if field.starts_with('"') && field.ends_with('"') && field.len() >= 2 {
        field[1..field.len() - 1].to_string()
    } else {
        field.to_string()
    }
}
```

### After (Using Utilities)

```rust
use crate::common::parsing::{quoted_strings, brackets, key_value};

// In packet.rs
let (name, was_quoted) = quoted_strings::parse_field(field_part);

// In sankey.rs
let field_content = quoted_strings::unquote(field);

// Bracket parsing
let (id, content) = brackets::parse_id_bracket_content("id[text content]");

// Key-value parsing
if let Some((key, value)) = key_value::parse_colon_separated(line) {
    // Process key-value pair
}
```

## Implementation Steps

### Phase 1: Create Utilities Module

1. **Create `src/common/parsing.rs`** with all utility functions
2. **Add comprehensive tests** for each utility function
3. **Add module to `src/common/mod.rs`**

### Phase 2: Migrate Parser Files

**Priority order based on duplication frequency:**

1. **Quoted string parsing** (affects `sankey.rs`, `packet.rs`, `er.rs`, `class.rs`)
2. **Key-value parsing** (affects `packet.rs`, `quadrant.rs`, `er.rs`)
3. **Bracket parsing** (affects `kanban.rs`, `flowchart.rs`, `packet.rs`)
4. **CSV field parsing** (affects `sankey.rs`, `gantt.rs`, `xy.rs`)
5. **Line processing** (affects all parser files)

### Phase 3: Add Enhanced Utilities

1. **Error context helpers** for better error messages
2. **Validation utilities** for common patterns
3. **Performance optimizations** for frequently used functions

## Benefits

- **Code reuse**: Eliminates duplicate parsing logic across parsers
- **Consistency**: All parsers handle similar patterns identically
- **Testing**: Utilities can be thoroughly unit tested
- **Maintainability**: Bug fixes and improvements in one place
- **Documentation**: Clear API for common parsing operations
- **Error handling**: Consistent error reporting across parsers

## Enhanced Features

### Pattern Matching Utilities

```rust
pub mod patterns {
    /// Match common diagram patterns
    pub fn match_arrow_pattern(line: &str) -> Option<(String, String, String)> {
        // Match patterns like "A --> B", "A ==> B", etc.
        // Returns (source, arrow_type, target)
    }
    
    /// Match node definition patterns
    pub fn match_node_pattern(line: &str) -> Option<(String, Option<String>, Option<String>)> {
        // Match patterns like "A[label]", "A(label)", "A{label}", etc.
        // Returns (id, shape_indicator, label)
    }
}
```

### Validation Utilities

```rust
pub mod validation {
    /// Validate that all referenced IDs exist
    pub fn validate_references(ids: &[String], references: &[String]) -> Vec<String> {
        references.iter()
            .filter(|&ref_id| !ids.contains(ref_id))
            .cloned()
            .collect()
    }
    
    /// Validate identifier naming conventions
    pub fn validate_naming_convention(name: &str, convention: NamingConvention) -> bool {
        match convention {
            NamingConvention::CamelCase => is_camel_case(name),
            NamingConvention::SnakeCase => is_snake_case(name),
            NamingConvention::KebabCase => is_kebab_case(name),
        }
    }
}
```

## Testing Strategy

Comprehensive unit tests for all utilities:

```rust
#[cfg(test)]
mod parsing_tests {
    use super::*;
    
    mod quoted_strings_tests {
        use super::*;
        
        #[test]
        fn test_unquote_double_quotes() {
            assert_eq!(quoted_strings::unquote("\"hello\""), "hello");
            assert_eq!(quoted_strings::unquote("hello"), "hello");
        }
        
        #[test]
        fn test_unquote_single_quotes() {
            assert_eq!(quoted_strings::unquote("'hello'"), "hello");
        }
        
        #[test]
        fn test_is_quoted() {
            assert!(quoted_strings::is_quoted("\"hello\""));
            assert!(quoted_strings::is_quoted("'hello'"));
            assert!(!quoted_strings::is_quoted("hello"));
            assert!(!quoted_strings::is_quoted("\"hello"));
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
        }
    }
    
    // Similar test modules for other utilities...
}
```

## Files to Create/Modify

### New Files
- `src/common/parsing.rs` (main utilities)
- Tests for parsing utilities

### Modified Files
- `src/common/mod.rs` (add parsing module)
- `src/parsers/sankey.rs` (quoted string parsing)
- `src/parsers/packet.rs` (bracket and key-value parsing)
- `src/parsers/kanban.rs` (bracket parsing)
- `src/parsers/er.rs` (quoted string parsing)
- `src/parsers/class.rs` (quoted string parsing)
- `src/parsers/quadrant.rs` (key-value parsing)
- `src/parsers/gantt.rs` (CSV parsing)
- `src/parsers/xy.rs` (CSV parsing)
- Other parser files as needed

## Migration Strategy

1. **Implement utilities with comprehensive tests**
2. **Migrate one parser at a time** starting with heaviest duplications
3. **Verify identical behavior** before and after migration
4. **Add enhanced utilities** as patterns emerge
5. **Document usage patterns** for future parser development

## Risk Assessment

**Low Risk**: 
- Each utility function can be tested independently
- Migration can be done incrementally
- Existing functionality preserved

**High Value**:
- Significant reduction in code duplication
- Foundation for consistent parsing patterns
- Easier maintenance and debugging