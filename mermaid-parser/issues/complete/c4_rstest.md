# Add rstest file-based tests for C4 parser

## Description
The C4 parser (`src/parsers/c4.rs`) for C4 architecture diagrams has been implemented but lacks comprehensive file-based testing using rstest. This is critical as C4 diagrams are listed as one of the most complex grammar types.

## Current State
- Parser implementation: ✅ Complete
- Unit tests in parser file: ✅ Present
- Test data files: ✅ Available in `test/c4/`
- File-based rstest: ❌ Missing

## Requirements
1. Create `tests/c4_test.rs` file
2. Implement rstest file-based testing following established patterns
3. Use `#[rstest]` macro to automatically run tests against all `.mermaid` files in `test/c4/`
4. Ensure all test files parse successfully
5. Validate C4-specific elements: contexts, containers, components, relationships

## Implementation Pattern
```rust
use rstest::rstest;
use std::fs;
use std::path::PathBuf;
use mermaid_parser::{parse_diagram, DiagramType};

#[rstest]
fn test_c4_files(#[files("test/c4/*.mermaid")] path: PathBuf) {
    let content = fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    let result = parse_diagram(&content);
    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result.err());
    
    if let Ok(DiagramType::C4(ast)) = result {
        // Validate C4-specific structures
        // Check for contexts, containers, components
        // Verify relationships between elements
    }
}
```

## Special Considerations
- C4 diagrams are complex enterprise architecture diagrams
- May include multiple abstraction levels (context, container, component)
- Rich relationship types and attributes
- Test should validate hierarchical structure integrity

## Success Criteria
- All `.mermaid` files in `test/c4/` parse without errors
- Complex C4 structures are properly validated
- Performance remains acceptable for large diagrams