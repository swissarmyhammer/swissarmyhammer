# Add rstest file-based tests for Architecture parser

## Description
The Architecture parser (`src/parsers/architecture.rs`) has been implemented but lacks comprehensive file-based testing using rstest. This is required per the original specification to validate the parser against real Mermaid diagram examples.

## Current State
- Parser implementation: ✅ Complete
- Unit tests in parser file: ✅ Present
- Test data files: ✅ Available in `test/architecture/`
- File-based rstest: ❌ Missing

## Requirements
1. Create `tests/architecture_test.rs` file
2. Implement rstest file-based testing following the pattern used in other test files (e.g., `tests/er_test.rs`)
3. Use `#[rstest]` macro to automatically run tests against all `.mermaid` files in `test/architecture/`
4. Ensure all test files parse successfully
5. Validate the AST structure matches expected architecture diagram elements

## Implementation Pattern
```rust
use rstest::rstest;
use std::fs;
use std::path::PathBuf;
use mermaid_parser::{parse_diagram, DiagramType};

#[rstest]
fn test_architecture_files(#[files("test/architecture/*.mermaid")] path: PathBuf) {
    let content = fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    let result = parse_diagram(&content);
    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result.err());
    
    if let Ok(DiagramType::Architecture(ast)) = result {
        // Validate basic AST structure
        // Add specific assertions as needed
    }
}
```

## Success Criteria
- All `.mermaid` files in `test/architecture/` parse without errors
- Test output clearly indicates which files are being tested
- Failing tests provide helpful error messages with file context