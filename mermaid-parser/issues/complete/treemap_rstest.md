# Add rstest file-based tests for Treemap parser

## Description
The Treemap parser (`src/parsers/treemap.rs`) has been implemented but lacks comprehensive file-based testing using rstest. This parser handles hierarchical data visualization.

## Current State
- Parser implementation: ✅ Complete (recently implemented per commit history)
- Unit tests in parser file: ✅ Present
- Test data files: ✅ Available in `test/treemap/`
- File-based rstest: ❌ Missing

## Requirements
1. Create `tests/treemap_test.rs` file
2. Implement rstest file-based testing following established patterns
3. Use `#[rstest]` macro to automatically run tests against all `.mermaid` files in `test/treemap/`
4. Validate hierarchical structure and value assignments
5. Ensure proper parent-child relationships

## Implementation Pattern
```rust
use rstest::rstest;
use std::fs;
use std::path::PathBuf;
use mermaid_parser::{parse_diagram, DiagramType};

#[rstest]
fn test_treemap_files(#[files("test/treemap/*.mermaid")] path: PathBuf) {
    let content = fs::read_to_string(&path)
        .expect(&format!("Failed to read file: {:?}", path));
    
    let result = parse_diagram(&content);
    assert!(result.is_ok(), "Failed to parse {:?}: {:?}", path, result.err());
    
    if let Ok(DiagramType::Treemap(ast)) = result {
        // Validate hierarchical structure
        // Check node values and relationships
        // Verify tree integrity
    }
}
```

## Special Considerations
- Hierarchical data structure validation
- Value propagation through tree levels
- Recent implementation needs validation
- May include nested categories and subcategories

## Success Criteria
- All `.mermaid` files in `test/treemap/` parse without errors
- Hierarchical relationships properly validated
- Values correctly associated with nodes
- Tree structure integrity maintained