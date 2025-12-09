# Add rstest file-based tests for Class parser

## Description
The Class parser (`src/parsers/class.rs`) has been implemented but lacks comprehensive file-based testing using rstest. Class diagrams are marked as "Advanced" complexity in Phase 3 of the implementation roadmap.

## Current State
- Parser implementation: ✅ Complete
- Unit tests in parser file: ✅ Present
- Test data files: ✅ Available in `test/class/` (212 sample files!)
- File-based rstest: ❌ Missing

## Requirements
1. Create `tests/class_test.rs` file
2. Implement rstest file-based testing following established patterns
3. Use `#[rstest]` macro to automatically run tests against all `.mermaid` files in `test/class/`
4. Handle the large number of test files (212) efficiently
5. Validate OOP concepts: classes, methods, properties, inheritance, relationships

## Implementation Pattern
```rust
use rstest::rstest;
use std::fs;
use std::path::PathBuf;
use mermaid_parser::{parse_diagram, DiagramType};

#[rstest]
fn test_class_files(#[files("test/class/*.mermaid")] path: PathBuf) {
    let content = fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    let result = parse_diagram(&content);
    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result.err());
    
    if let Ok(DiagramType::Class(ast)) = result {
        // Validate class structures
        // Check methods, properties, visibility modifiers
        // Verify inheritance and relationships
    }
}
```

## Special Considerations
- Large test suite (212 files) may require performance optimization
- Complex OOP relationships to validate
- Various notation styles (e.g., +public, -private, #protected)
- Multiple relationship types (inheritance, composition, aggregation)

## Success Criteria
- All 212 `.mermaid` files in `test/class/` parse without errors
- Tests complete in reasonable time despite large file count
- OOP concepts are properly validated in AST