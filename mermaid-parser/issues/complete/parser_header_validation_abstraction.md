# Parser Header Validation Abstraction

**Priority**: High  
**Impact**: 15+ files affected  
**Effort**: Medium  

## Problem

Currently, every parser implements identical header validation logic with slight variations. This creates maintenance burden and inconsistency across the codebase.

### Current Duplicated Pattern

Found in `architecture.rs`, `block.rs`, `packet.rs`, `state.rs`, `c4.rs`, `gantt.rs`, `pie.rs`, `timeline.rs`, and 7+ other parser files:

```rust
if !first_line_processed {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("%%") {
        continue;
    }
    if !(trimmed.starts_with("architecture") || trimmed.starts_with("architecture-beta")) {
        return Err(ParseError::SyntaxError {
            message: "Expected architecture header".to_string(),
            expected: vec!["architecture".to_string(), "architecture-beta".to_string()],
            found: trimmed.to_string(),
            line: line_num + 1,
            column: 1,
        });
    }
    first_line_processed = true;
    continue;
}
```

## Solution

Create a shared header validation utility in `src/common/parser_utils.rs`:

```rust
/// Validates diagram header and handles first line processing
/// Returns Ok(true) if line was handled (skip to next line)
/// Returns Ok(false) if line should be processed by parser
/// Returns Err() if invalid header found
pub fn validate_diagram_header(
    line: &str,
    line_num: usize,
    expected_headers: &[&str],
    first_line_processed: &mut bool,
) -> Result<bool> {
    if *first_line_processed {
        return Ok(false);
    }
    
    let trimmed = line.trim();
    if should_skip_line(trimmed) {
        return Ok(true); // Skip this line
    }
    
    if !expected_headers.iter().any(|h| trimmed.starts_with(h)) {
        return Err(ParseError::SyntaxError {
            message: format!("Expected {} header", expected_headers[0]),
            expected: expected_headers.iter().map(|s| s.to_string()).collect(),
            found: trimmed.to_string(),
            line: line_num + 1,
            column: 1,
        });
    }
    
    *first_line_processed = true;
    Ok(true)
}

/// Checks if a line should be skipped (empty, comments)
pub fn should_skip_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("%%")
}
```

### Usage in Parsers

```rust
// In architecture.rs
if validate_diagram_header(
    line, 
    line_num, 
    &["architecture", "architecture-beta"], 
    &mut first_line_processed
)? {
    continue;
}

// In state.rs  
if validate_diagram_header(
    line,
    line_num,
    &["stateDiagram", "stateDiagram-v2"],
    &mut first_line_processed
)? {
    continue;
}
```

## Implementation Steps

1. **Add utility functions to `src/common/parser_utils.rs`**
   - `validate_diagram_header()`
   - `should_skip_line()`

2. **Update parser files** (in order of complexity):
   - `src/parsers/architecture.rs`
   - `src/parsers/block.rs`
   - `src/parsers/packet.rs`
   - `src/parsers/state.rs`
   - `src/parsers/c4.rs`
   - `src/parsers/gantt.rs`
   - `src/parsers/pie.rs`
   - `src/parsers/timeline.rs`
   - `src/parsers/kanban.rs`
   - `src/parsers/mindmap.rs`
   - `src/parsers/quadrant.rs`
   - `src/parsers/radar.rs`
   - `src/parsers/requirement.rs`
   - `src/parsers/treemap.rs`
   - `src/parsers/xy.rs`

3. **Add tests for utility functions**
   - Test valid headers
   - Test invalid headers
   - Test comment skipping
   - Test error message format

4. **Run full test suite** to ensure no regressions

## Benefits

- **Reduced duplication**: Eliminates ~20 lines of code per parser (300+ lines total)
- **Consistent error messages**: All parsers will have identical error format
- **Easier maintenance**: Header validation logic centralized
- **Better testing**: Utility functions can be unit tested independently
- **Future extensibility**: Easy to add new header validation features

## Files to Modify

### New Files
- Tests for `parser_utils.rs` functions

### Modified Files
- `src/common/parser_utils.rs` (add functions)
- `src/parsers/*.rs` (15+ parser files)

## Testing

Add comprehensive tests for the new utility functions:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_diagram_header_success() {
        let mut first_line = false;
        let result = validate_diagram_header(
            "architecture", 
            0, 
            &["architecture", "architecture-beta"], 
            &mut first_line
        );
        assert!(result.is_ok());
        assert!(result.unwrap());
        assert!(first_line);
    }

    #[test]
    fn test_validate_diagram_header_invalid() {
        let mut first_line = false;
        let result = validate_diagram_header(
            "invalid", 
            0, 
            &["architecture"], 
            &mut first_line
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_should_skip_line() {
        assert!(should_skip_line(""));
        assert!(should_skip_line("   "));
        assert!(should_skip_line("// comment"));
        assert!(should_skip_line("%% comment"));
        assert!(!should_skip_line("actual content"));
    }
}
```

## Risk Assessment

**Low Risk**: The change is purely refactoring existing logic into shared utilities. Each parser's behavior should remain identical after the change.

**Mitigation**: Run full test suite after each parser migration to catch any behavioral changes.