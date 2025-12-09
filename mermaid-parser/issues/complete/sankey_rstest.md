# Add rstest file-based tests for Sankey parser

## Description
The Sankey parser (`src/parsers/sankey.rs`) has been implemented but lacks comprehensive file-based testing using rstest. This is marked as Priority 1 - the simplest grammar with CSV-like format, making it ideal for establishing testing patterns.

## Current State
- Parser implementation: ✅ Complete
- Unit tests in parser file: ✅ Present
- Test data files: ✅ Available in `test/sankey/` (7 sample files)
- File-based rstest: ❌ Missing

## Requirements
1. Create `tests/sankey_test.rs` file
2. Implement rstest file-based testing as the first/reference implementation
3. Use `#[rstest]` macro to automatically run tests against all `.mermaid` files in `test/sankey/`
4. Establish patterns for other parsers to follow
5. Validate CSV-like flow data structure

## Implementation Pattern
```rust
use rstest::rstest;
use std::fs;
use std::path::PathBuf;
use mermaid_parser::{parse_diagram, DiagramType};

#[rstest]
fn test_sankey_files(#[files("test/sankey/*.mermaid")] path: PathBuf) {
    let content = fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    let result = parse_diagram(&content);
    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result.err());
    
    if let Ok(DiagramType::Sankey(ast)) = result {
        // Validate sankey flow structure
        // Check source, target, and value fields
        // Ensure all flows have positive values
    }
}
```

## Special Considerations
- Simplest grammar - ideal first implementation
- CSV-like format: source,target,value
- Small test suite (7 files) for quick validation
- Should establish patterns for error handling and reporting

## Success Criteria
- All 7 `.mermaid` files in `test/sankey/` parse without errors
- Clear, reusable test pattern established
- Helpful error messages for debugging
- Fast execution as reference implementation