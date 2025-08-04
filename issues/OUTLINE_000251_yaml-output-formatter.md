# OUTLINE_000251: YAML Output Formatter

Refer to ./specification/outline_tool.md

## Summary

Implement the YAML output formatter that converts the hierarchical outline structure into well-formatted YAML output, following the specification's structure requirements and providing clean, readable output for code navigation and understanding.

## Context

The outline tool needs to generate YAML output that mirrors the file system hierarchy while containing detailed symbol information. The output should be human-readable, machine-parsable, and follow consistent formatting conventions.

## Requirements

### 1. YAML Structure Generation
- Mirror file system hierarchy in YAML structure
- Maintain nested symbol relationships within files
- Preserve all symbol metadata (names, kinds, signatures, docs, line numbers)
- Support clean indentation and formatting
- Handle special characters and escaping properly

### 2. Output Format Specification
Follow the exact format specified in the specification:
```yaml
src:
  utils:
    math.ts:
      children:
        - name: "Calculator"
          kind: "class"
          line: 3
          children:
            - name: "result"
              kind: "property"
              type: "number"
              line: 5
            - name: "add"
              kind: "method"
              signature: "(a: number, b: number) => number"
              line: 8
              doc: "Adds two numbers and returns the result."
```

### 3. Formatting Options
- Configurable indentation (2 or 4 spaces)
- Option to include/exclude empty directories
- Filtering by symbol kinds or visibility
- Sorting options (alphabetical, by kind, by line number)
- Compact vs. expanded format options

## Technical Details

### YAML Formatter Implementation
```rust
pub struct YamlFormatter {
    config: FormatterConfig,
}

#[derive(Debug, Clone)]
pub struct FormatterConfig {
    pub indent_size: usize,
    pub include_empty_dirs: bool,
    pub sort_order: SortOrder,
    pub include_private_symbols: bool,
    pub max_signature_length: Option<usize>,
    pub include_line_numbers: bool,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            indent_size: 2,
            include_empty_dirs: false,
            sort_order: SortOrder::SourceOrder,
            include_private_symbols: true,
            max_signature_length: Some(120),
            include_line_numbers: true,
        }
    }
}

impl YamlFormatter {
    pub fn new(config: FormatterConfig) -> Self;
    pub fn format_hierarchy(&self, hierarchy: &OutlineHierarchy) -> Result<String>;
    pub fn format_directory(&self, directory: &OutlineDirectory, depth: usize) -> Result<String>;
    pub fn format_file(&self, file: &OutlineFile, depth: usize) -> Result<String>;
    pub fn format_symbol(&self, symbol: &OutlineNode, depth: usize) -> Result<String>;
}
```

### Directory Structure Formatting
```rust
impl YamlFormatter {
    fn format_directory(&self, directory: &OutlineDirectory, depth: usize) -> Result<String> {
        let mut result = String::new();
        let indent = " ".repeat(depth * self.config.indent_size);
        
        // Skip root directory name if it's "."
        if directory.name != "." {
            result.push_str(&format!("{}{}:\n", indent, directory.name));
        }
        
        let child_depth = if directory.name == "." { depth } else { depth + 1 };
        
        // Format subdirectories first
        for subdir in &directory.subdirectories {
            if !subdir.is_empty() || self.config.include_empty_dirs {
                result.push_str(&self.format_directory(subdir, child_depth)?);
            }
        }
        
        // Format files
        for file in &directory.files {
            result.push_str(&self.format_file(file, child_depth)?);
        }
        
        Ok(result)
    }
}
```

### File Content Formatting
```rust
impl YamlFormatter {
    fn format_file(&self, file: &OutlineFile, depth: usize) -> Result<String> {
        let mut result = String::new();
        let indent = " ".repeat(depth * self.config.indent_size);
        
        // File name as key
        result.push_str(&format!("{}{}:\n", indent, file.name));
        
        if file.symbols.is_empty() {
            // Empty file notation
            result.push_str(&format!("{}  children: []\n", indent));
        } else {
            // Children array
            result.push_str(&format!("{}  children:\n", indent));
            
            for symbol in &file.symbols {
                result.push_str(&self.format_symbol(symbol, depth + 1)?);
            }
        }
        
        Ok(result)
    }
}
```

### Symbol Formatting
```rust
impl YamlFormatter {
    fn format_symbol(&self, symbol: &OutlineNode, depth: usize) -> Result<String> {
        let mut result = String::new();
        let indent = " ".repeat(depth * self.config.indent_size);
        let child_indent = " ".repeat((depth + 1) * self.config.indent_size);
        
        // Start symbol entry
        result.push_str(&format!("{}  - name: {}\n", indent, Self::escape_yaml_string(&symbol.name)));
        result.push_str(&format!("{}    kind: \"{}\"\n", indent, symbol.kind.as_str()));
        
        if self.config.include_line_numbers {
            result.push_str(&format!("{}    line: {}\n", indent, symbol.line));
        }
        
        // Optional signature
        if let Some(ref signature) = symbol.signature {
            let formatted_sig = self.format_signature(signature)?;
            result.push_str(&format!("{}    signature: {}\n", indent, 
                Self::escape_yaml_string(&formatted_sig)));
        }
        
        // Optional type information
        if let Some(ref type_info) = symbol.type_info {
            result.push_str(&format!("{}    type: {}\n", indent, 
                Self::escape_yaml_string(type_info)));
        }
        
        // Optional documentation
        if let Some(ref doc) = symbol.doc {
            let formatted_doc = self.format_documentation(doc)?;
            result.push_str(&format!("{}    doc: {}\n", indent, 
                Self::escape_yaml_string(&formatted_doc)));
        }
        
        // Optional children
        if let Some(ref children) = symbol.children {
            if !children.is_empty() {
                result.push_str(&format!("{}    children:\n", indent));
                for child in children {
                    result.push_str(&self.format_symbol(child, depth + 2)?);
                }
            }
        }
        
        Ok(result)
    }
    
    fn escape_yaml_string(s: &str) -> String {
        // Handle YAML string escaping
        if s.contains('\n') || s.contains('"') || s.contains('\\') {
            format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
        } else if s.is_empty() || s.chars().any(|c| c.is_whitespace()) {
            format!("\"{}\"", s)
        } else {
            s.to_string()
        }
    }
}
```

### Signature and Documentation Formatting
```rust
impl YamlFormatter {
    fn format_signature(&self, signature: &str) -> Result<String> {
        if let Some(max_len) = self.config.max_signature_length {
            if signature.len() > max_len {
                // Truncate long signatures with ellipsis
                let truncated = &signature[..max_len.saturating_sub(3)];
                return Ok(format!("{}...", truncated));
            }
        }
        
        Ok(signature.to_string())
    }
    
    fn format_documentation(&self, doc: &str) -> Result<String> {
        // Clean up documentation text
        let cleaned = doc
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        
        // Limit documentation length for readability
        if cleaned.len() > 200 {
            let truncated = &cleaned[..197];
            Ok(format!("{}...", truncated))
        } else {
            Ok(cleaned)
        }
    }
}
```

## Implementation Steps

1. Create `src/outline/formatter.rs` module
2. Implement basic YAML structure generation
3. Add directory hierarchy formatting
4. Implement file content formatting with children arrays
5. Add comprehensive symbol formatting with all metadata
6. Implement YAML string escaping and special character handling
7. Add configuration options for different formatting styles
8. Implement signature and documentation formatting
9. Add filtering and sorting capabilities
10. Create comprehensive unit tests with various symbol types
11. Add integration tests with complete hierarchies

## Testing Requirements

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_symbol_formatting() {
        let symbol = OutlineNode {
            name: "test_function".to_string(),
            kind: OutlineKind::Function,
            line: 42,
            signature: Some("fn test_function() -> bool".to_string()),
            doc: Some("Test function documentation".to_string()),
            type_info: None,
            children: None,
        };
        
        let formatter = YamlFormatter::new(FormatterConfig::default());
        let result = formatter.format_symbol(&symbol, 0).unwrap();
        
        assert!(result.contains("name: \"test_function\""));
        assert!(result.contains("kind: \"function\""));
        assert!(result.contains("line: 42"));
    }
    
    #[test]
    fn test_nested_symbol_hierarchy() {
        // Test symbols with children
    }
    
    #[test]
    fn test_yaml_string_escaping() {
        // Test various string escaping scenarios
    }
    
    #[test]
    fn test_different_formatter_configs() {
        // Test different configuration options
    }
}
```

### Integration Tests
- Test complete directory hierarchy formatting
- Verify YAML validity with external parser
- Test large codebase formatting performance
- Validate different language symbol formatting

### Sample Expected Output
For a simple Rust file:
```rust
/// User configuration
pub struct Config {
    /// Application name
    pub name: String,
    /// Debug mode flag
    pub debug: bool,
}

impl Config {
    /// Create new configuration
    pub fn new(name: String) -> Self {
        Self { name, debug: false }
    }
}
```

Expected YAML:
```yaml
src:
  config.rs:
    children:
      - name: "Config"
        kind: "struct"
        line: 2
        signature: "pub struct Config"
        doc: "User configuration"
        children:
          - name: "name"
            kind: "field"
            type: "String"
            line: 4
            doc: "Application name"
          - name: "debug"
            kind: "field"
            type: "bool"
            line: 6
            doc: "Debug mode flag"
      - name: "impl Config"
        kind: "impl"
        line: 9
        children:
          - name: "new"
            kind: "method"
            signature: "pub fn new(name: String) -> Self"
            line: 11
            doc: "Create new configuration"
```

## Integration Points

### With Hierarchy Builder
- Receive structured hierarchy from builder
- Traverse hierarchy efficiently for formatting
- Handle empty directories and files appropriately

### With MCP Tool
- Provide formatted YAML string as tool response
- Support different formatting configurations via tool parameters
- Handle large outputs efficiently

## Performance Considerations

- Efficient string building for large outputs
- Memory-efficient traversal of large hierarchies
- Streaming output for very large codebases
- Optimize string concatenation and escaping

## Error Handling

- Handle invalid YAML characters gracefully
- Provide clear error messages for formatting failures
- Graceful degradation for malformed hierarchies
- Validation of generated YAML structure

## Success Criteria

- Generates valid, well-formatted YAML output
- Follows specification format exactly
- Handles all symbol metadata correctly
- Supports configurable formatting options
- Efficient performance with large codebases
- Proper YAML string escaping and character handling
- Comprehensive test coverage
- Clean, readable output for human consumption

## Dependencies

- `serde_yaml` for YAML validation and utilities
- Hierarchical outline structures
- Standard library string handling
- Configuration management utilities

## Notes

The YAML formatter is the final step in the outline generation pipeline and directly affects user experience. The output should be both human-readable and machine-parsable. Consider providing options for different output styles (compact vs. expanded) to accommodate different use cases.